<script lang="ts">
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { CalendarClock, CircleAlert, CirclePause, Info, LoaderCircle, Lock, RefreshCw, ScrollText, ShieldCheck, TriangleAlert } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { AgentPlan, AgentRun, Repo, Schedule } from "$lib/api";
  import { agent, nextRun, offsiteSources, refreshAgent, scheduleLabel } from "$lib/agent.svelte";
  import { formatDate, formatTime } from "$lib/format";
  import { nextPlanSlot, planScheduleLabel, plansDiffer, slotDayLabel, slotTime } from "$lib/plans";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import AgentLogDialog from "./AgentLogDialog.svelte";
  import PauseDialog from "./PauseDialog.svelte";
  import { pauseOf, untilWords } from "$lib/pause.svelte";
  import AutoProgress from "./AutoProgress.svelte";
  import RunResult from "./RunResult.svelte";
  import HelpLink from "./HelpLink.svelte";
  import CollapseToggle from "./CollapseToggle.svelte";
  import { ui, setUi } from "$lib/ui.svelte";
  import { forgetIntent, pendingEditor, rememberIntent } from "$lib/intent.svelte";
  import Advanced from "./Advanced.svelte";
  import { toast } from "$lib/toast.svelte";

  let { repo }: { repo: Repo } = $props();

  let showLog = $state(false);
  /** Diálogo «Pausar copias automáticas» abierto. */
  let pausing = $state(false);
  // "Ahora" avanza para que el final de una pausa se note sin recargar.
  let now = $state(Date.now());

  onMount(() => {
    refreshAgent();
    // Mientras el agente copia se consulta cada 3 s; si alguna copia está por
    // empezar, cada 15 s.
    let ticks = 0;
    const t = setInterval(() => {
      ticks++;
      if (ticks % 10 === 0) now = Date.now();
      if (runningNow || (ticks % 5 === 0 && next && next.getTime() <= Date.now())) refreshAgent();
    }, 3_000);
    return () => clearInterval(t);
  });

  const info = $derived(agent.info);
  const current = $derived(info?.repos.find((r) => r.id === repo.id) ?? null);
  /** Destinos que suben su copia externa a este (entonces no necesita copias propias). */
  const sources = $derived(offsiteSources(repo.id));
  /** Pausa vigente de las copias automáticas de este destino. */
  const paused = $derived(current ? pauseOf(repo.id, now) : null);
  /** Horario de una versión anterior (cada N horas, diaria, semanal): el agente lo convierte en un plan. */
  const legacy = $derived(!!current && ["hours", "daily", "weekly"].includes(current.schedule.kind));
  const legacyRun = $derived(legacy ? (info?.state.runs[repo.id] ?? null) : null);
  const agentPlans = $derived(current?.schedule.kind === "plans" ? (current.plans ?? []) : []);
  /** Planes del repositorio con horario (lo que se programaría ahora). */
  const scheduledPlans = $derived((repo.plans ?? []).filter((p) => p.schedule));

  /** Próxima copia de un plan: la primera hora después de su última copia (o de cuándo se activó). */
  function planNext(p: AgentPlan, run: AgentRun | null) {
    const since = Math.max(new Date(p.enabled_at).getTime(), run ? new Date(run.started).getTime() : 0);
    return nextPlanSlot(p.schedule, new Date(since));
  }

  const rows = $derived(
    agentPlans.map((p) => {
      const run = info?.state.runs[`${repo.id}#${p.id}`] ?? null;
      return { plan: p, run, next: planNext(p, run) };
    }),
  );

  /** La copia más próxima de todas (para saber cuándo volver a consultar). */
  const next = $derived.by(() => {
    if (!current || paused) return null;
    if (legacy) return nextRun(current.schedule, new Date(legacyRun?.started ?? current.enabled_at));
    const times = rows.map((r) => r.next?.getTime()).filter((t): t is number => t != null);
    return times.length ? new Date(Math.min(...times)) : null;
  });

  /** El agente guarda una copia de los planes: avisar si ya no coinciden. */
  const stale = $derived(
    !!current && current.schedule.kind === "plans" && (current.location !== repo.location || plansDiffer(repo.plans ?? [], agentPlans)),
  );

  /** Copia en curso según el agente; si lleva más de 10 min sin actualizarse
   *  (el agente se cortó), se ignora. */
  const liveRunning = $derived.by(() => {
    const r = info?.state.running;
    if (!r) return null;
    const last = new Date(r.updated ?? r.started).getTime();
    return Date.now() - last < 10 * 60_000 ? r : null;
  });
  /** Copia automática de este repositorio en marcha ahora mismo. */
  const runningNow = $derived(liveRunning?.repo_id === repo.id ? liveRunning : null);
  const runningPlan = $derived(runningNow?.plan_id ? (agentPlans.find((p) => p.id === runningNow.plan_id) ?? null) : null);
  /** El agente se despierta cada 5 min: si lleva mucho sin hacerlo, algo va mal. */
  const agentAsleep = $derived.by(() => {
    if (!current || !info || liveRunning) return false; // durante una copia no se despierta
    const tick = info.state.last_tick ? new Date(info.state.last_tick).getTime() : 0;
    return !info.task_installed || Date.now() - Math.max(tick, new Date(current.enabled_at).getTime()) > 20 * 60_000;
  });

  /** "hoy 14:00", "mañana 07:00", "dom 4 oct 23:00" o "en los próximos minutos". */
  function nextLabel(d: Date | null) {
    if (!d) return "—";
    if (d.getTime() <= Date.now()) return "en los próximos minutos";
    return `${slotDayLabel(d)} ${slotTime(d)}`;
  }

  const summary = $derived.by(() => {
    if (!current) return "";
    if (current.schedule.kind === "plans") return `${agentPlans.length} ${agentPlans.length === 1 ? "copia programada" : "copias programadas"}`;
    return scheduleLabel(current.schedule);
  });

  // Editor
  let editing = $state(false);
  let kind = $state<"off" | "plans" | "monitor">("off");
  let every = $state(6);
  let error = $state("");
  /** Plegable solo con copias activas: sin ellas, el cuerpo explica qué hacer. Nunca se oculta un error ni una copia en curso. */
  const collapsible = $derived(!!current);
  const locked = $derived(editing || !!runningNow || !!error);
  const open = $derived(!collapsible || !ui.scheduleCollapsed || locked);

  function startEdit() {
    const s = current?.schedule;
    kind = s?.kind === "monitor" ? "monitor" : "plans";
    if (s?.kind === "hours" || s?.kind === "monitor") every = s.every;
    error = "";
    editing = true;
  }

  // Al volver de reabrir como administrador (o si otra parte de la app lo
  // pide), se abre el editor directamente.
  $effect(() => {
    if (pendingEditor.schedule === repo.id && info?.supported) {
      pendingEditor.schedule = null;
      if (info.elevated) startEdit();
    }
  });

  // Al volver de reabrir como administrador para pausar.
  $effect(() => {
    if (pendingEditor.pause === repo.id && info?.supported) {
      pendingEditor.pause = null;
      if (info.elevated && current) pausing = true;
    }
  });

  const draft = $derived<Schedule | null>(
    kind === "off" ? null : kind === "plans" ? { kind: "plans" } : { kind, every: Math.max(1, Math.floor(every) || 1) },
  );
  const canSave = $derived(kind === "off" ? !!current : kind === "plans" ? scheduledPlans.length > 0 : true);

  const plansList = () => scheduledPlans.map((p) => `«${p.name}» (${planScheduleLabel(p.schedule!)})`).join(", ");

  async function save() {
    error = "";
    const d = draft;
    const done = await withPassword({
      title: d ? "Programar copias automáticas" : "Desactivar copias automáticas",
      message: !d
        ? `Se dejarán de hacer copias automáticas de «${repo.name}» y se borrará la contraseña que tenía el agente.`
        : d.kind === "plans"
          ? `Resguardo hará solas las copias de «${repo.name}» con horario: ${plansList()}, aunque la app esté cerrada. La contraseña del repositorio se guardará cifrada para el agente de este equipo.`
          : `Resguardo vigilará las copias de «${repo.name}» (${scheduleLabel(d)}). La contraseña del repositorio se guardará cifrada para el agente de este equipo.`,
      repoName: repo.name,
      confirmLabel: d ? "Programar" : "Desactivar",
      danger: !d,
      action: async (password) => {
        agent.info = await api.agentSetSchedule(repo.id, d, password);
      },
    });
    if (done) {
      editing = false;
      toast(
        !d
          ? `Copias automáticas de «${repo.name}» desactivadas`
          : d.kind === "plans"
            ? `Copias automáticas de «${repo.name}»: ${scheduledPlans.length === 1 ? "1 copia programada" : `${scheduledPlans.length} copias programadas`}`
            : `«${repo.name}»: ${scheduleLabel(d)}`,
      );
    }
  }

  /** Vuelve a copiar al agente los planes actuales. */
  async function applyChanges() {
    error = "";
    if (!info?.elevated) return elevate();
    const done = await withPassword({
      title: "Aplicar los cambios de las copias",
      message: `El agente hará las copias de «${repo.name}» con horario: ${plansList()}.`,
      repoName: repo.name,
      confirmLabel: "Aplicar cambios",
      action: async (password) => {
        agent.info = await api.agentSetSchedule(repo.id, { kind: "plans" }, password);
      },
    });
    if (done) toast(`Copias de «${repo.name}» actualizadas en el agente`);
  }

  async function repair() {
    error = "";
    try {
      if (info?.elevated) agent.info = await api.agentRepair();
      else await api.relaunchAsAdmin();
    } catch (e) {
      error = String(e);
    }
  }

  /** Reabre como administrador y, al volver, abre el editor de este repositorio. */
  async function elevate() {
    error = "";
    rememberIntent(repo.id, "schedule");
    try {
      await api.relaunchAsAdmin();
    } catch (e) {
      forgetIntent();
      error = String(e);
    }
  }
