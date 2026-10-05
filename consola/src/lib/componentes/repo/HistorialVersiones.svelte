<script lang="ts">
  import { tip } from "$lib/tooltip";
  // «Historial y versiones» (docs/diseno.md §4): una sola sección con una sola
  // línea de tiempo para un repositorio, una copia o un equipo entero. Junta lo
  // que antes eran «Versiones guardadas» (el calendario de calor y la
  // bitácora) y «Historia» (cada vuelta, comprobación y subida): arriba el
  // calendario para saltar (pulsar una casilla o un día filtra; el día va en
  // la URL con `dia`/`alDia`), debajo los filtros y la bitácora por días, con
  // las versiones y lo demás en el mismo riel. Pulsar una versión la abre en
  // su fila con sus datos y sus acciones (Detalle, Explorar, Restaurar); un
  // suceso con su vuelta en el informe abre su detalle. En un equipo, un
  // desplegable elige una copia o un repositorio. «Cargar más» pide lo de
  // antes al historial del equipo.
  import { Archive, Check, Copy, FolderSearch, History, PanelRightOpen } from "@lucide/svelte";
  import "../detalle/pulsable.css";
  import type { CopiaResumen, EntradaHistorial, Informe, Regla, RepoInforme, RepositorioResumen, VersionInforme } from "$lib/tipos";
  import { bytes, fechaLarga, numero, plural, relativo } from "$lib/formato";
  import { anadidoDe, duracion, nVersiones, versionesDe } from "$lib/repo";
  import { retencionDe } from "$lib/lineaTiempo";
  import { sucesosDe, type Suceso } from "$lib/historial";
  import { textoRegla, type Periodo } from "$lib/retencion";
  import BotonCargando from "../BotonCargando.svelte";
  import LineaTiempoVersiones from "./LineaTiempoVersiones.svelte";

  interface Fuente {
    repo: RepositorioResumen;
    inf: RepoInforme | null;
    /** La retención que se le aplica (para «se conserva» / «la quitará la retención»). */
    regla?: Regla | null;
    quien?: string | null;
  }
  let {
    fuentes,
    copias,
    historial = [],
    ultimas = [],
    soloCopia = null,
    equipo = false,
    enlace,
    ahora,
    puedeRestaurar,
    vacio,
    titulo = "Historial y versiones",
    resumen = null,
    alAbrir,
    alAbrirVuelta,
    elegida = null,
    dia = null,
    alDia,
    hayMas = false,
    cargandoMas = false,
    alCargarMas,
  }: {
    fuentes: Fuente[];
    copias: CopiaResumen[];
    /** El historial que guarda el propio equipo (v1.23; vacío con un servidor anterior). */
    historial?: EntradaHistorial[];
    /** Las copias del último informe del equipo, con el resultado de sus pasos previos (v1.10). */
    ultimas?: NonNullable<Informe["datos"]["copias"]>;
    /** Solo lo de esta copia (y lo de su repositorio: comprobaciones y subidas). */
    soloCopia?: string | null;
    /** Un equipo entero: varios repositorios, con un desplegable para elegir uno o una copia. */
    equipo?: boolean;
    /** URL de restaurar para una versión de un repositorio (`todo`: la versión entera). */
    enlace: (repo: string, v: VersionInforme, todo: boolean) => string;
    ahora: number;
    puedeRestaurar: boolean;
    vacio: string;
    titulo?: string;
    /** Una línea más bajo el título (en una copia, sus vueltas de 60 días en una frase). */
    resumen?: string | null;
    /** Abrir el detalle de una versión (con un filtro: directamente sus archivos nuevos o cambiados). */
    alAbrir?: (v: VersionInforme, repo: string, filtro?: "nuevos" | "cambiados") => void;
    /** Abrir el detalle de una vuelta sin versión (fallida, sin cambios). */
    alAbrirVuelta?: (hora: string, repo: string | null) => void;
    /** La versión que se está viendo en detalle (se abre en la bitácora y se trae a la vista). */
    elegida?: string | null;
    dia?: string | null;
    alDia?: (k: string | null) => void;
    /** Hay más historial del equipo por leer («Cargar más»). */
    hayMas?: boolean;
    cargandoMas?: boolean;
    alCargarMas?: () => void;
  } = $props();

  const id = $props.id();

  // En un equipo: todo, un repositorio («r:…») o una copia («k:…»).
  let ver = $state("todo");
  const verRepo = $derived(ver.startsWith("r:") ? ver.slice(2) : ver.startsWith("k:") ? (copias.find((k) => k.id === ver.slice(2))?.repo ?? null) : null);
  const copia = $derived(soloCopia ?? (ver.startsWith("k:") ? ver.slice(2) : null));
  const vistas = $derived(verRepo ? fuentes.filter((f) => f.repo.id === verRepo) : fuentes);
  const varios = $derived(equipo && vistas.length > 1);

  const versiones = $derived(
    vistas.flatMap(({ repo, inf }) =>
      versionesDe(inf)
        .filter((v) => !copia || v.copia === copia)
        .map((v) => ({ v, repo: repo.id })),
    ),
  );
  const porId = $derived(new Map(versiones.map((x) => [x.v.id, x])));
  const linea = $derived(versiones.map(({ v, repo }) => ({ id: v.id, hora: v.hora, copia: v.copia, repo, bytes: v.total_bytes, anadido: anadidoDe(v), archivos: null, etiquetas: v.etiquetas ?? [] })));
  const sucesos = $derived(sucesosDe({ fuentes: vistas, copias, historial, ultimas, soloCopia: copia, conRepo: varios, conEquipo: equipo && !verRepo }));

  // La retención: la de cada repositorio, simulada con todas sus versiones (como en su página).
  const conRegla = $derived(vistas.filter((f) => f.regla));
  const motivos = $derived.by(() => {
    if (!conRegla.length) return null;
    const m = new Map<string, Periodo | null>();
    const visibles = new Set(versiones.map((x) => x.v.id));
    for (const f of conRegla) for (const [k, x] of retencionDe(versionesDe(f.inf), f.regla ?? null) ?? []) if (visibles.has(k)) m.set(k, x);
    return m;
  });
  const textoRetencion = $derived(
    conRegla.length === 1
      ? `Simulado con la retención ${conRegla[0].quien ? `que aplica ${conRegla[0].quien}` : vistas.length > 1 ? `de «${conRegla[0].repo.nombre}»` : "del repositorio"}: ${textoRegla(conRegla[0].regla!)}.`
      : conRegla.length > 1
        ? "Simulado con la retención de cada repositorio."
        : null,
  );

  // La abierta en la bitácora: la del detalle o la que se pulsa (pulsar otra vez la cierra).
  let abierta = $state<string | null>(null);
  $effect(() => {
    if (elegida) abierta = elegida;
  });
  const alternar = (x: string) => (abierta = abierta === x ? null : x);

  /** Cuánto cambió el tamaño de lo copiado respecto a la versión anterior de la misma copia. */
  const deltas = $derived.by(() => {
    const m = new Map<string, number>();
    for (const { repo, inf } of vistas) {
      const vs = versionesDe(inf);
      for (const [i, v] of vs.entries()) {
        if (v.total_bytes == null) continue;
        const antes = vs.slice(i + 1).find((x) => x.copia === v.copia && x.total_bytes != null);
        if (antes) m.set(`${repo.id}|${v.id}`, v.total_bytes - antes.total_bytes!);
      }
    }
    return m;
  });
  const fmtDelta = (d: number) => (Math.abs(d) < 1 ? "igual" : `${d > 0 ? "+" : "−"}${bytes(Math.abs(d))}`);

  let copiado = $state<string | null>(null);
  async function copiarId(x: string) {
    try {
      await navigator.clipboard.writeText(x);
      copiado = x;
      setTimeout(() => copiado === x && (copiado = null), 1400);
    } catch {
      /* sin portapapeles */
    }
  }
  const cambios = (v: VersionInforme) =>
    v.archivos_nuevos != null || v.archivos_cambiados != null ? `${numero(v.archivos_nuevos ?? 0)} nuevos · ${numero(v.archivos_cambiados ?? 0)} cambiados` : null;
  const nombreRepo = (r: string) => fuentes.find((f) => f.repo.id === r)?.repo.nombre ?? r;
  const abrirSuceso = (s: Suceso) => s.vuelta && alAbrirVuelta?.(s.vuelta, s.repo);

  // La línea de debajo del título: cuántas versiones y de cuándo es lo leído.
  const total = $derived(soloCopia || copia ? versiones.length : vistas.reduce((n, f) => n + nVersiones(f.repo, f.inf), 0));
  const leidas = $derived(
    vistas
      .map((f) => f.inf?.versiones_leidas)
      .filter((x): x is string => !!x)
      .sort()
      .at(-1),
  );
  const recortado = $derived(vistas.some((f) => f.inf?.recortado));
  const repos = $derived([...fuentes].sort((a, b) => a.repo.nombre.localeCompare(b.repo.nombre)));
