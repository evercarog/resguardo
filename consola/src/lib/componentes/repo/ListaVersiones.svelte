<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Versiones de los últimos 60 días (como SnapshotList de la app): los
  // cuadros por día arriba (pulsar uno filtra), y la lista agrupada por día
  // con la hora, la duración, el id, lo que ocupa y lo añadido. Cada versión
  // se puede explorar o restaurar entera. Con `alAbrir`, la hora y los
  // recuentos («3 nuevos», «2 cambiados») abren el detalle de la versión.
  import { Archive, Check, Copy, FolderSearch, History, PanelRightOpen, SearchX, X } from "@lucide/svelte";
  import "../detalle/pulsable.css";
  import type { CopiaResumen, RepoInforme, RepositorioResumen, VersionInforme } from "$lib/tipos";
  import { bytes, dia, fechaLarga, hora, numero, plural, relativo } from "$lib/formato";
  import { anadidoDe, claveDia, dias, duracion, nVersiones, versionesDe } from "$lib/repo";
  import DiasCuadros from "./DiasCuadros.svelte";

  let {
    repo,
    inf,
    copias,
    enlace,
    ahora,
    puedeRestaurar,
    vacio,
    titulo = "Versiones guardadas",
    cuadros: conCuadros = true,
    alAbrir,
    elegida = null,
    dia: diaElegido = null,
    alDia,
  }: {
    repo: RepositorioResumen;
    inf: RepoInforme | null;
    copias: CopiaResumen[];
    /** URL de restaurar para una versión (`todo`: la versión entera). */
    enlace: (v: VersionInforme, todo: boolean) => string;
    ahora: number;
    puedeRestaurar: boolean;
    vacio: string;
    titulo?: string;
    /** Los cuadros de 60 días encima (la página de una copia ya los muestra). */
    cuadros?: boolean;
    /** Abrir el detalle de una versión (con un filtro: directamente sus archivos nuevos o cambiados). */
    alAbrir?: (v: VersionInforme, filtro?: "nuevos" | "cambiados") => void;
    /** La versión que se está viendo en detalle (se marca y se trae a la vista). */
    elegida?: string | null;
    /** El día elegido, si lo lleva la página (en la URL); si no, se guarda aquí. */
    dia?: string | null;
    alDia?: (k: string | null) => void;
  } = $props();

  const versiones = $derived(versionesDe(inf));
  const cuadros = $derived(dias(inf, 60, ahora));
  let filtroLocal = $state<string | null>(null);
  const filtro = $derived(alDia ? diaElegido : filtroLocal);
  function elegirDia(k: string | null) {
    limite = PASO;
    if (alDia) alDia(k);
    else filtroLocal = k;
  }
  const vistas = $derived(filtro ? versiones.filter((v) => claveDia(new Date(v.hora)) === filtro) : versiones);
  const PASO = 30;
  let limite = $state(PASO);
  // La versión elegida, a la vista (y dentro de lo mostrado).
  $effect(() => {
    if (!elegida) return;
    const i = vistas.findIndex((v) => v.id === elegida);
    if (i >= limite) limite = i + 1;
    requestAnimationFrame(() => document.getElementById(`version-${elegida}`)?.scrollIntoView({ block: "nearest", behavior: "smooth" }));
  });
  const grupos = $derived.by(() => {
    const out: { dia: string; hace: string; items: VersionInforme[] }[] = [];
    for (const v of vistas.slice(0, limite)) {
      const d = dia(v.hora);
      if (out.at(-1)?.dia === d) out.at(-1)!.items.push(v);
      else out.push({ dia: d, hace: haceDias(v.hora), items: [v] });
    }
    return out;
  });
  /** «hoy», «ayer», «hace 5 días»: junto al día, para situarse sin contar. */
  function haceDias(iso: string) {
    const h = new Date(ahora);
    const d = new Date(iso);
    const n = Math.round((new Date(h.getFullYear(), h.getMonth(), h.getDate()).getTime() - new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()) / 86_400_000);
    return n <= 0 ? "hoy" : n === 1 ? "ayer" : `hace ${n} días`;
  }
  /**
   * Cuánto cambió el tamaño de lo copiado respecto a la versión anterior de
   * la misma copia (crece si se añadieron archivos, baja si se borraron).
   */
  const deltas = $derived.by(() => {
    const m = new Map<string, number>();
    for (const [i, v] of versiones.entries()) {
      if (v.total_bytes == null) continue;
      const antes = versiones.slice(i + 1).find((x) => x.copia === v.copia && x.total_bytes != null);
      if (antes) m.set(v.id, v.total_bytes - antes.total_bytes!);
    }
    return m;
  });
  const fmtDelta = (d: number) => (Math.abs(d) < 1 ? "igual" : `${d > 0 ? "+" : "−"}${bytes(Math.abs(d))}`);
  const nombreCopia = (id?: string | null) => copias.find((k) => k.id === id)?.nombre ?? null;
  const fmtFiltro = new Intl.DateTimeFormat("es", { weekday: "long", day: "numeric", month: "long" });

  let copiado = $state<string | null>(null);
  async function copiarId(id: string) {
    try {
      await navigator.clipboard.writeText(id);
      copiado = id;
      setTimeout(() => copiado === id && (copiado = null), 1400);
    } catch {
      /* sin portapapeles */
    }
  }
  const cambios = (v: VersionInforme) =>
    v.archivos_nuevos != null || v.archivos_cambiados != null
      ? `${numero(v.archivos_nuevos ?? 0)} nuevos · ${numero(v.archivos_cambiados ?? 0)} cambiados`
      : null;
