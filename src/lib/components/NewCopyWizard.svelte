<script lang="ts">
  import { CalendarClock, Check, CircleAlert, FolderSync, Info, Plus, ShieldCheck, TriangleAlert, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Plan, Repo } from "$lib/api";
  import { agent, refreshAgent } from "$lib/agent.svelte";
  import { forgetIntent, rememberIntent } from "$lib/intent.svelte";
  import { MAX_PLANS, planScheduleLabel, planScheduleSentence } from "$lib/plans";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { repoKind } from "$lib/repoKind";
  import { toast } from "$lib/toast.svelte";
  import AddRepoDialog from "./AddRepoDialog.svelte";
  import Stepper from "./Stepper.svelte";
  import Modal from "./Modal.svelte";
  import PlanEditor from "./PlanEditor.svelte";

  // Asistente «Nueva copia»: 1) dónde se guarda (un destino, o uno nuevo),
  // 2) qué, cuándo y cómo (el editor de copias) y, si tiene horario,
  // 3) programarla en el agente.
  interface Props {
    repos: Repo[];
    /** Destino ya elegido (desde «Nueva copia en este destino»): se salta el paso 1. */
    initialRepoId?: string | null;
    onclose: () => void;
    /** Se añadió un destino desde el asistente. */
    onrepoadded: (repo: Repo) => void;
    /** Un destino se guardó (con la copia nueva). */
    onchange: (repo: Repo) => void;
    /** Abrir la copia creada. */
    onopencopy: (repoId: string, planId: string) => void;
  }
  let { repos, initialRepoId = null, onclose, onrepoadded, onchange, onopencopy }: Props = $props();

  /** Un destino admite como mucho MAX_PLANS copias. */
  const isFull = (r: Repo) => r.plans.length >= MAX_PLANS;
  // svelte-ignore state_referenced_locally
  const initial = repos.find((r) => r.id === initialRepoId && !isFull(r)) ?? null;
  // svelte-ignore state_referenced_locally
  let step = $state<"dest" | "add" | "plan" | "schedule">(initial ? "plan" : repos.length ? "dest" : "add");
  // svelte-ignore state_referenced_locally
  let repoId = $state<string | null>(initial?.id ?? (repos.length === 1 && !isFull(repos[0]) ? repos[0].id : null));
  let created = $state<{ repo: Repo; plan: Plan } | null>(null);
  let error = $state("");
  /** El destino está en «Solo vigilar»: programar lo sustituye (hay que confirmarlo). */
  let replacesMonitor = $state(false);
  let monitorConfirmed = $state(false);
  /** Tras guardar, mientras se consulta el agente. */
  let checking = $state(false);
  /** Si el agente existe (Windows), puede venir el paso de programar. */
  const steps = $derived(agent.info?.supported ? 3 : 2);
  const STEP_LABELS_ALL = ["Repositorio", "Qué y cuándo", "Programar"];
  const STEP_LABELS = $derived(STEP_LABELS_ALL.slice(0, steps));

  const repo = $derived(repos.find((r) => r.id === repoId) ?? null);
  // «Solo guardar si hay cambios»: activado en las copias nuevas.
  const draft: Plan = { id: "", name: "", paths: [], excludes: [], tags: [], schedule: null, skip_unchanged: true };

  /** Después de guardar: si tiene horario, se ofrece programarla; si no, se abre ya. */
  async function saved(r: Repo, p: Plan) {
    onchange(r);
    created = { repo: r, plan: p };
    if (!p.schedule) return finish();
    // Estado actual del agente: nunca se pasa de «Solo vigilar» a planes sin saberlo.
    checking = true;
    await refreshAgent();
    checking = false;
    const info = agent.info;
    // Sin agente (fuera de Windows): no hay nada que programar aquí.
    if (!info?.supported) return finish();
    replacesMonitor = info.repos.find((x) => x.id === r.id)?.schedule.kind === "monitor";
    monitorConfirmed = false;
    step = "schedule";
  }

  function finish() {
    if (created) onopencopy(created.repo.id, created.plan.id);
    onclose();
  }

  async function schedule() {
    if (!created) return;
    if (replacesMonitor && !monitorConfirmed) return;
    error = "";
    const { repo: r, plan: p } = created;
    if (!agent.info?.elevated) {
      // Al volver como administrador se abre la copia y se pide programarla.
      rememberIntent(r.id, "apply", p.id);
      try {
        await api.relaunchAsAdmin();
      } catch (e) {
        forgetIntent();
        error = String(e);
      }
      return;
    }
    const list = r.plans.filter((x) => x.schedule).map((x) => `«${x.name}» (${planScheduleLabel(x.schedule!)})`).join(", ");
    const done = await withPassword({
      title: "Programar la copia",
      message: `${replacesMonitor ? `«${r.name}» dejará de estar en «Solo vigilar». ` : ""}Resguardo hará solas las copias de «${r.name}» con horario: ${list}, aunque la app esté cerrada. La contraseña del repositorio se guarda cifrada para el agente de este equipo.`,
      repoName: r.name,
      confirmLabel: "Programar",
      action: async (password) => {
        agent.info = await api.agentSetSchedule(r.id, { kind: "plans" }, password);
      },
    });
    if (done) {
      toast(`«${p.name}» programada: ${planScheduleLabel(p.schedule!)}`);
      finish();
    }
  }

  // El estado del agente hace falta para saber si hay que programar.
  refreshAgent();
