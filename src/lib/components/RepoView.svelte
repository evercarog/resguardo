<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { Check, Copy, FileSearch, KeyRound, Lock, LockOpen, Pencil, RefreshCw, RotateCcw, Trash2, User } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Repo, RepoStats, Snapshot } from "$lib/api";
  import { runs } from "$lib/backups.svelte";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { toast } from "$lib/toast.svelte";
  import { formatBytes, formatDate, formatRelative } from "$lib/format";
  import { repoKind } from "$lib/repoKind";
  import DestinationCopies from "./DestinationCopies.svelte";
  import SnapshotList from "./SnapshotList.svelte";
  import LargestDialog from "./LargestDialog.svelte";
  import SnapshotBrowser from "./SnapshotBrowser.svelte";
  import RestoreWizard from "./RestoreWizard.svelte";
  import RetentionPanel from "./RetentionPanel.svelte";
  import ActivityCard from "./ActivityCard.svelte";
  import ScheduleCard from "./ScheduleCard.svelte";
  import MaintenanceCard from "./MaintenanceCard.svelte";
  import PauseBanner from "./PauseBanner.svelte";
  import OffsiteHoldBanner from "./OffsiteHoldBanner.svelte";
  import OffsiteSourceCard from "./OffsiteSourceCard.svelte";
  import ProtectionHealth from "./ProtectionHealth.svelte";
  import PageSummary from "./PageSummary.svelte";
  import InfoTip from "./InfoTip.svelte";
  import FlowDiagram from "./FlowDiagram.svelte";
  import ImproveGuide from "./ImproveGuide.svelte";
  import RepoHistory from "./RepoHistory.svelte";
  import RepoErrorActions from "./RepoErrorActions.svelte";
  import { destinationSummary } from "$lib/summary.svelte";
  import { ui, setUi } from "$lib/ui.svelte";
  import SearchDialog from "./SearchDialog.svelte";
  import { browseUi, searchUi } from "$lib/search.svelte";
  import { agent, offsiteSources } from "$lib/agent.svelte";
  import { openKit } from "$lib/kit.svelte";
  import { pendingEditor } from "$lib/intent.svelte";
  import PauseDialog from "./PauseDialog.svelte";
  import { setSnapshots } from "$lib/status.svelte";

  // Un destino (un repositorio de restic): dónde está, qué copias guardan
  // en él, sus copias automáticas, mantenimiento, versiones y retención.
  interface Props {
    repo: Repo;
    /** Todos los destinos (para la copia externa hacia otro destino). */
    repos: Repo[];
    onchange: (repo: Repo) => void;
    onremoved: (id: string) => void;
    /** Se cambió el nombre del destino (lugar) de este repositorio. */
    onplacerenamed?: (placeId: string, name: string) => void;
    onopencopy: (planId: string) => void;
    onnewcopy: () => void;
    /** Abrir otro destino (p. ej. el origen de la copia externa que llega aquí). */
    onopendestination?: (id: string) => void;
  }
  let { repo, repos, onchange, onremoved, onopencopy, onnewcopy, onopendestination, onplacerenamed }: Props = $props();

  // Renombrar el destino (lugar): solo su nombre visible, sin contraseña.
  let placeEditing = $state(false);
  let placeDraft = $state("");
  const siblings = $derived(repos.filter((r) => r.place_id && r.place_id === repo.place_id && r.id !== repo.id));
  async function savePlaceName(event: SubmitEvent) {
    event.preventDefault();
    const name = placeDraft.trim();
    if (!repo.place_id || !name || name === repo.place_name) {
      placeEditing = false;
      return;
    }
    try {
      const place = await api.renamePlace(repo.place_id, name);
      onplacerenamed?.(place.id, place.name);
      placeEditing = false;
      toast(`Destino renombrado a «${place.name}»`);
    } catch (e) {
      toast(String(e), "error");
    }
  }

  /** Destinos que suben su copia externa aquí. */
  const sources = $derived(offsiteSources(repo.id));
  let tabsEl: HTMLElement | undefined = $state();

  let scheduleEl: HTMLElement | undefined = $state();
  let maintenanceEl: HTMLElement | undefined = $state();
  let protectionEl: HTMLElement | undefined = $state();
  let copiesEl: HTMLElement | undefined = $state();
  const goTo = (el?: HTMLElement) => el?.scrollIntoView({ behavior: "smooth", block: "start" });

  /** Asistente «Mejorar la protección» abierto. */
  let improving = $state(false);
  // Peticiones desde Ctrl+K: restaurar, el asistente o la historia de este destino.
  $effect(() => {
    if (pendingEditor.restore === repo.id) {
      pendingEditor.restore = null;
      browsing = null;
      restoring = true;
    }
    if (pendingEditor.improve === repo.id) {
      pendingEditor.improve = null;
      improving = true;
    }
    if (pendingEditor.historyTab === repo.id && tabsEl) {
      pendingEditor.historyTab = null;
      tab = "history";
      goTo(tabsEl);
    }
  });
  /** Asistente: abrir lo que arregla cada comprobación (el editor de la página). */
  function improveFix(id: string) {
    const isLocal = !repo.location.startsWith("rest:") && !kind.cloud;
    if (id === "kit") openKit([repo.id]);
    else if (id === "restauracion") pendingEditor.maintEditor = { repoId: repo.id, which: "restore_test" };
    else if (id === "verificacion") pendingEditor.maintEditor = { repoId: repo.id, which: "verify" };
    else if (id === "externa" || (id === "borrado" && isLocal)) pendingEditor.maintEditor = { repoId: repo.id, which: "offsite" };
    else if (id === "borrado") goTo(protectionEl);
    else if (id === "copias") {
      setUi("scheduleCollapsed", false);
      pendingEditor.schedule = repo.id;
      goTo(scheduleEl);
    } else fix(id);
  }

  /** Esquema: la copia externa (su editor si aún no hay ninguna, o su tarjeta). */
  function openCloud() {
    if (!agent.info?.repos.find((r) => r.id === repo.id)?.offsite) pendingEditor.maintEditor = { repoId: repo.id, which: "offsite" };
    else {
      setUi("maintCollapsed", false);
      goTo(maintenanceEl);
    }
  }

  /** «Salud de la protección»: llevar a donde se arregla cada cosa. */
  function fix(id: string) {
    const go = (el?: HTMLElement) => el?.scrollIntoView({ behavior: "smooth", block: "start" });
    if (id === "kit") openKit([repo.id]);
    else if (id === "retencion") {
      tab = "retention";
      go(tabsEl);
    } else if (id === "copias") {
      setUi("scheduleCollapsed", false);
      go(scheduleEl);
    } else {
      setUi("maintCollapsed", false);
      go(maintenanceEl);
    }
  }

  /** «Gestionar desde «X»»: abre el origen y muestra su «Mantenimiento». */
  function openSource(id: string) {
    pendingEditor.maintenance = id;
    onopendestination?.(id);
  }

  let snapshots = $state<Snapshot[]>([]);
  let loading = $state(true);
  let error = $state("");
  let copied = $state(false);
  let browsing = $state<Snapshot | null>(null);
  /** Carpeta en la que abrir la versión (desde «Lo que más ocupa»). */
  let browseDir = $state<string | null>(null);
  /** Versión de la que se ve «Lo que más ocupa». */
  let largestOf = $state<Snapshot | null>(null);
  /** Asistente «Restaurar archivos» abierto. */
  let restoring = $state(false);
  let tab = $state<"snapshots" | "retention" | "history">(ui.repoTab);
  /** «Buscar un archivo» abierto (también desde Ctrl+K). */
  let searching = $state(false);
  $effect(() => {
    if (searchUi.open === repo.id) {
      searchUi.open = null;
      browsing = null;
      restoring = false;
      searching = true;
    }
  });
  $effect(() => {
    if (tab !== ui.repoTab) setUi("repoTab", tab);
  });
  // «Abrir en esa versión» desde «Ver versiones»: en cuanto estén las versiones.
  $effect(() => {
    const want = browseUi.pending;
    if (!want || want.repoId !== repo.id || loading) return;
    browseUi.pending = null;
    const snap = snapshots.find((s) => s.id === want.snapshotId);
    if (!snap) {
      toast("Esa versión ya no está en este repositorio.", "info");
      return;
    }
    searching = false;
    restoring = false;
    browseDir = want.dir;
    browsing = snap;
  });
  /** «Cambiar la duración» de la pausa abierto. */
  let pausing = $state(false);
  /** Abrir la versión en la pestaña «Cambios» (desde el aviso de cambio inusual). */
  let browseChanges = $state(false);

  /** «Ver qué cambió»: la versión con el cambio inusual frente a la anterior. */
  function viewChanges(id: string) {
    const snap = snapshots.find((s) => s.id === id || s.short_id === id || s.id.startsWith(id));
    if (!snap) {
      toast("Esa versión aún no aparece en la lista: pulsa «Actualizar» en Versiones.", "info");
      return;
    }
    browseChanges = true;
    browsing = snap;
  }

  // Espacio real en disco: puede tardar en repositorios grandes, se carga aparte.
  let stats = $state<RepoStats | null>(null);
  let statsAt = $state<number | null>(null);
  let statsLoading = $state(true);
  let statsError = $state("");
  /** Lo que ocuparían todos los snapshots sin deduplicación ni compresión. */
  const logicalTotal = $derived(snapshots.reduce((n, s) => n + (s.summary?.total_bytes_processed ?? 0), 0));
  const saving = $derived(stats && logicalTotal > 0 ? Math.max(0, 1 - stats.total_size / logicalTotal) : null);
  /** Cuántas veces más ocuparían las copias sin deduplicar ni comprimir. */
  const factor = $derived(stats && stats.total_size > 0 && logicalTotal > 0 ? logicalTotal / stats.total_size : null);
  /** Texto del ahorro. Un porcentaje nunca llega al 100 % (ocuparía cero):
   *  cuando se acerca, se muestra cuántas veces menos ocupa, que es más claro. */
  const savingLabel = $derived.by(() => {
    if (saving === null || factor === null) return null;
    if (factor >= 10) return `${factor >= 100 ? Math.round(factor).toLocaleString("es") : factor.toFixed(1).replace(".", ",")} veces menos`;
    return `Ahorro del ${Math.floor(saving * 100)} %`;
  });

  /** Solo ejecuta restic si cambió la lista de snapshots desde el último cálculo (o si se fuerza). */
  async function loadStats(force = false) {
    statsLoading = true;
    try {
      const result = await api.repoStats(
        repo.id,
        snapshots.map((s) => s.id),
        force,
      );
      stats = result.stats;
      statsAt = result.computed_at;
      statsError = "";
    } catch (e) {
      statsError = String(e);
    } finally {
      statsLoading = false;
    }
  }

  // Renombrar (solo el nombre que muestra la app).
  let editing = $state(false);
  let draft = $state("");
  function startRename() {
    draft = repo.name;
    editing = true;
  }
  async function saveName(event: SubmitEvent) {
    event.preventDefault();
    const name = draft.trim();
    if (!name || name === repo.name) {
      editing = false;
      return;
    }
    const done = await withPassword({
      title: "Renombrar repositorio",
      message: `«${repo.name}» pasará a llamarse «${name}». Solo cambia el nombre que ves en Resguardo; lo guardado no se toca.`,
      repoName: repo.name,
      confirmLabel: "Renombrar",
      action: async (password) => onchange(await api.renameRepo(repo.id, name, password)),
    });
    if (done) {
      editing = false;
      toast(`Repositorio renombrado a «${name}»`);
    }
  }
  function focusSelect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  const kind = $derived(repoKind(repo.location));
  const latest = $derived(snapshots.reduce<Snapshot | null>((a, s) => (!a || s.time > a.time ? s : a), null));
  const running = $derived(runs[repo.id]?.running ?? false);
  const isRest = $derived(repo.location.startsWith("rest:"));
  const summary = $derived.by(() => {
    void now;
    return destinationSummary(repo, repos);
  });
  const isHttps = $derived(repo.location.startsWith("rest:https://"));

  // Refresca "hace X minutos" sin recargar nada.
  let now = $state(Date.now());
  onMount(() => {
    load();
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });

  async function load() {
    loading = true;
    error = "";
    try {
      snapshots = await api.listSnapshots(repo.id);
      setSnapshots(repo.id, snapshots);
      loadStats();
    } catch (e) {
      error = String(e);
      statsLoading = false;
    } finally {
      loading = false;
    }
  }

  async function remove() {
    const done = await withPassword({
      title: `Quitar el repositorio «${repo.name}»`,
      message: `Resguardo olvidará este repositorio${repo.plans.length ? ` y ${repo.plans.length === 1 ? "su copia" : `sus ${repo.plans.length} copias`} (${repo.plans.map((p) => `«${p.name}»`).join(", ")})` : ""}, y borrará las contraseñas que tiene guardadas. Las versiones guardadas no se tocan: podrás volver a conectarlo cuando quieras.`,
      repoName: repo.name,
      confirmLabel: "Quitar repositorio",
      danger: true,
      action: (password) => api.removeRepo(repo.id, password),
    });
    if (done) onremoved(repo.id);
  }

  async function copyLocation() {
    try {
      await navigator.clipboard.writeText(repo.location);
      copied = true;
      setTimeout(() => (copied = false), 1400);
    } catch {
      /* sin portapapeles */
    }
  }
