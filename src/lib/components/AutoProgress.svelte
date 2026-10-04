<script lang="ts">
  import { slide } from "svelte/transition";
  import { dur } from "$lib/motion";
  import type { AgentRunning } from "$lib/api";
  import { formatBytes, formatDuration, formatNumber, formatTime } from "$lib/format";

  // Progreso de una copia automática que está haciendo el agente.
  interface Props {
    running: AgentRunning;
    /** Nombre de la copia, si hay que decir cuál es (en el destino). */
    name?: string;
  }
  let { running, name }: Props = $props();

  const percent = $derived(running.percent != null ? Math.min(100, Math.max(0, running.percent * 100)) : null);
</script>

<div class="auto-progress" transition:slide={{ duration: dur(200) }}>
  <div class="top">
    <span>
      <strong>{percent != null ? `${percent.toFixed(0)} %` : "Preparando…"}</strong>
      <span class="faint">· {name ? `copiando «${name}» ` : ""}desde las {formatTime(running.started)}</span>
    </span>
    {#if running.seconds_remaining != null && percent != null}
      <span class="faint">Quedan {formatDuration(running.seconds_remaining)}</span>
    {/if}
  </div>
  <div
    class="track"
    role="progressbar"
    aria-label="Progreso de la copia automática"
    aria-valuenow={percent != null ? Math.round(percent) : undefined}
    aria-valuemin={0}
    aria-valuemax={100}
  >
    <div class="fill" class:indeterminate={percent == null} style:width={percent != null ? `${percent}%` : undefined}></div>
  </div>
  {#if running.total_files}
    <span class="faint facts">
      {formatNumber(running.files_done ?? 0)} de {formatNumber(running.total_files)} archivos ·
      {formatBytes(running.bytes_done)} de {formatBytes(running.total_bytes)}
    </span>
  {/if}
</div>

<style>
  .auto-progress {
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
    font-size: var(--fs-h2);
    font-variant-numeric: tabular-nums;
  }
  .top > .faint,
  .facts {
    font-variant-numeric: tabular-nums;
  }
  .facts {
    font-size: var(--fs-xs);
  }
  .track {
    height: 6px;
    border-radius: 999px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: inherit;
    background: var(--accent);
    transition: width 0.4s ease-out;
  }
  .fill.indeterminate {
    width: 35%;
    animation: ap-slide 1.3s ease-in-out infinite;
  }
  @keyframes ap-slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(290%);
    }
  }
</style>