</script>

<section class="card versiones" aria-labelledby="t-versiones">
  <header>
    <div>
      <h2 class="section-title" id="t-versiones">{titulo}</h2>
      <p class="faint">
        {#if filtro}{plural(vistas.length, "versión", "versiones")} el {fmtFiltro.format(new Date(`${filtro}T12:00:00`))}
        {:else}{plural(nVersiones(repo, inf), "versión", "versiones")}{nVersiones(repo, inf) > versiones.length && versiones.length ? ` · aquí, las de los últimos 60 días (${numero(versiones.length)})` : ""}{/if}{#if inf?.versiones_leidas}<span class="leidas">{" · "}leídas {relativo(inf.versiones_leidas, ahora)}</span>{/if}
      </p>
    </div>
    {#if filtro}<button class="btn btn-sm btn-ghost" onclick={() => elegirDia(null)}><X size={14} />Ver todas</button>{/if}
  </header>

  {#if conCuadros && (versiones.length || inf?.ejecuciones.length)}
    <div class="tira">
      <DiasCuadros dias={cuadros} etiqueta="Resultado de las copias en los últimos 60 días" elegido={filtro} alElegir={elegirDia} leyenda />
      <div class="pie-tira">
        <p class="faint pista">Últimos 60 días · pulsa un día con versiones para verlas</p>
        <!-- Lo mismo que los cuadros, con un control de tamaño cómodo (WCAG 2.5.8) y para el teclado. -->
        <label class="elegir-dia">
          <span class="sr-only">Ver las versiones de un día</span>
          <select class="input" value={filtro ?? ""} onchange={(e) => elegirDia(e.currentTarget.value || null)}>
            <option value="">Todos los días</option>
            {#each cuadros.filter((d) => d.n > 0 || d.estado !== "nada").reverse() as d (d.clave)}<option value={d.clave}>{d.titulo}</option>{/each}
          </select>
        </label>
      </div>
    </div>
  {/if}

  {#if !versiones.length}
    <div class="empty-state">
      <Archive size={28} strokeWidth={1.6} />
      <strong>{nVersiones(repo, inf) ? "El equipo aún no ha enviado el detalle de sus versiones" : "Todavía no hay versiones"}</strong>
      <span class="faint">{vacio}</span>
    </div>
  {:else if !vistas.length}
    <div class="empty-state">
      <SearchX size={28} strokeWidth={1.6} />
      <strong>Ese día no dejó versiones</strong>
      <button class="btn btn-sm btn-ghost" onclick={() => elegirDia(null)}>Ver todas</button>
    </div>
  {:else}
    {#each grupos as g (g.dia)}
      <h3 class="overline dia">{g.dia}<span class="hace">{" · "}{g.hace}</span></h3>
      <ul class="lista">
        {#each g.items as v (v.id)}
          <li class="fila-v" class:elegida={elegida === v.id} id="version-{v.id}">
            {#if alAbrir}
              <button class="pulsable-bloque hora num" use:tip={`${fechaLarga(v.hora)} · Ver detalle`} aria-label="Ver el detalle de la versión de las {hora(v.hora)}" onclick={() => alAbrir(v)}>
                <span class="h">{hora(v.hora)}</span>
                {#if v.duracion_s != null}<small>{duracion(v.duracion_s)}</small>{/if}
              </button>
            {:else}
              <span class="hora num" use:tip={fechaLarga(v.hora)}>
                {hora(v.hora)}
                {#if v.duracion_s != null}<small use:tip={"Duración de la copia"}>{duracion(v.duracion_s)}</small>{/if}
              </span>
            {/if}
            <span class="que">
              <span class="linea">
                <span class="copia">{nombreCopia(v.copia) ?? "Copia"}</span>
                {#each v.etiquetas ?? [] as t (t)}<span class="badge badge-sm tone-info">{t}</span>{/each}
              </span>
              <span class="id-wrap">
                <code class="id selectable">{v.id}</code>
                <button class="icon-btn copiar" use:tip={"Copiar el id"} aria-label="Copiar el id de la versión {v.id}" onclick={() => copiarId(v.id)}>
                  {#if copiado === v.id}<Check size={12} />{:else}<Copy size={12} />{/if}
                </button>
                {#if alAbrir && (v.archivos_nuevos != null || v.archivos_cambiados != null)}
                  <span class="faint cambios">
                    <button class="pulsable" use:tip={"Ver qué archivos son nuevos"} onclick={() => alAbrir(v, "nuevos")}>{numero(v.archivos_nuevos ?? 0)} nuevos</button>
                    ·
                    <button class="pulsable" use:tip={"Ver qué archivos cambiaron"} onclick={() => alAbrir(v, "cambiados")}>{numero(v.archivos_cambiados ?? 0)} cambiados</button>
                  </span>
                {:else if cambios(v)}<span class="faint cambios">{cambios(v)}</span>{/if}
              </span>
            </span>
            <span class="tam num">
              <span use:tip={"Tamaño de lo copiado en esta versión"}>{bytes(v.total_bytes)}</span>
              {#if deltas.has(v.id)}
                {@const d = deltas.get(v.id)!}
                <span class="delta" use:tip={"Cambio de tamaño respecto a la versión anterior de esta copia"}>{fmtDelta(d)}</span>
              {/if}
              {#if anadidoDe(v) != null}<span class="anadido" use:tip={"Lo nuevo que ocupó en el repositorio (comprimido y sin duplicados)"}>{bytes(anadidoDe(v))} nuevos en disco</span>{/if}
            </span>
            {#if puedeRestaurar || alAbrir}
              <span class="acc">
                {#if alAbrir}<button class="btn btn-sm btn-ghost" use:tip={"Ver detalle: qué cambió, cómo se hizo, lo que más ocupa"} onclick={() => alAbrir(v)}><PanelRightOpen size={14} />Detalle</button>{/if}
                {#if puedeRestaurar}
                <a class="btn btn-sm btn-ghost" href={enlace(v, false)} use:tip={"Ver los archivos de esta versión"}><FolderSearch size={14} />Explorar</a>
                <a class="btn btn-sm" href={enlace(v, true)} use:tip={"Restaurar la versión entera"}><History size={14} />Restaurar</a>
                {/if}
              </span>
            {/if}
          </li>
        {/each}
      </ul>
    {/each}
    {#if inf?.recortado}<p class="faint recortado">Para que el informe del equipo no pese demasiado, aquí faltan las versiones más antiguas de estos 60 días.</p>{/if}
    {#if vistas.length > limite}
      <button class="btn btn-ghost mas" onclick={() => (limite += PASO)}>Mostrar más ({numero(vistas.length - limite)} más)</button>
    {/if}
  {/if}
</section>

<style>
  .versiones {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-5);
  }
  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--sp-3);
  }
  header p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .tira {
    padding: var(--sp-3) var(--sp-4);
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .pie-tira {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 6px var(--sp-3);
    margin-top: 6px;
  }
  .pista {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .elegir-dia .input {
    width: auto;
    max-width: min(100%, 16rem);
    height: 28px;
    padding: 0 8px;
    font-size: var(--fs-xs);
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
  .fila-v {
    display: grid;
    grid-template-columns: 64px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: var(--sp-3);
    min-height: 52px;
    padding: 8px 0;
    border-top: 1px solid var(--border);
  }
  .hora {
    display: flex;
    flex-direction: column;
    font-weight: 500;
  }
  button.hora {
    width: auto;
    border-radius: var(--radius-sm);
  }
  button.hora:hover .h {
    color: var(--accent-text);
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .fila-v.elegida {
    background: var(--accent-soft);
    box-shadow: inset 3px 0 0 var(--accent);
    border-radius: var(--radius-sm);
  }
  .hora small {
    font-size: var(--fs-xs);
    font-weight: 400;
    color: var(--text-3);
  }
  .que {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .linea {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .copia {
    font-weight: 500;
    overflow-wrap: anywhere;
  }
  .id-wrap {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    font-size: var(--fs-xs);
  }
  .id {
    font-size: 11.5px;
    color: var(--text-2);
  }
  .copiar {
    width: 24px;
    height: 24px;
  }
  .cambios {
    margin-left: 4px;
  }
  .tam {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    font-size: var(--fs-sm);
  }
  /* Cifras en tinta neutra: el color queda para los estados. */
  .delta,
  .anadido {
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    color: var(--text-3);
  }
  .delta {
    color: var(--text-2);
  }
  .hace {
    font-weight: 500;
    letter-spacing: 0;
    text-transform: none;
  }
  .acc {
    display: flex;
    gap: 4px;
  }
  .mas {
    align-self: center;
  }
  .leidas {
    color: var(--text-3);
  }
  .recortado {
    margin: var(--sp-2) 0 0;
    font-size: var(--fs-xs);
  }
  @media (max-width: 640px) {
    .fila-v {
      grid-template-columns: 52px minmax(0, 1fr) auto;
    }
    .acc {
      grid-column: 2 / -1;
    }
  }
</style>