</script>

{#if restoring}
  <RestoreWizard
    {repo}
    {snapshots}
    all={snapshots}
    {loading}
    {error}
    onrefresh={load}
    context={`Repositorio «${repo.name}» · ${kind.label}`}
    onclose={() => (restoring = false)}
  />
{:else if browsing}
  <SnapshotBrowser
    {repo}
    snapshot={browsing}
    {snapshots}
    startDir={browseDir}
    startTab={browseChanges ? "changes" : "files"}
    {onchange}
    onclose={() => ((browsing = null), (browseDir = null), (browseChanges = false))}
  />
{:else}
<div class="page" in:fade={{ duration: dur(160) }}>
  <header class="page-head">
    <div class="page-icon"><kind.icon size={22} strokeWidth={1.9} /></div>
    <div class="page-head-text">
      {#if repo.place_id}
        <div class="place-crumb">
          {#if placeEditing}
            <form class="rename" onsubmit={savePlaceName}>
              <input
                class="input rename-input small"
                bind:value={placeDraft}
                maxlength="80"
                use:focusSelect
                onkeydown={(e) => e.key === "Escape" && (placeEditing = false)}
                aria-label="Nuevo nombre del destino"
              />
              <button class="btn btn-primary btn-sm">Guardar</button>
              <button type="button" class="btn btn-ghost btn-sm" onclick={() => (placeEditing = false)}>Cancelar</button>
            </form>
          {:else}
            <span class="faint">Destino</span> <InfoTip id="destino" />
            <strong title="El lugar donde está este repositorio{siblings.length ? `, junto con ${siblings.map((s) => `«${s.name}»`).join(', ')}` : ''}">{repo.place_name ?? kind.label}</strong>
            {#if siblings.length}<span class="faint">· {siblings.length + 1} repositorios</span>{/if}
            <button
              class="icon-btn rename-btn"
              title="Renombrar el destino"
              aria-label="Renombrar el destino"
              onclick={() => ((placeDraft = repo.place_name ?? ""), (placeEditing = true))}><Pencil size={12} /></button
            >
            <span class="faint" aria-hidden="true">›</span>
            <span class="faint">Repositorio</span> <InfoTip id="repositorio" />
          {/if}
        </div>
      {/if}
      <div class="page-title-line">
        {#if editing}
          <form class="rename" onsubmit={saveName}>
            <input
              class="input rename-input"
              bind:value={draft}
              maxlength="80"
              use:focusSelect
              onkeydown={(e) => e.key === "Escape" && (editing = false)}
              aria-label="Nuevo nombre"
            />
            <button class="btn btn-primary btn-sm"><Lock size={12} /> Guardar</button>
            <button type="button" class="btn btn-ghost btn-sm" onclick={() => (editing = false)}>Cancelar</button>
          </form>
        {:else}
          <h1 class="page-title" title={repo.name}>{repo.name}</h1>
          <button class="icon-btn rename-btn" title="Renombrar el repositorio" aria-label="Renombrar el repositorio" onclick={startRename}><Pencil size={14} /></button>
        {/if}
        <span class="kind">{kind.label}</span>
        {#if isRest}
          <span class="kind" class:warn={!isHttps} title={isHttps ? "Conexión cifrada con TLS" : "HTTP sin cifrar: las credenciales del servidor viajan en claro"}>
            {#if isHttps}<Lock size={11} /> HTTPS{:else}<LockOpen size={11} /> HTTP{/if}
          </span>
          {#if repo.rest_username}<span class="kind" title="Usuario del servidor"><User size={11} /> {repo.rest_username}</span>{/if}
        {/if}
      </div>
      <div class="location">
        <span class="mono selectable" title={repo.location}>{repo.location}</span>
        <button class="icon-btn" title={copied ? "Copiada" : "Copiar ubicación"} aria-label="Copiar la ubicación del repositorio" onclick={copyLocation}>
          {#if copied}<Check size={13} />{:else}<Copy size={13} />{/if}
        </button>
      </div>
    </div>
    <div class="page-actions">
      <!-- Restaurar es la acción principal solo si hay algo que restaurar: si no, lo es crear la primera copia. -->
      <button
        class="btn"
        class:btn-primary={snapshots.length > 0}
        onclick={() => (restoring = true)}
        title={!loading && !snapshots.length && !error ? "Todavía no hay versiones que restaurar" : "Recuperar archivos de una versión guardada"}
      >
        <RotateCcw size={14} /> Restaurar archivos…
      </button>
      <button class="btn" onclick={() => (searching = true)} title="¿Cuándo existió un archivo y en qué versiones está? (también con Ctrl+K)">
        <FileSearch size={14} /> Buscar un archivo
      </button>
      <button class="btn btn-ghost" onclick={() => openKit([repo.id])} title="Hoja para imprimir con lo necesario para abrir estas copias si pierdes este equipo">
        <KeyRound size={14} /> Kit de recuperación
      </button>
      <button
        class="btn btn-ghost"
        onclick={remove}
        disabled={running}
        title={running ? "Espera a que termine la copia" : "Quitar este repositorio de Resguardo (lo guardado no se toca)"}
      >
        <Trash2 size={14} /> Quitar repositorio
      </button>
    </div>
  </header>

  <PageSummary {summary} onimprove={() => (improving = true)} />
  <FlowDiagram {repo} {repos} {now} onsources={() => goTo(copiesEl)} ondestination={() => goTo(protectionEl)} oncloud={openCloud} />


  <OffsiteHoldBanner {repo} onviewchanges={viewChanges} />
  {#if sources.length}
    <OffsiteSourceCard
      {repo}
      {sources}
      onopensource={openSource}
      onretention={() => {
        tab = "retention";
        tabsEl?.scrollIntoView({ behavior: "smooth", block: "start" });
      }}
    />
  {/if}
  <PauseBanner {repo} change={() => (pausing = true)} />

  {#if error && (error.startsWith(api.STALE_LOCK) || /^Contraseña incorrecta/.test(error))}
    <div class="notice notice-danger fixable" role="alert">
      <p>{error.startsWith(api.STALE_LOCK) ? error : "La contraseña guardada ya no abre este repositorio. ¿La cambiaste en otro equipo o con restic?"}</p>
      <RepoErrorActions repoId={repo.id} repoName={repo.name} message={error} onfixed={load} />
    </div>
  {/if}

  <div class="anchor" bind:this={protectionEl}><ProtectionHealth {repo} {onchange} onfix={fix} onimprove={() => (improving = true)} /></div>

  <div class="stats">
    <div class="stat">
      <span class="stat-label">Última copia <InfoTip id="ultima-copia" /></span>
      <span class="stat-value" title={latest ? formatDate(latest.time) : ""}>
        {#key now}{latest ? formatRelative(latest.time) : loading ? "…" : "Nunca"}{/key}
      </span>
    </div>
    <div class="stat">
      <span class="stat-label">Versiones <InfoTip id="versiones" /></span>
      <span class="stat-value">{loading && !snapshots.length ? "…" : snapshots.length}</span>
    </div>
    <div class="stat">
      <span class="stat-label">Tamaño protegido <InfoTip id="tamano-protegido" /></span>
      <span class="stat-value">{latest ? formatBytes(latest.summary?.total_bytes_processed) : "—"}</span>
      <span class="stat-sub">en la última versión</span>
    </div>
    <div
      class="stat"
      title={stats
        ? `Ocupa ${formatBytes(stats.total_size)} en disco (${formatBytes(stats.total_uncompressed_size)} sin comprimir; la compresión ahorra ${stats.compression_space_saving.toFixed(0)} %). Sin deduplicación, las ${snapshots.length} versiones ocuparían ${formatBytes(logicalTotal)}${saving !== null ? ` (ahorro del ${(saving * 100).toLocaleString("es", { maximumFractionDigits: 2 })} %)` : ""}.`
        : statsError}
    >
      <span class="stat-label">
        Espacio en disco <InfoTip id="espacio-disco" />
        {#if stats && !statsLoading}
          <button
            class="icon-btn recalc"
            title="Recalcular (calculado {statsAt ? formatRelative(new Date(statsAt * 1000).toISOString()) : ''})"
            aria-label="Recalcular el espacio en disco"
            onclick={() => loadStats(true)}
          >
            <RefreshCw size={12} />
          </button>
        {/if}
      </span>
      {#if stats}
        <span class="stat-value" class:shimmer={statsLoading}>{formatBytes(stats.total_size)}</span>
        {#if savingLabel}<span class="stat-sub good">{savingLabel}</span>{/if}
      {:else if statsLoading}
        <span class="stat-value shimmer">Calculando…</span>
        <span class="stat-sub">puede tardar en repositorios grandes</span>
      {:else}
        <span class="stat-value">—</span>
        <button class="stat-sub retry" onclick={() => loadStats(true)}>No se pudo calcular · reintentar</button>
      {/if}
    </div>
  </div>

  <div class="anchor" bind:this={scheduleEl}><ScheduleCard {repo} /></div>
  <div class="anchor" bind:this={copiesEl}><DestinationCopies {repo} {onopencopy} {onnewcopy} ondone={load} receivesFrom={sources.map((s) => s.name)} /></div>
  <div class="anchor" bind:this={maintenanceEl}><MaintenanceCard {repo} {repos} {onchange} /></div>
  <!-- Pestañas: con las flechas se cambia de una a otra (patrón de pestañas accesible). -->
  <div
    bind:this={tabsEl}
    class="tabs"
    role="tablist"
    aria-label="Versiones, retención e historia"
    tabindex="-1"
    onkeydown={(e) => {
      if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
      e.preventDefault();
      const order = ["snapshots", "retention", "history"] as const;
      tab = order[(order.indexOf(tab) + (e.key === "ArrowRight" ? 1 : order.length - 1)) % order.length];
      (e.currentTarget.querySelector(`#tab-${tab}`) as HTMLElement | null)?.focus();
    }}
  >
    <button
      id="tab-snapshots"
      role="tab"
      aria-selected={tab === "snapshots"}
      aria-controls="tabpanel"
      tabindex={tab === "snapshots" ? 0 : -1}
      class:on={tab === "snapshots"}
      onclick={() => (tab = "snapshots")}
    >
      Versiones
    </button>
    <button
      id="tab-retention"
      role="tab"
      aria-selected={tab === "retention"}
      aria-controls="tabpanel"
      tabindex={tab === "retention" ? 0 : -1}
      class:on={tab === "retention"}
      onclick={() => (tab = "retention")}
    >
      Retención{#if repo.retention}<span class="tab-dot" title="Con una política de retención"></span>{/if}
    </button>
    <button
      id="tab-history"
      role="tab"
      aria-selected={tab === "history"}
      aria-controls="tabpanel"
      tabindex={tab === "history" ? 0 : -1}
      class:on={tab === "history"}
      onclick={() => (tab = "history")}
    >
      Historia
    </button>
  </div>

  <div id="tabpanel" class="tabpanel" role="tabpanel" aria-labelledby="tab-{tab}">
  {#if tab === "snapshots"}
    <ActivityCard {snapshots} />
    <SnapshotList
      {snapshots}
      {loading}
      {error}
      title="Todas las versiones"
      emptyHint={repo.plans.length ? "Pulsa «Copiar ahora» en una de sus copias para guardar la primera versión." : "Crea una copia en este repositorio para guardar la primera versión."}
      onrefresh={load}
      onopen={(s) => (browsing = s)}
      onlargest={(s) => (largestOf = s)}
    />
  {:else if tab === "retention"}
    <RetentionPanel {repo} {onchange} />
  {:else}
    <RepoHistory {repo} />
  {/if}
  </div>
</div>
{/if}

{#if largestOf}
  <LargestDialog
    {repo}
    snapshot={largestOf}
    {onchange}
    onclose={() => (largestOf = null)}
    onopenfolder={(path) => {
      browseDir = path;
      browsing = largestOf;
      largestOf = null;
    }}
  />
{/if}

{#if searching}
  <SearchDialog
    {repo}
    {snapshots}
    {loading}
    onclose={() => (searching = false)}
    onopen={(snap, dir) => {
      searching = false;
      browseDir = dir;
      browsing = snap;
    }}
  />
{/if}

{#if pausing}<PauseDialog {repo} onclose={() => (pausing = false)} />{/if}

{#if improving && !browsing && !restoring}<ImproveGuide {repo} onfix={improveFix} onclose={() => (improving = false)} />{/if}

<style>
  .fixable {
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }
  .page-title-line {
    flex-wrap: wrap;
    row-gap: 4px;
  }
  .kind {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: none;
    padding: 0 9px;
    font-size: var(--fs-xs);
    font-weight: 600;
    line-height: 22px;
    border-radius: 999px;
    color: var(--text-2);
    background: var(--surface-3);
  }
  .kind.warn {
    color: var(--warn);
    background: var(--warn-soft);
  }
  .location {
    display: flex;
    align-items: center;
    gap: 2px;
    min-width: 0;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .location span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .location .icon-btn {
    width: 24px;
    height: 24px;
    flex: none;
  }

  .tabs {
    display: flex;
    gap: 4px;
    margin: 4px 0 -6px;
    border-bottom: 1px solid var(--border);
  }
  .tabs button {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px 10px;
    font: inherit;
    font-weight: 600;
    color: var(--text-3);
    background: none;
    border: none;
    cursor: pointer;
    transition: color 0.15s;
  }
  .tabs button:hover {
    color: var(--text-1);
  }
  .tabs button.on {
    color: var(--text-1);
  }
  .tabs button::after {
    content: "";
    transform: scaleX(0);
    transition: transform 0.2s cubic-bezier(0.2, 0.8, 0.2, 1);
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: -1px;
    height: 2px;
    border-radius: 2px;
    background: var(--accent);
  }
  .tabs button.on::after {
    transform: scaleX(1);
  }
  .tabs button:focus-visible {
    outline-offset: -2px;
    border-radius: var(--radius-sm);
  }
  /* El panel ocupa el mismo hueco que antes tenían sus piezas sueltas. */
  .tabpanel {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .tab-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }
  .rename {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
    min-width: 0;
  }
  .rename-input {
    max-width: 360px;
    height: 34px;
    font-family: var(--font-display);
    font-size: 17px;
    font-weight: 650;
  }
  .rename-btn {
    opacity: 0;
    flex: none;
    transition: opacity 0.15s;
  }
  .page-title-line:hover .rename-btn,
  .rename-btn:focus-visible {
    opacity: 1;
  }
  .recalc {
    width: 22px;
    height: 22px;
    margin-right: -8px;
    opacity: 0;
    transition: opacity 0.15s;
  }
  .stat:hover .recalc,
  .recalc:focus-visible {
    opacity: 1;
  }
  .stat-sub.good {
    color: var(--ok);
  }
  .retry {
    padding: 0;
    font: inherit;
    font-size: var(--fs-xs);
    text-align: left;
    color: var(--bad);
    background: none;
    border: none;
    cursor: pointer;
  }
  .retry:hover {
    text-decoration: underline;
  }
  .shimmer {
    color: var(--text-3);
    animation: fade-pulse 1.2s ease-in-out infinite alternate;
  }
  .place-crumb {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 2px;
    font-size: var(--fs-sm);
  }
  .place-crumb strong {
    font-weight: 600;
  }
  .rename-input.small {
    height: 30px;
    width: 240px;
  }
  .anchor {
    scroll-margin-top: 16px;
  }
</style>
