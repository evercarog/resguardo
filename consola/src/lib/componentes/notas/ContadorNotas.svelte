<script lang="ts">
  import { untrack } from "svelte";
  // Señal pequeña en listas y tarjetas: el objeto tiene observaciones (con su
  // primera línea en el tooltip) o comentarios (cuántos). Nada si no tiene.
  import { MessageSquare, NotebookPen } from "@lucide/svelte";
  import { actual } from "$lib/estado.svelte";
  import { asegurarIndice, notaDe, notas } from "$lib/notas.svelte";
  import type { TipoNota } from "$lib/tipos";
  import { tip } from "$lib/tooltip";

  let { tipo, objeto }: { tipo: TipoNota; objeto: string } = $props();

  $effect(() => {
    const c = actual.id;
    if (c) untrack(() => void asegurarIndice(c));
  });
  const n = $derived(notas.disponible ? notaDe(tipo, objeto) : undefined);
  const texto = $derived.by(() => {
    if (!n) return "";
    const partes = [];
    if (n.observacion) partes.push(n.titulo ? `Observaciones: ${n.titulo}` : "Tiene observaciones");
    if (n.comentarios) partes.push(n.comentarios === 1 ? "1 comentario" : `${n.comentarios} comentarios`);
    return partes.join(" · ");
  });
</script>

{#if n}
  <span class="contador-notas" role="img" aria-label={texto} use:tip={texto}>
    {#if n.observacion}<NotebookPen size={13} />{/if}
    {#if n.comentarios}<MessageSquare size={13} /><span class="n">{n.comentarios}</span>{/if}
  </span>
{/if}

<style>
  .contador-notas {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 0 5px;
    height: 20px;
    font-size: var(--fs-xs);
    font-variant-numeric: tabular-nums;
    color: var(--text-2);
    background: var(--surface-2);
    border-radius: 999px;
    vertical-align: middle;
    flex: none;
  }
  .n {
    line-height: 1;
  }
</style>
