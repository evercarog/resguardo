<script lang="ts">
  // Cabecera de una página de la consola (docs/diseno.md §4): migas encima,
  // icono, título en --fs-title, una línea de resumen y, a la derecha, la
  // acción principal (un único primario) y las secundarias. Así todas las
  // secciones del cliente empiezan igual.
  import type { Component, Snippet } from "svelte";
  import Migas from "./Migas.svelte";

  let {
    titulo,
    resumen,
    icono: Icono,
    migas,
    acciones,
    detalle,
    junto,
  }: {
    titulo: string;
    /** La línea de debajo del título (texto)… */
    resumen?: string;
    icono?: Component<{ size?: number }>;
    migas?: { texto: string; href?: string }[];
    /** Botones de la derecha. */
    acciones?: Snippet;
    /** …o lo que haga falta debajo del título (con enlaces, ayuda…). */
    detalle?: Snippet;
    /** 0.8.1 (15t): algo pequeño junto al título (p. ej. el lápiz para cambiar el nombre). */
    junto?: Snippet;
  } = $props();
</script>

{#if migas?.length}<Migas items={migas} />{/if}
<header class="cabecera-pagina">
  {#if Icono}<span class="page-icon" aria-hidden="true"><Icono size={22} /></span>{/if}
  <div class="cp-texto">
    {#if junto}
      <div class="cp-titulo"><h1 class="page-title">{titulo}</h1>{@render junto()}</div>
    {:else}
      <h1 class="page-title">{titulo}</h1>
    {/if}
    {#if resumen}<p class="cp-resumen">{resumen}</p>{/if}
    {#if detalle}<div class="cp-resumen">{@render detalle()}</div>{/if}
  </div>
  {#if acciones}<div class="page-actions cp-acciones">{@render acciones()}</div>{/if}
</header>

<style>
  .cp-titulo {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-1);
  }
</style>
