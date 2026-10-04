<script lang="ts" module>
  import type { Largest } from "$lib/api";

  /**
   * Cálculos ya hechos o en marcha, por destino y versión. Una versión no
   * cambia, así que se reutilizan; y si se cierra el diálogo mientras se
   * calcula, al volver a abrirlo se sigue esperando el mismo cálculo.
   */
  const cache = new Map<string, { promise: Promise<Largest>; started: number }>();
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { ChartBar, CircleAlert, File, Folder, FolderOpen, Info, ListX, RefreshCw, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Repo, SizeItem, Snapshot } from "$lib/api";
  import { formatBytes, formatDate, formatNumber } from "$lib/format";
  import { displayPath, isWindowsSnapshot, splitPath } from "$lib/paths";
  import Modal from "./Modal.svelte";
  import ExcludeDialog from "./ExcludeDialog.svelte";
  import HelpLink from "./HelpLink.svelte";

  // «Lo que más ocupa»: las carpetas y los archivos más grandes de una versión,
  // con la opción de excluirlos de las próximas copias.
  interface Props {
    repo: Repo;
    snapshot: Snapshot;
    onclose: () => void;
    /** Abrir una carpeta en el explorador de la versión (ruta del snapshot). */
    onopenfolder?: (path: string) => void;
    /** Sin él no se ofrece excluir (p. ej. en el asistente de restauración). */
    onchange?: (repo: Repo) => void;
  }
  let { repo, snapshot, onclose, onopenfolder, onchange }: Props = $props();

  // svelte-ignore state_referenced_locally
  const windowsStyle = isWindowsSnapshot(snapshot.paths);
  // svelte-ignore state_referenced_locally
  const key = `${repo.id}:${snapshot.id}`;

  let data = $state<Largest | null>(null);
  let error = $state("");
  let loading = $state(true);
  let started = $state(Date.now());
  let now = $state(Date.now());
  let tab = $state<"folders" | "files">("folders");
  /** Rutas del snapshot marcadas (de las dos pestañas). */
  let selected = $state<Set<string>>(new Set());
  let excluding = $state<string[] | null>(null);

  const items = $derived<SizeItem[]>(data ? (tab === "folders" ? data.folders : data.files) : []);
  const elapsed = $derived(Math.max(0, Math.round((now - started) / 1000)));
  const elapsedLabel = $derived(elapsed < 60 ? `${elapsed} s` : `${Math.floor(elapsed / 60)} min ${elapsed % 60} s`);
  const allSelected = $derived(items.length > 0 && items.every((i) => selected.has(i.path)));
  const someSelected = $derived(items.some((i) => selected.has(i.path)));

  onMount(() => {
    load();
    const t = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(t);
  });

  async function load(force = false) {
    if (force) cache.delete(key);
    let entry = cache.get(key);
    if (!entry) {
      entry = { promise: api.snapshotLargest(repo.id, snapshot.id, 50), started: Date.now() };
      cache.set(key, entry);
    }
    started = entry.started;
    now = Date.now();
    loading = true;
    error = "";
    try {
      data = await entry.promise;
    } catch (e) {
      // Un error no se guarda: al reintentar se vuelve a calcular.
      if (cache.get(key) === entry) cache.delete(key);
      error = String(e);
    } finally {
      loading = false;
    }
  }

  const pct = (n: number) => (data && data.total_size > 0 ? (n / data.total_size) * 100 : 0);
  const pctLabel = (n: number) => {
    const p = pct(n);
    return p >= 10 ? `${Math.round(p)} %` : p >= 0.1 ? `${p.toFixed(1).replace(".", ",")} %` : "< 0,1 %";
  };

  function toggle(path: string) {
    const next = new Set(selected);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    selected = next;
  }

  function toggleAll() {
    const next = new Set(selected);
    for (const i of items) {
      if (allSelected) next.delete(i.path);
      else next.add(i.path);
    }
    selected = next;
  }
</script>

