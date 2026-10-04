<script lang="ts">
  // Un bloque de texto para copiar de una consola a otra (datos de un
  // cliente o de un servidor). Se muestra entero y se copia de una vez.
  import { Check, Copy } from "@lucide/svelte";

  let { texto, etiqueta = "Copiar", alto = 9 }: { texto: string; etiqueta?: string; alto?: number } = $props();
  let copiado = $state(false);

  async function copiar() {
    await navigator.clipboard.writeText(texto);
    copiado = true;
    setTimeout(() => (copiado = false), 2000);
  }
</script>

<div class="bloque">
  <pre class="selectable" style="max-height: {alto * 1.5}em">{texto}</pre>
  <button type="button" class="btn btn-sm" onclick={copiar}>
    {#if copiado}<Check size={14} />Copiado{:else}<Copy size={14} />{etiqueta}{/if}
  </button>
</div>

<style>
  .bloque {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 6px;
    padding: 10px;
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  pre {
    width: 100%;
    margin: 0;
    overflow: auto;
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 11.5px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-all;
  }
</style>