</script>

<section class="card historial" aria-labelledby="t-historial">
  <header class="cab">
    <div>
      <h2 class="section-title" id="t-historial">{titulo}</h2>
      <p class="faint">
        {plural(total, "versión", "versiones")}{!copia && total > versiones.length && versiones.length ? ` · aquí, las de los últimos 60 días (${numero(versiones.length)})` : ""}{equipo && !verRepo ? ` en ${plural(fuentes.length, "repositorio", "repositorios")}` : ""}
        {#if leidas}<span class="leidas">{" · "}leídas {relativo(leidas, ahora)}</span>{/if}
      </p>
      {#if resumen}<p class="faint num">{resumen}</p>{/if}
    </div>
  </header>

  {#if !versiones.length && !sucesos.sucesos.length}
    <div class="empty-state">
      <Archive size={28} strokeWidth={1.6} />
      <strong>{total ? "El equipo aún no ha enviado el detalle de sus versiones" : "Todavía no hay versiones"}</strong>
      <span class="faint">{vacio}</span>
    </div>
  {:else}
    <LineaTiempoVersiones
      versiones={linea}
      {copias}
      motivosDados={motivos}
      {textoRetencion}
      {ahora}
      seleccion={abierta}
      alElegir={alternar}
      etiqueta={equipo ? "Historial y versiones del equipo" : `Historial y versiones${vistas[0] ? ` de «${vistas[0].repo.nombre}»` : ""}`}
      dia={dia}
      {alDia}
      sucesos={sucesos.sucesos}
      notas={sucesos.notas}
      alAbrirSuceso={alAbrirVuelta ? abrirSuceso : undefined}
      {nombreRepo}
    >
      {#snippet herramientas()}
        {#if equipo && (fuentes.length > 1 || copias.length > 1)}
          <label class="ver">
            <span class="sr-only">Qué copia o repositorio ver</span>
            <select class="input" bind:value={ver}>
              <option value="todo">Todas las copias</option>
              {#each repos as f (f.repo.id)}
                <optgroup label={f.repo.nombre}>
                  <option value="r:{f.repo.id}">Todo «{f.repo.nombre}»</option>
                  {#each copias.filter((k) => k.repo === f.repo.id) as k (k.id)}<option value="k:{k.id}">Copia «{k.nombre}»</option>{/each}
                </optgroup>
              {/each}
            </select>
          </label>
        {/if}
      {/snippet}
      {#snippet acciones(vid)}
        {@const x = porId.get(vid)}
        {#if x}
          {@const v = x.v}
          <div class="datos">
            <span class="num">{fechaLarga(v.hora)}{#if v.duracion_s != null}<span class="faint">{" · "}<span use:tip={"Duración de la copia"}>{duracion(v.duracion_s)}</span></span>{/if}{#if varios}<span class="faint">{" · "}«{nombreRepo(x.repo)}»</span>{/if}</span>
            <span class="id-wrap">
              <code class="id selectable">{v.id}</code>
              <button class="icon-btn copiar" use:tip={"Copiar el id"} aria-label="Copiar el id de la versión {v.id}" onclick={() => copiarId(v.id)}>
                {#if copiado === v.id}<Check size={12} />{:else}<Copy size={12} />{/if}
              </button>
              {#if alAbrir && (v.archivos_nuevos != null || v.archivos_cambiados != null)}
                <span class="faint cambios">
                  <button class="pulsable" use:tip={"Ver qué archivos son nuevos"} onclick={() => alAbrir(v, x.repo, "nuevos")}>{numero(v.archivos_nuevos ?? 0)} nuevos</button>
                  ·
                  <button class="pulsable" use:tip={"Ver qué archivos cambiaron"} onclick={() => alAbrir(v, x.repo, "cambiados")}>{numero(v.archivos_cambiados ?? 0)} cambiados</button>
                </span>
              {:else if cambios(v)}<span class="faint cambios">{cambios(v)}</span>{/if}
            </span>
            <span class="tam num">
              <span use:tip={"Tamaño de lo copiado en esta versión"}>{bytes(v.total_bytes)}</span>
              {#if deltas.has(`${x.repo}|${v.id}`)}<span class="delta" use:tip={"Cambio de tamaño respecto a la versión anterior de esta copia"}>{" · "}{fmtDelta(deltas.get(`${x.repo}|${v.id}`)!)}</span>{/if}
              {#if anadidoDe(v) != null}<span class="anadido" use:tip={"Lo nuevo que ocupó en el repositorio (comprimido y sin duplicados)"}>{" · "}{bytes(anadidoDe(v))} nuevos en disco</span>{/if}
            </span>
          </div>
          {#if alAbrir}<button class="btn btn-sm" use:tip={"Qué cambió, cómo se hizo, lo que más ocupa"} onclick={() => alAbrir(v, x.repo)}><PanelRightOpen size={14} />Detalle</button>{/if}
          {#if puedeRestaurar}
            <a class="btn btn-sm btn-ghost" href={enlace(x.repo, v, false)} use:tip={"Ver los archivos de esta versión"}><FolderSearch size={14} />Explorar</a>
            <a class="btn btn-sm btn-primary" href={enlace(x.repo, v, true)} use:tip={"Restaurar la versión entera"}><History size={14} />Restaurar</a>
          {/if}
        {/if}
      {/snippet}
      {#snippet pie()}
        {#if hayMas && alCargarMas}
          <p class="faint mas-historial" id="{id}-mas">
            Se ha leído lo más reciente del historial del equipo ({plural(historial.length, "entrada", "entradas")}).
            <BotonCargando class="btn btn-ghost btn-sm" cargando={cargandoMas} textoCargando="Cargando…" onclick={alCargarMas}>Cargar más</BotonCargando>
          </p>
        {/if}
      {/snippet}
    </LineaTiempoVersiones>
    {#if recortado}<p class="faint recortado">Para que el informe del equipo no pese demasiado, aquí faltan las versiones más antiguas de estos 60 días.</p>{/if}
  {/if}
</section>

<style>
  .historial {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-5);
  }
  .cab p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .leidas {
    color: var(--text-3);
  }
  .ver .input {
    width: auto;
    max-width: min(100%, 18rem);
    height: 28px;
    padding: 0 8px;
    font-size: var(--fs-xs);
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
  .mas-historial {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
    margin: var(--sp-2) 0 0;
    font-size: var(--fs-xs);
  }
  .recortado {
    margin: 0;
    font-size: var(--fs-xs);
  }
  @media (max-width: 640px) {
    .ver,
    .ver .input {
      width: 100%;
      max-width: none;
    }
  }
</style>
