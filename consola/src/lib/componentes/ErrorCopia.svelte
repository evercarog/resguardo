<!--
  El error de una copia (o de una verificación, una derivada…) en palabras (15oo):
  la frase con qué hacer y, plegado, el «Detalle técnico» con el texto original.
  Traduce también lo que mandaban los agentes anteriores (el error del motor de
  copias tal cual: en inglés, en JSON y cortado). La tabla es la del agente
  (lib/errorCopia.ts).
-->
<script lang="ts">
  import { enPalabras } from "$lib/errorCopia";

  let {
    mensaje,
    detalle = null,
  }: {
    /** Lo que dijo el equipo (`mensaje_corto`, `mensaje`). */
    mensaje: string | null | undefined;
    /** v1.8x: el texto técnico completo, que el agente manda aparte. */
    detalle?: string | null;
  } = $props();

  const e = $derived(enPalabras(mensaje, detalle));
</script>

{#if e.frase}
  <span class="error-copia">
    <span>{e.frase}</span>
    {#if e.detalle}
      <details class="tecnico">
        <summary>Detalle técnico</summary>
        <code class="selectable">{e.detalle}</code>
      </details>
    {/if}
  </span>
{/if}

<style>
  .error-copia {
    display: inline-flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    max-width: 100%;
  }
  .tecnico summary {
    cursor: pointer;
    font-size: var(--fs-xs);
    color: var(--text-3);
    width: max-content;
  }
  .tecnico code {
    display: block;
    margin-top: 4px;
    padding: var(--sp-2) var(--sp-3);
    border-radius: 6px;
    background: var(--surface-2);
    font-family: var(--mono);
    font-size: var(--fs-xs);
    color: var(--text-2);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
