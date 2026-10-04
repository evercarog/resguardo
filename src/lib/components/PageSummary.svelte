<script lang="ts">
  import { CircleAlert, CircleCheck, Info, LoaderCircle, ShieldAlert, Sparkles } from "@lucide/svelte";
  import type { Summary } from "$lib/summary.svelte";

  // Resumen en lenguaje natural de arriba de un destino o una copia: cómo
  // está, lo último que pasó y lo más importante que falta. Con `onimprove`,
  // un enlace al asistente «Mejorar la protección».
  interface Props {
    summary: Summary;
    onimprove?: () => void;
  }
  let { summary, onimprove }: Props = $props();
</script>

<div class="summary tone-{summary.tone}" role="status" aria-live="polite">
  <span class="ic" aria-hidden="true">
    {#if summary.tone === "ok"}<CircleCheck size={18} />{:else if summary.tone === "bad"}<ShieldAlert size={18} />{:else if summary.tone === "warn"}<CircleAlert
        size={18}
      />{:else if summary.tone === "muted"}<span class="spin" style="display:grid"><LoaderCircle size={18} /></span>{:else}<Info size={18} />{/if}
  </span>
  <p>
    <strong>{summary.head}</strong>
    {#if summary.facts}<span>{summary.facts}</span>{/if}
    {#if summary.missing}<span class="missing">{summary.missing}</span>{/if}
    {#if onimprove && summary.missing}
      <button class="link improve" onclick={onimprove}><Sparkles size={13} /> Mejorar la protección</button>
    {/if}
  </p>
</div>

<style>
  .summary {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    /* Siempre el mismo alto mínimo: no empuja la página al cargar. */
    min-height: 48px;
    padding: 12px 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
  }
  .ic {
    display: grid;
    flex: none;
    margin-top: 1px;
    color: var(--text-3);
  }
  .tone-ok .ic {
    color: var(--ok);
  }
  .tone-warn .ic {
    color: var(--warn);
  }
  .tone-bad .ic {
    color: var(--bad);
  }
  .tone-info .ic {
    color: var(--info);
  }
  .tone-bad {
    border-color: color-mix(in srgb, var(--bad) 30%, var(--border));
    background: var(--bad-soft);
  }
  p {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.55;
    color: var(--text-2);
  }
  p > * + * {
    margin-left: 4px;
  }
  strong {
    color: var(--text-1);
    font-weight: 600;
  }
  .missing {
    color: var(--text-1);
  }
  .improve {
    margin-left: 8px;
    white-space: nowrap;
  }
</style>
