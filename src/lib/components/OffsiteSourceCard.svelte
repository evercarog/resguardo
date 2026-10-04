<script lang="ts">
  import { ArrowRight, CloudCheck, CloudUpload } from "@lucide/svelte";
  import type { AgentRepo, AgentRun, Repo } from "$lib/api";
  import { agent, liveTask, scheduleLabel } from "$lib/agent.svelte";
  import { policySummary } from "$lib/retention";
  import RunResult from "./RunResult.svelte";
  import TaskProgress from "./TaskProgress.svelte";
  import RelTime from "./RelTime.svelte";

  // En el destino de una copia externa (`destino:<id>`): de dónde le llegan las
  // versiones y todo lo automático, que se gestiona en el origen.
  interface Props {
    /** Este destino (el que recibe). */
    repo: Repo;
    /** Destinos del agente que suben aquí. */
    sources: AgentRepo[];
    /** Abrir el origen (su «Mantenimiento»). */
    onopensource: (id: string) => void;
    /** Abrir la pestaña de retención de este destino. */
    onretention: () => void;
  }
  let { repo, sources, onopensource, onretention }: Props = $props();

  const tasks = $derived(agent.info?.tasks ?? null);
  const run = (kind: string, id: string): AgentRun | null => tasks?.runs[`${kind}:${id}`] ?? null;
</script>

{#each sources as src (src.id)}
  {@const o = src.offsite!}
  {@const upload = run("offsite", src.id)}
  {@const check = run("verify_offsite", src.id)}
  {@const task = liveTask(src.id)}
  <section class="card source">
    <header>
      <span class="ic"><CloudUpload size={18} /></span>
      <div>
        <h2 class="section-title">Recibe la copia externa de «{src.name}»</h2>
        <p class="faint">Lo automático (subidas, retención y verificación) lo hace el agente desde «{src.name}». Aquí no hacen falta copias propias.</p>
      </div>
    </header>

    <dl class="facts">
      <div>
        <dt>Subidas</dt>
        <dd>{scheduleLabel(o.schedule)}</dd>
      </div>
      <div>
        <dt>Última subida</dt>
        <dd>
          {#if upload}<RunResult run={upload} alwaysMessage />{:else}<span class="faint">todavía ninguna</span>{/if}
        </dd>
      </div>
      <div>
        <dt>Retención aquí</dt>
        <dd>
          <span>{o.retention ? policySummary(o.retention) : repo.retention ? "La de este repositorio, pero no se aplica (recibe de otros orígenes o la desactivaste)." : "Ninguna: se guardan todas las versiones."}</span>
          <button class="link" onclick={onretention}>Editar la retención de «{repo.name}»</button>
        </dd>
      </div>
      <div>
        <dt><CloudCheck size={13} /> Verificación</dt>
        <dd>
          {#if o.verify}
            <span>
              {scheduleLabel(o.verify.schedule)} ·
              {#if check}<RelTime iso={check.finished} />, {check.result === "ok" ? "sin errores" : check.message}{:else}todavía ninguna{/if}
            </span>
          {:else}
            <span class="faint">Sin programar (se configura en «{src.name}» → Copia externa).</span>
          {/if}
        </dd>
      </div>
    </dl>

    {#if task}
      <TaskProgress {task} target={repo.name} />
    {/if}

    <footer>
      <button class="btn btn-sm" onclick={() => onopensource(src.id)}>Gestionar desde «{src.name}» <ArrowRight size={14} /></button>
    </footer>
  </section>
{/each}

<style>
  .source {
    padding: 18px 22px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    border-color: color-mix(in srgb, var(--accent) 40%, var(--border));
  }
  header {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }
  header p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .ic {
    display: grid;
    place-items: center;
    flex: none;
    width: 36px;
    height: 36px;
    border-radius: var(--radius);
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .facts {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 0;
  }
  .facts > div {
    display: grid;
    grid-template-columns: 130px minmax(0, 1fr);
    gap: 10px;
    align-items: baseline;
  }
  dt {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--text-3);
  }
  dd {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    min-width: 0;
    margin: 0;
    font-size: var(--fs-sm);
  }
  dd .link {
    font-size: var(--fs-sm);
  }
  footer {
    display: flex;
    justify-content: flex-end;
  }
</style>
