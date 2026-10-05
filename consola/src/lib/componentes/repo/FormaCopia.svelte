<script lang="ts">
  // La marca de una copia en la línea de tiempo (docs/diseno.md §4, «Colores
  // por copia»): su color y su forma (círculo, cuadrado, rombo; raya en
  // «Otras»), para que el color nunca vaya solo. `quita`: la próxima
  // retención la quitaría (hueca y tenue). Decorativa: el texto va al lado.
  let { hueco, quita = false, tamano = 10 }: { hueco: 0 | 1 | 2 | 3; quita?: boolean; tamano?: number } = $props();
</script>

<span class="forma h{hueco}" class:quita style:--t="{tamano}px" aria-hidden="true"></span>

<style>
  .forma {
    display: inline-block;
    flex: none;
    box-sizing: border-box;
    width: var(--t);
    height: var(--t);
    background: var(--c);
    border: 1.5px solid var(--c);
    border-radius: 999px;
  }
  .h1 {
    border-radius: 2px;
  }
  .h2 {
    border-radius: 2px;
    rotate: 45deg;
    scale: 0.82;
  }
  .h3 {
    height: calc(var(--t) * 0.42);
    margin-block: calc(var(--t) * 0.29);
  }
  .quita {
    background: transparent;
    opacity: 0.75;
  }
  /* Colores de las copias: los de las gráficas en vivo (--onda-1…3, validados
   * para daltonismo en claro y en oscuro); la cuarta en adelante, tinta tenue. */
  .h0 {
    --c: var(--onda-1);
  }
  .h1 {
    --c: var(--onda-2);
  }
  .h2 {
    --c: var(--onda-3);
  }
  .h3 {
    --c: var(--text-3);
  }
</style>
