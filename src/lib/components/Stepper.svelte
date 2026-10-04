<script lang="ts">
  // Indicador de pasos de un asistente (docs/diseno.md): número o ✓, nombre
  // del paso y un conector fino. Los pasos ya hechos se pueden pulsar si se
  // da `onstep` y `canGo` lo permite.
  import { Check } from "@lucide/svelte";

  interface Props {
    labels: string[];
    /** Paso actual, empezando en 1. */
    current: number;
    onstep?: (n: number) => void;
    canGo?: (n: number) => boolean;
    compact?: boolean;
  }
  let { labels, current, onstep, canGo = (n) => n < current, compact = false }: Props = $props();
</script>

<ol class="stepper" class:compact aria-label="Pasos">
  {#each labels as label, i}
    {@const n = i + 1}
    <li class:done={n < current} class:current={n === current} aria-current={n === current ? "step" : undefined}>
      <button type="button" onclick={() => onstep?.(n)} disabled={!onstep || !canGo(n)} tabindex={onstep && canGo(n) ? 0 : -1}>
        <span class="n">{#if n < current}<Check size={12} strokeWidth={3} />{:else}{n}{/if}</span>
        <span class="label">{label}</span>
      </button>
    </li>
  {/each}
</ol>

<style>
  .stepper {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    align-items: center;
  }
  li + li::before {
    content: "";
    width: 20px;
    height: 1px;
    margin-right: 6px;
    background: var(--border-strong);
  }
  li.done + li::before {
    background: var(--accent);
  }
  button {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 10px 3px 3px;
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 500;
    color: var(--text-3);
    background: none;
    border: none;
    border-radius: 999px;
    cursor: default;
  }
  button:disabled {
    opacity: 1;
  }
  li.done button:not(:disabled) {
    cursor: pointer;
  }
  li.done button:not(:disabled):hover {
    background: var(--surface-2);
  }
  .n {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    font-size: var(--fs-xs);
    font-variant-numeric: tabular-nums;
    border-radius: 50%;
    box-shadow: inset 0 0 0 1px var(--border-strong);
    background: var(--surface);
  }
  li.current button {
    color: var(--text-1);
  }
  li.current .n {
    color: var(--accent-contrast);
    background: var(--accent);
    box-shadow: none;
  }
  li.done button {
    color: var(--text-2);
  }
  li.done .n {
    color: var(--accent-text);
    background: var(--accent-soft);
    box-shadow: none;
  }
  .compact li + li::before {
    width: 12px;
  }
  .compact .label {
    display: none;
  }
  .compact li.current .label {
    display: inline;
  }
  @media (max-width: 760px) {
    li:not(.current) .label {
      display: none;
    }
  }
</style>