</script>

{#if step === "dest"}
  <Modal {onclose} labelledby="wiz-title" width={600}>
    <header class="dlg-head">
      <div class="dlg-title">
        <span class="ticon"><FolderSync size={19} /></span>
        <div>
          <h2 id="wiz-title">Nueva copia</h2>
          <p class="faint">¿Dónde se guarda?</p>
        </div>
      </div>
      <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={onclose}><X size={17} /></button>
    </header>
    <div class="wiz-steps"><Stepper labels={STEP_LABELS} current={1} /></div>

    <p class="lead">Elige el repositorio donde se guardarán las versiones de esta copia (está en un destino: un disco, un servidor o la nube).</p>

    <div class="dests" role="radiogroup" aria-label="Repositorio">
      {#each repos as r (r.id)}
        {@const k = repoKind(r.location)}
        {@const full = isFull(r)}
        <button
          class="dest"
          class:on={repoId === r.id}
          role="radio"
          aria-checked={repoId === r.id}
          disabled={full}
          title={full ? `Este repositorio ya tiene el máximo de ${MAX_PLANS} copias: elimina o mueve alguna para añadir otra` : undefined}
          onclick={() => (repoId = r.id)}
          ondblclick={() => !full && ((repoId = r.id), (step = "plan"))}
        >
          <span class="dicon"><k.icon size={18} /></span>
          <span class="dtext">
            <strong title={r.name}>{r.name}</strong>
            <span class="faint">{k.label}</span>
            <span class="faint small">
              {full
                ? `Lleno: ya tiene el máximo de ${MAX_PLANS} copias`
                : r.plans.length
                  ? `${r.plans.length} ${r.plans.length === 1 ? "copia" : "copias"}`
                  : "Sin copias todavía"}
            </span>
          </span>
          {#if repoId === r.id}<span class="check"><Check size={15} /></span>{/if}
        </button>
      {/each}
      <button class="dest add" onclick={() => (step = "add")}>
        <span class="dicon"><Plus size={18} /></span>
        <span class="dtext">
          <strong>Añadir un repositorio nuevo</strong>
          <span class="faint">Un disco, un servidor o la nube</span>
        </span>
      </button>
    </div>

    <footer>
      <button class="btn btn-ghost" onclick={onclose}>Cancelar</button>
      <button class="btn btn-primary" onclick={() => (step = "plan")} disabled={!repo || isFull(repo)} title={!repo ? "Elige un repositorio" : isFull(repo) ? "Ese repositorio ya tiene el máximo de copias" : undefined}>Siguiente</button>
    </footer>
  </Modal>
{:else if step === "add"}
  <AddRepoDialog
    {repos}
    steps={{ labels: STEP_LABELS, current: 1 }}
    onclose={() => (repos.length ? (step = "dest") : onclose())}
    oncreated={(r) => {
      onrepoadded(r);
      repoId = r.id;
      step = "plan";
    }}
  />
{:else if step === "plan" && repo}
  <PlanEditor
    {repo}
    plan={draft}
    steps={initial ? (steps === 3 ? { labels: STEP_LABELS_ALL.slice(1), current: 1 } : undefined) : { labels: STEP_LABELS, current: 2 }}
    onclose={() => !created && !checking && onclose()}
    onback={initial ? undefined : () => (step = "dest")}
    onsaved={saved}
  />
{:else if step === "schedule" && created}
  <Modal onclose={finish} labelledby="wiz-sched-title" width={520}>
    <div class="done">
      <span class="done-icon"><CalendarClock size={22} /></span>
      <div>
        <h2 id="wiz-sched-title">«{created.plan.name}» está lista</h2>
        <p class="faint step">Falta un paso: que se haga sola</p>
      </div>
      <div class="wiz-steps">
        <Stepper labels={initial ? STEP_LABELS_ALL.slice(1) : STEP_LABELS} current={initial ? 2 : 3} />
      </div>
      <p class="muted">
        Tiene horario: <strong>{planScheduleSentence(created.plan.schedule!).toLocaleLowerCase()}</strong>. Para que se haga sola, aunque la app esté
        cerrada, hay que programarla en el agente de este equipo.
      </p>
      {#if replacesMonitor}
        <div class="notice notice-warn">
          <TriangleAlert size={16} />
          <p>
            «{created.repo.name}» está en <strong>«Solo vigilar»</strong>: Resguardo solo revisa las copias que hace otro programa. Programar esta
            copia sustituye «Solo vigilar»: el agente hará las copias de este repositorio según sus horarios.
          </p>
        </div>
        <label class="confirm"><input type="checkbox" bind:checked={monitorConfirmed} /> Entiendo: dejar de solo vigilar y programar esta copia</label>
      {/if}
      {#if !agent.info?.elevated}
        <div class="notice notice-info">
          <Info size={16} />
          <p>Programar crea una tarea del sistema, así que Resguardo se reabrirá como administrador (Windows lo pedirá) y volverás a esta copia.</p>
        </div>
      {/if}
      {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}
    </div>
      <footer>
        <button class="btn btn-ghost" onclick={finish}>Ahora no</button>
        <button class="btn btn-primary" onclick={schedule} disabled={replacesMonitor && !monitorConfirmed} title={replacesMonitor && !monitorConfirmed ? "Confirma antes que dejas de solo vigilar este repositorio" : undefined}>
          {#if agent.info?.elevated}<CalendarClock size={15} /> Programar{:else}<ShieldCheck size={15} /> Programar como administrador{/if}
        </button>
      </footer>
  </Modal>
{/if}

<style>
  .wiz-steps {
    margin: 0 0 var(--sp-5);
  }
  .lead {
    margin: 0 0 14px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .dests {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: 8px;
  }
  .dest {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px;
    font: inherit;
    text-align: left;
    color: var(--text-1);
    background: var(--surface);
    border: 1.5px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    transition:
      border-color 0.15s,
      background 0.15s;
  }
  .dest:hover:not(:disabled) {
    border-color: var(--border-strong);
  }
  .dest.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .dest:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }
  .dest.add {
    border-style: dashed;
    color: var(--text-2);
  }
  .dest.add:hover {
    border-color: var(--accent);
    color: var(--accent-text);
  }
  .dicon {
    display: grid;
    place-items: center;
    flex: none;
    width: 36px;
    height: 36px;
    border-radius: var(--radius);
    color: var(--text-2);
    background: var(--surface-3);
  }
  .dest.on .dicon {
    color: var(--accent-contrast);
    background: var(--accent);
  }
  .dtext {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .dtext strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dtext .faint {
    font-size: var(--fs-xs);
  }
  .small {
    font-size: var(--fs-xs) !important;
  }
  .check {
    display: grid;
    align-self: flex-start;
    color: var(--accent-text);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }
  .done {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .done-icon {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: var(--radius-lg);
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .done .step {
    margin-top: 2px;
    font-size: var(--fs-sm);
  }
  .done p {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.55;
  }
  .confirm {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
    font-weight: 600;
    cursor: pointer;
  }
</style>
