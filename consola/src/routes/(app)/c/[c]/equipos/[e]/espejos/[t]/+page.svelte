<script lang="ts">
  // La página de un espejo (plan 0.8.0, C2; plan-mejoras 14i): su estado y la
  // próxima subida, la subida en vivo (14p), el historial de sus vueltas con
  // gráficas como las de las copias (lo subido, cuánto tardó, lo que ocupa en
  // el destino), qué repositorios guarda y cuánto ocupa cada uno allí, sus
  // comprobaciones y lo que quitó su retención. Los enlaces a un espejo (camino
  // de la copia, página del destino, mapa) llegan aquí. Lo hace el almacén o el
  // propio equipo `e`; `t` es el id del trabajo.
  import { page } from "$app/state";
  import { untrack } from "svelte";
  import { ArrowRight, CalendarClock, ChartColumn, Cloud, Database, HardDrive, History, Info, Play, Settings2, ShieldAlert, ShieldCheck, Trash2 } from "@lucide/svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import Cifra from "$lib/componentes/Cifra.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import EnMarcha from "$lib/componentes/EnMarcha.svelte";
  import OrdenDialog from "$lib/componentes/OrdenDialog.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import TipoDestino from "$lib/componentes/TipoDestino.svelte";
  import GraficaBarras from "$lib/componentes/repo/GraficaBarras.svelte";
  import * as api from "$lib/api";
  import { actual, cargarCliente, puede, reloj } from "$lib/estado.svelte";
  import { seguirCambios, tocaEquipo } from "$lib/vivo.svelte";
  import { tareasDe } from "$lib/progreso.svelte";
  import { bytes, fechaLarga, numero, plural, relativo } from "$lib/formato";
  import { duracion } from "$lib/repo";
  import { diaLegible } from "$lib/espejo";
  import { hrefDestino, reposDeTrabajo } from "$lib/fichaDestino";
  import { catalogoDe, cargarCatalogo } from "$lib/catalogoDestinos.svelte";
  import { destinosParaPasos } from "$lib/cadenas";
  import { claveNube, claveZona, nombreZonaPorDefecto, zonasDe } from "$lib/destinos";
  import { clasificacionDeVista } from "$lib/regla321";
  import { corta, tipoDeNube } from "$lib/tipoDestino";
  import { AVISO_IGUAL, equiposDelAlmacen, estadoTrabajo, hrefEspejos, nombrePorDefecto, opcionesRepos, textoCuando, textoFreno, textoQue, textoRetencion, trabajosDelAlmacen, trabajosDelEquipo } from "$lib/espejoTrabajos";
  import { filasSubidas, ocupaPorRepo, resumenSubidas, TEXTO_MOTIVO } from "$lib/espejoPagina";
  import type { EquipoDetalle, TrabajoEspejoResumen } from "$lib/tipos";

  const c = $derived(page.params.c ?? "");
  const id = $derived(page.params.e ?? "");
  const tid = $derived(page.params.t ?? "");
  let equipo = $state<EquipoDetalle | null>(null);
  let error = $state("");

  $effect(() => {
    const [cc, ee] = [c, id];
    untrack(() => cargar(cc, ee));
  });
  function cargar(cc = c, ee = id) {
    api.equipo(cc, ee).then((e) => ee === id && (equipo = e), (e) => (error = e.message));
  }
  // Cada vuelta (y cada orden) al día, sin recargar.
  $effect(() => {
    const [cc, ee] = [c, id];
    return untrack(() =>
      seguirCambios(() => api.equipo(cc, ee).then((e) => ee === id && (equipo = e), () => {}), {
        ms: 0,
        toca: (x) => (x.t === "informe" || x.t === "equipo" || x.t === "orden") && tocaEquipo(x, ee),
      }),
    );
  });
  $effect(() => {
    const cc = c;
    if (cc) untrack(() => void cargarCatalogo(cc));
  });

  const deAlmacen = $derived(trabajosDelAlmacen(equipo).find((x) => x.id === tid) ?? null);
  const t = $derived<TrabajoEspejoResumen | null>(deAlmacen ?? trabajosDelEquipo(equipo).find((x) => x.id === tid) ?? null);
  const quien = $derived<"almacen" | "equipo">(deAlmacen ? "almacen" : "equipo");
  const equipos = $derived(equipo ? [...actual.equipos.filter((x) => x.id !== equipo!.id), equipo] : actual.equipos);
  const admin = $derived(!!actual.cliente && puede.administrar(actual.cliente.rol));
  const nombre = $derived(t ? t.nombre || nombrePorDefecto(t.adonde) : "Espejo");

  const estado = $derived(t ? estadoTrabajo(t) : "nuevo");
  const TONO = { ok: "ok", error: "bad", pausado: "paused", nuevo: "info" } as const;
  const TEXTO = { ok: "Al día", error: "Con error", pausado: "En pausa", nuevo: "Aún no ha terminado ninguna subida" } as const;

  // Adónde va: su nombre del catálogo, su tipo (icono) y la página del destino si la tiene.
  const vistas = $derived(destinosParaPasos(equipos, catalogoDe(c)));
  const destino = $derived.by(() => {
    if (!t || !equipo) return null;
    const hace = equipo;
    if (t.adonde.tipo === "zona") {
      const z = zonasDe(hace).find((x) => x.id === t.adonde.carpeta);
      const v = vistas.find((x) => x.zona && x.zona.almacen.id === hace.id && x.zona.id === t.adonde.carpeta);
      return { nombre: v?.nombre ?? (z ? nombreZonaPorDefecto(z) : "Otra zona"), tipo: v ? corta(clasificacionDeVista(v, equipos)) : null, href: hrefDestino(c, claveZona(hace.id, t.adonde.carpeta)) };
    }
    if (t.adonde.tipo === "nube") {
      const v = vistas.find((x) => x.nube && x.nube.equipo.id === hace.id && x.nube.nombre === t.adonde.nube);
      const tipo = [...(hace.resumen?.nubes ?? []), ...(hace.resumen?.guarda_copias?.nubes ?? [])].find((n) => n.nombre === t.adonde.nube)?.tipo;
      return {
        nombre: `${v?.nombre ?? t.adonde.nube} · ${t.adonde.carpeta}`,
        tipo: v ? corta(clasificacionDeVista(v, equipos)) : tipo ? tipoDeNube(tipo) : null,
        href: hrefDestino(c, claveNube(hace.id, t.adonde.nube ?? "")),
      };
    }
    return { nombre: t.adonde.carpeta, tipo: { tipo: "local" as const, inmutable: false, aislado: false, bloqueoDias: null }, href: null };
  });
  const nombreRepo = (r: string) => {
    if (!t || !equipo) return r;
    const o = opcionesRepos(equipo, quien, equipos, t.zona || "principal").find((x) => x.valor === r);
    return o ? (o.equipo ? `${o.nombre} (${o.equipo})` : o.nombre) : r;
  };
  const nombreEquipo = (u: string) => (equipo ? (equiposDelAlmacen(equipo, equipos).find((x) => x.usuario === u)?.nombre ?? u) : u);
  const nombreDe = (x: string) => {
    const o = [...trabajosDelAlmacen(equipo), ...trabajosDelEquipo(equipo)].find((y) => y.id === x);
    return o ? o.nombre || nombrePorDefecto(o.adonde) : "otro espejo";
  };

  // Su registro de vueltas (14i) y lo que ocupa en el destino.
  const filas = $derived(t ? filasSubidas(t) : []);
  const resumen = $derived(resumenSubidas(filas));
  const conOcupa = $derived(filas.filter((f) => f.ocupa != null));
  const repos = $derived(t && equipo ? reposDeTrabajo(t, equipo, equipos) : []);
  const ocupa = $derived(t ? ocupaPorRepo(t) : []);
  const filasRepos = $derived(
    repos.map((r) => ({ ...r, aqui: t?.ocupa?.repos?.[r.valor] ?? null })).sort((a, b) => (b.aqui ?? -1) - (a.aqui ?? -1)),
  );
  // Lo que ocupa algún repositorio que ya no entra (lo retiene el destino hasta que su retención lo quite).
  const fuera = $derived(ocupa.filter((o) => !repos.some((r) => r.valor === o.clave)));
  const vivo = $derived(equipo && t ? tareasDe(equipo.id, { repo: t.id, tipos: ["espejo"] }) : []);

  let orden = $state<{ cuerpo: Record<string, unknown>; titulo: string; descripcion: string } | null>(null);
  function ahora() {
    if (!t) return;
    orden = {
      cuerpo: { espejo_ahora: { trabajo: t.id, ...(quien === "equipo" ? { quien: "equipo" } : {}) } },
      titulo: `Hacer ahora «${nombre}»`,
      descripcion: "Empieza en cuanto pueda, sin esperar a su hora. Solo copia lo que falta: no cambia nada más. Se ve en vivo aquí y en todas las consolas.",
    };
  }