<Modal {onclose} labelledby="largest-title" width={780}>
  <header class="dlg-head">
    <div class="dlg-title">
      <span class="ticon"><ChartBar size={19} /></span>
      <div>
        <h2 id="largest-title">Lo que más ocupa <HelpLink topic="ocupa-espacio" label="encontrar qué ocupa espacio" /></h2>
        <p class="faint">
          Versión <span class="mono">{snapshot.short_id}</span> · {formatDate(snapshot.time)}{#if data}{" · "}<strong>{formatBytes(data.total_size)}</strong>
            en {formatNumber(data.total_files)} archivos{/if}
        </p>
      </div>
    </div>
    <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={onclose}><X size={17} /></button>
  </header>

  {#if loading}
    <div class="loading" role="status">
      <div class="bar" aria-hidden="true"><span></span></div>
      <p><strong>Recorriendo la versión…</strong> <span class="faint">{elapsedLabel}</span></p>
      <p class="faint small">
        Se suma el tamaño de cada carpeta sin descargar los archivos. En repositorios remotos o versiones con muchos archivos puede tardar
        unos minutos. Puedes cerrar esta ventana: el cálculo sigue y, si vuelves a abrirla, lo verás al terminar.
      </p>
    </div>
  {:else if error}
    <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>
    <footer>
      <button class="btn btn-ghost" onclick={onclose}>Cerrar</button>
      <button class="btn" onclick={() => load(true)}><RefreshCw size={14} /> Reintentar</button>
    </footer>
  {:else if data}
    <p class="explain faint">
      <Info size={13} /> Tamaño dentro de esta versión. Lo que ya estaba en otras versiones no vuelve a ocupar espacio (deduplicación).
    </p>

    <div class="tabs" role="tablist">
      <button role="tab" aria-selected={tab === "folders"} class:on={tab === "folders"} onclick={() => (tab = "folders")}>
        Carpetas <span class="count">{data.folders.length}</span>
      </button>
      <button role="tab" aria-selected={tab === "files"} class:on={tab === "files"} onclick={() => (tab = "files")}>
        Archivos <span class="count">{data.files.length}</span>
      </button>
    </div>

    {#if items.length === 0}
      <div class="empty faint">
        <FolderOpen size={22} />
        <span>{tab === "folders" ? "Esta versión no tiene carpetas con archivos." : "Esta versión no tiene archivos."}</span>
      </div>
    {:else}
      <div class="table" role="table" aria-label={tab === "folders" ? "Carpetas más grandes" : "Archivos más grandes"}>
        <div class="row head" class:no-check={!onchange} class:files={tab === "files"} role="row">
          {#if onchange}
            <span class="check">
              <input
                type="checkbox"
                checked={allSelected}
                indeterminate={someSelected && !allSelected}
                onchange={toggleAll}
                aria-label="Seleccionar todo"
              />
            </span>
          {/if}
          <span>{tab === "folders" ? "Carpeta" : "Archivo"}</span>
          <span class="share">Parte de la versión</span>
          <span class="num">Tamaño</span>
          {#if tab === "folders"}<span class="num fcount">Archivos</span>{/if}
        </div>
        <div class="rows">
          {#each items as item (item.path)}
            {@const shown = displayPath(item.path, windowsStyle)}
            {@const parts = splitPath(shown)}
            <div class="row" class:sel={selected.has(item.path)} class:no-check={!onchange} class:files={tab === "files"} role="row">
              {#if onchange}
                <span class="check">
                  <input
                    type="checkbox"
                    checked={selected.has(item.path)}
                    onchange={() => toggle(item.path)}
                    aria-label="Seleccionar {shown}"
                  />
                </span>
              {/if}
              {#if tab === "folders" && onopenfolder}
                <button class="path link-path" title="{shown}&#10;Abrir en el explorador de la versión" onclick={() => onopenfolder(item.path)}>
                  <span class="picon"><Folder size={15} fill="currentColor" fill-opacity="0.18" /></span>
                  <span class="parent mono">{parts.parent}</span><span class="name mono">{parts.name}</span>
                </button>
              {:else}
                <span class="path" title={shown}>
                  <span class="picon">{#if tab === "folders"}<Folder size={15} fill="currentColor" fill-opacity="0.18" />{:else}<File size={15} />{/if}</span>
                  <span class="parent mono">{parts.parent}</span><span class="name mono">{parts.name}</span>
                </span>
              {/if}
              <span class="share" title="{pctLabel(item.size)} del total de la versión">
                <span class="meter"><span style:width="{Math.max(1.5, pct(item.size))}%"></span></span>
                <span class="pct">{pctLabel(item.size)}</span>
              </span>
              <span class="num size">{formatBytes(item.size)}</span>
              {#if tab === "folders"}<span class="num faint fcount">{formatNumber(item.files)}</span>{/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <footer>
      {#if onchange && selected.size > 0}
        <span class="selinfo">
          <strong>{formatNumber(selected.size)}</strong>
          {selected.size === 1 ? "elemento seleccionado" : "elementos seleccionados"}
          <button class="link" onclick={() => (selected = new Set())}>Quitar selección</button>
        </span>
      {/if}
      <span class="grow"></span>
      <button class="btn btn-ghost" onclick={onclose}>Cerrar</button>
      {#if onchange}
        <button
          class="btn btn-primary"
          disabled={selected.size === 0}
          title={selected.size ? "Elegir en qué copias dejar de incluirlos" : "Marca antes las carpetas o archivos que no quieres copiar"}
          onclick={() => (excluding = [...selected])}
        >
          <ListX size={15} /> Excluir de las próximas copias…
        </button>
      {/if}
    </footer>
  {/if}
</Modal>

{#if excluding && onchange}
  <ExcludeDialog
    {repo}
    paths={excluding}
    {windowsStyle}
    onclose={() => (excluding = null)}
    onsaved={(r) => {
      onchange(r);
      selected = new Set();
    }}
  />
{/if}

<style>
  .loading {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 18px 0 6px;
  }
  .loading p {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .small {
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .bar {
    position: relative;
    height: 6px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-3);
  }
  .bar span {
    position: absolute;
    inset: 0 auto 0 0;
    width: 35%;
    border-radius: inherit;
    background: var(--accent);
    animation: slide 1.3s cubic-bezier(0.4, 0, 0.2, 1) infinite;
  }
  @keyframes slide {
    from {
      left: -35%;
    }
    to {
      left: 100%;
    }
  }
  .explain {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 0 10px;
    font-size: var(--fs-sm);
  }
  .explain :global(svg) {
    flex: none;
  }
  .tabs {
    display: flex;
    gap: 2px;
    border-bottom: 1px solid var(--border);
  }
  .tabs button {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 7px 12px 9px;
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--text-3);
    background: none;
    border: none;
    cursor: pointer;
  }
  .tabs button:hover,
  .tabs button.on {
    color: var(--text-1);
  }
  .tabs button::after {
    content: "";
    position: absolute;
    left: 8px;
    right: 8px;
    bottom: -1px;
    height: 2px;
    border-radius: 2px;
    background: var(--accent);
    transform: scaleX(0);
    transition: transform 0.2s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .tabs button.on::after {
    transform: scaleX(1);
  }
  .count {
    padding: 0 6px;
    font-size: var(--fs-overline);
    line-height: 17px;
    border-radius: 999px;
    background: var(--surface-3);
    color: var(--text-2);
  }
  .row {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) 150px 82px 70px;
    align-items: center;
    gap: 10px;
    padding: 0 6px 0 0;
  }
  .row.files {
    grid-template-columns: 32px minmax(0, 1fr) 150px 82px;
  }
  .row.no-check {
    grid-template-columns: minmax(0, 1fr) 150px 82px 70px;
    padding-left: 8px;
  }
  .row.no-check.files {
    grid-template-columns: minmax(0, 1fr) 150px 82px;
  }
  .head {
    height: 32px;
    font-size: var(--fs-overline);
    font-weight: 650;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--text-3);
    border-bottom: 1px solid var(--border);
  }
  .rows {
    max-height: min(52vh, 440px);
    overflow: auto;
  }
  .rows .row {
    min-height: 36px;
    border-bottom: 1px solid var(--border);
  }
  .rows .row:last-child {
    border-bottom: none;
  }
  .rows .row:hover {
    background: var(--surface-2);
  }
  .rows .row.sel {
    background: var(--accent-soft);
  }
  .check {
    display: grid;
    place-items: center;
  }
  input[type="checkbox"] {
    width: 15px;
    height: 15px;
    accent-color: var(--accent);
    cursor: pointer;
  }
  .path {
    display: flex;
    align-items: center;
    min-width: 0;
    font-size: var(--fs-sm);
  }
  .link-path {
    height: 100%;
    padding: 0;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-1);
    text-align: left;
    background: none;
    border: none;
    cursor: pointer;
  }
  .link-path:hover .name {
    color: var(--accent-text);
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .picon {
    display: grid;
    flex: none;
    margin-right: 8px;
    color: var(--text-3);
  }
  .link-path .picon {
    color: var(--accent);
  }
  .parent {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-3);
  }
  .name {
    flex: 0 1 auto;
    min-width: 3ch;
    max-width: 75%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  .share {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .meter {
    flex: 1;
    height: 6px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-3);
  }
  .meter span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--accent);
  }
  .pct {
    width: 44px;
    font-size: var(--fs-xs);
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--text-2);
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
    font-size: var(--fs-sm);
  }
  .size {
    font-weight: 600;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 30px 0;
    font-size: var(--fs-sm);
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 16px;
  }
  .grow {
    flex: 1;
  }
  .selinfo {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
    white-space: nowrap;
  }
  .link {
    padding: 0;
    margin-left: 4px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--accent-text);
    background: none;
    border: none;
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  @media (max-width: 760px) {
    .row,
    .row.files {
      grid-template-columns: 32px minmax(0, 1fr) 82px;
    }
    .row.no-check,
    .row.no-check.files {
      grid-template-columns: minmax(0, 1fr) 82px;
    }
    .share,
    .fcount {
      display: none;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .bar span {
      animation-duration: 3s;
    }
  }
</style>
