<script lang="ts">
  import { tip } from "$lib/tooltip";
  // «Versiones guardadas» (docs/diseno.md §4): las versiones de los últimos 60
  // días del informe en el tiempo (LineaTiempoVersiones): el calendario de
  // calor arriba (pulsar una casilla o un día filtra; el día va en la URL con
  // `dia`/`alDia`) y la bitácora por días debajo. Pulsar una versión la abre
  // en su fila: la duración, el id (se copia), lo nuevo y lo cambiado (abren
  // el detalle filtrado), el tamaño y cuánto cambió, y sus acciones (Detalle,
  // Explorar, Restaurar). La que se ve en el detalle (`elegida`) se abre sola.
  import { Archive, Check, Copy, FolderSearch, History, PanelRightOpen } from "@lucide/svelte";
  import "../detalle/pulsable.css";
  import type { CopiaResumen, Regla, RepoInforme, RepositorioResumen, VersionInforme } from "$lib/tipos";
  import { bytes, fechaLarga, numero, plural, relativo } from "$lib/formato";
  import { anadidoDe, duracion, nVersiones, versionesDe } from "$lib/repo";
  import LineaTiempoVersiones from "./LineaTiempoVersiones.svelte";

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
    regla = null,
    quien = null,
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
    /** El calendario encima (la página de una copia ya enseña sus días). */
    cuadros?: boolean;
    /** Abrir el detalle de una versión (con un filtro: directamente sus archivos nuevos o cambiados). */
    alAbrir?: (v: VersionInforme, filtro?: "nuevos" | "cambiados") => void;
    /** La versión que se está viendo en detalle (se abre en la bitácora y se trae a la vista). */
    elegida?: string | null;
    /** El día elegido, si lo lleva la página (en la URL); si no, se guarda aquí. */
    dia?: string | null;
    alDia?: (k: string | null) => void;
    /** La retención que se le aplica (para «se conserva» / «la quitará la retención»). */
    regla?: Regla | null;
    quien?: string | null;
  } = $props();

  const versiones = $derived(versionesDe(inf));
  const porId = $derived(new Map(versiones.map((v) => [v.id, v])));
  const linea = $derived(versiones.map((v) => ({ id: v.id, hora: v.hora, copia: v.copia, bytes: v.total_bytes, anadido: anadidoDe(v), archivos: null, etiquetas: v.etiquetas ?? [] })));

  // La abierta en la bitácora: la del detalle o la que se pulsa (pulsar otra vez la cierra).
  let abierta = $state<string | null>(null);
  $effect(() => {
    if (elegida) abierta = elegida;
  });
  const alternar = (id: string) => (abierta = abierta === id ? null : id);

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
    <h2 class="section-title" id="t-versiones">{titulo}</h2>
    <p class="faint">
      {plural(nVersiones(repo, inf), "versión", "versiones")}{nVersiones(repo, inf) > versiones.length && versiones.length ? ` · aquí, las de los últimos 60 días (${numero(versiones.length)})` : ""}{#if inf?.versiones_leidas}<span class="leidas">{" · "}leídas {relativo(inf.versiones_leidas, ahora)}</span>{/if}
    </p>
  </header>

  {#if !versiones.length}
    <div class="empty-state">
      <Archive size={28} strokeWidth={1.6} />
      <strong>{nVersiones(repo, inf) ? "El equipo aún no ha enviado el detalle de sus versiones" : "Todavía no hay versiones"}</strong>
      <span class="faint">{vacio}</span>
    </div>
  {:else}
    <LineaTiempoVersiones
      versiones={linea}
      {copias}
      {regla}
      {quien}
      {ahora}
      seleccion={abierta}
      alElegir={alternar}
      etiqueta="Versiones de «{repo.nombre}»"
      conCalendario={conCuadros}
      dia={diaElegido}
      {alDia}
    >
      {#snippet acciones(id)}
        {@const v = porId.get(id)}
        {#if v}
          <div class="datos">
            <span class="num">{fechaLarga(v.hora)}{#if v.duracion_s != null}<span class="faint">{" · "}<span use:tip={"Duración de la copia"}>{duracion(v.duracion_s)}</span></span>{/if}</span>
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
            <span class="tam num">
              <span use:tip={"Tamaño de lo copiado en esta versión"}>{bytes(v.total_bytes)}</span>
              {#if deltas.has(v.id)}<span class="delta" use:tip={"Cambio de tamaño respecto a la versión anterior de esta copia"}>{" · "}{fmtDelta(deltas.get(v.id)!)}</span>{/if}
              {#if anadidoDe(v) != null}<span class="anadido" use:tip={"Lo nuevo que ocupó en el repositorio (comprimido y sin duplicados)"}>{" · "}{bytes(anadidoDe(v))} nuevos en disco</span>{/if}
            </span>
          </div>
          {#if alAbrir}<button class="btn btn-sm" use:tip={"Qué cambió, cómo se hizo, lo que más ocupa"} onclick={() => alAbrir(v)}><PanelRightOpen size={14} />Detalle</button>{/if}
          {#if puedeRestaurar}
            <a class="btn btn-sm btn-ghost" href={enlace(v, false)} use:tip={"Ver los archivos de esta versión"}><FolderSearch size={14} />Explorar</a>
            <a class="btn btn-sm btn-primary" href={enlace(v, true)} use:tip={"Restaurar la versión entera"}><History size={14} />Restaurar</a>
          {/if}
        {/if}
      {/snippet}
    </LineaTiempoVersiones>
    {#if inf?.recortado}<p class="faint recortado">Para que el informe del equipo no pese demasiado, aquí faltan las versiones más antiguas de estos 60 días.</p>{/if}
  {/if}
</section>

<style>
  .versiones {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-5);
  }
  header p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .leidas {
    color: var(--text-3);
  }
  /* Los datos de la versión abierta: encima de sus acciones, a todo lo ancho. */
  .datos {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex-basis: 100%;
    margin-bottom: 4px;
    font-size: var(--fs-sm);
    color: var(--text-2);
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
  /* Cifras en tinta neutra: el color queda para los estados. */
  .tam {
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .anadido {
    color: var(--text-3);
  }
  .recortado {
    margin: 0;
    font-size: var(--fs-xs);
  }
</style>
