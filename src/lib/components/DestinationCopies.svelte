<script lang="ts">
  import { slide } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { CalendarClock, ChevronRight, FolderSync, LoaderCircle, Play, Plus } from "@lucide/svelte";
  import type { Plan, Repo } from "$lib/api";
  import { runs, startBackup } from "$lib/backups.svelte";
  import { copyStatus } from "$lib/copies.svelte";
  import { formatRelative } from "$lib/format";
  import { MAX_PLANS, planContentsLabel, planScheduleSentence } from "$lib/plans";
  import RunProgress from "./RunProgress.svelte";

  // «Copias que guardan aquí»: las copias (planes) de un destino, con su
  // estado y acceso directo a cada una.
  interface Props {
    repo: Repo;
    onopencopy: (planId: string) => void;
    onnewcopy: () => void;
    /** Se llama al terminar bien una copia (para recargar las versiones). */
    ondone: () => void;
    /** Destinos cuya copia externa llega aquí (entonces no necesita copias propias). */
    receivesFrom?: string[];
  }
  let { repo, onopencopy, onnewcopy, ondone, receivesFrom = [] }: Props = $props();

  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 60_000);
    return () => clearInterval(t);
  });

  const plans = $derived(repo.plans ?? []);
  const run = $derived(runs[repo.id]);
  const busy = $derived(run?.running ?? false);
  const rows = $derived(plans.map((plan) => ({ plan, status: copyStatus(repo, plan, now) })));

  async function start(p: Plan) {
    await startBackup(repo.id, repo.name, p.id, p.name);
    if (runs[repo.id]?.result) ondone();
  }
</script>

<section class="card panel">
  <header>
    <div>
      <h2 class="section-title">Copias que guardan aquí</h2>
      <p class="faint">
        {#if plans.length === 0 && receivesFrom.length}
          Recibe la copia externa de {receivesFrom.map((n) => `«${n}»`).join(" y ")}.
        {:else if plans.length === 0}
          Todavía ninguna copia usa este repositorio.
        {:else}
          {plans.length} {plans.length === 1 ? "copia" : "copias"} · {plans.filter((p) => p.schedule).length} con horario
        {/if}
      </p>
    </div>
    {#if plans.length}
      <button class="btn btn-sm" onclick={onnewcopy} disabled={plans.length >= MAX_PLANS} title={plans.length >= MAX_PLANS ? `Máximo ${MAX_PLANS} copias por repositorio` : undefined}>
        <Plus size={14} /> Nueva copia aquí
      </button>
    {/if}
  </header>

  {#if plans.length === 0 && receivesFrom.length}
    <p class="faint receives">
      No necesita copias propias: recibe la copia externa de {receivesFrom.map((n) => `«${n}»`).join(" y ")}.
      <button class="link" onclick={onnewcopy}>Crear una copia propia de todos modos</button>
    </p>
  {:else if plans.length === 0}
    <div class="empty-state">
      <FolderSync size={28} strokeWidth={1.6} />
      <p>
        <strong>Crea una copia para empezar a proteger archivos aquí.</strong>
        <span class="muted">Una copia dice qué carpetas guardar, qué dejar fuera y, si quieres, cuándo hacerlo sola.</span>
      </p>
      <button class="btn btn-primary" onclick={onnewcopy}><Plus size={15} /> Nueva copia aquí</button>
    </div>
  {:else}
    <ul class="copies">
      {#each rows as { plan, status } (plan.id)}
        {@const when = plan.schedule ? planScheduleSentence(plan.schedule) : "Solo a mano"}
        {@const last = status.last
          ? status.unchanged
            ? `revisada ${formatRelative(status.last.toISOString())}, sin cambios`
            : `última ${formatRelative(status.last.toISOString())}`
          : ""}
        <li class="copy" class:active={status.level === "running"} transition:slide={{ duration: dur(150) }}>
          <button class="copy-main" onclick={() => onopencopy(plan.id)} title="Abrir «{plan.name}»">
            <span class="dot lvl-{status.level}" aria-hidden="true">
              {#if status.level === "running"}<span class="spin" style="display:grid"><LoaderCircle size={12} /></span>{/if}
            </span>
            <span class="copy-text">
              <span class="copy-title">
                <strong>{plan.name}</strong>
                <span class="state lvl-{status.level}">{status.label}</span>
              </span>
              <!-- Una sola línea que se recorta con «…»; entera en el tooltip. -->
              <span class="copy-line" title="{planContentsLabel(plan)} · {when}{last ? ` · ${last}` : ''}">
                {planContentsLabel(plan)} ·
                <CalendarClock size={12} />
                {when}
                {#if last}<span class="faint">· {last}</span>{/if}
              </span>
            </span>
            <span class="go"><ChevronRight size={16} /></span>
          </button>
          <button
            class="icon-btn"
            title={busy ? "Espera a que termine la copia en curso" : `Copiar «${plan.name}» ahora`}
            aria-label={`Copiar ${plan.name} ahora`}
            onclick={() => start(plan)}
            disabled={busy || !!status.agentRunning}
          >
            <Play size={14} />
          </button>
        </li>
      {/each}
    </ul>
    {#if run && (run.running || plans.some((p) => p.id === run.planId))}
      <RunProgress {repo} showName />
    {/if}
  {/if}
</section>

<style>
  .panel {
    padding: 20px 22px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  header {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    justify-content: space-between;
    gap: 10px 16px;
  }
  header p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .copies {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .copy {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-right: 8px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    transition: border-color 0.15s;
  }
  .copy:hover {
    border-color: var(--border-strong);
  }
  .copy.active {
    border-color: var(--accent);
  }
  .copy-main {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
    min-width: 0;
    padding: 10px 6px 10px 14px;
    font: inherit;
    text-align: left;
    color: var(--text-1);
    background: none;
    border: none;
    border-radius: var(--radius);
    cursor: pointer;
  }
  .copy-main:hover strong {
    color: var(--accent-text);
  }
  .copy-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }
  .copy-title {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    font-size: var(--fs-body);
  }
  .copy-title strong {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Bloque (no flex) para que el «…» funcione al recortar. */
  .copy-line {
    display: block;
    font-size: var(--fs-sm);
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .copy-line :global(svg) {
    margin: 0 1px;
    vertical-align: -2px;
    color: var(--text-3);
  }
  .state {
    flex: none;
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--lvl);
  }
  .go {
    display: grid;
    color: var(--text-3);
  }
  .dot {
    display: grid;
    place-items: center;
    flex: none;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--lvl);
  }
  .dot.lvl-running {
    width: 16px;
    height: 16px;
    background: none;
    color: var(--accent);
  }
  .dot.lvl-never,
  .dot.lvl-loading {
    background: none;
    border: 2px solid var(--border-strong);
  }
  .lvl-running {
    --lvl: var(--accent);
  }
  .lvl-ok {
    --lvl: var(--ok);
  }
  .lvl-warning,
  .lvl-late {
    --lvl: var(--warn);
  }
  .lvl-error {
    --lvl: var(--bad);
  }
  .lvl-paused,
  .lvl-never,
  .lvl-loading {
    --lvl: var(--text-3);
  }
  .receives {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .receives .link {
    display: inline;
    font-size: inherit;
  }
</style>
