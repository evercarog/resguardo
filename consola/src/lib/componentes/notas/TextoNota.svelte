<script lang="ts">
  // El texto de una observación o de un comentario con su Markdown ligero
  // (lib/markdown.ts: se escapa todo antes de poner negritas, listas y enlaces).
  import { markdown } from "$lib/markdown";

  let { texto }: { texto: string } = $props();
  const html = $derived(markdown(texto));
</script>

<!-- eslint-disable-next-line svelte/no-at-html-tags -- markdown() escapa el texto entero antes de añadir sus etiquetas -->
<div class="texto-nota">{@html html}</div>

<style>
  .texto-nota {
    min-width: 0;
    overflow-wrap: anywhere;
    line-height: 1.5;
  }
  .texto-nota :global(p),
  .texto-nota :global(ul),
  .texto-nota :global(ol) {
    margin: 0;
  }
  .texto-nota :global(p + p),
  .texto-nota :global(p + ul),
  .texto-nota :global(p + ol),
  .texto-nota :global(ul + p),
  .texto-nota :global(ol + p) {
    margin-top: 6px;
  }
  .texto-nota :global(ul),
  .texto-nota :global(ol) {
    padding-left: 1.3em;
  }
  .texto-nota :global(code) {
    font-family: var(--mono);
    font-size: 0.92em;
    padding: 0 4px;
    background: var(--bg-subtle);
    border-radius: 4px;
  }
  .texto-nota :global(a) {
    color: var(--accent-text);
    text-decoration: underline;
    text-underline-offset: 2px;
  }
</style>
