<script lang="ts">
  // Anillo de protección: pista neutra y arco del tono global (mal si algo
  // falla, aviso si falta algo, bien si todo está en orden). Grosor 7 sobre 64.
  import type { Proteccion } from "$lib/repo";

  let { proteccion, tamano = 18, cifra = false }: { proteccion: Proteccion; tamano?: number; cifra?: boolean } = $props();
  const R = 28.5;
  const C = 2 * Math.PI * R;
</script>

<span class="anillo tone-{proteccion.tono}" style:width="{tamano}px" style:height="{tamano}px">
  <svg width={tamano} height={tamano} viewBox="0 0 64 64" aria-hidden="true">
    <circle cx="32" cy="32" r={R} class="pista" />
    {#if proteccion.ratio > 0}<circle cx="32" cy="32" r={R} class="arco" stroke-dasharray="{C * proteccion.ratio} {C}" transform="rotate(-90 32 32)" />{/if}
  </svg>
  {#if cifra}<span class="cifra num" aria-hidden="true">{proteccion.puntuacion}<small>/{proteccion.total}</small></span>{/if}
</span>

<style>
  .anillo {
    position: relative;
    display: inline-grid;
    flex: none;
    place-items: center;
  }
  svg {
    grid-area: 1 / 1;
  }
  .pista {
    fill: none;
    stroke: var(--surface-3);
    stroke-width: 7;
  }
  .arco {
    fill: none;
    stroke: var(--tone);
    stroke-width: 7;
    stroke-linecap: round;
    transition: stroke-dasharray var(--dur-slow) var(--ease-out);
  }
  .cifra {
    grid-area: 1 / 1;
    font-size: var(--fs-h2);
    line-height: 1;
    font-weight: 600;
  }
  .cifra small {
    margin-left: 1px;
    font-size: 0.6em;
    font-weight: 400;
    color: var(--text-3);
  }
</style>
