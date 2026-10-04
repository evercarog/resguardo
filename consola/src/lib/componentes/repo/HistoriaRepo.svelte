<script lang="ts">
  import { tip } from "$lib/tooltip";
  // «Historia» de un repositorio: todo lo que le ha pasado en una sola línea
  // de tiempo (cada vuelta de sus copias, las verificaciones y las pruebas de
  // restauración), agrupada por día y con filtro. Con el historial que guarda
  // el propio equipo (v1.23) se ven también las verificaciones, pruebas y
  // ganchos de antes (no solo los últimos), y lo de antes de llegar a esta
  // consola (lo de hace más de un año, resumido por copia y día).
  import { ArchiveRestore, CircleAlert, CloudUpload, Database, CircleCheck, CircleDashed, History, RefreshCw, ShieldCheck, TriangleAlert } from "@lucide/svelte";
  import type { CopiaResumen, EntradaHistorial, Informe, RepoInforme, RepositorioResumen, ResultadoGancho } from "$lib/tipos";
  import { NOMBRE_GANCHO } from "$lib/ganchos";
  import { bytes, dia, fechaLarga, hora } from "$lib/formato";
  import { anadidoDe, pruebaRestauracion, TEXTO_RESULTADO, TEXTO_TAREA, TONO_RESULTADO, TONO_TAREA, verificacion, versionDeVuelta, versionesDe } from "$lib/repo";
  import type { Tono } from "$lib/salud";
  import { claveDia } from "$lib/repo";
  import "../detalle/pulsable.css";

  let {
    repo,
    inf,
    copias,
    ultimas = [],
    historial = [],
    alAbrirVersion,
    alAbrirVuelta,
    dia: diaElegido = null,
  }: {
    repo: RepositorioResumen;
    inf: RepoInforme | null;
    copias: CopiaResumen[];
    /** Las copias del último informe del equipo, con el resultado de sus ganchos (v1.10). */
    ultimas?: NonNullable<Informe["datos"]["copias"]>;
    /** v1.23: el historial del equipo (de cualquier repositorio; aquí se filtra). */
    historial?: EntradaHistorial[];
    /** Abrir el detalle de una vuelta del informe: su versión o, si no dejó, la vuelta. */
    alAbrirVersion?: (id: string) => void;
    alAbrirVuelta?: (hora: string) => void;
    /** Solo lo de este día (`AAAA-MM-DD`). */
    dia?: string | null;
  } = $props();

  type Grupo = "copias" | "mantenimiento";
  interface Item {
    clave: string;
    hora: string;
    grupo: Grupo;
    tono: Tono;
    icono: typeof History;
    titulo: string;
    chip: string;
    detalle: string | null;
    meta: string | null;
    /** Lo que abre (solo las vueltas del informe). */
    version?: string | null;
    vuelta?: string | null;
  }

  const items = $derived.by(() => {
    const out: Item[] = [];
    // Lo que ya está (para no contar dos veces lo que traen el informe y el historial).
    const vistas = new Set<string>();
    const ms = (h: string) => Date.parse(h);
    const deCopia = (id: string | null | undefined) => copias.find((k) => k.id === id)?.nombre;
    const gancho = (clave: string, copia: string, nombre: string | undefined, g: ResultadoGancho, hora: string): Item => ({
      clave,
      hora,
      grupo: "copias",
      tono: g.estado === "ok" ? "ok" : g.estado === "aviso" ? "warn" : "bad",
      icono: Database,
      titulo: `${NOMBRE_GANCHO[g.tipo] ?? g.tipo} · «${nombre ?? copia}»`,
      chip: g.estado === "ok" ? "Correcto" : g.estado === "aviso" ? "Con avisos" : "Falló",
      detalle: g.mensaje,
      meta: "antes de copiar",
    });
    // Lo añadido de cada vuelta, desde su versión (misma copia y misma hora).
    for (const e of inf?.ejecuciones ?? []) {
      const v = versionDeVuelta(versionesDe(inf), e);
      const nombre = deCopia(e.copia);
      vistas.add(`c|${e.copia}|${ms(e.hora)}`);
      out.push({
        clave: `e|${e.copia}|${e.hora}`,
        hora: e.hora,
        grupo: "copias",
        tono: TONO_RESULTADO[e.resultado],
        icono: RefreshCw,
        titulo: nombre ? `Copia «${nombre}»` : "Copia",
        chip: TEXTO_RESULTADO[e.resultado],
        detalle: e.resultado === "ok" || e.resultado === "sin_cambios" ? null : (e.mensaje_corto ?? null),
        meta: v ? [v.duracion_s != null ? `${Math.round(v.duracion_s / 60) || 1} min` : null, anadidoDe(v) != null ? `+${bytes(anadidoDe(v))}` : null].filter(Boolean).join(" · ") || null : null,
        version: v?.id ?? null,
        vuelta: e.hora,
      });
    }
    // El historial del equipo: vueltas de antes (las que el informe ya no trae),
    // sus ganchos y cada verificación, prueba de restauración y copia externa.
    const TAREA: Record<string, [typeof History, string]> = {
      verificacion: [ShieldCheck, "Verificación"],
      prueba_restauracion: [ArchiveRestore, "Prueba de restauración"],
      externa: [CloudUpload, "Copia externa"],
    };
    for (const h of historial) {
      if (h.repo !== repo.id) continue;
      if (h.tipo === "copia" && h.resultado) {
        const nombre = deCopia(h.copia);
        if (!vistas.has(`c|${h.copia}|${ms(h.hora)}`)) {
          vistas.add(`c|${h.copia}|${ms(h.hora)}`);
          out.push({
            clave: `h|${h.id}`,
            hora: h.hora,
            grupo: "copias",
            tono: TONO_RESULTADO[h.resultado],
            icono: RefreshCw,
            titulo: nombre ? `Copia «${nombre}»` : "Copia",
            chip: TEXTO_RESULTADO[h.resultado],
            detalle: h.resultado === "ok" || h.resultado === "sin_cambios" ? null : (h.mensaje ?? null),
            meta: [h.duracion_s != null ? `${Math.round(h.duracion_s / 60) || 1} min` : null, h.anadido != null && h.resultado !== "sin_cambios" ? `+${bytes(h.anadido)}` : null].filter(Boolean).join(" · ") || null,
          });
        }
        for (const [gi, g] of (h.ganchos ?? []).entries()) out.push(gancho(`hg|${h.id}|${gi}`, h.copia ?? "", nombre, g, h.hora));
        if (h.ganchos?.length) vistas.add(`g|${h.copia}|${ms(h.hora)}`);
      } else if (h.tipo === "resumen_dia") {
        // Más de un año atrás: el equipo lo guarda resumido, una entrada por copia y día.
        const nombre = deCopia(h.copia);
        const [ok, mal, igual] = [h.ok ?? 0, h.fallidas ?? 0, h.sin_cambios ?? 0];
        const partes = [ok ? `${ok} ${ok === 1 ? "correcta" : "correctas"}` : null, mal ? `${mal} ${mal === 1 ? "fallida" : "fallidas"}` : null, igual ? `${igual} sin cambios` : null];
        out.push({
          clave: `h|${h.id}`,
          hora: h.hora,
          grupo: "copias",
          tono: mal && !ok && !igual ? "bad" : mal ? "warn" : "ok",
          icono: RefreshCw,
          titulo: nombre ? `Copias del día · «${nombre}»` : "Copias del día",
          chip: mal ? "Con fallos" : "Correctas",
          detalle: [partes.filter(Boolean).join(" · "), mal && h.ultimo_error ? `Último error: ${h.ultimo_error}` : null].filter(Boolean).join(". ") || null,
          meta: [h.duracion_s ? `${Math.round(h.duracion_s / 60) || 1} min en total` : null, h.anadido ? `+${bytes(h.anadido)}` : null].filter(Boolean).join(" · ") || null,
        });
      } else if (TAREA[h.tipo] && h.resultado && h.resultado !== "sin_cambios") {
        const [icono, titulo] = TAREA[h.tipo];
        vistas.add(`t|${h.tipo}|${ms(h.hora)}`);
        out.push({ clave: `h|${h.id}`, hora: h.hora, grupo: "mantenimiento", tono: TONO_TAREA[h.resultado], icono, titulo, chip: TEXTO_TAREA[h.resultado], detalle: h.mensaje ?? null, meta: null });
      }
    }
    const tareas: [string, string, typeof History, string, ReturnType<typeof verificacion>][] = [
      ["verificacion", "verificacion", ShieldCheck, "Verificación", verificacion(repo, inf)],
      ["prueba", "prueba_restauracion", ArchiveRestore, "Prueba de restauración", pruebaRestauracion(repo, inf)],
      ["externa", "externa", CloudUpload, "Copia externa", inf?.externa ?? null],
    ];
    for (const [k, tipo, icono, titulo, t] of tareas) {
      if (!t?.ultima || vistas.has(`t|${tipo}|${ms(t.ultima)}`)) continue;
      out.push({ clave: k, hora: t.ultima, grupo: "mantenimiento", tono: TONO_TAREA[t.resultado], icono, titulo, chip: TEXTO_TAREA[t.resultado], detalle: t.mensaje_corto ?? null, meta: null });
    }
    for (const k of ultimas) {
      if (!copias.some((x) => x.id === k.id && x.repo === repo.id) || !k.cuando || vistas.has(`g|${k.id}|${ms(k.cuando)}`)) continue;
      for (const [gi, g] of (k.ganchos ?? []).entries()) out.push(gancho(`g|${k.id}|${gi}|${k.cuando}`, k.id, k.nombre ?? undefined, g, k.cuando));
    }
    return out.sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora));
  });
  /** 60 días con lo que trae el informe; con el historial del equipo, desde lo más antiguo que tenga. */
  /** El día, con el año si no es este (el historial del equipo puede ser de hace años). */
  const diaDe = (iso: string) => {
    const y = new Date(iso).getFullYear();
    return y === new Date().getFullYear() ? dia(iso) : `${dia(iso)} de ${y}`;
  };
  const desde = $derived(items.length && Date.parse(items.at(-1)!.hora) < Date.now() - 61 * 86_400_000 ? items.at(-1)!.hora : null);

  const FILTROS: { id: "todo" | Grupo | "problemas"; texto: string }[] = [
    { id: "todo", texto: "Todo" },
    { id: "copias", texto: "Copias" },
    { id: "mantenimiento", texto: "Mantenimiento" },
    { id: "problemas", texto: "Problemas" },
  ];
  let filtro = $state<(typeof FILTROS)[number]["id"]>("todo");
  const PASO = 25;
  let limite = $state(PASO);
  const delDia = $derived(diaElegido ? items.filter((i) => claveDia(new Date(i.hora)) === diaElegido) : items);
  const vistos = $derived(filtro === "todo" ? delDia : filtro === "problemas" ? delDia.filter((i) => i.tono === "bad" || i.tono === "warn") : delDia.filter((i) => i.grupo === filtro));
  const abrible = (i: Item) => !!((i.version && alAbrirVersion) || (i.vuelta && alAbrirVuelta));
  const abrir = (i: Item) => (i.version && alAbrirVersion ? alAbrirVersion(i.version) : i.vuelta && alAbrirVuelta ? alAbrirVuelta(i.vuelta) : undefined);
  const grupos = $derived.by(() => {
    const out: { dia: string; items: Item[] }[] = [];
    for (const i of vistos.slice(0, limite)) {
      const d = diaDe(i.hora);
      if (out.at(-1)?.dia === d) out.at(-1)!.items.push(i);
      else out.push({ dia: d, items: [i] });
    }
    return out;
  });
  const ICONO_TONO = { ok: CircleCheck, warn: TriangleAlert, bad: CircleAlert, info: CircleCheck, paused: CircleDashed, neutral: CircleDashed };
