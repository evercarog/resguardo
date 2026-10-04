<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getVersion } from "@tauri-apps/api/app";
  import { fade } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { CircleAlert } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { LocatedPath, Repo } from "$lib/api";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import RepoView from "$lib/components/RepoView.svelte";
  import PlaceView from "$lib/components/PlaceView.svelte";
  import SharedPlacesDialog from "$lib/components/SharedPlacesDialog.svelte";
  import CopyView from "$lib/components/CopyView.svelte";
  import AddRepoDialog from "$lib/components/AddRepoDialog.svelte";
  import NewCopyWizard from "$lib/components/NewCopyWizard.svelte";
  import PasswordPrompt from "$lib/components/PasswordPrompt.svelte";
  import SettingsView from "$lib/components/SettingsView.svelte";
  import { settingsNav, takeSettingsIntent } from "$lib/settingsNav.svelte";
  import CloseDialog from "$lib/components/CloseDialog.svelte";
  import RecoveryKit from "$lib/components/RecoveryKit.svelte";
  import { kitView, openKit } from "$lib/kit.svelte";
  import PauseDialog from "$lib/components/PauseDialog.svelte";
  import { syncTray, trayStatus } from "$lib/tray.svelte";
  import VersionsDialog from "$lib/components/VersionsDialog.svelte";
  import { browseUi } from "$lib/search.svelte";
  import StatusView from "$lib/components/StatusView.svelte";
  import Onboarding from "$lib/components/Onboarding.svelte";
  import EquiposView from "$lib/components/EquiposView.svelte";
  import ManagedView from "$lib/components/ManagedView.svelte";
  import ActivityView from "$lib/components/ActivityView.svelte";
  import Toaster from "$lib/components/Toaster.svelte";
  import HelpCenter from "$lib/components/HelpCenter.svelte";
  import QuickSwitcher from "$lib/components/QuickSwitcher.svelte";
  import NewsDialog from "$lib/components/NewsDialog.svelte";
  import { checkNews, news } from "$lib/news.svelte";
  import { closeHelp, help, openHelp } from "$lib/help.svelte";
  import { anyModalOpen } from "$lib/components/Modal.svelte";
  import { refreshAll } from "$lib/status.svelte";
  import { agent, refreshAgent } from "$lib/agent.svelte";
  import { runs, startBackup } from "$lib/backups.svelte";
  import { allCopies } from "$lib/copies.svelte";
  import type { Selection } from "$lib/nav";
  import { pendingEditor, takeIntent } from "$lib/intent.svelte";
  import { toast } from "$lib/toast.svelte";

  let version = $state("");
  let appVersion = $state("");
  let repos = $state<Repo[]>([]);
  let selection = $state<Selection>({ kind: "status" });
  let showAdd = $state(false);
  /** «Usar un destino compartido» abierto. */
  let sharedOpen = $state(false);
  /** Asistente «Nueva copia» (con el destino ya elegido, si viene de un destino). */
  let wizard = $state<{ repoId: string | null } | null>(null);
  /** «Ir a…» (Ctrl+K). */
  let switcher = $state(false);
  let loaded = $state(false);
  let error = $state("");

  const selectedRepo = $derived.by(() => {
    const sel = selection;
    return sel.kind === "copy" || sel.kind === "destination" ? (repos.find((r) => r.id === sel.repoId) ?? null) : null;
  });
  const selectedPlan = $derived.by(() => {
    const sel = selection;
    return sel.kind === "copy" && selectedRepo ? (selectedRepo.plans.find((p) => p.id === sel.planId) ?? null) : null;
  });
  const copies = $derived(allCopies(repos));

  // Si lo abierto desaparece (se quitó el destino o se eliminó la copia), se vuelve a algo que existe.
  $effect(() => {
    if (!loaded) return;
    if (selection.kind === "copy" && !selectedPlan) selection = selectedRepo ? { kind: "destination", repoId: selectedRepo.id } : { kind: "status" };
    else if (selection.kind === "destination" && !selectedRepo) selection = { kind: "status" };
  });

  // Ajustes pedidos desde otra parte de la app («Cambiar en Ajustes»).
  $effect(() => {
    const want = settingsNav.request;
    if (!want) return;
    settingsNav.request = null;
    selection = { kind: "ajustes", section: want };
  });

  // ---------- Bandeja del sistema ----------

  /** «Pausar copias automáticas 1 hora» desde la bandeja: destinos con copias automáticas. */
  let trayPause = $state<Repo[] | null>(null);
  let trayNow = $state(Date.now());
  $effect(() => {
    syncTray(trayStatus(repos, loaded, trayNow));
  });
  onMount(() => {
    // El estado del agente (copias automáticas y subidas en marcha) se mira cada minuto.
    const t = setInterval(() => {
      trayNow = Date.now();
      if (agent.info?.task_installed) refreshAgent();
    }, 60_000);
    const offs = [
      listen<{ repo_id: string; plan_id: string }>("tray-copy", ({ payload }) => {
        const repo = repos.find((r) => r.id === payload.repo_id);
        const plan = repo?.plans.find((p) => p.id === payload.plan_id);
        if (!repo || !plan) return;
        if (runs[repo.id]?.running) {
          toast(`Ya hay una copia en marcha en «${repo.name}»`, "info");
          return;
        }
        toast(`Copiando «${plan.name}»…`, "info");
        void startBackup(repo.id, repo.name, plan.id, plan.name);
      }),
      // Vista pedida desde un aviso de Windows o desde la bandeja.
      listen("open-view", () => void takeView()),
      // «Ver versiones en Resguardo» desde el Explorador (con la app ya abierta).
      listen("open-versions", () => void takeVersions()),
    ];
    return () => {
      clearInterval(t);
      offs.forEach((off) => off.then((f) => f()));
    };
  });

  /** Abre lo que se pidió desde fuera de la ventana (si hay algo pendiente). */
  async function takeView() {
    const view = await api.takeView().catch(() => null);
    if (!view) return;
    if (view.kind === "copy") openCopy(view.repoId, view.planId);
    else if (view.kind === "destination") openDestination(view.repoId);
    else if (view.kind === "kit") openKit(view.ids);
    else if (view.kind === "pause") {
      const scheduled = repos.filter((r) => agent.info?.repos.some((a) => a.id === r.id));
      if (scheduled.length) trayPause = scheduled;
      else toast("Aún no hay copias automáticas que pausar", "info");
    }
  }

  // ---------- «Ver versiones en Resguardo» (Explorador) ----------

  let versions = $state<LocatedPath | null>(null);
  async function takeVersions() {
    const raw = await api.versionsPending().catch(() => null);
    if (!raw) return;
    try {
      versions = await api.locatePath(raw);
    } catch (e) {
      toast(String(e), "error", 6000);
    }
  }
  /** «Abrir en esa versión»: el destino, con esa versión abierta en esa carpeta. */
  function openVersion(repoId: string, snapshotId: string, dir: string) {
    versions = null;
    browseUi.pending = { repoId, snapshotId, dir };
    openDestination(repoId);
  }

  // Comprobación ligera (solo la lista de snapshots) cada 15 minutos y al volver a la app.
  let lastCheck = Date.now();
  function periodicCheck() {
    if (Date.now() - lastCheck < 5 * 60_000) return;
    lastCheck = Date.now();
    refreshAll(repos);
    refreshAgent();
  }
  onMount(() => {
    const t = setInterval(() => ((lastCheck = 0), periodicCheck()), 15 * 60_000);
    const onVisible = () => document.visibilityState === "visible" && periodicCheck();
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      clearInterval(t);
      document.removeEventListener("visibilitychange", onVisible);
    };
  });

  onMount(async () => {
    api.resticVersion().then((v) => (version = v), () => (version = ""));
    getVersion().then(
      (v) => {
        appVersion = v;
        // Tras una actualización: «Novedades».
        checkNews(v);
      },
      () => (appVersion = ""),
    );
    refreshAgent();
    try {
      repos = await api.listRepos();
      selection = { kind: "status" }; // portada: panel de Estado
      // Al volver de reabrir como administrador desde Ajustes, se vuelve allí.
      const settingsIntent = takeSettingsIntent();
      if (settingsIntent) selection = { kind: "ajustes", section: settingsIntent };
      // Al volver de reabrir como administrador, retoma lo que se estaba haciendo.
      const intent = takeIntent();
      const repo = intent ? repos.find((r) => r.id === intent.repoId) : null;
      if (intent && repo) {
        if (intent.intent === "apply" && intent.planId && repo.plans.some((p) => p.id === intent.planId)) {
          selection = { kind: "copy", repoId: repo.id, planId: intent.planId };
          pendingEditor.apply = intent.planId;
        } else {
          selection = { kind: "destination", repoId: repo.id };
          if (intent.intent !== "apply") pendingEditor[intent.intent] = repo.id;
        }
      }
      refreshAll(repos);
      // Lo pedido desde fuera antes de que se abriera (o desbloqueara) la ventana.
      void takeView();
      void takeVersions();
    } catch (e) {
      error = String(e);
    } finally {
      loaded = true;
    }
  });

  function updateRepo(repo: Repo) {
    repos = repos.map((r) => (r.id === repo.id ? repo : r));
  }

  function openCopy(repoId: string, planId: string) {
    selection = { kind: "copy", repoId, planId };
  }
  function openDestination(repoId: string) {
    selection = { kind: "destination", repoId };
  }
  /** Abrir «Actividad», filtrada por un destino ("repoId") o una copia ("repoId#planId"). */
  function openActivity(filter?: string) {
    selection = { kind: "activity", filter };
  }

  /** Destino añadido (desde «Añadir destino» o desde el asistente de copia). */
  // «Equipos gestionados» se ve en la barra lateral si este equipo es Servidor de copias.
  let managedShown = $state(false);
  $effect(() => {
    if (selection.kind === "ajustes") return; // al salir de Ajustes se vuelve a mirar
    api
      .serverStatus()
      .then((s) => (managedShown = s.enabled))
      .catch(() => {});
  });

  function repoAdded(repo: Repo) {
    if (!repos.some((r) => r.id === repo.id)) repos = [...repos, repo];
    refreshAll([repo]);
  }

  function created(repo: Repo) {
    repoAdded(repo);
    showAdd = false;
    openDestination(repo.id);
    toast(`Repositorio «${repo.name}» añadido. Ahora crea una copia que se guarde en él.`);
  }

  function removed(id: string) {
    const name = repos.find((r) => r.id === id)?.name ?? "";
    repos = repos.filter((r) => r.id !== id);
    selection = { kind: "status" };
    toast(`«${name}» se quitó de Resguardo. Las versiones guardadas siguen intactas.`);
  }

  // Título de la ventana: lo que está abierto y, si hay una copia en marcha, su progreso.
  const title = $derived.by(() => {
    const active = repos.find((r) => runs[r.id]?.running);
    if (active) {
      const run = runs[active.id];
      const percent = Math.round((run.status?.percent ?? 0) * 100);
      return `${percent} % · Copiando «${run.planName || active.name}» · Resguardo`;
    }
    if (selectedPlan) return `«${selectedPlan.name}» · Resguardo`;
    if (selection.kind === "activity") return "Actividad · Resguardo";
    if (selection.kind === "equipos") return "Todos mis equipos · Resguardo";
    if (selection.kind === "gestionados") return "Equipos gestionados · Resguardo";
    if (selection.kind === "ajustes") return "Ajustes · Resguardo";
    if (selection.kind === "place") {
      const placeId = selection.placeId;
      const name = repos.find((r) => r.place_id === placeId)?.place_name;
      if (name) return `Destino «${name}» · Resguardo`;
    }
    return selectedRepo ? `«${selectedRepo.name}» · Resguardo` : "Resguardo";
  });

  /** Lista de la sección abierta, para moverse con Ctrl+↑/↓. */
  function sectionItems(): Selection[] {
    if (selection.kind === "destination") return repos.map((r) => ({ kind: "destination", repoId: r.id }));
    return copies.map((c) => ({ kind: "copy", repoId: c.repo.id, planId: c.plan.id }));
  }
  const sameSel = (a: Selection, b: Selection) =>
    a.kind === b.kind &&
    (a.kind === "status" ||
      a.kind === "activity" ||
      a.kind === "equipos" ||
      a.kind === "gestionados" ||
      a.kind === "ajustes" ||
      (a.kind === "place" ? a.placeId === (b as { placeId: string }).placeId : a.repoId === (b as { repoId: string }).repoId && (a.kind !== "copy" || a.planId === (b as { planId: string }).planId)));

  /** Atajos: F1 ayuda, Ctrl+K ir a…, Ctrl+N nueva copia, Ctrl+Mayús+N añadir destino, Ctrl+1 estado, Ctrl+2 actividad, Ctrl+↑/↓ anterior o siguiente de la sección. */
  function onkeydown(e: KeyboardEvent) {
    // F1: ayuda, desde cualquier sitio (también con un diálogo abierto, encima de él).
    if (e.key === "F1" && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey) {
      e.preventDefault();
      if (!help.open) openHelp();
      return;
    }
    if (!e.ctrlKey || e.altKey || e.metaKey || anyModalOpen() || showAdd || wizard || switcher) return;
    // Ctrl+, : Ajustes.
    if (e.key === "," && !e.shiftKey) {
      e.preventDefault();
      selection = { kind: "ajustes" };
      return;
    }
    // Ctrl+K funciona también desde un campo de texto (no escribe nada en él).
    if ((e.key === "k" || e.key === "K") && !e.shiftKey && loaded) {
      e.preventDefault();
      switcher = true;
      return;
    }
    const t = e.target as HTMLElement | null;
    if (t?.closest("input, textarea, select, [contenteditable]:not([contenteditable='false'])")) return;
    if (e.key === "n" || e.key === "N") {
      e.preventDefault();
      // Sin destinos, el asistente de copia empieza añadiendo uno.
      if (e.shiftKey) showAdd = true;
      else wizard = { repoId: selection.kind === "destination" ? selection.repoId : null };
    } else if (e.shiftKey) {
      return;
    } else if (e.key === "1" && repos.length) {
      e.preventDefault();
      selection = { kind: "status" };
    } else if (e.key === "2" && repos.length) {
      e.preventDefault();
      openActivity();
    } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      const items = sectionItems();
      if (!items.length) return;
      e.preventDefault();
      const i = items.findIndex((s) => sameSel(s, selection));
      const next = e.key === "ArrowDown" ? (i < 0 ? 0 : Math.min(items.length - 1, i + 1)) : i < 0 ? items.length - 1 : Math.max(0, i - 1);
      selection = items[next];
    }
  }
