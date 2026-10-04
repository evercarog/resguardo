<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { dur } from "$lib/motion";
  import {
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ChartBar,
    ChevronRight,
    CircleAlert,
    File,
    FileSymlink,
    Folder,
    FolderCheck,
    FolderOpen,
    HardDrive,
    History,
    ListX,
    Monitor,
    RotateCcw,
    Search,
    X,
  } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Entry, Repo, Snapshot } from "$lib/api";
  import { formatBytes, formatDate, formatDuration, formatNumber } from "$lib/format";
  import { breadcrumbs, commonDir, isWindowsSnapshot, joinPath, parentDir } from "$lib/paths";
  import RestoreDialog from "./RestoreDialog.svelte";
  import ChangesView from "./ChangesView.svelte";
  import LargestDialog from "./LargestDialog.svelte";
  import ExcludeDialog from "./ExcludeDialog.svelte";

  interface Props {
    repo: Repo;
    snapshot: Snapshot;
    /** Todos los snapshots del repositorio, para encontrar la copia anterior. */
    snapshots: Snapshot[];
    onclose: () => void;
    /**
     * Modo «elegir» (asistente de restauración): sin cabecera propia, y en vez
     * de abrir el diálogo de restaurar se devuelve la selección.
     */
    onpick?: (sel: { dir: string; names: string[]; label: string }) => void;
    /** Carpeta en la que empezar (p. ej. la misma que se veía en otra versión). */
    startDir?: string | null;
    /** Avisa al cambiar de carpeta. */
    ondirchange?: (dir: string) => void;
    /** El destino cambió (exclusiones nuevas). Sin él no se ofrece excluir. */
    onchange?: (repo: Repo) => void;
    /** Pestaña inicial («changes»: qué cambió respecto a la versión anterior). */
    startTab?: "files" | "changes";
  }
  let { repo, snapshot, snapshots, onclose, onpick, startDir = null, ondirchange, onchange, startTab = "files" }: Props = $props();

  /** «Lo que más ocupa» abierto. */
  let showLargest = $state(false);
  /** Rutas (del snapshot) que se van a excluir de las próximas copias. */
  let excluding = $state<string[] | null>(null);

  // svelte-ignore state_referenced_locally
  let tab = $state<"files" | "changes">(startTab);

  /** Copia anterior: la "padre" que registró restic o, si no, la anterior del mismo equipo y carpetas. */
  const sameSet = (s: Snapshot) =>
    s.hostname === snapshot.hostname && [...s.paths].sort().join("|") === [...snapshot.paths].sort().join("|");
  const previous = $derived(
    snapshots.find((s) => s.id === snapshot.parent) ??
      snapshots.filter((s) => s.time < snapshot.time && sameSet(s)).sort((a, b) => b.time.localeCompare(a.time))[0] ??
      null,
  );

  function reveal(target: string) {
    tab = "files";
    load(target);
  }

  const PAGE = 400;

  // svelte-ignore state_referenced_locally
  const home = commonDir(snapshot.paths);
  // Carpeta con la que se abrió (para saber si una carpeta vacía es que no existe en esta versión).
  // svelte-ignore state_referenced_locally
  const initialDir = startDir;
  // svelte-ignore state_referenced_locally
  let dir = $state(startDir ?? home);
  let entries = $state<Entry[]>([]);
  let loading = $state(true);
  let error = $state("");
  let filter = $state("");
  let selected = $state<Set<string>>(new Set());
  let limit = $state(PAGE);
  let restoring = $state<{ names: string[]; label: string } | null>(null);
  let filterInput = $state<HTMLInputElement>();

  // svelte-ignore state_referenced_locally
  const windowsStyle = isWindowsSnapshot(snapshot.paths);
  const crumbs = $derived(breadcrumbs(dir, windowsStyle));
  // Si la ruta no cabe, se ve el final (la carpeta actual), en el orden normal.
  let crumbsEl = $state<HTMLElement>();
  $effect(() => {
    void crumbs;
    if (crumbsEl) crumbsEl.scrollLeft = crumbsEl.scrollWidth;
  });
  const visible = $derived(
    filter.trim() ? entries.filter((e) => e.name.toLowerCase().includes(filter.trim().toLowerCase())) : entries,
  );
  const allSelected = $derived(visible.length > 0 && visible.every((e) => selected.has(e.name)));
  const selectedEntries = $derived(entries.filter((e) => selected.has(e.name)));
  const selectedBytes = $derived(selectedEntries.reduce((n, e) => n + (e.kind === "dir" ? 0 : (e.size ?? 0)), 0));
  const selectedHasDirs = $derived(selectedEntries.some((e) => e.kind === "dir"));
  // svelte-ignore state_referenced_locally
  const sum = snapshot.summary;
  const duration =
    sum?.backup_start && sum?.backup_end
      ? (new Date(sum.backup_end).getTime() - new Date(sum.backup_start).getTime()) / 1000
      : null;

  const dirName = $derived(crumbs.at(-1)?.label ?? "la raíz de la versión");

  onMount(() => load(dir));

  let request = 0;
  async function load(target: string) {
    const mine = ++request;
    dir = target;
    ondirchange?.(target);
    loading = true;
    error = "";
    filter = "";
    selected = new Set();
    limit = PAGE;
    try {
      const list = await api.listSnapshotDir(repo.id, snapshot.id, target);
      if (mine === request) entries = list;
    } catch (e) {
      if (mine === request) {
        entries = [];
        error = String(e);
      }
    } finally {
      if (mine === request) loading = false;
    }
  }

  function toggle(name: string) {
    const next = new Set(selected);
    if (next.has(name)) next.delete(name);
    else next.add(name);
    selected = next;
  }

  function toggleAll() {
    selected = allSelected ? new Set() : new Set(visible.map((e) => e.name));
  }

  function open(entry: Entry) {
    if (entry.kind === "dir") load(joinPath(dir, entry.name));
    else toggle(entry.name);
  }

  function restoreSelection() {
    const names = [...selected];
    const label =
      names.length === 1 ? `«${names[0]}»` : `${formatNumber(names.length)} elementos de «${dirName}»`;
    if (onpick) onpick({ dir, names, label });
    else restoring = { names, label };
  }

  function restoreAll() {
    const label = `todo el contenido de «${dirName}»`;
    if (onpick) onpick({ dir, names: [], label });
    else restoring = { names: [], label };
  }

  function excludeSelection() {
    excluding = [...selected].map((name) => joinPath(dir, name));
  }

  function onkeydown(e: KeyboardEvent) {
    if (restoring || excluding || showLargest || document.querySelector("[aria-modal='true']")) return;
    const typing = e.target instanceof HTMLInputElement;
    if (e.key === "Backspace" && !typing && dir !== "/") load(parentDir(dir));
    if (e.key === "Escape" && !typing) selected.size ? (selected = new Set()) : onclose();
    // Ctrl+F: al filtro de la carpeta.
    if (e.ctrlKey && (e.key === "f" || e.key === "F")) {
      e.preventDefault();
      filterInput?.focus();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="browser" class:embedded={!!onpick} in:fade={{ duration: dur(160) }}>
  {#if !onpick}
  <header class="top">
    <button class="btn btn-ghost btn-sm back" onclick={onclose}><ArrowLeft size={15} /> Volver</button>
    <div class="top-row">
      <div class="snap-info">
        <span class="snap-icon"><History size={18} /></span>
        <div>
          <h2>Versión <span class="mono">{snapshot.short_id}</span></h2>
          <p class="faint">
            {formatDate(snapshot.time)} · <Monitor size={12} />
            {snapshot.hostname}{#if snapshot.tags.length} · {snapshot.tags.join(", ")}{/if}
          </p>
        </div>
      </div>
      <button class="btn btn-sm" onclick={() => (showLargest = true)} title="Carpetas y archivos más grandes de esta versión">
        <ChartBar size={14} /> Lo que más ocupa
      </button>
    </div>
  </header>

  {#if sum}
    <dl class="facts">
      <div>
        <dt>Tamaño</dt>
        <dd>{formatBytes(sum.total_bytes_processed)}</dd>
        <span class="faint">{formatNumber(sum.total_files_processed ?? 0)} archivos</span>
      </div>
      <div title="Lo que esta versión añadió al repositorio. Lo que no cambió respecto a versiones anteriores no ocupa espacio de nuevo.">
        <dt>Nuevo en disco</dt>
        <dd>{formatBytes(sum.data_added_packed)}</dd>
        <span class="faint">{formatBytes(sum.data_added)} sin comprimir</span>
      </div>
      <div>
        <dt>Cambios</dt>
        <dd>{formatNumber(sum.files_new ?? 0)} nuevos · {formatNumber(sum.files_changed ?? 0)} modificados</dd>
        <span class="faint">{formatNumber(sum.files_unmodified ?? 0)} sin cambios</span>
      </div>
      <div>
        <dt>Duración</dt>
        <dd>{formatDuration(duration)}</dd>
        <span class="faint" title={snapshot.program_version ?? ""}>{[snapshot.username ?? snapshot.hostname, snapshot.program_version?.replace("restic ", "restic v")].filter(Boolean).join(" · ")}</span>
      </div>
    </dl>
  {/if}
  {/if}

  <section class="card pane">
    <div class="tabs" role="tablist">
      <button role="tab" aria-selected={tab === "files"} class:on={tab === "files"} onclick={() => (tab = "files")}>Archivos</button>
      <button
        role="tab"
        aria-selected={tab === "changes"}
        class:on={tab === "changes"}
        onclick={() => (tab = "changes")}
        disabled={snapshots.length < 2}
        title={snapshots.length < 2 ? "No hay otra versión con la que comparar" : "Qué cambió respecto a otra versión"}
      >
        Qué cambió
      </button>
    </div>
    {#if tab === "changes" && snapshots.length > 1}
      <ChangesView {repo} {snapshot} {snapshots} initial={previous} {windowsStyle} onreveal={reveal} />
    {:else}
    <div class="toolbar">
      <button class="icon-btn" title={dir === "/" ? "Ya estás en la carpeta raíz" : "Subir un nivel (Retroceso)"} aria-label="Subir un nivel" disabled={dir === "/"} onclick={() => load(parentDir(dir))}>
        <ArrowUp size={16} />
      </button>
      <nav class="crumbs" aria-label="Ruta" bind:this={crumbsEl}>
        <button class="crumb root" onclick={() => load("/")} title="Raíz de la versión"><HardDrive size={14} /></button>
        {#each crumbs as c, i (c.path)}
          <ChevronRight size={14} class="sep" />
          <button class="crumb" class:current={i === crumbs.length - 1} onclick={() => load(c.path)}>{c.label}</button>
        {/each}
      </nav>
      <label class="search">
        <Search size={14} />
        <input class="input" placeholder="Buscar en esta carpeta" title="Busca por nombre en la carpeta que estás viendo (Ctrl+F)" bind:value={filter} bind:this={filterInput} spellcheck="false" aria-label="Buscar en esta carpeta" />
        {#if filter}<button class="icon-btn clear" onclick={() => (filter = "")} title="Limpiar el filtro" aria-label="Limpiar el filtro"><X size={13} /></button>{/if}
      </label>
      <button class="btn btn-sm" onclick={restoreAll} disabled={loading || !!error || entries.length === 0}>
        {#if onpick}<FolderCheck size={14} /> Elegir toda la carpeta{:else}<RotateCcw size={14} /> Restaurar carpeta{/if}
      </button>
    </div>

    <div class="table" role="grid" aria-busy={loading}>
      <div class="row head" role="row">
        <span class="check">
          <input
            type="checkbox"
            checked={allSelected}
            indeterminate={selected.size > 0 && !allSelected}
            onchange={toggleAll}
            disabled={!visible.length}
            aria-label="Seleccionar todo"
          />
        </span>
        <span>Nombre</span>
        <span class="num">Tamaño</span>
        <span>Modificado</span>
      </div>

      {#if loading}
        <div class="state">
          {#each [0, 1, 2, 3, 4] as i}<div class="sk" style:animation-delay="{i * 90}ms"></div>{/each}
        </div>
      {:else if error}
        <div class="state"><div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div></div>
      {:else if entries.length === 0}
        <div class="state empty">
          <FolderOpen size={26} />
          {#if initialDir && dir === initialDir && dir !== home}
            <strong>Esta versión no tiene esa carpeta</strong>
            <span>No existía o estaba vacía en esta versión. Prueba con otra.</span>
            <button class="btn btn-sm" onclick={() => load(home)}>Ir al inicio de la versión</button>
          {:else}
            <span>Esta carpeta está vacía en esta versión.</span>
          {/if}
        </div>
      {:else if visible.length === 0}
        <div class="state empty">
          <Search size={22} />
          <span>Nada en esta carpeta coincide con «{filter}».</span>
          <span class="faint">La búsqueda solo mira la carpeta que estás viendo: entra en otra o prueba otra palabra.</span>
        </div>
      {:else}
        <div class="rows">
          {#each visible.slice(0, limit) as e (e.path)}
            <div class="row item" class:sel={selected.has(e.name)} role="row">
              <span class="check">
                <input type="checkbox" checked={selected.has(e.name)} onchange={() => toggle(e.name)} aria-label="Seleccionar {e.name}" />
              </span>
              <button class="name" class:dir={e.kind === "dir"} onclick={() => open(e)} title={e.name}>
                <span class="ficon">
                  {#if e.kind === "dir"}<Folder size={17} fill="currentColor" fill-opacity="0.18" />{:else if e.kind === "symlink"}<FileSymlink size={16} />{:else}<File size={16} />{/if}
                </span>
                <span class="label">{e.name}</span>
              </button>
              <span class="num faint">{e.kind === "dir" ? "—" : formatBytes(e.size)}</span>
              <span class="faint date">{e.mtime ? formatDate(e.mtime) : ""}</span>
            </div>
          {/each}
          {#if visible.length > limit}
            <button class="more" onclick={() => (limit += PAGE)}>
              Mostrar {formatNumber(Math.min(PAGE, visible.length - limit))} más de {formatNumber(visible.length - limit)} restantes
            </button>
          {/if}
        </div>
      {/if}
    </div>
    {/if}
  </section>

  {#if selected.size > 0}
    <div class="actionbar" transition:fly={{ y: 16, duration: dur(180) }}>
      <span>
        <strong>{formatNumber(selected.size)}</strong>
        {selected.size === 1 ? "elemento seleccionado" : "elementos seleccionados"}
        {#if selectedBytes > 0}<span class="faint"> · {formatBytes(selectedBytes)}{selectedHasDirs ? " + carpetas" : ""}</span>{/if}
      </span>
      <span class="actions">
        <button class="btn btn-ghost btn-sm" onclick={() => (selected = new Set())}>Quitar selección</button>
        {#if onchange && !onpick}
          <button class="btn btn-sm" onclick={excludeSelection} title="No copiar estos elementos en las próximas copias">
            <ListX size={14} /> Excluir de las próximas copias…
          </button>
        {/if}
        {#if onpick}
          <button class="btn btn-primary" onclick={restoreSelection}>Siguiente: dónde restaurar <ArrowRight size={15} /></button>
        {:else}
          <button class="btn btn-primary" onclick={restoreSelection}><RotateCcw size={15} /> Restaurar…</button>
        {/if}
      </span>
    </div>
  {/if}
</div>

{#if restoring}
  <RestoreDialog
    {repo}
    {snapshot}
    {dir}
    names={restoring.names}
    label={restoring.label}
    onclose={() => (restoring = null)}
    ondone={() => (selected = new Set())}
  />
{/if}

{#if showLargest}
  <LargestDialog
    {repo}
    {snapshot}
    {onchange}
    onclose={() => (showLargest = false)}
    onopenfolder={(path) => {
      showLargest = false;
      tab = "files";
      load(path);
    }}
  />
{/if}

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
  .browser {
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 1080px;
    margin: 0 auto;
    padding-bottom: 70px;
  }
  .browser.embedded {
    width: 100%;
  }
  .top {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
  }
  .back {
    margin-left: -8px;
  }
  .top-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    width: 100%;
  }
  .snap-info {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .snap-icon {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: var(--radius-lg);
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  h2 {
    font-size: 20px;
    font-weight: 650;
  }
  h2 .mono {
    font-size: 0.85em;
    color: var(--text-2);
  }
  .snap-info p {
    display: flex;
    align-items: center;
    gap: 5px;
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }

  .facts {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
    margin: 0;
  }
  .facts > div {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    padding: 11px 14px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .facts dt {
    font-size: var(--fs-xs);
    font-weight: 650;
    color: var(--text-3);
  }
  .facts dd {
    margin: 0;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .facts .faint {
    font-size: var(--fs-xs);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pane {
    overflow: hidden;
  }
  .tabs {
    display: flex;
    gap: 2px;
    padding: 6px 8px 0;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
  }
  .tabs button {
    position: relative;
    padding: 8px 12px 10px;
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--text-3);
    background: none;
    border: none;
    cursor: pointer;
  }
  .tabs button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .tabs button:not(:disabled):hover,
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
  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--border);
  }
  .toolbar .icon-btn:disabled {
    opacity: 0.35;
    cursor: default;
    background: none;
  }
  .crumbs {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 1px;
    overflow: hidden;
    white-space: nowrap;
    mask-image: linear-gradient(90deg, transparent, #000 12px);
    -webkit-mask-image: linear-gradient(90deg, transparent, #000 12px);
  }
  .crumbs :global(.sep) {
    flex: none;
    color: var(--text-3);
  }
  .crumb {
    flex: none;
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    padding: 4px 7px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-2);
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .crumb:hover {
    background: var(--surface-3);
    color: var(--text-1);
  }
  .crumb.current {
    color: var(--text-1);
    font-weight: 600;
  }
  .crumb.root {
    display: grid;
    place-items: center;
    padding: 5px 6px;
  }
  .search {
    position: relative;
    display: flex;
    align-items: center;
    width: 220px;
    flex: none;
  }
  .search > :global(svg) {
    position: absolute;
    left: 10px;
    color: var(--text-3);
    pointer-events: none;
  }
  .search .input {
    height: 30px;
    padding-left: 30px;
    padding-right: 28px;
    font-size: var(--fs-sm);
  }
  .search .clear {
    position: absolute;
    right: 2px;
    width: 24px;
    height: 24px;
  }

  .row {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) 96px 170px;
    align-items: center;
    gap: 8px;
    padding: 0 12px 0 4px;
  }
  .head {
    height: 34px;
    font-size: var(--fs-xs);
    font-weight: 650;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--text-3);
    background: var(--surface-2);
    border-bottom: 1px solid var(--border);
  }
  .rows {
    max-height: calc(100vh - 290px);
    min-height: 120px;
    overflow: auto;
  }
  .item {
    height: 38px;
    border-bottom: 1px solid var(--border);
    transition: background 0.1s;
  }
  .item:last-child {
    border-bottom: none;
  }
  .item:hover {
    background: var(--surface-2);
  }
  .item.sel {
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
  .name {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    height: 100%;
    padding: 0;
    font: inherit;
    color: var(--text-1);
    text-align: left;
    background: none;
    border: none;
    cursor: default;
  }
  .name.dir {
    cursor: pointer;
  }
  .name.dir:hover .label {
    color: var(--accent-text);
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .ficon {
    display: grid;
    flex: none;
    color: var(--text-3);
  }
  .name.dir .ficon {
    color: var(--accent);
  }
  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .date {
    font-size: var(--fs-sm);
    white-space: nowrap;
  }
  .more {
    display: block;
    width: 100%;
    padding: 10px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--accent-text);
    background: var(--surface-2);
    border: none;
    cursor: pointer;
  }
  .state {
    padding: 18px;
    min-height: 120px;
  }
  .state.empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    text-align: center;
    color: var(--text-3);
  }
  .state.empty strong {
    color: var(--text-1);
  }
  .state.empty .faint {
    font-size: var(--fs-sm);
  }
  .sk {
    height: 18px;
    margin: 10px 0;
    border-radius: var(--radius-sm);
    background: var(--surface-3);
    animation: pulse 1.1s ease-in-out infinite alternate;
  }
  @keyframes pulse {
    to {
      opacity: 0.4;
    }
  }

  .actionbar {
    position: fixed;
    left: calc(264px + (100vw - 264px) / 2);
    bottom: 20px;
    translate: -50% 0;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 24px;
    padding: 10px 10px 10px 18px;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    white-space: nowrap;
  }
  .actions {
    display: flex;
    gap: 6px;
  }

  @media (max-width: 900px) {
    .row {
      grid-template-columns: 40px minmax(0, 1fr) 90px;
    }
    .row > :nth-child(4) {
      display: none;
    }
    .search {
      width: 140px;
    }
  }
</style>
