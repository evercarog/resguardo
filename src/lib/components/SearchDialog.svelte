<script lang="ts">
  // «Buscar un archivo»: ¿cuándo existió y en qué versiones está?
  import { untrack } from "svelte";
  import { ChevronRight, File, FileSearch, Folder, FolderOpen, LoaderCircle, RotateCcw, Search, Square, X } from "@lucide/svelte";
  import type { Repo, SearchRequest, Snapshot } from "$lib/api";
  import { formatBytes, formatDate, formatNumber } from "$lib/format";
  import { displayPath, isWindowsSnapshot } from "$lib/paths";
  import { planSnapshots } from "$lib/copies.svelte";
  import { cancelSearch, groupResults, searches, startSearch, type FileGroup, type Variant } from "$lib/search.svelte";
  import Modal from "./Modal.svelte";
  import RelTime from "./RelTime.svelte";
  import RestoreDialog from "./RestoreDialog.svelte";
  import HelpLink from "./HelpLink.svelte";

  interface Props {
    repo: Repo;
    /** Versiones del destino (en cualquier orden). */
    snapshots: Snapshot[];
    loading?: boolean;
    /** Abrir la versión en el explorador, en esa carpeta. */
    onopen: (snapshot: Snapshot, dir: string) => void;
    onclose: () => void;
  }
  let { repo, snapshots, loading = false, onopen, onclose }: Props = $props();

  /** Más de esto, no se pasan los ID uno a uno (la línea de comandos tiene límite). */
  const MAX_IDS = 800;

  const sorted = $derived([...snapshots].sort((a, b) => b.time.localeCompare(a.time)));
  const run = $derived(searches[repo.id] ?? null);
  const windowsStyle = $derived(isWindowsSnapshot(sorted[0]?.paths ?? []));

  // La búsqueda anterior de este destino (si la hay) rellena el campo al abrir.
  let pattern = $state(untrack(() => searches[repo.id]?.request.pattern ?? ""));
  let scope = $state("all");

  const scopes = $derived([
    { id: "all", label: "Todas las versiones", count: sorted.length },
    ...(sorted.length > 10 ? [{ id: "recent:10", label: "Las 10 más recientes", count: 10 }] : []),
    ...(sorted.length > 50 ? [{ id: "recent:50", label: "Las 50 más recientes", count: 50 }] : []),
    ...repo.plans.map((p) => ({ id: `plan:${p.id}`, label: `Solo la copia «${p.name}»`, count: planSnapshots(sorted, p).length })),
  ]);

  function search(e?: SubmitEvent) {
    e?.preventDefault();
    if (!pattern.trim() || run?.running) return;
    const chosen = scopes.find((s) => s.id === scope) ?? scopes[0];
    let list = sorted;
    const req: SearchRequest = { pattern: pattern.trim() };
    if (scope.startsWith("recent:")) {
      list = sorted.slice(0, Number(scope.slice(7)));
      req.snapshots = list.map((s) => s.short_id);
    } else if (scope.startsWith("plan:")) {
      const plan = repo.plans.find((p) => `plan:${p.id}` === scope);
      if (plan) {
        list = planSnapshots(sorted, plan);
        if (!list.length) return;
        if (list.length <= MAX_IDS) req.snapshots = list.map((s) => s.short_id);
        else ((req.paths = plan.paths), (req.tags = plan.tags));
      }
    }
    void startSearch(repo.id, req, list, chosen.label.toLowerCase());
  }

  const groups = $derived(run ? groupResults(run) : []);
  const total = $derived(groups.reduce((n, g) => n + g.count, 0));
  /** restic recorre de la más reciente a la más antigua: hasta dónde ha llegado. */
  const progress = $derived.by(() => {
    if (!run?.running || !run.scope.length) return null;
    let last = -1;
    run.scope.forEach((s, i) => {
      if (run.hits[s.id]) last = i;
    });
    return last < 0 ? null : (last + 1) / run.scope.length;
  });

  let expanded = $state<Record<string, boolean>>({});
  let restoring = $state<{ group: FileGroup; snapshot: Snapshot } | null>(null);

  const variantLabel = (v: Variant) =>
    [v.mtime ? `modificado el ${formatDate(v.mtime)}` : null, v.size != null ? formatBytes(v.size) : null].filter(Boolean).join(" · ");

  function rangeText(v: Variant) {
    const newest = v.snapshots[0];
    const oldest = v.snapshots[v.snapshots.length - 1];
    const n = v.snapshots.length;
    return n === 1 ? `En la versión del ${formatDate(newest.time)}` : `En ${n} versiones, del ${formatDate(oldest.time)} al ${formatDate(newest.time)}`;
  }
</script>