</script>

<svelte:head><title>{title}</title></svelte:head>
<svelte:window {onkeydown} />

<div class="app">
  <Sidebar
    {repos}
    {selection}
    {version}
    {appVersion}
    onstatus={() => (selection = { kind: "status" })}
    onactivity={() => openActivity()}
    onequipos={() => (selection = { kind: "equipos" })}
    managed={managedShown}
    ongestionados={() => (selection = { kind: "gestionados" })}
    onselectcopy={openCopy}
    onselectdestination={openDestination}
    onselectplace={(placeId) => (selection = { kind: "place", placeId })}
    onnewcopy={() => (wizard = { repoId: null })}
    onadddestination={() => (showAdd = true)}
    onappearance={() => (selection = { kind: "ajustes" })}
    onhelp={() => openHelp()}
    onnavigate={(sel) => (selection = sel)}
  />

  <main>
    {#if error}
      <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>
    {/if}

    {#if selectedPlan && selectedRepo}
      {#key `${selectedRepo.id}#${selectedPlan.id}`}
        <CopyView
          repo={selectedRepo}
          plan={selectedPlan}
          {repos}
          onchange={updateRepo}
          onopencopy={openCopy}
          onopendestination={() => openDestination(selectedRepo.id)}
          onopenactivity={() => openActivity(`${selectedRepo.id}#${selectedPlan.id}`)}
          ondeleted={() => openDestination(selectedRepo.id)}
          onadddestination={() => (showAdd = true)}
        />
      {/key}
    {:else if selectedRepo && selection.kind === "destination"}
      {#key selectedRepo.id}
        <RepoView
          repo={selectedRepo}
          {repos}
          onchange={updateRepo}
          onremoved={removed}
          onopencopy={(planId) => openCopy(selectedRepo.id, planId)}
          onnewcopy={() => (wizard = { repoId: selectedRepo.id })}
          onopendestination={openDestination}
          onplacerenamed={(id, name) => (repos = repos.map((r) => (r.place_id === id ? { ...r, place_name: name } : r)))}
        />
      {/key}
    {:else if loaded && selection.kind === "place" && repos.some((r) => r.place_id === (selection as { placeId: string }).placeId)}
      {#key selection.placeId}
        <PlaceView
          placeId={selection.placeId}
          {repos}
          onopenrepo={openDestination}
          onrepoadded={repoAdded}
          onplacerenamed={(id, name) => (repos = repos.map((r) => (r.place_id === id ? { ...r, place_name: name } : r)))}
        />
      {/key}
    {:else if loaded && selection.kind === "equipos"}
      <EquiposView />
    {:else if loaded && selection.kind === "gestionados"}
      <ManagedView
        onopenrepo={(repo) => {
          repoAdded(repo);
          openDestination(repo.id);
        }}
        onsettings={() => (selection = { kind: "ajustes", section: "equipo" })}
      />
    {:else if loaded && selection.kind === "ajustes"}
      <SettingsView {repos} section={selection.section} {appVersion} resticVersion={version} onequipos={() => (selection = { kind: "equipos" })} />
    {:else if loaded && repos.length && selection.kind === "activity"}
      {#key selection.filter ?? ""}
        <ActivityView {repos} filter={selection.filter} onnavigate={(sel) => (selection = sel)} />
      {/key}
    {:else if loaded && repos.length}
      <StatusView
        {repos}
        onopen={(r) => openDestination(r.id)}
        onopencopy={openCopy}
        onnewcopy={(repoId) => (wizard = { repoId: repoId ?? null })}
        onadddestination={() => (showAdd = true)}
        onchange={updateRepo}
        onopenactivity={() => openActivity()}
      />
    {:else if loaded}
      <Onboarding
        variant="page"
        {repos}
        onadd={() => (wizard = { repoId: null })}
        onnewcopy={() => (wizard = { repoId: null })}
        onopen={(r) => openDestination(r.id)}
      />
    {/if}
  </main>
</div>

{#if showAdd}
  <AddRepoDialog
    {repos}
    onclose={() => (showAdd = false)}
    oncreated={created}
    onshared={() => ((showAdd = false), (sharedOpen = true))}
  />
{/if}

{#if sharedOpen}
  <SharedPlacesDialog
    onclose={() => (sharedOpen = false)}
    onequipos={() => ((sharedOpen = false), (selection = { kind: "equipos" }))}
    oncreated={(repo, generated) => {
      sharedOpen = false;
      repoAdded(repo);
      openDestination(repo.id);
      if (generated) openKit([repo.id]);
    }}
  />
{/if}

{#if wizard}
  <NewCopyWizard
    {repos}
    initialRepoId={wizard.repoId}
    onclose={() => (wizard = null)}
    onrepoadded={repoAdded}
    onchange={updateRepo}
    onopencopy={openCopy}
  />
{/if}

{#if news.open}
  <NewsDialog onclose={() => (news.open = false)} />
{/if}

{#if switcher}
  <QuickSwitcher
    {repos}
    onpick={(sel) => (selection = sel)}
    onclose={() => (switcher = false)}
    onnewcopy={() => (wizard = { repoId: selection.kind === "destination" ? selection.repoId : null })}
    onadddestination={() => (showAdd = true)}
    onshared={() => (sharedOpen = true)}
  />
{/if}


{#if kitView.open}
  {#key kitView.ids?.join() ?? "todos"}
    <RecoveryKit {repos} ids={kitView.ids} onclose={() => (kitView.open = false)} onchange={updateRepo} />
  {/key}
{/if}

{#if versions}
  {#key versions.path}
    <VersionsDialog
      located={versions}
      {repos}
      onopen={openVersion}
      onnewcopy={() => ((versions = null), (wizard = { repoId: null }))}
      onclose={() => (versions = null)}
    />
  {/key}
{/if}

{#if trayPause}
  <PauseDialog repo={trayPause[0]} repos={trayPause} initial="1h" onclose={() => (trayPause = null)} />
{/if}

<PasswordPrompt />
{#if help.open}
  <HelpCenter topic={help.topic} onclose={closeHelp} />
{/if}
<CloseDialog />
<Toaster />

<style>
  .app {
    display: grid;
    grid-template-columns: var(--sidebar-w) minmax(0, 1fr);
    height: 100vh;
    background: var(--bg);
  }
  main {
    overflow: auto;
    padding: var(--sp-8) var(--sp-10) var(--sp-12);
    scrollbar-gutter: stable;
  }
  /* Ventanas estrechas: barra lateral más angosta y menos margen. */
  @media (max-width: 1100px) {
    .app {
      grid-template-columns: 232px minmax(0, 1fr);
    }
    main {
      padding: var(--sp-6) var(--sp-6) var(--sp-10);
    }
  }
</style>
