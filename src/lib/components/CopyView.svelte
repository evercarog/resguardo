<script lang="ts">
  import { onMount } from "svelte";
  import { fade, slide } from "svelte/transition";
  import { dur } from "$lib/motion";
  import {
    ArrowRightLeft,
    CalendarClock,
    ChevronRight,
    CircleAlert,
    CircleCheck,
    CirclePause,
    Copy,
    Folder,
    FolderSync,
    Hand,
    Info,
    LoaderCircle,
    Pencil,
    Play,
    RefreshCw,
    RotateCcw,
    ShieldCheck,
    Tag,
    Trash2,
    TriangleAlert,
    Eye,
    FilterX,
  } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Plan, Repo, Snapshot } from "$lib/api";
  import { agent, refreshAgent } from "$lib/agent.svelte";
  import { runs, startBackup } from "$lib/backups.svelte";
  import { copyStatus, planSnapshots } from "$lib/copies.svelte";
  import { copySummary } from "$lib/summary.svelte";
  import { protections, refreshProtection } from "$lib/protection.svelte";
  import PageSummary from "./PageSummary.svelte";
  import InfoTip from "./InfoTip.svelte";
  import { formatBytes, formatDate, formatRelative, formatShortWhen } from "$lib/format";
  import { forgetIntent, pendingEditor, rememberIntent } from "$lib/intent.svelte";
  import { MAX_PLANS, nextPlanSlots, planScheduleLabel, planScheduleSentence, slotDayLabel, slotTime, slotsPreview } from "$lib/plans";
  import { splitPath } from "$lib/paths";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { repoKind } from "$lib/repoKind";
  import { health, setSnapshots } from "$lib/status.svelte";
  import { toast } from "$lib/toast.svelte";
  import MoveCopyDialog from "./MoveCopyDialog.svelte";
  import PlanEditor from "./PlanEditor.svelte";
  import RunProgress from "./RunProgress.svelte";
  import RestoreWizard from "./RestoreWizard.svelte";
  import SnapshotBrowser from "./SnapshotBrowser.svelte";
  import SnapshotList from "./SnapshotList.svelte";
  import LargestDialog from "./LargestDialog.svelte";
  import RecentActivity from "./RecentActivity.svelte";
  import RunResult from "./RunResult.svelte";
  import AutoProgress from "./AutoProgress.svelte";
  import PauseBanner from "./PauseBanner.svelte";
  import PauseDialog from "./PauseDialog.svelte";
  import { untilWords } from "$lib/pause.svelte";
  import { activity, refreshActivity } from "$lib/activity.svelte";

  // Una copia (un plan de un destino): qué copia, cuándo, su estado y su
  // historial de versiones.
  interface Props {
    repo: Repo;
    plan: Plan;
    /** Todos los destinos (para «Cambiar destino»). */
    repos: Repo[];
    /** Un destino cambió (se guardó). */
    onchange: (repo: Repo) => void;
    /** Abrir otra copia (tras duplicar o mover). */
    onopencopy: (repoId: string, planId: string) => void;
    /** Abrir el destino de esta copia. */
    onopendestination: () => void;
    /** La copia se eliminó. */
    ondeleted: () => void;
    /** Abrir «Actividad» filtrada por esta copia. */
    onopenactivity: () => void;
    /** Añadir un destino (desde «Cambiar destino», si no hay otro). */
    onadddestination?: () => void;
  }
  let { repo, plan, repos, onchange, onopencopy, onopendestination, ondeleted, onopenactivity, onadddestination }: Props = $props();

  let loading = $state(false);
  let error = $state("");
  let browsing = $state<Snapshot | null>(null);
  /** Carpeta en la que abrir la versión (desde «Lo que más ocupa»). */
  let browseDir = $state<string | null>(null);
  /** Versión de la que se ve «Lo que más ocupa». */
  let largestOf = $state<Snapshot | null>(null);
  /** Asistente «Restaurar archivos» abierto. */
  let restoring = $state(false);
  let editing = $state<Plan | null>(null);
  let moving = $state(false);
  let autoError = $state("");
  /** Diálogo «Pausar copias automáticas» (del destino) abierto. */
  let pausing = $state(false);
  /** Ver todas las versiones del destino (no solo las de esta copia). */
  let showAll = $state(false);

  // "Ahora" avanza solo para que «hace X minutos» y los retrasos se actualicen.
  let now = $state(Date.now());

  const kind = $derived(repoKind(repo.location));
  const status = $derived(copyStatus(repo, plan, now));
  const summary = $derived(copySummary(repo, plan, now));
  // Lo que falta en su destino (para el resumen), si aún no se calculó.
  $effect(() => {
    if (!protections[repo.id]) void refreshProtection(repo);
  });
  const all = $derived(health[repo.id]?.snapshots ?? []);
  const mine = $derived(planSnapshots(all, plan));
  const run = $derived(runs[repo.id]);
  /** Copia a mano en marcha en este destino (de esta u otra copia: solo una a la vez). */
  const busy = $derived(run?.running ?? false);
  const busyOther = $derived(busy && run?.planId !== plan.id);
  const showRun = $derived(!!run && run.planId === plan.id);
  const scheduledInRepo = $derived(repo.plans.filter((p) => p.schedule));
  const preview = $derived(plan.schedule ? nextPlanSlots(plan.schedule, new Date(now), 4) : []);
  const info = $derived(agent.info);
  /** Copias correctas sin cambios (historial y última automática): días al día aunque sin versión nueva. */
  const checks = $derived([
    ...new Set([
      ...activity.entries.filter((e) => e.unchanged && e.repo_id === repo.id && e.plan_id === plan.id).map((e) => e.finished),
      ...(status.lastRun?.unchanged ? [status.lastRun.finished] : []),
    ]),
  ]);

  onMount(() => {
    refreshAgent();
    // Sin datos recientes del destino, se piden (es una consulta ligera).
    const h = health[repo.id];
    if (!h || !h.loadedAt || Date.now() - h.loadedAt > 60_000) load();
    let ticks = 0;
    const t = setInterval(() => {
      ticks++;
      if (ticks % 10 === 0) now = Date.now();
      // Mientras el agente copia esta copia se consulta cada 3 s; si toca pronto, cada 15 s.
      if (status.agentRunning || (ticks % 5 === 0 && status.next && status.next.getTime() <= Date.now())) refreshAgent();
    }, 3_000);
    return () => clearInterval(t);
  });

  async function load() {
    loading = true;
    error = "";
    try {
      setSnapshots(repo.id, await api.listSnapshots(repo.id));
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function start() {
    await startBackup(repo.id, repo.name, plan.id, plan.name);
    if (runs[repo.id]?.result) load();
    refreshActivity();
  }

  function duplicate() {
    // Nombre libre: «Laboral (copia)», «Laboral (copia 2)»…
    let name = `${plan.name} (copia)`;
    for (let i = 2; repo.plans.some((q) => q.name.toLocaleLowerCase() === name.toLocaleLowerCase()); i++) name = `${plan.name} (copia ${i})`;
    editing = { ...$state.snapshot(plan), id: "", name: name.slice(0, 60) };
  }

  async function remove() {
    // Al eliminarla, `plan` deja de existir en el destino: se guardan antes sus datos.
    const { id: planId, name: planName } = plan;
    // Si el agente copia este destino por planes, hay que actualizarlo también.
    const inAgent = !!info?.repos.some((r) => r.id === repo.id && r.schedule.kind === "plans");
    const elevated = !!info?.elevated;
    const agentNote = !(inAgent && plan.schedule)
      ? ""
      : elevated
        ? " y el agente dejará de hacerla según su horario"
        : ". El agente la seguirá haciendo según su horario hasta que apliques el cambio en «Copias automáticas» como administrador";
    let agentError = "";
    const done = await withPassword({
      title: `Eliminar «${plan.name}»`,
      message: `Se eliminará la copia «${plan.name}»${agentNote}. Las versiones que ya guardó se quedan en «${repo.name}» y puedes seguir restaurándolas desde ese repositorio.`,
      repoName: repo.name,
      confirmLabel: "Eliminar copia",
      danger: true,
      action: async (password) => {
        onchange(await api.setPlans(repo.id, $state.snapshot(repo.plans.filter((q) => q.id !== planId)) as Plan[], password));
        if (inAgent && elevated) {
          try {
            try {
              agent.info = await api.agentSetSchedule(repo.id, { kind: "plans" }, password);
            } catch (e) {
              // Sin copias con horario (ni verificación ni copia externa): se quita del agente.
              if (!String(e).includes("Ningún plan tiene horario")) throw e;
              agent.info = await api.agentSetSchedule(repo.id, null, password);
            }
          } catch (e) {
            // La copia ya se eliminó: no se repite, solo se avisa.
            agentError = String(e);
          }
        }
      },
    });
    if (done) {
      toast(`Copia «${planName}» eliminada`);
      if (agentError) toast(`No se pudo actualizar el agente: ${agentError}`, "error", 8000);
      ondeleted();
    }
  }

  /** Copia al agente las copias con horario de este destino (requiere administrador). */
  async function applyToAgent() {
    autoError = "";
    if (!info?.elevated) return elevate();
    // Si ya ninguna copia del destino tiene horario, se desactivan sus copias automáticas.
    const schedule = scheduledInRepo.length ? ({ kind: "plans" } as const) : null;
    const list = scheduledInRepo.map((p) => `«${p.name}» (${planScheduleLabel(p.schedule!)})`).join(", ");
    const done = await withPassword({
      title: schedule ? "Programar las copias" : "Quitar las copias automáticas",
      message: schedule
        ? `${status.auto === "monitor" ? `«${repo.name}» dejará de estar en «Solo vigilar». ` : ""}Resguardo hará solas las copias de «${repo.name}» con horario: ${list}, aunque la app esté cerrada. La contraseña del repositorio se guarda cifrada para el agente de este equipo.`
        : `Ninguna copia de «${repo.name}» tiene ya horario: se desactivarán sus copias automáticas.`,
      repoName: repo.name,
      confirmLabel: schedule ? "Programar" : "Desactivar",
      danger: !schedule,
      action: async (password) => {
        agent.info = await api.agentSetSchedule(repo.id, schedule, password);
      },
    });
    if (done) toast(schedule ? `«${plan.name}» programada` : `Copias automáticas de «${repo.name}» desactivadas`);
  }

  /** Reabre como administrador y, al volver, abre esta copia y la programa. */
  async function elevate() {
    rememberIntent(repo.id, "apply", plan.id);
    try {
      await api.relaunchAsAdmin();
    } catch (e) {
      forgetIntent();
      autoError = String(e);
    }
  }

  // Al volver de reabrir como administrador, se ofrece programar directamente
  // («Solo vigilar» solo si se confirmó sustituirlo en el asistente de copia nueva).
  $effect(() => {
    if (pendingEditor.apply === plan.id && info?.supported) {
      pendingEditor.apply = null;
      if (info.elevated && (status.auto === "stale" || status.auto === "unscheduled" || (status.auto === "monitor" && plan.schedule)))
        applyToAgent();
    }
  });

  /** "hoy 14:00", "mañana 07:00", "dom 4 oct 23:00" o "en los próximos minutos". */
  function nextLabel(d: Date | null) {
    if (!d) return "—";
    if (d.getTime() <= now) return "en los próximos minutos";
    return `${slotDayLabel(d)} ${slotTime(d)}`;
  }

  const LEVEL_TONE: Record<string, string> = {
    running: "accent",
    paused: "muted",
    error: "danger",
    warning: "warn",
    late: "warn",
    ok: "success",
    never: "muted",
    loading: "muted",
  };
</script>

{#if restoring}
  <RestoreWizard
    {repo}
    snapshots={mine.length ? mine : all}
    {all}
    {loading}
    {error}
    onrefresh={load}
    context={mine.length || !all.length ? `Copia «${plan.name}» · se guarda en «${repo.name}»` : `Esta copia aún no tiene versiones: se muestran todas las de «${repo.name}»`}
    onclose={() => (restoring = false)}
  />
{:else if browsing}
  <SnapshotBrowser {repo} snapshot={browsing} snapshots={all} startDir={browseDir} {onchange} onclose={() => ((browsing = null), (browseDir = null))} />
{:else}
  <div class="page" in:fade={{ duration: dur(160) }}>
    <header class="page-head">
      <div class="page-icon"><FolderSync size={22} strokeWidth={1.9} /></div>
      <div class="page-head-text">
        <div class="page-title-line">
          <h1 class="page-title" title={plan.name}>{plan.name}</h1>
          <span class="badge tone-{LEVEL_TONE[status.level]}">
            {#if status.level === "running"}<span class="spin" style="display:grid"><LoaderCircle size={12} /></span>{/if}
            {status.label}
          </span>
          <InfoTip id="copia-{status.level}" />
        </div>
        <div class="sub">
          <span>Se guarda en</span>
          <button class="dest-chip" onclick={onopendestination} title="Abrir el repositorio «{repo.name}»">
            <kind.icon size={13} />
            <span class="dest-name">{repo.name}</span>
            <span class="faint">· {kind.label}</span>
            <ChevronRight size={13} />
          </button>
        </div>
      </div>
      <div class="page-actions">
        <button
          class="btn btn-primary"
          onclick={start}
          disabled={busy || !!status.agentRunning}
          title={busyOther
            ? `Espera: se está copiando «${run?.planName}» en este repositorio`
            : status.agentRunning
              ? "El agente la está copiando ahora"
              : "Hacer una copia ahora mismo"}
        >
          <Play size={13} fill="currentColor" /> Copiar ahora
        </button>
        <button class="btn" onclick={() => (restoring = true)} title="Recuperar archivos de una versión guardada">
          <RotateCcw size={14} /> Restaurar archivos…
        </button>
        <button class="btn" onclick={() => (editing = $state.snapshot(plan))} title="Cambiar qué se copia y cuándo"><Pencil size={14} /> Editar</button>
        <!-- Acciones secundarias: se mantienen juntas si la ventana es estrecha. -->
        <span class="tools">
        <span class="sep" aria-hidden="true"></span>
        <button
          class="icon-btn"
          title={repo.plans.length >= MAX_PLANS ? `Este repositorio ya tiene el máximo de ${MAX_PLANS} copias` : "Duplicar"}
          aria-label="Duplicar esta copia"
          onclick={duplicate}
          disabled={repo.plans.length >= MAX_PLANS}
        >
          <Copy size={16} />
        </button>
        <button class="icon-btn" title="Cambiar repositorio" aria-label="Cambiar el repositorio de esta copia" onclick={() => (moving = true)} disabled={showRun && busy}>
          <ArrowRightLeft size={16} />
        </button>
        <button class="icon-btn danger" title="Eliminar" aria-label="Eliminar esta copia" onclick={remove} disabled={showRun && busy}>
          <Trash2 size={16} />
        </button>
        </span>
      </div>
    </header>

    <PageSummary {summary} />

    {#if status.paused}<PauseBanner {repo} change={() => (pausing = true)} />{/if}

    <div class="stats">
      <div class="stat">
        <!-- Sin cambios («Solo guardar si hay cambios»): se revisó, pero la última versión es anterior. -->
        <span class="stat-label">{status.unchanged ? "Última revisión" : "Última copia"} <InfoTip id={status.unchanged ? "ultima-revision" : "ultima-copia"} /></span>
        <span class="stat-value" title={status.last ? formatDate(status.last.toISOString()) : ""}>
          {status.last ? formatRelative(status.last.toISOString()) : status.level === "loading" ? "…" : "Nunca"}
        </span>
        {#if status.unchanged}
          {@const sub = `sin cambios (${status.lastSnapshot ? `última versión: ${formatShortWhen(status.lastSnapshot.time)}` : "aún sin versiones"})`}
          <span class="stat-sub" title="No había cambios: no hizo falta guardar una versión nueva · {sub}">{sub}</span>
        {:else if status.lastRun && status.lastRun.result !== "ok" && status.lastRun.finished >= (status.lastSnapshot?.time ?? "")}
          <span class="stat-sub" class:bad={status.lastRun.result === "error"} class:warn={status.lastRun.result === "warning"}>
            {status.lastRun.result === "error" ? "falló" : "con avisos"}
          </span>
        {/if}
      </div>
      <div class="stat">
        <span class="stat-label">Próxima <InfoTip id="proxima" /></span>
        <span class="stat-value">
          {#if status.level === "running"}En curso{:else if status.paused}En pausa{:else if status.auto === "scheduled" || status.auto === "stale"}{nextLabel(status.next)}{:else if plan.schedule}Sin programar{:else}A mano{/if}
        </span>
      </div>
      <div class="stat">
        <span class="stat-label">Versiones guardadas <InfoTip id="versiones" /></span>
        <span class="stat-value">{loading && !all.length ? "…" : mine.length}</span>
      </div>
      <div class="stat">
        <span class="stat-label">Tamaño protegido <InfoTip id="tamano-protegido" /></span>
        <span class="stat-value">{status.lastSnapshot ? formatBytes(status.lastSnapshot.summary?.total_bytes_processed) : "—"}</span>
        <span class="stat-sub">en la última versión</span>
      </div>
    </div>

    {#if showRun}
      <section class="card run-card" transition:slide={{ duration: dur(200) }}>
        <RunProgress {repo} planId={plan.id} />
      </section>
    {/if}

    <div class="cols">
      <section class="card box">
        <h2 class="section-title">Qué se copia</h2>
        <dl class="facts">
          <div>
            <dt><Folder size={14} /> Origen</dt>
            <dd>
              <ul class="paths">
                {#each plan.paths as p}
                  {@const sp = splitPath(p)}
                  <li class="mono selectable" title={p}><span class="faint">{sp.parent}</span>{sp.name}</li>
                {/each}
              </ul>
            </dd>
          </div>
          <div>
            <dt><FilterX size={14} /> Exclusiones</dt>
            <dd>
              {#if plan.excludes.length}
                <ul class="paths">
                  {#each plan.excludes as x}<li class="mono selectable" title={x}>{x}</li>{/each}
                </ul>
              {:else}<span class="faint">Ninguna: se copia todo</span>{/if}
            </dd>
          </div>
          <div>
            <dt><Tag size={14} /> Etiquetas</dt>
            <dd>
              {#if plan.tags.length}
                <span class="tags">{#each plan.tags as t}<span class="tag">{t}</span>{/each}</span>
              {:else}<span class="faint">Ninguna</span>{/if}
            </dd>
          </div>
          <div>
            <dt><CalendarClock size={14} /> Horario</dt>
            <dd>
              {#if plan.schedule}
                <span>{planScheduleSentence(plan.schedule)}</span>
                {#if preview.length}<span class="faint small">Próximas: {slotsPreview(preview)}</span>{/if}
              {:else}
                <span>Solo a mano</span>
              {/if}
              {#if plan.skip_unchanged}<span class="faint small">Solo guarda una versión si hay cambios</span>{/if}
            </dd>
          </div>
        </dl>
      </section>

      <section class="card box">
        <h2 class="section-title">Copia automática</h2>
        {#if status.auto === "loading"}
          <p class="faint">Consultando…</p>
        {:else if status.auto === "unsupported"}
          <p class="state"><span class="sicon"><Info size={16} /></span><span>Las copias automáticas solo están disponibles en Windows. Aquí se copia con «Copiar ahora».</span></p>
        {:else if status.auto === "manual"}
          <p class="state">
            <span class="sicon"><Hand size={16} /></span>
            <span>
              <strong>Solo a mano.</strong>
              <span class="muted">Se copia cuando pulsas «Copiar ahora». Para que se haga sola, pulsa <button class="link" onclick={() => (editing = $state.snapshot(plan))}>Editar</button> y ponle un horario.</span>
            </span>
          </p>
        {:else if status.auto === "monitor"}
          <p class="state">
            <span class="sicon"><Eye size={16} /></span>
            <span>
              <strong>El repositorio está en «Solo vigilar».</strong>
              <span class="muted">Resguardo no hace copias automáticas en «{repo.name}»: solo revisa las que hace otro programa. Cámbialo en
                <button class="link" onclick={onopendestination}>Copias automáticas del repositorio</button>.</span>
            </span>
          </p>
        {:else}
          {#if status.paused}
            <p class="state">
              <span class="sicon"><CirclePause size={16} /></span>
              <span><strong>En pausa.</strong> <span class="muted">Las copias automáticas de «{repo.name}» están en pausa {untilWords(status.paused)}.</span></span>
            </p>
          {:else if status.auto === "scheduled"}
            <p class="state ok">
              <span class="sicon"><CircleCheck size={16} /></span>
              <span><strong>Programada.</strong> <span class="muted">Se hace sola aunque la app esté cerrada.</span></span>
            </p>
          {:else if status.auto === "unscheduled"}
            <div class="notice notice-info">
              <Info size={16} />
              <p>
                Tiene horario, pero todavía no está programada en este equipo.
                <button class="notice-action" onclick={applyToAgent}>
                  {#if info?.elevated}Programar ahora{:else}<ShieldCheck size={12} /> Programar (requiere administrador){/if}
                </button>
              </p>
            </div>
          {:else if status.auto === "stale"}
            <div class="notice notice-warn">
              <TriangleAlert size={16} />
              <p>
                {plan.schedule ? "Hay cambios sin aplicar: el agente sigue usando la versión anterior de esta copia." : "Le quitaste el horario, pero el agente la sigue haciendo con el anterior."}
                <button class="notice-action" onclick={applyToAgent}>
                  {#if info?.elevated}<RefreshCw size={12} /> Aplicar cambios{:else}<ShieldCheck size={12} /> Aplicar (requiere administrador){/if}
                </button>
              </p>
            </div>
          {/if}

          {#if status.agentPlan}
            <div class="fact-boxes">
              <div>
                <span class="faint">Próxima</span>
                {#if status.agentRunning}
                  <strong class="running"><span class="spin" style="display:grid"><LoaderCircle size={14} /></span> en curso</strong>
                {:else if status.paused}
                  <strong class="faint">en pausa</strong>
                {:else}
                  <strong>{nextLabel(status.next)}</strong>
                {/if}
              </div>
              <div>
                <span class="faint">Última automática</span>
                {#if status.lastRun}
                  <RunResult run={status.lastRun} planRef={{ repoId: repo.id, planId: plan.id, name: plan.name }} {repo} />
                {:else}
                  <strong class="faint">todavía ninguna</strong>
                {/if}
              </div>
            </div>
          {/if}

          {#if status.agentRunning}
            <AutoProgress running={status.agentRunning} />
          {/if}
          {#if status.agentPlan && !status.paused}
            <button class="btn btn-ghost btn-sm pause-btn" onclick={() => (pausing = true)} title="Pausa las copias automáticas de «{repo.name}» (todas sus copias), sin perder su configuración">
              <CirclePause size={14} /> Pausar copias automáticas
            </button>
          {/if}
        {/if}
        {#if autoError}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{autoError}</p></div>{/if}
      </section>
    </div>

    <RecentActivity
      title="Últimas ejecuciones"
      {repos}
      select={(e) => e.kind === "backup" && e.repo_id === repo.id && e.plan_id === plan.id}
      limit={5}
      showWhat={false}
      emptyText="Todavía no hay ejecuciones registradas de esta copia. Aparecerán aquí en cuanto se haga una, a mano o automática."
      onviewall={onopenactivity}
    />

    <SnapshotList
      snapshots={showAll ? all : mine}
      checks={showAll ? [] : checks}
      {loading}
      {error}
      title={showAll ? `Todas las versiones de «${repo.name}»` : "Historial de esta copia"}
      emptyHint={all.length
        ? "Esta copia todavía no ha guardado ninguna versión en este repositorio. Pulsa «Copiar ahora» para hacer la primera."
        : "Pulsa «Copiar ahora» para hacer la primera."}
      onrefresh={load}
      onopen={(s) => (browsing = s)}
      onlargest={(s) => (largestOf = s)}
    />
    {#if all.length > mine.length}
      <p class="faint scope">
        {#if showAll}
          Se muestran las {all.length} versiones del repositorio.
          <button class="link" onclick={() => (showAll = false)}>Ver solo las de esta copia</button>
        {:else}
          Se muestran las versiones con las mismas carpetas{plan.tags.length ? " y etiquetas" : ""} que esta copia. «{repo.name}» guarda {all.length - mine.length} más.
          <button class="link" onclick={() => (showAll = true)}>Ver todas</button>
        {/if}
      </p>
    {/if}
  </div>
{/if}

{#if editing}
  <PlanEditor
    {repo}
    plan={editing}
    onclose={() => (editing = null)}
    onsaved={(r, p) => {
      onchange(r);
      if (p.id !== plan.id) onopencopy(r.id, p.id);
    }}
  />
{/if}

{#if pausing}<PauseDialog {repo} onclose={() => (pausing = false)} />{/if}

{#if moving}
  <MoveCopyDialog
    {repo}
    {plan}
    {repos}
    {onadddestination}
    onclose={() => (moving = false)}
    onmoved={(updated, to, p) => {
      moving = false;
      for (const r of updated) onchange(r);
      onopencopy(to.id, p.id);
    }}
  />
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

<style>
  .sub {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    font-size: var(--fs-sm);
    color: var(--text-3);
    white-space: nowrap;
  }
  .dest-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    padding: 0 6px 0 9px;
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 600;
    line-height: 24px;
    color: var(--text-1);
    background: var(--surface-3);
    border: 1px solid transparent;
    border-radius: 999px;
    cursor: pointer;
    transition:
      border-color 0.15s,
      background 0.15s;
  }
  .dest-chip:hover {
    border-color: var(--border-strong);
    background: var(--surface);
  }
  .dest-chip :global(svg),
  .dest-chip .faint {
    flex: none;
  }
  .dest-chip .faint {
    font-weight: 400;
  }
  .dest-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .sep {
    width: 1px;
    height: 22px;
    margin: 0 4px;
    background: var(--border);
  }
  /* Con las acciones debajo del título, las secundarias van a la derecha y sin separador. */
  @media (max-width: 900px) {
    .tools {
      margin-left: auto;
    }
    .sep {
      display: none;
    }
  }
  .icon-btn.danger:hover:not(:disabled) {
    color: var(--bad);
    background: var(--bad-soft);
  }
  .stat-sub.bad {
    color: var(--bad);
    font-weight: 600;
  }
  .stat-sub.warn {
    color: var(--warn);
    font-weight: 600;
  }

  .run-card {
    padding: 18px 22px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
    gap: 12px;
    align-items: start;
  }
  .box {
    padding: 18px 22px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .facts {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin: 0;
  }
  .facts > div {
    display: grid;
    grid-template-columns: 116px minmax(0, 1fr);
    gap: 10px;
    align-items: baseline;
  }
  dt {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--text-3);
  }
  dd {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    margin: 0;
    font-size: var(--fs-sm);
  }
  .small {
    font-size: var(--fs-sm);
  }
  .paths {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .paths li {
    font-size: var(--fs-sm);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .tag {
    padding: 0 8px;
    font-size: var(--fs-xs);
    font-weight: 600;
    line-height: 20px;
    border-radius: 999px;
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .state {
    display: flex;
    gap: 10px;
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .state > span:last-child {
    display: block;
  }
  .state strong {
    display: block;
  }
  /* Enlace dentro de una frase: mismo tamaño que el texto y subrayado. */
  .state .link {
    display: inline;
    font-size: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .sicon {
    display: grid;
    place-items: center;
    flex: none;
    width: 30px;
    height: 30px;
    border-radius: var(--radius);
    color: var(--text-2);
    background: var(--surface-3);
  }
  .state.ok .sicon {
    color: var(--ok);
    background: var(--ok-soft);
  }
  .running {
    display: flex;
    align-items: center;
    gap: 5px;
    color: var(--accent);
  }
  .pause-btn {
    align-self: flex-start;
  }
  .scope {
    margin: -8px 0 0;
    font-size: var(--fs-sm);
    text-align: center;
  }
</style>
