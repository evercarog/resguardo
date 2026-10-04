<script lang="ts">
  import { ArrowRight, CircleAlert, FilePen, FileMinus, FilePlus, FileCog, FolderOpen, Search, Undo2, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Diff, Repo, Snapshot } from "$lib/api";
  import { formatBytes, formatDate, formatNumber } from "$lib/format";
  import { displayPath, parentDir } from "$lib/paths";

  interface Props {
    repo: Repo;
    /** El snapshot que se está explorando. */
    snapshot: Snapshot;
    /** Todos los snapshots del repositorio (para elegir con cuál comparar). */
    snapshots: Snapshot[];
    /** Copia con la que se compara por defecto (la anterior). */
    initial: Snapshot | null;
    windowsStyle: boolean;
    /** Ir a la carpeta de un cambio en la pestaña Archivos. */
    onreveal: (dir: string) => void;
  }
  let { repo, snapshot, snapshots, initial, windowsStyle, onreveal }: Props = $props();

  const sameSet = (s: Snapshot) =>
    s.hostname === snapshot.hostname && [...s.paths].sort().join("|") === [...snapshot.paths].sort().join("|");
  /** Candidatas: primero las del mismo equipo y carpetas; cada grupo de la más reciente a la más antigua. */
  const candidates = $derived(
    snapshots.filter((s) => s.id !== snapshot.id).sort((a, b) => b.time.localeCompare(a.time)),
  );
  const same = $derived(candidates.filter(sameSet));
  const others = $derived(candidates.filter((s) => !sameSet(s)));

  // svelte-ignore state_referenced_locally
  let otherId = $state(initial?.id ?? candidates[0]?.id ?? "");
  const other = $derived(candidates.find((s) => s.id === otherId) ?? null);
  /** Siempre de la más antigua a la más reciente. */
  const from = $derived(other && other.time < snapshot.time ? other : snapshot);
  const to = $derived(other && other.time < snapshot.time ? snapshot : other);
  const otherIsNewer = $derived(!!other && other.time > snapshot.time);

  const PAGE = 300;
  type Kind = "added" | "modified" | "removed" | "other";
  const KINDS: { id: Kind; label: string; icon: typeof FilePlus }[] = [
    { id: "added", label: "Añadidos", icon: FilePlus },
    { id: "modified", label: "Modificados", icon: FilePen },
    { id: "removed", label: "Eliminados", icon: FileMinus },
    { id: "other", label: "Otros", icon: FileCog },
  ];

  let diff = $state<Diff | null>(null);
  let error = $state("");
  let filter = $state<Kind | "all">("all");
  let query = $state("");
  let limit = $state(PAGE);

  let request = 0;
  $effect(() => {
    if (!from || !to) return;
    const mine = ++request;
    const [a, b] = [from.id, to.id];
    diff = null;
    error = "";
    limit = PAGE;
    api.snapshotDiff(repo.id, a, b).then(
      (d) => mine === request && (diff = d),
      (e) => mine === request && (error = String(e)),
    );
  });

  const optionLabel = (s: Snapshot) =>
    `${formatDate(s.time)} · ${s.short_id}${s.id === initial?.id ? " (anterior)" : ""}${s.time > snapshot.time ? " · posterior" : ""}`;

  /** metadata/type/other se agrupan en "Otros"; las carpetas (acaban en /) no se listan. */
  const group = (k: string): Kind => (k === "added" || k === "modified" || k === "removed" ? k : "other");
  const files = $derived((diff?.changes ?? []).filter((c) => !c.path.endsWith("/")));
  const counts = $derived(
    Object.fromEntries(KINDS.map((k) => [k.id, files.filter((c) => group(c.kind) === k.id).length])) as Record<Kind, number>,
  );
  const shown = $derived(
    files.filter(
      (c) => (filter === "all" || group(c.kind) === filter) && (!query.trim() || c.path.toLowerCase().includes(query.trim().toLowerCase())),
    ),
  );

  function split(path: string) {
    const shownPath = displayPath(path, windowsStyle);
    const i = Math.max(shownPath.lastIndexOf("\\"), shownPath.lastIndexOf("/"));
    return { dir: shownPath.slice(0, i + 1), name: shownPath.slice(i + 1) };
  }
</script>

