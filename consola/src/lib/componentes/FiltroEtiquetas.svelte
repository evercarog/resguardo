<script lang="ts">
  // Filtrar por etiqueta (Equipos, Avisos, Estado). El filtro se recuerda al
  // pasar de una pantalla a otra del mismo cliente. Sin etiquetas, no se ve.
  // v1.52: «Ajustar» (administradores) abre sus colores, plantillas y avisos.
  import { Settings2, Tag } from "@lucide/svelte";
  import { actual, puede } from "$lib/estado.svelte";
  import { etiquetasDe, filtroEtiqueta } from "$lib/etiquetas.svelte";
  import { tip } from "$lib/tooltip";
  import EtiquetaChip from "./EtiquetaChip.svelte";
  import GestionarEtiquetas from "./GestionarEtiquetas.svelte";

  let ajustar = $state(false);

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
    {#if puede.administrar(actual.cliente?.rol)}
      <button type="button" class="btn btn-sm btn-ghost ajustar" onclick={() => (ajustar = true)} use:tip={"Color, plantilla para los equipos nuevos y avisos de cada etiqueta"}><Settings2 size={14} />Ajustar</button>
    {/if}
  </div>
{/if}

{#if ajustar}<GestionarEtiquetas onclose={() => (ajustar = false)} />{/if}

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
  .ajustar {
    height: 26px;
  }
  /* «Todas» sin punto de color. */
  .filtro > :global(button:first-of-type .punto) {
    display: none;
  }
</style>
