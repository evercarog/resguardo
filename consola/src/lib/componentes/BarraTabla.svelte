<script lang="ts">
  // 16t: la barra de una lista larga (restauraciones, órdenes, actividad, versiones del
  // agente…): buscar por palabras, el rango de fechas (el mismo globo que «Historial y
  // versiones») y, delante, los filtros propios de cada lista. Con «Quitar filtros» y la
  // cuenta de lo que se ve.
  import type { Snippet } from "svelte";
  import { Search, X } from "@lucide/svelte";
  import ElegirFechas from "./repo/ElegirFechas.svelte";
  import { hayRango, type Rango } from "$lib/tablas";

  let {
    buscar = $bindable(""),
    placeholder = "Buscar",
    rango,
    alElegirRango,
    ahora,
    diasCon,
    cuenta,
    hayMas = false,
    limpiar,
    children,
  }: {
    buscar?: string;
    placeholder?: string;
    /** Sin `alElegirRango`, la lista no tiene fechas que filtrar. */
    rango?: Rango;
    alElegirRango?: (r: Rango) => void;
    ahora: number;
    diasCon?: Set<string>;
    /** «12 de 80 restauraciones». */
    cuenta?: string;
    /** Los demás filtros de la lista tienen algo puesto (para enseñar «Quitar filtros»). */
    hayMas?: boolean;
    limpiar?: () => void;
    children?: Snippet;
  } = $props();

  const conFiltros = $derived(!!buscar.trim() || (rango ? hayRango(rango) : false) || hayMas);
</script>

<div class="barra-tabla" role="search">
  <label class="buscar campo-buscar">
    <Search size={15} />
    <input class="input" type="search" {placeholder} bind:value={buscar} aria-label={placeholder} />
  </label>
  {#if alElegirRango && rango}
    <ElegirFechas desde={rango.desde} hasta={rango.hasta} {ahora} {diasCon} alElegir={(desde, hasta) => alElegirRango({ desde, hasta })} />
  {/if}
  {@render children?.()}
  {#if conFiltros && limpiar}
    <button type="button" class="btn btn-sm btn-ghost" onclick={limpiar}><X size={14} />Quitar filtros</button>
  {/if}
  {#if cuenta}<span class="cuenta faint num" aria-live="polite">{cuenta}</span>{/if}
</div>

<style>
  .barra-tabla {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  .buscar {
    flex: 1 1 220px;
    min-width: min(220px, 100%);
  }
  .barra-tabla > :global(select),
  .barra-tabla > :global(.field-inline) {
    width: auto;
    max-width: 100%;
  }
  .cuenta {
    margin-left: auto;
    font-size: var(--fs-sm);
  }
</style>