<div class="changes">
  <div class="picker">
    <label>
      <span class="faint">Comparar con</span>
      <select class="input" bind:value={otherId}>
        {#if same.length}
          <optgroup label="Mismo equipo y carpetas">
            {#each same as s (s.id)}<option value={s.id}>{optionLabel(s)}</option>{/each}
          </optgroup>
        {/if}
        {#if others.length}
          <optgroup label="Otro equipo o carpetas">
            {#each others as s (s.id)}<option value={s.id}>{optionLabel(s)} · {s.hostname}</option>{/each}
          </optgroup>
        {/if}
      </select>
    </label>
    {#if initial && otherId !== initial.id}
      <button class="btn btn-ghost btn-sm" onclick={() => (otherId = initial.id)}><Undo2 size={14} /> Volver a la anterior</button>
    {/if}
  </div>

  {#if from && to}
    <p class="faint compare">
      Cambios desde <strong>{formatDate(from.time)}</strong> <span class="mono">{from.short_id}</span>
      <ArrowRight size={13} />
      hasta <strong>{formatDate(to.time)}</strong> <span class="mono">{to.short_id}</span>
      {#if otherIsNewer}· la versión elegida es posterior a esta{/if}
    </p>
  {/if}

  {#if error}
    <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>
  {:else if !diff}
    <div class="loading">
      {#each [0, 1, 2, 3] as i}<div class="sk" style:animation-delay="{i * 90}ms"></div>{/each}
      <span class="faint">Comparando las dos versiones…</span>
    </div>
  {:else}
    <div class="summary">
      <button class="chip" class:on={filter === "all"} onclick={() => (filter = "all")}>
        Todos <strong>{formatNumber(files.length)}</strong>
      </button>
      {#each KINDS as k}
        {#if counts[k.id] || k.id !== "other"}
          <button class="chip k-{k.id}" class:on={filter === k.id} onclick={() => (filter = filter === k.id ? "all" : k.id)} disabled={!counts[k.id]}>
            <k.icon size={14} />
            {k.label} <strong>{formatNumber(counts[k.id])}</strong>
          </button>
        {/if}
      {/each}
      <span class="bytes faint" title="Datos que entran y salen del repositorio entre las dos versiones">
        +{formatBytes(diff.stats.added.bytes)} · −{formatBytes(diff.stats.removed.bytes)}
      </span>
    </div>

    {#if files.length === 0}
      <div class="empty faint">No cambió ningún archivo entre estas dos versiones.</div>
    {:else}
      <label class="search">
        <Search size={14} />
        <input class="input" placeholder="Buscar en los cambios" bind:value={query} spellcheck="false" />
        {#if query}<button class="icon-btn clear" onclick={() => (query = "")} title="Limpiar la búsqueda" aria-label="Limpiar la búsqueda"><X size={13} /></button>{/if}
      </label>

      <ul class="list">
        {#each shown.slice(0, limit) as c (c.path)}
          {@const p = split(c.path)}
          {@const k = KINDS.find((k) => k.id === group(c.kind))!}
          <li class="k-{k.id}">
            <span class="ic" title={k.label}><k.icon size={15} /></span>
            <span class="path selectable" title={p.dir + p.name}><span class="faint">{p.dir}</span><span class="name">{p.name}</span></span>
            {#if c.kind === "metadata"}<span class="tag faint">solo metadatos</span>{/if}
            {#if c.kind !== "removed"}
              <button class="icon-btn go" title="Ver la carpeta en la versión" aria-label="Ver la carpeta de {c.path} en la versión" onclick={() => onreveal(parentDir(c.path))}>
                <FolderOpen size={14} />
              </button>
            {/if}
          </li>
        {/each}
      </ul>
      {#if shown.length > limit}
        <button class="more" onclick={() => (limit += PAGE)}>Mostrar más ({formatNumber(shown.length - limit)} restantes)</button>
      {/if}
      {#if diff.total > diff.changes.length}
        <p class="faint small">Se muestran los primeros {formatNumber(diff.changes.length)} de {formatNumber(diff.total)} cambios.</p>
      {/if}
    {/if}
  {/if}
</div>

<style>
  .changes {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px 16px 16px;
  }
  .picker {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .picker label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .picker select {
    width: auto;
    max-width: 440px;
    height: 32px;
    padding: 0 8px;
    font-size: var(--fs-sm);
  }
  .compare {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 5px;
    margin: 0;
    font-size: var(--fs-sm);
  }
  .compare strong {
    color: var(--text-2);
  }
  .summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 12px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
    cursor: pointer;
    transition:
      border-color 0.15s,
      background 0.15s;
  }
  .chip strong {
    color: var(--text-1);
    font-variant-numeric: tabular-nums;
  }
  .chip:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .chip.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .k-added {
    --k: var(--ok);
  }
  .k-modified {
    --k: var(--warn);
  }
  .k-removed {
    --k: var(--bad);
  }
  .k-other {
    --k: var(--text-3);
  }
  .chip :global(svg),
  .ic {
    color: var(--k);
  }
  .bytes {
    margin-left: auto;
    font-size: var(--fs-sm);
    font-variant-numeric: tabular-nums;
  }
  .search {
    position: relative;
    display: flex;
    align-items: center;
  }
  .search > :global(svg) {
    position: absolute;
    left: 10px;
    color: var(--text-3);
    pointer-events: none;
  }
  .search .input {
    height: 32px;
    padding-left: 30px;
    font-size: var(--fs-sm);
  }
  .search .clear {
    position: absolute;
    right: 3px;
    width: 26px;
    height: 26px;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: calc(100vh - 360px);
    min-height: 120px;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .list li {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 36px;
    padding: 0 8px 0 12px;
    border-bottom: 1px solid var(--border);
    content-visibility: auto;
    contain-intrinsic-size: auto 36px;
  }
  .list li:last-child {
    border-bottom: none;
  }
  .list li:hover {
    background: var(--surface-2);
  }
  .ic {
    display: grid;
    flex: none;
  }
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-sm);
    direction: rtl;
    text-align: left;
  }
  .path > span {
    direction: ltr;
    unicode-bidi: isolate;
  }
  .name {
    font-weight: 600;
  }
  .k-removed .name {
    text-decoration: line-through;
    color: var(--text-2);
  }
  .tag {
    flex: none;
    font-size: var(--fs-xs);
  }
  .go {
    width: 26px;
    height: 26px;
    opacity: 0;
  }
  .list li:hover .go,
  .go:focus-visible {
    opacity: 1;
  }
  .more {
    padding: 8px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--accent-text);
    background: none;
    border: none;
    cursor: pointer;
  }
  .small {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .empty {
    padding: 24px;
    text-align: center;
  }
  .loading {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .sk {
    height: 18px;
    border-radius: var(--radius-sm);
    background: var(--surface-3);
    animation: pulse 1.1s ease-in-out infinite alternate;
  }
  @keyframes pulse {
    to {
      opacity: 0.4;
    }
  }
</style>