<Modal {onclose} labelledby="search-title" width={780}>
  <header class="dlg-head">
    <div class="dlg-title">
      <span class="ticon"><FileSearch size={19} /></span>
      <div>
        <h2 id="search-title">Buscar un archivo <HelpLink topic="buscar-archivo" label="buscar un archivo" /></h2>
        <p class="faint">En las versiones de «{repo.name}»: cuándo existió y dónde está.</p>
      </div>
    </div>
    <button class="icon-btn" title="Cerrar (la búsqueda sigue)" aria-label="Cerrar" onclick={onclose}><X size={17} /></button>
  </header>

  <form class="bar" onsubmit={search}>
    <span class="field-wrap">
      <Search size={15} />
      <input
        class="input"
        type="text"
        bind:value={pattern}
        placeholder="factura_123.xlsx, informe, *.pdf…"
        aria-label="Nombre o parte del nombre"
        spellcheck="false"
        autocomplete="off"
        disabled={run?.running}
      />
    </span>
    <select class="input scope" bind:value={scope} aria-label="Dónde buscar" disabled={run?.running}>
      {#each scopes as s (s.id)}
        <option value={s.id} disabled={s.count === 0}>{s.label} ({formatNumber(s.count)})</option>
      {/each}
    </select>
    {#if run?.running}
      <button type="button" class="btn" onclick={() => cancelSearch(repo.id)}><Square size={12} fill="currentColor" /> Detener</button>
    {:else}
      <button
        class="btn btn-primary"
        disabled={!pattern.trim() || loading || !sorted.length}
        title={!pattern.trim() ? "Escribe qué buscas" : !sorted.length ? "Este repositorio aún no tiene versiones" : undefined}
      >
        Buscar
      </button>
    {/if}
  </form>
  <p class="faint tip">Sin comodines se busca como parte del nombre. No distingue mayúsculas. Usa <code>*</code> para cualquier texto: <code>*.xlsx</code>.</p>

  {#if run}
    <div class="status" role="status" aria-live="polite">
      {#if run.running}
        <span class="spin" style="display:grid"><LoaderCircle size={15} /></span>
        <span>
          Buscando «{run.request.pattern}» en {run.scopeLabel} ({formatNumber(run.scope.length)})…
          {#if total}{formatNumber(groups.length)} {groups.length === 1 ? "archivo" : "archivos"} hasta ahora{/if}
        </span>
      {:else if run.failure}
        <span class="err">{run.failure}</span>
      {:else if !run.outcome}
        <span class="faint">Búsqueda detenida{groups.length ? `: ${formatNumber(groups.length)} ${groups.length === 1 ? "archivo encontrado" : "archivos encontrados"} hasta entonces` : ""}.</span>
      {:else if !groups.length}
        <span>Nada coincide con «{run.request.pattern}» en {run.scopeLabel}. Prueba con una parte del nombre o con otro alcance.</span>
      {:else}
        <span>
          <strong>{formatNumber(groups.length)} {groups.length === 1 ? "archivo" : "archivos"}</strong> con «{run.request.pattern}» en {run.scopeLabel}.
          {#if run.outcome.truncated}<span class="warn-text">Se muestran las primeras {formatNumber(run.outcome.matches)} coincidencias: concreta más el nombre.</span>{/if}
        </span>
      {/if}
    </div>
    {#if run.running}
      <div class="bar-track" class:indeterminate={progress === null} aria-hidden="true">
        <div style:width={progress === null ? undefined : `${progress * 100}%`}></div>
      </div>
    {/if}

    {#if groups.length}
      <ul class="results">
        {#each groups as g (g.path)}
          {@const open = expanded[g.path] ?? groups.length <= 3}
          <li class="group">
            <button class="ghead" aria-expanded={open} onclick={() => (expanded[g.path] = !open)}>
              <span class="chev" class:open><ChevronRight size={15} /></span>
              <span class="gicon">{#if g.isDir}<Folder size={16} />{:else}<File size={16} />{/if}</span>
              <span class="gtext">
                <span class="gname" title={displayPath(g.path, windowsStyle)}>{g.name}</span>
                <span class="gdir faint" title={displayPath(g.dir, windowsStyle)}>{displayPath(g.dir, windowsStyle)}</span>
              </span>
              <span class="gmeta">
                {#if !run.running}
                  <span class="badge badge-sm {g.gone ? 'tone-warn' : 'tone-success'}" title={g.gone ? "No está en la versión más reciente buscada" : "Está en la versión más reciente buscada"}>{g.gone ? "Ya no está" : "En la más reciente"}</span>
                {/if}
                <span class="faint">{g.count} {g.count === 1 ? "versión" : "versiones"}</span>
              </span>
            </button>
            {#if open}
              <div class="gbody">
                <p class="life faint">
                  Aparece por primera vez en la versión del <strong>{formatDate(g.first.time)}</strong>{#if g.count > 1}{" "}y por última vez en la del
                    <strong>{formatDate(g.last.time)}</strong>{/if}.
                  {#if g.gone}No está en la versión más reciente buscada: se borró, se movió o se renombró después.{/if}
                </p>
                <ul class="variants">
                  {#each g.variants as v, vi (vi)}
                    {@const snap = v.snapshots[0]}
                    <li>
                      <div class="vtext">
                        <span>{rangeText(v)}</span>
                        <span class="faint">{variantLabel(v) || "—"}{g.variants.length > 1 && vi === 0 ? " · la más reciente" : ""}</span>
                      </div>
                      <div class="vact">
                        <button class="btn btn-ghost btn-sm" onclick={() => onopen(snap, g.dir)} title="Abrir la versión del {formatDate(snap.time)} en esta carpeta">
                          <FolderOpen size={14} /> Abrir en esa versión
                        </button>
                        <button class="btn btn-sm" onclick={() => (restoring = { group: g, snapshot: snap })} title="Restaurar el archivo tal como estaba en la versión del {formatDate(snap.time)}">
                          <RotateCcw size={13} /> Restaurar
                        </button>
                      </div>
                    </li>
                  {/each}
                </ul>
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
    {#if !run.running && run.outcome}
      <p class="faint foot">Búsqueda <RelTime iso={new Date(run.started).toISOString()} />.</p>
    {/if}
  {:else if !sorted.length && !loading}
    <p class="faint empty">Este repositorio aún no tiene versiones: haz una copia y luego podrás buscar en ella.</p>
  {/if}
</Modal>

{#if restoring}
  <RestoreDialog
    {repo}
    snapshot={restoring.snapshot}
    dir={restoring.group.dir}
    names={[restoring.group.name]}
    label={`«${restoring.group.name}»`}
    onclose={() => (restoring = null)}
    ondone={() => (restoring = null)}
  />
{/if}

<style>
  .bar {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .field-wrap {
    position: relative;
    flex: 1 1 260px;
    display: flex;
    align-items: center;
    color: var(--text-3);
  }
  .field-wrap :global(svg) {
    position: absolute;
    left: 11px;
    pointer-events: none;
  }
  .field-wrap .input {
    padding-left: 33px;
  }
  .scope {
    flex: 0 1 240px;
    width: auto;
  }
  .tip {
    margin: 8px 0 0;
    font-size: var(--fs-xs);
  }
  .status {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin-top: 16px;
    font-size: var(--fs-sm);
  }
  .warn-text {
    color: var(--warn);
  }
  .bar-track {
    position: relative;
    height: 4px;
    margin-top: 10px;
    border-radius: 2px;
    overflow: hidden;
    background: var(--surface-3);
  }
  .bar-track > div {
    height: 100%;
    background: var(--accent);
    transition: width 0.3s;
  }
  .bar-track.indeterminate > div {
    position: absolute;
    width: 30%;
    animation: slide 1.3s ease-in-out infinite;
  }
  @keyframes slide {
    from {
      left: -30%;
    }
    to {
      left: 100%;
    }
  }
  .results {
    list-style: none;
    margin: 14px 0 0;
    padding: 0;
    max-height: min(52vh, 520px);
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .group + .group {
    border-top: 1px solid var(--border);
  }
  .ghead {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 10px 12px;
    font: inherit;
    text-align: left;
    color: var(--text-1);
    background: none;
    border: none;
    cursor: pointer;
  }
  .ghead:hover {
    background: var(--surface-2);
  }
  .chev {
    display: grid;
    flex: none;
    color: var(--text-3);
    transition: transform 0.15s;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .gicon {
    display: grid;
    flex: none;
    color: var(--text-3);
  }
  .gtext {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .gname {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .gdir {
    font-size: var(--fs-xs);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .gmeta {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: none;
    font-size: var(--fs-sm);
  }
  .gbody {
    padding: 0 12px 12px 55px;
  }
  .life {
    margin: 0 0 8px;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .variants {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .variants li {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
  }
  .vtext {
    display: flex;
    flex-direction: column;
    flex: 1 1 260px;
    min-width: 0;
    font-size: var(--fs-sm);
  }
  .vtext .faint {
    font-size: var(--fs-xs);
  }
  .vact {
    display: flex;
    gap: 6px;
    flex: none;
  }
  .foot,
  .empty {
    margin: 12px 0 0;
    font-size: var(--fs-sm);
  }
  @media (max-width: 640px) {
    .gbody {
      padding-left: 12px;
    }
    .gmeta .faint {
      display: none;
    }
  }
</style>