</script>

<section class="card historia" aria-labelledby="t-historia">
  <div class="cab">
    <h2 class="section-title" id="t-historia">Historia <span class="count">· {diaElegido ? `el ${diaDe(`${diaElegido}T12:00:00`)}` : desde ? `desde el ${diaDe(desde)}` : "últimos 60 días"}</span></h2>
    <div class="segmented inline" role="group" aria-label="Qué mostrar">
      {#each FILTROS as f (f.id)}
        <button class:on={filtro === f.id} aria-pressed={filtro === f.id} onclick={() => ((filtro = f.id), (limite = PASO))}>{f.texto}</button>
      {/each}
    </div>
  </div>

  {#if !vistos.length}
    <div class="empty-state">
      <History size={28} strokeWidth={1.5} />
      <p>{filtro === "problemas" ? desde ? "Sin fallos ni avisos." : "Sin fallos ni avisos en estos 60 días." : "Todavía no hay nada que contar aquí."}</p>
    </div>
  {:else}
    {#each grupos as g (g.dia)}
      <h3 class="overline dia">{g.dia}</h3>
      <ul class="lista">
        {#each g.items as i (i.clave)}
          {@const Icono = i.icono}
          {@const IconoTono = ICONO_TONO[i.tono]}
          <li class="fila-h tone-{i.tono}">
            <span class="ic" aria-hidden="true"><Icono size={16} /></span>
            <div class="principal">
              <div class="linea">
                {#if abrible(i)}<button class="pulsable titulo" use:tip={"Ver detalle"} onclick={() => abrir(i)}>{i.titulo}</button>{:else}<strong>{i.titulo}</strong>{/if}
                <span class="badge badge-sm tone-{i.tono}"><IconoTono size={12} />{i.chip}</span>
              </div>
              {#if i.meta}<span class="faint meta">{i.meta}</span>{/if}
              {#if i.detalle}<span class="detalle">{i.detalle}</span>{/if}
            </div>
            <span class="cuando num" use:tip={fechaLarga(i.hora)}>{hora(i.hora)}</span>
          </li>
        {/each}
      </ul>
    {/each}
    {#if vistos.length > limite}
      <button class="btn btn-ghost mas" onclick={() => (limite += PASO)}>Mostrar más ({vistos.length - limite} más)</button>
    {/if}
  {/if}
</section>

<style>
  .historia {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-5);
  }
  .cab {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
  }
  .count {
    font-weight: 400;
    color: var(--text-3);
  }
  .dia {
    margin: var(--sp-3) 0 var(--sp-1);
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .lista {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .fila-h {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr) auto;
    align-items: start;
    gap: var(--sp-3);
    min-height: 44px;
    padding: 10px 0;
    border-top: 1px solid var(--border);
  }
  .ic {
    display: grid;
    padding-top: 2px;
    color: var(--text-3);
  }
  .principal {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .linea {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  .linea strong,
  .linea .titulo {
    font-weight: 500;
  }
  .meta,
  .detalle {
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
    overflow-wrap: anywhere;
  }
  .detalle {
    color: var(--text-2);
  }
  .tone-bad .detalle {
    color: var(--bad);
  }
  .cuando {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .mas {
    align-self: center;
  }
</style>
