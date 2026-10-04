<script lang="ts">
  import { slide } from "svelte/transition";
  import { dur } from "$lib/motion";
  import type { RunningTask } from "$lib/api";
  import { formatBytes, formatDate } from "$lib/format";
  import RelTime from "./RelTime.svelte";

  // Progreso de una tarea del agente (subida a la nube o verificación), como
  // el de una copia automática. `compact`: una línea (Estado).
  interface Props {
    task: RunningTask;
    /** Adónde se sube (nombre del destino o del proveedor), para la frase. */
    target?: string;
    compact?: boolean;
  }
  let { task, target, compact = false }: Props = $props();

  const percent = $derived(task.percent != null ? Math.min(100, Math.max(0, task.percent * 100)) : null);

  /** «quedan ~25 min», «quedan ~2 h 10 min», «queda menos de 1 min». */
  function etaLabel(s: number) {
    if (s < 60) return "queda menos de 1 min";
    if (s < 3600) return `quedan ~${Math.round(s / 60)} min`;
    const h = Math.floor(s / 3600);
    const m = Math.round((s % 3600) / 60);
    return `quedan ~${h} h${m ? ` ${m} min` : ""}`;
  }

  /** «Subiendo a «X»: versión 96 de 704 · 14 % · quedan ~25 min». */
  const line = $derived.by(() => {
    const parts: string[] = [];
    if (task.kind === "offsite") {
      const where = target ? `Subiendo a «${target}»` : "Subiendo a la copia externa";
      const n = task.total ? Math.min(task.total, (task.done ?? 0) + 1) : null;
      parts.push(task.total && percent != null && percent < 100 ? `${where}: versión ${n} de ${task.total}` : task.stage || `${where}…`);
    } else {
      parts.push(task.stage || "Verificando…");
    }
    if (percent != null && percent < 100) parts.push(`${percent < 10 ? percent.toFixed(1).replace(".", ",") : Math.floor(percent)} %`);
    if (task.eta_s != null) parts.push(etaLabel(task.eta_s));
    return parts.join(" · ");
  });
</script>

{#if compact}
  <span class="compact">
    <span class="mini-track"><span class="mini-fill" class:indeterminate={percent == null} style:width={percent != null ? `${percent}%` : undefined}></span></span>
    <span class="compact-text" title={line}>{line}</span>
  </span>
{:else}
  <div class="task-progress" transition:slide={{ duration: dur(200) }}>
    <div class="top">
      <strong>{line}</strong>
      <span class="faint">desde <RelTime iso={task.started} /></span>
    </div>
    <div
      class="track"
      role="progressbar"
      aria-label={task.kind === "offsite" ? "Progreso de la subida" : "Progreso de la verificación"}
      aria-valuenow={percent != null ? Math.round(percent) : undefined}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <div class="fill" class:indeterminate={percent == null} style:width={percent != null ? `${percent}%` : undefined}></div>
    </div>
    {#if task.kind === "offsite" && (task.current_snapshot_time || task.bytes_total)}
      <span class="faint facts">
        {#if task.current_snapshot_time}Versión del {formatDate(task.current_snapshot_time)}{/if}
        {#if task.bytes_total}{task.current_snapshot_time ? " · " : ""}~{formatBytes(task.bytes_done ?? 0)} de ~{formatBytes(task.bytes_total)}{/if}
      </span>
    {:else if task.kind === "verify" && task.total}
      <span class="faint facts">{task.done} de {task.total}</span>
    {/if}
  </div>
{/if}

<style>
  .task-progress {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    border-radius: var(--radius);
    background: var(--surface-2);
  }
  .top {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: baseline;
    gap: 2px 12px;
    font-size: var(--fs-sm);
  }
  .top strong {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .facts {
    font-size: var(--fs-xs);
    font-variant-numeric: tabular-nums;
  }
  .track {
    height: 6px;
    border-radius: 999px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .fill,
  .mini-fill {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--accent);
    transition: width 0.4s ease-out;
  }
  .fill.indeterminate,
  .mini-fill.indeterminate {
    width: 35%;
    animation: tp-slide 1.3s ease-in-out infinite;
  }
  @keyframes tp-slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(290%);
    }
  }
  .compact {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    font-size: var(--fs-sm);
  }
  .mini-track {
    display: block;
    height: 4px;
    border-radius: 999px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .compact-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--accent-text);
    font-variant-numeric: tabular-nums;
  }
</style>