</script>

<section class="card sched">
  <header>
    <div class="title">
      <span class="card-icon" class:on={!!current}><CalendarClock size={18} /></span>
      <div>
        <h2 class="section-title">Copias automáticas <HelpLink topic="auto-agente" label="las copias automáticas" /></h2>
        <p class="faint">
          {#if !info}
            Consultando…
          {:else if !info.supported}
            Disponible en Windows.
          {:else if current && paused}
            <strong class="paused-text">En pausa</strong> {untilWords(paused)} · {summary}
          {:else if current}
            <strong class="on-text">Activadas</strong> · {summary}
          {:else if sources.length}
            No hacen falta: recibe la copia externa de {sources.map((s) => `«${s.name}»`).join(" y ")}.
          {:else}
            Desactivadas: las copias solo se hacen desde esta ventana.
          {/if}
        </p>
      </div>
    </div>
    {#if info?.supported && !editing}
      <span class="head-actions">
        {#if current && !paused}
          <button class="btn btn-sm" onclick={() => (pausing = true)} title="Dejar de hacer copias automáticas un tiempo, sin perder su configuración">
            <CirclePause size={14} /> Pausar copias automáticas
          </button>
        {/if}
        {#if !current && sources.length}
          <!-- Recibe una copia externa: programar copias propias queda como opción secundaria. -->
          <button class="link" onclick={info.elevated ? startEdit : elevate}>Programar copias propias de todos modos</button>
        {:else if info.elevated}
          <button class="btn btn-sm" onclick={startEdit} title="Hacer solas las copias con horario, o solo vigilar este repositorio">
            {current ? "Cambiar" : "Programar"}
          </button>
        {:else}
          <button class="btn btn-sm" onclick={elevate} title="Resguardo se reabrirá como administrador (Windows lo pedirá) y volverás aquí">
            <ShieldCheck size={14} />
            {current ? "Cambiar (requiere administrador)" : "Programar (requiere administrador)"}
          </button>
        {/if}
      </span>
    {/if}
    {#if collapsible}
      <CollapseToggle {open} label="Copias automáticas" controls="sched-body-{repo.id}" {locked} ontoggle={() => setUi("scheduleCollapsed", open)} />
    {/if}
  </header>

  {#if open}
  <div class="body" id="sched-body-{repo.id}" transition:slide={{ duration: dur(180) }}>

  {#if !current && sources.length && !editing}
    <div class="notice notice-info">
      <Info size={16} />
      <p>
        Este repositorio recibe la copia externa de {sources.map((s) => `«${s.name}»`).join(" y ")}; no necesita copias propias. Su verificación se programa en
        «{sources[0].name}» → Copia externa.
      </p>
    </div>
  {:else if info?.supported && !info.elevated && !editing}
    {#if !current}
      <div class="notice notice-info">
        <Info size={16} />
        <p>Programar copias crea una tarea del sistema, así que hace falta abrir Resguardo como administrador. Windows lo pedirá y volverás a este repositorio.</p>
      </div>
    {/if}
    {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}
  {:else if error && !editing}
    <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>
  {/if}

  {#if current && !editing && current.schedule.kind === "monitor"}
    <p class="faint monitor-note">
      Resguardo no hace copias en este destino: revisa sus versiones cada hora y lo informa a la web si el equipo está vinculado.
    </p>
  {:else if current && !editing && legacy}
    <div class="fact-boxes">
      <div>
        {#if runningNow}
          <span class="faint">Copia automática</span>
          <strong class="res res-running"><span class="spin"><LoaderCircle size={14} /></span> en curso desde {formatTime(runningNow.started)}</strong>
        {:else}
          <span class="faint">Próxima copia</span>
          <strong>{paused ? "en pausa" : next && next.getTime() <= Date.now() ? "en los próximos minutos" : next ? formatDate(next.toISOString()) : "—"}</strong>
        {/if}
      </div>
      <div>
        <span class="faint">Última copia automática</span>
        {#if legacyRun}
          <RunResult run={legacyRun} />
        {:else}
          <strong class="faint">todavía ninguna</strong>
        {/if}
      </div>
    </div>
    <div class="notice notice-info">
      <Info size={16} />
      <p>Programado con una versión anterior de Resguardo. Se convertirá en una copia automáticamente la próxima vez que se ejecute el agente.</p>
    </div>
  {:else if current && !editing}
    <ul class="aplans">
      {#each rows as { plan, run, next: planNextAt } (plan.id)}
        {@const isRunning = runningNow?.plan_id === plan.id}
        <li class="aplan" class:active={isRunning}>
          <div class="ap-name">
            <strong title={plan.name}>{plan.name}</strong>
            <span class="faint">{planScheduleLabel(plan.schedule)}</span>
          </div>
          <div class="ap-col">
            <span class="faint">Próxima</span>
            {#if isRunning}
              <strong class="res res-running"><span class="spin"><LoaderCircle size={14} /></span> en curso</strong>
            {:else if paused}
              <strong class="faint">en pausa</strong>
            {:else}
              <strong>{nextLabel(planNextAt)}</strong>
            {/if}
          </div>
          <div class="ap-col">
            <span class="faint">Última</span>
            {#if run}
              <RunResult {run} planRef={{ repoId: repo.id, planId: plan.id, name: plan.name }} {repo} />
            {:else}
              <strong class="faint">todavía ninguna</strong>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  {#if current && !editing && runningNow}
    <AutoProgress running={runningNow} name={runningPlan?.name} />
  {/if}

  {#if current && !editing && stale}
    <div class="notice notice-warn" transition:slide={{ duration: dur(180) }}>
      <TriangleAlert size={16} />
      {#if scheduledPlans.length}
        <p>
          Las copias cambiaron desde que se programaron: el agente sigue usando las versiones anteriores.
          <button class="notice-action" onclick={applyChanges}>
            {#if info?.elevated}<RefreshCw size={12} /> Aplicar cambios{:else}Abrir como administrador para aplicarlos{/if}
          </button>
        </p>
      {:else}
        <p>Ya ninguna copia tiene horario: el agente sigue usando los anteriores. Ponle horario a alguna copia o desactiva las copias automáticas.</p>
      {/if}
    </div>
  {/if}

  {#if current && !editing && agentAsleep}
    <div class="notice notice-danger" role="alert">
      <CircleAlert size={16} />
      <p>
        {info?.task_installed ? "El agente no se ha ejecutado en los últimos 20 minutos." : "El agente no está activo: falta su tarea programada."}
        <button class="notice-action" onclick={repair}>{info?.elevated ? "Reactivar el agente" : "Abrir como administrador para reactivarlo"}</button>
      </p>
    </div>
  {/if}

  {#if info?.supported && !editing && (current || info.state.last_tick)}
    <button class="log-link" onclick={() => (showLog = true)}><ScrollText size={13} /> Ver registro del agente</button>
  {/if}

  {#if editing && info}
    <div class="editor" transition:slide={{ duration: dur(180) }}>
      <div class="segmented" role="group" aria-label="Modo de las copias automáticas">
        {#each [["off", "Desactivadas"], ["plans", "Según sus horarios"]] as [k, label]}
          <button class:on={kind === k} aria-pressed={kind === k} onclick={() => (kind = k as typeof kind)}>{label}</button>
        {/each}
      </div>
      <!-- «Solo vigilar» es para destinos cuyas copias hace otro programa: casi nadie lo necesita. -->
      <Advanced id="automaticas" hint={kind === "monitor" ? "Solo vigilar: las copias las hace otro programa" : "«Solo vigilar» copias que hace otro programa"} custom={kind === "monitor"}>
        <label class="switch-row">
          <input type="checkbox" class="switch" checked={kind === "monitor"} onchange={(e) => (kind = e.currentTarget.checked ? "monitor" : "plans")} />
          <span>
            <strong>Solo vigilar</strong>
            <span class="faint">Resguardo no copia nada: revisa las copias que hace otro programa y avisa si se retrasan.</span>
          </span>
        </label>
      </Advanced>

      {#if kind === "monitor"}
        <label class="row">Se esperan copias cada <input class="input num" type="number" min="1" max="744" bind:value={every} /> horas</label>
        <p class="faint icon-note">
          <Info size={14} />
          Para repositorios cuyas copias hace otro programa (por ejemplo, un script de restic). Resguardo no copia nada: revisa sus
          copias cada hora, informa a la web y avisa si pasan más horas de las indicadas sin copias.
          <HelpLink topic="auto-solo-vigilar" label="«Solo vigilar»" />
        </p>
      {:else if kind === "plans"}
        {#if scheduledPlans.length}
          <ul class="to-schedule">
            {#each scheduledPlans as p (p.id)}
              <li><strong>{p.name}</strong> <span class="faint">· {planScheduleLabel(p.schedule!)}</span></li>
            {/each}
          </ul>
          {#if scheduledPlans.length < (repo.plans ?? []).length}
            <p class="faint small">Las copias sin horario solo se hacen a mano.</p>
          {/if}
          <p class="faint icon-note">
            <ShieldCheck size={14} />
            Se ejecutan con el Programador de tareas de Windows como sistema, aunque la app esté cerrada y nadie haya iniciado
            sesión, con instantánea de disco para copiar archivos abiertos. Si el equipo está apagado a la hora prevista, la copia se
            hace al encenderlo.
          </p>
        {:else}
          <div class="notice notice-info">
            <Info size={16} />
            <p>Ninguna copia de este repositorio tiene horario: abre una copia, pulsa «Editar» y activa «Hacerla sola, con horario».</p>
          </div>
        {/if}
      {/if}

      {#if !info.elevated}
        <div class="notice notice-info">
          <Info size={16} />
          <p>Programar copias crea una tarea del sistema, así que hace falta abrir Resguardo como administrador (Windows lo pedirá).</p>
        </div>
      {/if}
      {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}

      <footer>
        <button class="btn btn-ghost btn-sm" onclick={() => (editing = false)}>Cancelar</button>
        {#if info.elevated}
          <button class="btn btn-sm {draft ? 'btn-primary' : 'btn-danger'}" onclick={save} disabled={!canSave} title={canSave ? undefined : kind === "plans" ? "Ponle horario al menos a una copia" : "Las copias automáticas ya están desactivadas"}>
            <Lock size={12} /> {draft ? "Guardar" : "Desactivar"}
          </button>
        {:else}
          <button class="btn btn-primary btn-sm" onclick={elevate}><ShieldCheck size={14} /> Abrir como administrador</button>
        {/if}
      </footer>
    </div>
  {/if}
  </div>
  {/if}
</section>

{#if showLog}<AgentLogDialog onclose={() => (showLog = false)} />{/if}
{#if pausing}<PauseDialog {repo} onclose={() => (pausing = false)} />{/if}

<style>
  .sched {
    padding: 20px 22px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px 16px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1 1 260px;
    min-width: 0;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .body:empty {
    display: none;
  }
  header p {
    margin: 1px 0 0;
    font-size: var(--fs-sm);
  }
  .on-text {
    color: var(--accent-text);
  }
  .paused-text {
    color: var(--text-2);
  }
  .head-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .monitor-note {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .res {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .res-running {
    color: var(--accent);
  }
  .res-running .spin {
    display: grid;
  }
  .editor {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .row .input {
    width: auto;
    height: 32px;
  }
  .num {
    width: 76px !important;
  }
  .icon-note :global(svg) {
    color: var(--ok);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  /* Copias programadas */
  .aplans {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .aplan {
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) minmax(0, 0.8fr) minmax(0, 1fr);
    gap: 12px;
    align-items: start;
    padding: 10px 14px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-size: var(--fs-sm);
  }
  .aplan.active {
    border-color: var(--accent);
  }
  .ap-name,
  .ap-col {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .ap-name strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ap-name .faint,
  .ap-col > .faint {
    font-size: var(--fs-xs);
  }
  /* Ventanas estrechas: el nombre arriba y «Próxima» / «Última» debajo. */
  @media (max-width: 900px) {
    .aplan {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      row-gap: 8px;
    }
    .ap-name {
      grid-column: 1 / -1;
    }
  }
  .to-schedule {
    margin: 0;
    padding-left: 18px;
    font-size: var(--fs-sm);
    line-height: 1.6;
  }
  .small {
    margin: -4px 0 0;
    font-size: var(--fs-sm);
  }
  .log-link {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    align-self: flex-start;
    padding: 2px 0;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-3);
    background: none;
    border: 0;
    cursor: pointer;
  }
  .log-link:hover {
    color: var(--accent-text);
  }
</style>
