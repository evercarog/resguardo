<script lang="ts">
  // Filtrar por etiquetas (Avisos). Las mismas que en la barra de filtros de Equipos y
  // Estado (16e): varias a la vez, recordadas por persona. Sin etiquetas, no se ve.
  // v1.52: «Ajustar» (administradores) abre sus colores, plantillas y avisos.
  import { Settings2, Tag } from "@lucide/svelte";
  import { actual, puede } from "$lib/estado.svelte";
  import { etiquetasDe, mismaEtiqueta } from "$lib/etiquetas.svelte";
  import { filtrosEquipos } from "$lib/filtrosEquipos.svelte";
  import { alternar } from "$lib/filtrosEquipos";
  import { tip } from "$lib/tooltip";
  import EtiquetaChip from "./EtiquetaChip.svelte";
  import GestionarEtiquetas from "./GestionarEtiquetas.svelte";

  let ajustar = $state(false);

  const todas = $derived(etiquetasDe(actual.equipos));
  const f = $derived(filtrosEquipos.valor);
</script>

{#if todas.length}
  <div class="filtro" role="group" aria-label="Filtrar por etiqueta">
    <span class="faint lbl"><Tag size={13} />Etiquetas</span>
    <EtiquetaChip nombre="Todas" activa={!f.etiquetas.length} onclick={() => filtrosEquipos.poner({ ...f, etiquetas: [] })} />
    {#each todas as t (t.nombre)}
      <EtiquetaChip nombre={t.nombre} n={t.n} activa={f.etiquetas.some((x) => mismaEtiqueta(x, t.nombre))} onclick={() => filtrosEquipos.poner({ ...f, etiquetas: alternar(f.etiquetas, t.nombre) })} />
    {/each}
    {#if f.etiquetas.length > 1}
      <button type="button" class="btn btn-sm btn-ghost" onclick={() => filtrosEquipos.poner({ ...f, modo: f.modo === "todas" ? "cualquiera" : "todas" })} use:tip={"Cambiar entre «cualquiera de ellas» y «todas a la vez»"}>{f.modo === "todas" ? "Con todas a la vez" : "Con cualquiera de ellas"}</button>
    {/if}
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
  .ajustar,
  .filtro > :global(.btn) {
    height: 26px;
  }
  /* «Todas» sin punto de color. */
  .filtro > :global(button:first-of-type .punto) {
    display: none;
  }
</style>
