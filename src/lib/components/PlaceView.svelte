<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { ChevronRight, CopyPlus, FolderPlus, Info, LoaderCircle, Lock, Package, Pencil, RefreshCw } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { FoundRepo, Repo } from "$lib/api";
  import { dur } from "$lib/motion";
  import { openKit } from "$lib/kit.svelte";
  import { repoKind } from "$lib/repoKind";
  import { health, LEVEL_LABEL, statusOf } from "$lib/status.svelte";
  import { toast } from "$lib/toast.svelte";
  import CloneDialog from "./CloneDialog.svelte";
  import InfoTip from "./InfoTip.svelte";
  import PlaceRepoDialog from "./PlaceRepoDialog.svelte";
  import PlaceShareCard from "./PlaceShareCard.svelte";

  // Página de un destino (lugar): sus repositorios (los de este equipo, los
  // que se encuentran al listarlo y los que se recuerdan), con «Usar este»,
  // «Crear uno nuevo aquí» y «Clonar a otro destino».
  interface Props {
    placeId: string;
    repos: Repo[];
    onopenrepo: (id: string) => void;
    onrepoadded: (repo: Repo) => void;
    onplacerenamed: (id: string, name: string) => void;
  }
  let { placeId, repos, onopenrepo, onrepoadded, onplacerenamed }: Props = $props();

  const mine = $derived(repos.filter((r) => r.place_id === placeId));
  const template = $derived(mine[0] ?? null);
  const name = $derived(template?.place_name ?? "Destino");
  const kind = $derived(template ? repoKind(template.location) : null);

  let found = $state<FoundRepo[] | null>(null);
  let note = $state<string | null>(null);
  let scanning = $state(false);
  let scanError = $state("");
  let deviceName = $state("");

  async function scan() {
    scanning = true;
    scanError = "";
    try {
      const r = await api.placeScan(placeId);
      found = r.found;
      note = r.note;
    } catch (e) {
      scanError = String(e);
    } finally {
      scanning = false;
    }
  }

  onMount(() => {
    void scan();
    api.webInfo().then(
      (w) => (deviceName = w.default_name),
      () => {},
    );
  });

  /** Filas: lo encontrado; mientras se lista, los repositorios de este equipo. */
  const rows = $derived<FoundRepo[]>(
    found ?? mine.map((r) => ({ location: r.location, path: r.location, repo_id: r.id, listed: false })),
  );
  const repoOf = (id: string | null) => (id ? (repos.find((r) => r.id === id) ?? null) : null);

  const lead = $derived.by(() => {
    const parts = [`${mine.length} ${mine.length === 1 ? "repositorio de este equipo" : "repositorios de este equipo"}`];
    const more = found ? found.length - mine.length : 0;
    if (more > 0) parts.push(`${more} más ${more === 1 ? "encontrado o recordado" : "encontrados o recordados"}`);
    return `${parts.join(" · ")}. Cada repositorio tiene su propia contraseña.`;
  });

  /** Ruta corta dentro del destino («servidor-oficina/»), sin la ubicación entera. */
  function shortPath(f: FoundRepo) {
    if (f.path === ".") return "(la raíz del destino)";
    const p = f.path === f.location ? (f.location.replace(/[\\/]+$/, "").split(/[\\/]/).pop() ?? f.location) : f.path;
    return `${p.replace(/[\\/]+$/, "")}/`;
  }
  /** Nombre amable de un repositorio que este equipo aún no usa: el último tramo de su ruta. */
  function friendly(f: FoundRepo) {
    const p = shortPath(f).replace(/\/$/, "");
    return f.path === "." ? name : (p.split(/[\\/]/).pop() ?? p);
  }

  // Renombrar el destino.
  let editing = $state(false);
  let draft = $state("");
  async function rename(e: SubmitEvent) {
    e.preventDefault();
    const n = draft.trim();
    if (!n || n === name) return void (editing = false);
    try {
      const p = await api.renamePlace(placeId, n);
      onplacerenamed(p.id, p.name);
      editing = false;
      toast(`Destino renombrado a «${p.name}»`);
    } catch (err) {
      toast(String(err), "error");
    }
  }
  function focusSelect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  let dialog = $state<{ mode: "use" | "new"; location?: string; path?: string } | null>(null);
  let cloning = $state<Repo | null>(null);

  function created(repo: Repo, generated: boolean) {
    dialog = null;
    onrepoadded(repo);
    void scan();
    // Una contraseña generada solo está en este equipo: al kit.
    if (generated) openKit([repo.id]);
  }

  const TONE: Record<string, string> = { ok: "ok", late: "warn", overdue: "bad", error: "bad", empty: "neutral", paused: "paused", loading: "neutral" };
</script>

