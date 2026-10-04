<script lang="ts">
  // Una cifra de la fila de cuatro (docs/diseno.md §4 «Cifras»): etiqueta con
  // icono de 14 en --text-3, el número grande y una línea. Va dentro de un
  // <div class="cifras" role="list">. `de`: el «de 4» pequeño tras el número;
  // `mal`: la línea en rojo (vueltas fallidas…); `extra`: algo entre el número
  // y la línea (la barra del reparto de Estado).
  import type { Component, Snippet } from "svelte";
  import { tip } from "$lib/tooltip";

  let {
    icono: Icono,
    etiqueta,
    valor,
    de,
    sub,
    mal = false,
    detalle,
    extra,
  }: {
    icono: Component<{ size?: number }>;
    etiqueta: string;
    valor: string;
    de?: string;
    sub?: string;
    mal?: boolean;
    /** Tooltip del número (p. ej. la fecha exacta). */
    detalle?: string;
    extra?: Snippet;
  } = $props();
</script>

<div class="cifra" role="listitem">
  <span class="c-et"><span class="c-ic" aria-hidden="true"><Icono size={14} /></span>{etiqueta}</span>
  <span class="c-val num" use:tip={detalle}>{valor}{#if de}<small>{` ${de}`}</small>{/if}</span>
  {@render extra?.()}
  {#if sub}<span class="c-sub" class:mal>{sub}</span>{/if}
</div>
