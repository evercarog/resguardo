<script lang="ts">
  // «Ver versiones en Resguardo» (menú del Explorador): las versiones guardadas
  // de un archivo o carpeta concretos. Un archivo se muestra por contenidos
  // (cada vez que cambió, una fila) y cada fila se despliega con todas las
  // fechas en las que estaba así; cualquier fecha se puede abrir o restaurar
  // (con otro nombre junto al original, nunca reemplaza).
  import { untrack } from "svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import {
    ChevronDown,
    ChevronRight,
    CircleAlert,
    CircleCheck,
    File,
    FileClock,
    Folder,
    FolderOpen,
    FolderSearch,
    Info,
    LoaderCircle,
    Plus,
    RotateCcw,
    Search,
    X,
  } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { LocatedPath, Repo, Snapshot } from "$lib/api";
  import { formatBytes, formatDate } from "$lib/format";
  import { isWithin, toSnapshotPath } from "$lib/paths";
  import { planSnapshots } from "$lib/copies.svelte";
  import { health } from "$lib/status.svelte";
  import { groupResults, searches, startSearch, type Variant } from "$lib/search.svelte";
  import { toast } from "$lib/toast.svelte";
  import Modal from "./Modal.svelte";
  import HelpLink from "./HelpLink.svelte";

  interface Props {
    located: LocatedPath;
    repos: Repo[];
    /** Abrir una versión en el explorador de versiones, en una carpeta. */
    onopen: (repoId: string, snapshotId: string, dir: string) => void;
    /** Crear una copia nueva (si ninguna incluye la ruta). */
    onnewcopy: () => void;
    onclose: () => void;
  }
  let { located, repos, onopen, onnewcopy, onclose }: Props = $props();

  /** Más de esto, no se pasan los ID uno a uno (la línea de comandos tiene límite). */
  const MAX_IDS = 800;

  /** Copias que la incluyen (las que siguen existiendo). */
  const options = $derived(
    located.found.flatMap((f) => {
      const repo = repos.find((r) => r.id === f.repo_id);
      const plan = repo?.plans.find((p) => p.id === f.plan_id);
      return repo && plan ? [{ key: `${repo.id}#${plan.id}`, repo, plan, kind: f.kind }] : [];
    }),
  );
  let chosenKey = $state(untrack(() => options[0]?.key ?? ""));
  const chosen = $derived(options.find((o) => o.key === chosenKey) ?? options[0] ?? null);
  /** "plan": solo las versiones de la copia elegida; "all": todas las del destino que pueden tenerlo. */
  let scope = $state<"plan" | "all">("plan");
  /** Cambiar de copia o de alcance: la búsqueda del destino es una sola, así que se vuelve a lanzar. */
  function choose(next: "plan" | "all") {
    started = {};
    restoring = null;
    done = null;
    scope = next;
  }

  // Versiones del destino: las que ya cargó la app o, si no, se piden.
  let loaded = $state<Record<string, Snapshot[]>>({});
  let loadError = $state("");
  $effect(() => {
    const repo = chosen?.repo;
    if (!repo || loaded[repo.id]) return;
    const known = health[repo.id];
    if (known?.loadedAt) {
      loaded[repo.id] = known.snapshots;
      return;
    }
    api.listSnapshots(repo.id).then(
      (list) => (loaded[repo.id] = list),
      (e) => (loadError = String(e)),
    );
  });

  const snapPath = $derived(toSnapshotPath(located.path));
  const windowsStyle = $derived(/^[A-Za-z]:/.test(located.path));
  const allSnaps = $derived(chosen ? [...(loaded[chosen.repo.id] ?? [])].sort((a, b) => b.time.localeCompare(a.time)) : []);
  const planSnaps = $derived(chosen ? planSnapshots(allSnaps, chosen.plan) : []);
  /**
   * Todas las versiones del destino que pueden tenerlo: las de esta copia, las
   * de otras copias del mismo destino y las de antes de cambiar sus carpetas.
   */
  const coverSnaps = $derived(
    allSnaps.filter((s) =>
      s.paths.some((p) => {
        const sp = toSnapshotPath(p);
        return isWithin(snapPath, sp, windowsStyle) || isWithin(sp, snapPath, windowsStyle);
      }),
    ),
  );
  const scopeSnaps = $derived(scope === "all" ? coverSnaps : planSnaps);
  const extra = $derived(Math.max(0, coverSnaps.length - planSnaps.length));
  const scopeLabel = $derived(
    !chosen ? "" : scope === "all" ? `todas las versiones de «${chosen.repo.name}»` : `las versiones de «${chosen.plan.name}»`,
  );

  // Se busca la ruta exacta en las versiones del alcance elegido.
  let started = $state<Record<string, boolean>>({});
  $effect(() => {
    const c = chosen;
    const key = `${c?.key}|${scope}`;
    if (!c || !loaded[c.repo.id] || started[key]) return;
    untrack(() => {
      started[key] = true;
      const list = scopeSnaps;
      if (!list.length) return;
      const req: api.SearchRequest = { pattern: located.path };
      if (list.length <= MAX_IDS) req.snapshots = list.map((s) => s.short_id);
      else if (scope === "plan") ((req.paths = c.plan.paths), (req.tags = c.plan.tags));
      void startSearch(c.repo.id, req, list, scopeLabel);
    });
  });

  const run = $derived.by(() => {
    const r = chosen ? searches[chosen.repo.id] : null;
    return r && r.request.pattern === located.path && r.scopeLabel === scopeLabel ? r : null;
  });
  const group = $derived(run ? (groupResults(run).find((g) => g.path.toLowerCase() === snapPath.toLowerCase()) ?? null) : null);
  const searching = $derived(!chosen || !loaded[chosen.repo.id] || !!run?.running || (scopeSnaps.length > 0 && !run));

  // ---------- Contenidos y fechas ----------

  /** Filas: un archivo, por contenidos (de la más reciente a la más antigua); una carpeta, una por versión. */
  const rows = $derived.by((): Variant[] => {
    if (!group) return [];
    if (!located.is_dir) return group.variants;
    return group.variants.flatMap((v) => v.snapshots.map((s) => ({ size: null, mtime: null, snapshots: [s] })));
  });
  const versionCount = $derived(group?.count ?? 0);
  /** Cuántas filas están desplegadas (por la primera versión de cada una). */
  let expanded = $state<Record<string, boolean>>({});
  const rowKey = (v: Variant) => v.snapshots[0].id;
  /** Si nunca cambió, sus fechas se ven sin desplegar. */
  const single = $derived(!located.is_dir && rows.length === 1 && rows[0].snapshots.length > 1);

  const seconds = (iso: string | null | undefined) => (iso ? Math.floor(Date.parse(iso) / 1000) : null);
  function sameAsNow(v: Variant) {
    if (located.is_dir || located.size == null) return null;
    return v.size === located.size && seconds(v.mtime) === seconds(located.mtime);
  }
  function compareText(v: Variant) {
    const same = sameAsNow(v);
    if (same === null) return "";
    if (same) return "Igual que la actual";
    if (v.size != null && located.size != null && v.size !== located.size) {
      const diff = v.size - located.size;
      return `Distinta: ${formatBytes(Math.abs(diff))} ${diff > 0 ? "más" : "menos"} que la actual`;
    }
    return "Distinta de la actual";
  }
  const allSame = $derived(rows.length === 1 && sameAsNow(rows[0]) === true);

  // ---------- Restaurar ----------

  const pad = (n: number) => String(n).padStart(2, "0");
  /** «Informe (versión del 30-09).xlsx». */
  function defaultName(time: string) {
    const d = new Date(time);
    const tag = `(versión del ${pad(d.getDate())}-${pad(d.getMonth() + 1)})`;
    const dot = located.name.lastIndexOf(".");
    if (located.is_dir || dot <= 0) return `${located.name} ${tag}`;
    return `${located.name.slice(0, dot)} ${tag}${located.name.slice(dot)}`;
  }

  let restoring = $state<{ snapshot: Snapshot; name: string } | null>(null);
  let busy = $state(false);
  let error = $state("");
  let done = $state<string | null>(null);

  function askRestore(snapshot: Snapshot) {
    restoring = { snapshot, name: defaultName(snapshot.time) };
    error = "";
    done = null;
  }

  async function restore(e: SubmitEvent) {
    e.preventDefault();
    if (!restoring || !chosen || busy) return;
    busy = true;
    error = "";
    try {
      done = await api.restoreVersion(chosen.repo.id, restoring.snapshot.id, located.path, restoring.name.trim());
      restoring = null;
      toast(`Restaurado como «${done.slice(done.lastIndexOf("\\") + 1)}»`, "success");
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  function open(snapshot: Snapshot) {
    if (!chosen || !group) return;
    onopen(chosen.repo.id, snapshot.id, located.is_dir ? group.path : group.dir);
  }

  function focusInput(node: HTMLInputElement) {
    node.focus();
    // Se selecciona el nombre sin la extensión, como al renombrar en el Explorador.
    const dot = located.is_dir ? -1 : node.value.lastIndexOf(".");
    node.setSelectionRange(0, dot > 0 ? dot : node.value.length);
  }

  const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
</script>

{#snippet restoreForm(s: Snapshot)}
  {#if restoring?.snapshot.id === s.id}
    <form class="restore" onsubmit={restore}>
      <p><strong>Restaurar la versión del {formatDate(s.time)}</strong> junto al original, con este nombre:</p>
      <input class="input" bind:value={restoring.name} aria-label="Nombre de la copia restaurada" spellcheck="false" disabled={busy} use:focusInput />
      <p class="faint icon-note"><Info size={14} /> Se guarda en {located.parent}. El original no se toca y nunca se reemplaza nada: si el nombre ya existe, se añade «(2)».</p>
      {#if error}<p class="err" role="alert"><CircleAlert size={14} /> {error}</p>{/if}
      <div class="row-actions">
        <button type="button" class="btn btn-ghost" onclick={() => (restoring = null)} disabled={busy}>Cancelar</button>
        <button class="btn btn-primary" disabled={busy || !restoring.name.trim()}>
          {#if busy}<span class="spin" style="display:grid"><LoaderCircle size={14} /></span> Restaurando…{:else}<RotateCcw size={14} /> Restaurar{/if}
        </button>
      </div>
    </form>
  {/if}
{/snippet}

{#snippet actions(s: Snapshot, small: boolean)}
  <div class="vact">
    <button class="btn btn-ghost btn-sm" onclick={() => open(s)} title="Ver la versión del {formatDate(s.time)} en Resguardo">
      <FolderOpen size={14} />{small ? " Abrir" : " Abrir en esa versión"}
    </button>
    <button class="btn btn-sm" onclick={() => askRestore(s)} disabled={busy} title="Restaurar la versión del {formatDate(s.time)} junto al original">
      <RotateCcw size={13} />{small ? " Restaurar" : " Restaurar esta versión"}
    </button>
  </div>
{/snippet}

<Modal {onclose} labelledby="versions-title" width={760}>
  <header class="dlg-head">
    <div class="dlg-title">
      <span class="ticon"><FileClock size={19} /></span>
      <div>
        <h2 id="versions-title">Versiones de «{located.name}» <HelpLink topic="ver-versiones" label="ver versiones desde el Explorador" /></h2>
        <p class="faint path" title={located.path}>{located.parent}</p>
      </div>
    </div>
    <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={onclose}><X size={17} /></button>
  </header>

  {#if !options.length}
    <div class="empty-state">
      <FolderSearch size={28} />
      <strong>Ninguna copia incluye {located.is_dir ? "esta carpeta" : "este archivo"}</strong>
      <p>
        Resguardo solo guarda las carpetas que eliges en cada copia, y ninguna de tus copias incluye «{located.name}». Si quieres tener sus versiones a partir de
        ahora, añade su carpeta a una copia o crea una nueva.
      </p>
      <div class="row-actions">
        <button class="btn btn-ghost" onclick={onclose}>Cerrar</button>
        <button class="btn btn-primary" onclick={onnewcopy}><Plus size={15} /> Nueva copia</button>
      </div>
    </div>
  {:else}
    {#if options.length > 1}
      <label class="pick">
        <span class="faint">En la copia</span>
        <select class="input" bind:value={chosenKey} onchange={() => choose("plan")}>
          {#each options as o (o.key)}<option value={o.key}>{o.plan.name} · {o.repo.name}</option>{/each}
        </select>
      </label>
    {:else if chosen}
      <p class="faint pick-one">En la copia <strong>«{chosen.plan.name}»</strong>, que se guarda en «{chosen.repo.name}».</p>
    {/if}

    {#if loadError}
      <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>No se pudo leer la lista de versiones: {loadError}</p></div>
    {:else if searching}
      <p class="status" role="status">
        <span class="spin" style="display:grid"><LoaderCircle size={15} /></span>
        Buscando en {plural(scopeSnaps.length, "versión", "versiones")}{scope === "all" ? ` de «${chosen?.repo.name}»` : ` de la copia «${chosen?.plan.name}»`}…
      </p>
    {:else if run?.failure}
      <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{run.failure}</p></div>
    {:else if !group}
      <div class="notice notice-info">
        <Info size={16} />
        <p>
          {#if !scopeSnaps.length}
            Esta copia aún no tiene versiones: haz una copia y luego podrás volver a cualquier momento.
          {:else}
            No aparece en ninguna de las {plural(scopeSnaps.length, "versión", "versiones")}
            {scope === "all" ? `de «${chosen?.repo.name}»` : `de la copia «${chosen?.plan.name}»`}. Puede que sea nuevo (aún no se ha hecho una copia desde
            que lo creaste) o que esté excluido de la copia.
          {/if}
          {#if scope === "plan" && extra > 0}
            <button class="link notice-action" onclick={() => choose("all")}><Search size={13} /> Buscar en las {extra} versiones más de «{chosen?.repo.name}»</button>
          {/if}
        </p>
      </div>
    {:else}
      <p class="summary">
        {#if located.is_dir}<Folder size={15} />{:else}<File size={15} />{/if}
        <span>
          {#if located.is_dir}
            La carpeta está en <strong>{plural(versionCount, "versión guardada", "versiones guardadas")}</strong>. La más reciente, primero.
          {:else if allSame}
            Este archivo no ha cambiado en {versionCount === 1 ? "la única versión guardada" : `las ${versionCount} versiones guardadas`}: {versionCount === 1
              ? "tiene"
              : "cualquiera tiene"} el mismo contenido que el actual.
          {:else if rows.length === 1}
            Este archivo no cambió en {versionCount === 1 ? "la única versión guardada" : `las ${versionCount} versiones guardadas`}, pero el actual es distinto: lo
            cambiaste después de la última copia.
          {:else}
            Está en <strong>{plural(versionCount, "versión guardada", "versiones guardadas")}</strong> y cambió {plural(rows.length - 1, "vez", "veces")}: cada
            fila es un contenido distinto, el más reciente primero.
          {/if}
        </span>
      </p>
      <ul class="variants">
        {#each rows as v (rowKey(v))}
          {@const same = sameAsNow(v)}
          {@const newest = v.snapshots[0]}
          {@const open_ = single || !!expanded[rowKey(v)]}
          {@const n = v.snapshots.length}
          <li>
            <div class="vrow">
              <div class="vtext">
                <span class="when">Versión del {formatDate(newest.time)}</span>
                <span class="faint meta">
                  {#if !located.is_dir}
                    <span
                      >{[v.size != null ? formatBytes(v.size) : null, v.mtime ? `modificado el ${formatDate(v.mtime)}` : null].filter(Boolean).join(" · ") ||
                        "—"}</span
                    >
                  {/if}
                  {#if compareText(v)}
                    <span class="badge badge-sm {same ? 'tone-ok' : 'tone-neutral'}">{#if same}<CircleCheck size={11} />{/if} {compareText(v)}</span>
                  {/if}
                </span>
                {#if n > 1 && !single}
                  <button class="link dates-toggle" aria-expanded={open_} onclick={() => (expanded[rowKey(v)] = !open_)}>
                    {#if open_}<ChevronDown size={13} />{:else}<ChevronRight size={13} />{/if}
                    {open_ ? "Ocultar" : "Ver"} las {n} versiones con este contenido
                  </button>
                {/if}
              </div>
              {#if !open_}{@render actions(newest, false)}{/if}
            </div>
            {#if !open_}{@render restoreForm(newest)}{/if}
            {#if open_}
              <ul class="dates" aria-label="Fechas con este contenido">
                {#each v.snapshots as s (s.id)}
                  <li>
                    <div class="drow">
                      <span>{formatDate(s.time)}</span>
                      {@render actions(s, true)}
                    </div>
                    {@render restoreForm(s)}
                  </li>
                {/each}
              </ul>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}

    {#if done && !restoring}
      <div class="notice notice-success">
        <CircleCheck size={16} />
        <p>Restaurado como «{done.slice(done.lastIndexOf("\\") + 1)}», junto al original.</p>
        <button class="btn btn-sm" onclick={() => done && revealItemInDir(done).catch(() => {})}>Mostrar en la carpeta</button>
      </div>
    {/if}

    {#if chosen && !searching && !loadError && scopeSnaps.length}
      <p class="scope faint">
        <Search size={13} />
        {#if scope === "all"}
          Buscado en {plural(scopeSnaps.length, "versión", "versiones")} del repositorio «{chosen.repo.name}».
          <button class="link" onclick={() => choose("plan")}>Solo las de la copia «{chosen.plan.name}»</button>
        {:else}
          Buscado en {plural(scopeSnaps.length, "versión", "versiones")} de la copia «{chosen.plan.name}».
          {#if extra > 0 && group}
            <button class="link" onclick={() => choose("all")}>Buscar también en las otras {extra} de «{chosen.repo.name}»</button>
          {/if}
        {/if}
      </p>
    {/if}
  {/if}
</Modal>

<style>
  .path {
    max-width: 560px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pick {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: var(--fs-sm);
  }
  .pick .input {
    width: auto;
    height: 34px;
  }
  .pick-one {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .status,
  .summary {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 14px 0 8px;
    font-size: var(--fs-sm);
  }
  .summary :global(svg) {
    flex: none;
  }
  .variants {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    max-height: 380px;
    overflow: auto;
  }
  .variants > li {
    padding: 10px 12px;
  }
  .variants > li + li {
    border-top: 1px solid var(--border);
  }
  .vrow,
  .drow {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .vtext {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 3px;
    min-width: 0;
    font-size: var(--fs-sm);
  }
  .when {
    font-weight: 550;
  }
  .meta {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
  }
  .dates-toggle {
    font-weight: 450;
  }
  .dates {
    list-style: none;
    margin: 8px 0 0;
    padding: 0 0 0 14px;
    border-left: 2px solid var(--border);
  }
  .dates li {
    padding: 4px 0;
    font-size: var(--fs-sm);
  }
  .vact {
    display: flex;
    gap: 6px;
    flex: none;
  }
  .restore {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 10px;
    padding: 14px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    background: var(--surface-2);
  }
  .restore p {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .err {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--bad);
  }
  .row-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .empty-state .row-actions {
    justify-content: center;
  }
  .notice {
    margin-top: 14px;
  }
  .notice-success .btn {
    margin-left: auto;
  }
  .scope {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    margin: 10px 0 0;
    font-size: var(--fs-sm);
  }
</style>