</script>

<svelte:head><title>{nombre} · Espejos de {equipo?.nombre ?? "equipo"} · Resguardo Server</title></svelte:head>

<div class="page">
  {#if !equipo || !actual.cliente}
    {#if error}<div class="notice notice-danger"><p>{error}</p></div>{:else}<Cargando />{/if}
  {:else if !t}
    <CabeceraPagina titulo="Este espejo ya no está" icono={Cloud} migas={[{ texto: "Equipos", href: `/c/${c}/equipos` }, { texto: equipo.nombre, href: `/c/${c}/equipos/${id}` }, { texto: "Espejos", href: `/c/${c}/equipos/${id}/espejos` }]} />
    <div class="notice notice-info"><Info size={16} /><p>Puede que se quitara. Los demás están en <a href="/c/{c}/equipos/{id}/espejos">los espejos de {equipo.nombre}</a>.</p></div>
  {:else}
    <CabeceraPagina
      titulo={nombre}
      icono={t.adonde.tipo === "nube" ? Cloud : HardDrive}
      migas={[{ texto: "Equipos", href: `/c/${c}/equipos` }, { texto: equipo.nombre, href: `/c/${c}/equipos/${id}` }, { texto: "Espejos", href: `/c/${c}/equipos/${id}/espejos` }, { texto: nombre }]}
    >
      {#snippet detalle()}
        <p class="linea">
          <Chip tono={TONO[estado]} texto={TEXTO[estado]} pequeno />
          <span>Lo hace {equipo?.nombre}{quien === "equipo" ? " con los repositorios de sus discos" : ""}:</span>
          <span>{textoQue(t, nombreRepo, nombreEquipo)}</span>
          <ArrowRight size={14} class="faint" aria-hidden="true" />
          <span class="dest">{#if destino?.tipo}<TipoDestino {...destino.tipo} />{/if}{#if destino?.href}<a class="link-suave" href={destino.href}>{destino.nombre}</a>{:else}{destino?.nombre}{/if}</span>
        </p>
      {/snippet}
      {#snippet acciones()}
        {#if admin && t.activo}<button class="btn btn-primary" onclick={ahora} disabled={!!vivo.length}><Play size={15} />Hacer ahora</button>{/if}
        {#if admin}<a class="btn" href={hrefEspejos(c, id, undefined, t.id)}><Settings2 size={15} />Cambiar</a>{/if}
      {/snippet}
    </CabeceraPagina>

    {#if vivo.length}
      <section class="card p" aria-label="Subida en marcha">
        <EnMarcha equipo={equipo.id} repo={t.id} tipos={["espejo"]} alTerminar={() => cargar()} />
      </section>
    {/if}

    {#if t.nunca_bien}
      <div class="notice notice-info"><Info size={16} /><p>Aún no ha terminado ninguna subida{t.ultima ? " bien" : ""}: hasta entonces no protege nada, así que se puede cambiar o quitar sin esperar.</p></div>
    {/if}
    {#if estado === "error" && t.resultado}<div class="notice notice-danger"><ShieldAlert size={16} /><p>{t.resultado.replace(/^ERROR:\s*/, "")}</p></div>{/if}
    {#if t.freno_aviso || t.retenidos}
      <div class="notice notice-warn"><ShieldAlert size={16} /><p><strong>Freno:</strong> {t.freno_aviso ? t.freno_aviso.replace(/\.?$/, ".") : `se conservan ${plural(t.retenidos?.archivos ?? 0, "archivo", "archivos")} que faltan en el original.`} Confírmalo desde <a href={hrefEspejos(c, id)}>los espejos de {equipo.nombre}</a> si sabes por qué falta.</p></div>
    {/if}

    <div class="cifras" role="list" aria-label="Cifras del espejo">
      <Cifra icono={History} etiqueta="Última subida" valor={t.ultima ? relativo(t.ultima, reloj.ahora) : "Todavía no"} detalle={t.ultima ? fechaLarga(t.ultima) : undefined} sub={t.bien && t.bien !== t.ultima ? `la última bien, ${relativo(t.bien, reloj.ahora)}` : estado === "error" ? "falló" : estado === "ok" ? "bien" : t.activo ? "pendiente" : "en pausa"} mal={estado === "error"} />
      <Cifra icono={CalendarClock} etiqueta="Próxima" valor={t.en_cola ? "En cola" : t.activo && t.proxima ? relativo(t.proxima, reloj.ahora) : t.activo ? "Tras una copia" : "En pausa"} detalle={t.proxima ? fechaLarga(t.proxima) : undefined} sub={textoCuando(t, nombreDe)} />
      <Cifra icono={Database} etiqueta="En el destino" valor={t.ocupa ? bytes(t.ocupa.bytes) : "—"} sub={t.ocupa ? `contado ${relativo(t.ocupa.cuando, reloj.ahora)}` : "se sabrá tras la próxima subida"} />
      <Cifra
        icono={ShieldCheck}
        etiqueta="Comprobación"
        valor={t.verificacion ? (t.verificacion.mal ? `${numero(t.verificacion.mal)} mal` : "Bien") : "—"}
        detalle={t.verificacion ? fechaLarga(t.verificacion.ultima) : undefined}
        sub={t.verificacion ? `${plural(t.verificacion.archivos, "archivo", "archivos")}, ${relativo(t.verificacion.ultima, reloj.ahora)}` : t.verificar_pct ? `el ${t.verificar_pct} % cada día` : "sin comprobación"}
        mal={!!t.verificacion?.mal}
      />
    </div>

    {#if filas.filter((f) => f.resultado === "ok").length >= 2}
      <section class="card p graficas" aria-labelledby="t-graf">
        <h2 class="section-title" id="t-graf"><ChartColumn size={16} />Sus subidas</h2>
        <!-- Como en las copias (14vv): a la izquierda el tamaño, a la derecha la actividad. -->
        <div class="col-graf" aria-label="Tamaño">
          <GraficaBarras titulo="Datos subidos" datos={filas} valor={(x) => x.bytes} formato={(x) => bytes(x)} />
          {#if conOcupa.length >= 2}<GraficaBarras titulo="Lo que ocupa en el destino" datos={conOcupa} valor={(x) => x.ocupa} formato={(x) => bytes(x)} />{/if}
        </div>
        <div class="col-graf" aria-label="Actividad">
          <GraficaBarras titulo="Archivos subidos" datos={filas} valor={(x) => x.archivos} formato={(x) => numero(Math.round(x))} />
          <GraficaBarras titulo="Duración de cada subida" datos={filas} valor={(x) => x.duracion_s} formato={(x) => duracion(x)} />
        </div>
      </section>
    {/if}

    <section aria-labelledby="t-hist">
      <div class="section-head">
        <h2 id="t-hist">Historial de subidas <span class="count">· {resumen.vueltas ? `las últimas ${resumen.vueltas}` : "ninguna todavía"}</span></h2>
      </div>
      {#if filas.length}
        <p class="faint resumen-hist">
          {plural(resumen.vueltas, "vuelta", "vueltas")}{resumen.fallidas ? `, ${resumen.fallidas} con error` : ""} · {bytes(resumen.bytes)} subidos{resumen.mediaDuracion != null ? ` · tarda ${duracion(resumen.mediaDuracion)} de media` : ""}{resumen.borrados ? ` · su retención quitó ${plural(resumen.borrados, "archivo", "archivos")} (${bytes(resumen.borradosBytes)})` : ""}
        </p>
        <div class="card p-0">
          <ul class="subidas">
            {#each filas as f (f.inicio + f.fin)}
              <li class:mal={f.resultado !== "ok"}>
                <div class="cuando">
                  <strong><Tiempo iso={f.fin} /></strong>
                  <span class="faint">{TEXTO_MOTIVO[f.motivo ?? ""] ?? ""}</span>
                </div>
                <div class="que num">
                  {#if f.resultado === "ok"}
                    <span>{f.archivos ? `${plural(f.archivos, "archivo", "archivos")} · ${bytes(f.bytes)}` : "Nada nuevo"}</span>
                    {#if f.duracion_s != null}<span class="faint">en {duracion(f.duracion_s)}</span>{/if}
                    {#if f.borrados}<span class="faint"><Trash2 size={12} aria-hidden="true" /> quitó {plural(f.borrados, "archivo", "archivos")} ({bytes(f.borrados_bytes ?? 0)})</span>{/if}
                    {#if f.verificados}<span class="faint">comprobó {plural(f.verificados, "archivo", "archivos")}{f.mal ? `, ${f.mal} mal` : ""}</span>{/if}
                  {:else}
                    <span class="error">{f.error ?? "Falló"}</span>
                  {/if}
                </div>
                <Chip pequeno tono={f.resultado === "ok" ? (f.mal ? "warn" : "ok") : "bad"} texto={f.resultado === "ok" ? (f.mal ? "Con avisos" : "Bien") : "Falló"} />
              </li>
            {/each}
          </ul>
        </div>
      {:else}
        <p class="faint">{t.ultima ? "Este equipo aún no guardaba el historial de cada subida: aparecerá desde la próxima." : "Todavía no se ha hecho ninguna."}</p>
      {/if}
    </section>

    <section aria-labelledby="t-repos">
      <div class="section-head"><h2 id="t-repos">Qué guarda <span class="count">· {plural(repos.length, "repositorio", "repositorios")}{t.ocupa ? ` · ${bytes(t.ocupa.bytes)} en el destino` : ""}</span> <Ayuda id="espejo" /></h2></div>
      {#if filasRepos.length || fuera.length}
        <div class="card p-0">
          <ul class="repos">
            {#each filasRepos as r (r.valor)}
              <li>
                <Database size={14} class="faint" aria-hidden="true" />
                {#if r.equipo && r.repo}<a class="link-suave" href="/c/{c}/equipos/{r.equipo.id}/repositorios/{encodeURIComponent(r.repo.id)}">{r.nombre}</a>{:else}<span class="mono">{r.nombre}</span>{/if}
                <span class="faint">{r.equipo ? r.equipo.nombre : "de un equipo que no está en esta consola"}</span>
                <span class="num tam">{r.aqui != null ? bytes(r.aqui) : "—"}</span>
              </li>
            {/each}
            {#each fuera as o (o.clave)}
              <li>
                <Database size={14} class="faint" aria-hidden="true" />
                <span class="mono">{o.clave}</span>
                <span class="faint">ya no entra: queda en el destino hasta que lo quite su retención</span>
                <span class="num tam">{bytes(o.bytes)}</span>
              </li>
            {/each}
          </ul>
        </div>
      {:else}
        <p class="faint">Ningún repositorio todavía.</p>
      {/if}
    </section>

    <section aria-labelledby="t-ret">
      <div class="section-head"><h2 id="t-ret">Retención y comprobación</h2></div>
      <div class="card p detalles">
        <p><strong>{textoRetencion(t)}.</strong>{#if t.retencion.modo === "igual"} {AVISO_IGUAL}{/if}</p>
        {#if t.por_borrar?.archivos}
          <p>{plural(t.por_borrar.archivos, "archivo espera", "archivos esperan")} para borrarse ({bytes(t.por_borrar.bytes)}){t.por_borrar.primero ? `: el primero, el ${diaLegible(t.por_borrar.primero)}` : ""}.</p>
        {/if}
        {#if resumen.borrados}<p>En las últimas {resumen.vueltas} vueltas quitó {plural(resumen.borrados, "archivo", "archivos")} ({bytes(resumen.borradosBytes)}) que ya no estaban en el original.</p>{/if}
        <p class="faint">Freno: {textoFreno(t.freno)}.</p>
        <p class="faint">
          {#if t.verificacion}Última comprobación {relativo(t.verificacion.ultima, reloj.ahora)}: {plural(t.verificacion.archivos, "archivo", "archivos")}{t.verificacion.mal ? `, ${t.verificacion.mal} mal` : ", todos bien"}.{:else}Sin comprobaciones todavía.{/if}
          {#if t.verificar_pct}Cada día se comprueba el {t.verificar_pct} % de lo que hay en el destino, sin contraseñas.{/if}
        </p>
      </div>
    </section>
  {/if}
</div>

{#if orden && equipo && actual.cliente}
  <OrdenDialog cliente={actual.cliente} {equipo} tipo="guarda_copias" cuerpo={orden.cuerpo} titulo={orden.titulo} descripcion={orden.descripcion} onclose={() => (orden = null)} alTerminar={() => (cargar(), void cargarCliente(c, { silencioso: true }))} />
{/if}

<style>
  .linea {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    overflow-wrap: anywhere;
  }
  .dest {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .cifras {
    margin-top: var(--sp-4);
  }
  section {
    margin-top: var(--sp-6);
  }
  .graficas {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-6);
  }
  .graficas > h2 {
    display: flex;
    align-items: center;
    gap: 6px;
    grid-column: 1 / -1;
    margin: 0 0 calc(var(--sp-3) - var(--sp-6));
  }
  .col-graf {
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
    min-width: 0;
  }
  .resumen-hist {
    margin: 0 0 var(--sp-2);
    font-size: var(--fs-sm);
  }
  .subidas,
  .repos {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .subidas li {
    display: grid;
    grid-template-columns: minmax(9rem, 12rem) minmax(0, 1fr) auto;
    gap: var(--sp-3);
    align-items: center;
    padding: 10px 16px;
    font-size: var(--fs-sm);
  }
  .subidas li + li,
  .repos li + li {
    border-top: 1px solid var(--border);
  }
  .cuando {
    display: grid;
    gap: 2px;
  }
  .cuando .faint {
    font-size: var(--fs-xs);
  }
  .que {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 10px;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .que .faint {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .error {
    color: var(--bad);
  }
  .repos li {
    display: grid;
    grid-template-columns: auto minmax(0, auto) minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    padding: 9px 16px;
    font-size: var(--fs-sm);
    overflow-wrap: anywhere;
  }
  .repos .faint {
    font-size: var(--fs-xs);
  }
  .tam {
    font-weight: 600;
  }
  .detalles {
    display: grid;
    gap: 6px;
  }
  .detalles p {
    margin: 0;
  }
  @media (max-width: 720px) {
    .graficas {
      grid-template-columns: minmax(0, 1fr);
    }
    .subidas li {
      grid-template-columns: minmax(0, 1fr) auto;
    }
    .subidas .que {
      grid-column: 1 / -1;
      grid-row: 2;
    }
    .repos li {
      grid-template-columns: auto minmax(0, 1fr) auto;
    }
    .repos li > .faint {
      grid-column: 2 / -1;
      grid-row: 2;
    }
  }
</style>