<div class="page" in:fade={{ duration: dur(160) }}>
  <header class="page-head">
    <div class="page-icon">{#if kind}<kind.icon size={22} strokeWidth={1.9} />{/if}</div>
    <div class="page-head-text">
      <div class="crumb faint">Destino <InfoTip id="destino" /></div>
      <div class="page-title-line">
        {#if editing}
          <form class="rename" onsubmit={rename}>
            <input class="input" bind:value={draft} maxlength="80" use:focusSelect onkeydown={(e) => e.key === "Escape" && (editing = false)} aria-label="Nuevo nombre del destino" />
            <button class="btn btn-primary btn-sm">Guardar</button>
            <button type="button" class="btn btn-ghost btn-sm" onclick={() => (editing = false)}>Cancelar</button>
          </form>
        {:else}
          <h1 class="page-title" title={name}>{name}</h1>
          <button class="icon-btn" title="Renombrar el destino" aria-label="Renombrar el destino" onclick={() => ((draft = name), (editing = true))}><Pencil size={14} /></button>
          {#if kind}<span class="kind">{kind.label}</span>{/if}
        {/if}
      </div>
      <p class="faint lead">{lead}</p>
    </div>
    <div class="page-actions">
      <button class="btn btn-primary" onclick={() => (dialog = { mode: "new" })} disabled={!template}><FolderPlus size={14} /> Crear uno nuevo aquí</button>
      <button class="btn" onclick={scan} disabled={scanning} title="Volver a mirar qué repositorios hay">
        {#if scanning}<span class="spin" style="display:grid"><LoaderCircle size={14} /></span>{:else}<RefreshCw size={14} />{/if} Actualizar
      </button>
    </div>
  </header>

  {#if note}
    <div class="notice notice-info"><Info size={16} /><p>{note}</p></div>
  {/if}
  {#if scanError}
    <div class="notice notice-danger" role="alert"><Info size={16} /><p>No se pudo mirar qué hay en el destino: {scanError}</p></div>
  {/if}

  <section class="card list">
    <h2 class="section-title">Repositorios {#if scanning}<span class="faint small">· buscando…</span>{/if}</h2>
    <ul>
      {#each rows as f (f.location)}
        {@const r = repoOf(f.repo_id)}
        {@const st = r ? statusOf(r, health[r.id]) : null}
        <li class="row">
          <span class="ic"><Package size={16} /></span>
          <span class="txt">
            {#if r}
              <button class="link name" onclick={() => onopenrepo(r.id)}>{r.name} <ChevronRight size={13} /></button>
            {:else}
              <strong class="name">{friendly(f)}</strong>
            {/if}
            <span class="faint mono path" title={f.location}>{shortPath(f)}</span>
          </span>
          <span class="tags">
            {#if r && st}
              <span class="badge badge-sm tone-{TONE[st.level] ?? 'neutral'}">{LEVEL_LABEL[st.level]}</span>
            {:else if f.listed}
              <span class="badge badge-sm tone-neutral">Encontrado</span> <InfoTip id="repo-encontrado" />
            {:else}
              <span class="badge badge-sm tone-neutral">Recordado</span> <InfoTip id="repo-recordado" />
            {/if}
          </span>
          <span class="acts">
            {#if r}
              <button class="btn btn-ghost btn-sm" onclick={() => (cloning = r)} title="Copiar todas sus versiones a un repositorio nuevo en otro destino">
                <CopyPlus size={13} /> Clonar…
              </button>
            {:else if template}
              <button class="btn btn-sm" onclick={() => (dialog = { mode: "use", location: f.location, path: f.path })}><Lock size={12} /> Usar este</button>
            {/if}
          </span>
        </li>
      {:else}
        <li class="empty faint">Aún no hay repositorios en este destino.</li>
      {/each}
    </ul>
  </section>

  <PlaceShareCard {placeId} placeName={name} repos={mine} />
</div>

{#if dialog && template}
  <PlaceRepoDialog
    mode={dialog.mode}
    placeName={name}
    {template}
    location={dialog.location}
    path={dialog.path}
    {deviceName}
    onclose={() => (dialog = null)}
    oncreated={created}
  />
{/if}

{#if cloning}
  <CloneDialog
    repo={cloning}
    {repos}
    onclose={() => (cloning = null)}
    oncreated={(r) => {
      cloning = null;
      onrepoadded(r);
      void scan();
      openKit([r.id]);
    }}
  />
{/if}

<style>
  .crumb {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: var(--fs-sm);
  }
  .rename {
    display: flex;
    gap: 6px;
  }
  .kind {
    padding: 2px 8px;
    font-size: var(--fs-xs);
    font-weight: 600;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text-2);
  }
  .lead {
    margin: 4px 0 0;
    font-size: var(--fs-sm);
  }
  .list {
    padding: 16px 20px;
  }
  .list ul {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
  }
  .row {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) auto auto;
    gap: 12px;
    align-items: center;
    padding: 10px 0;
    border-top: 1px solid var(--border);
  }
  .ic {
    display: grid;
    color: var(--text-3);
  }
  .txt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .name {
    font-size: var(--fs-body, 15px);
    font-weight: 600;
    color: var(--text-1);
  }
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-xs);
  }
  .tags {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .acts {
    display: flex;
    gap: 6px;
  }
  .empty {
    padding: 14px 0;
    font-size: var(--fs-sm);
  }
  .small {
    font-size: var(--fs-sm);
    font-weight: 400;
  }
</style>
