<script lang="ts">
  // Filtrar por etiqueta (Equipos, Avisos, Estado). El filtro se recuerda al
  // pasar de una pantalla a otra del mismo cliente. Sin etiquetas, no se ve.
  import { Tag } from "@lucide/svelte";
  import { actual } from "$lib/estado.svelte";
  import { etiquetasDe, filtroEtiqueta } from "$lib/etiquetas.svelte";
  import EtiquetaChip from "./EtiquetaChip.svelte";

  const todas = $derived(etiquetasDe(actual.equipos));
  // Una etiqueta que ya nadie lleva deja de filtrar.
  $effect(() => {
    const v = filtroEtiqueta.valor;
    if (v && actual.cargado && !todas.some((t) => t.nombre.toLowerCase() === v.toLowerCase())) filtroEtiqueta.poner("");
  });
</script>

{#if todas.length}
  <div class="filtro" role="group" aria-label="Filtrar por etiqueta">
    <span class="faint lbl"><Tag size={13} />Etiqueta</span>
    <EtiquetaChip nombre="Todas" activa={!filtroEtiqueta.valor} onclick={() => filtroEtiqueta.poner("")} />
    {#each todas as t (t.nombre)}
      <EtiquetaChip nombre={t.nombre} n={t.n} activa={filtroEtiqueta.valor.toLowerCase() === t.nombre.toLowerCase()} onclick={() => filtroEtiqueta.poner(filtroEtiqueta.valor.toLowerCase() === t.nombre.toLowerCase() ? "" : t.nombre)} />
    {/each}
  </div>
{/if}

<style>
  .filtro {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .lbl {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-right: 2px;
    font-size: var(--fs-sm);
  }
  /* «Todas» sin punto de color. */
  .filtro > :global(button:first-of-type .punto) {
    display: none;
  }
</style>
