<script lang="ts">
  // Línea pequeña de tendencia (docs/diseno.md §4, «Minigráfica»): trazo fino
  // en tinta neutra con un área muy suave, el último punto en el acento y, al
  // pasar el ratón o con las flechas, una guía vertical con el dato del día.
  // Sin ejes: el valor de hoy va al lado, en texto.
  let {
    valores,
    etiquetas,
    formato,
    titulo,
    alto = 36,
  }: { valores: number[]; etiquetas: string[]; formato: (v: number) => string; titulo: string; alto?: number } = $props();

  const W = 100;
  const min = $derived(Math.min(...valores));
  const max = $derived(Math.max(...valores));
  // Con una serie plana, la línea va a media altura (no pegada al suelo).
  const y = (v: number) => (max === min ? alto / 2 : 3 + (1 - (v - min) / (max - min)) * (alto - 6));
  const x = (i: number) => (valores.length < 2 ? W / 2 : (i / (valores.length - 1)) * W);
  const linea = $derived(valores.map((v, i) => `${i ? "L" : "M"}${x(i).toFixed(2)},${y(v).toFixed(2)}`).join(""));
  const area = $derived(`${linea}L${W},${alto}L0,${alto}Z`);

  let sobre = $state<number | null>(null);
  let caja = $state<HTMLElement>();
  function mover(e: PointerEvent) {
    if (!caja || valores.length < 2) return;
    const r = caja.getBoundingClientRect();
    const f = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
    sobre = Math.round(f * (valores.length - 1));
  }
  function tecla(e: KeyboardEvent) {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    e.preventDefault();
    const i = sobre ?? valores.length - 1;
    sobre = Math.min(valores.length - 1, Math.max(0, i + (e.key === "ArrowLeft" ? -1 : 1)));
  }
  const resumen = $derived(valores.length ? `${titulo}: de ${formato(valores[0])} a ${formato(valores.at(-1)!)}` : titulo);
  const id = $props.id();
</script>

{#if valores.length >= 2}
  <!-- Se enfoca para recorrer los días con las flechas; el resumen va en aria-label. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="spark"
    bind:this={caja}
    style:height="{alto}px"
    role="img"
    aria-label="{resumen}. Usa las flechas para recorrer los días."
    tabindex="0"
    onpointermove={mover}
    onpointerleave={() => (sobre = null)}
    onblur={() => (sobre = null)}
    onkeydown={tecla}
  >
    <svg viewBox="0 0 {W} {alto}" preserveAspectRatio="none" aria-hidden="true">
      <defs>
        <linearGradient id="spark-{id}" x1="0" x2="0" y1="0" y2="1">
          <stop offset="0" class="area-arriba" />
          <stop offset="1" class="area-abajo" />
        </linearGradient>
      </defs>
      <path class="area" d={area} fill="url(#spark-{id})" />
      <path class="linea" d={linea} />
      {#if sobre !== null}<line class="guia" x1={x(sobre)} x2={x(sobre)} y1="0" y2={alto} />{/if}
    </svg>
    <!-- Los puntos van fuera del SVG estirado para que sigan siendo redondos. -->
    <span class="punto fin" style:left="100%" style:top="{y(valores.at(-1)!)}px"></span>
    {#if sobre !== null}
      <span class="punto" style:left="{(x(sobre) / W) * 100}%" style:top="{y(valores[sobre])}px"></span>
      <span class="graf-tip tip" class:izq={sobre > valores.length / 2} style:left="{(x(sobre) / W) * 100}%">
        <strong class="num">{formato(valores[sobre])}</strong><span>{etiquetas[sobre]}</span>
      </span>
    {/if}
  </div>
  <!-- Fuera del role="img": el dato del día elegido con las flechas. -->
  <span class="sr-only" aria-live="polite" aria-atomic="true">{sobre !== null ? `${etiquetas[sobre]}: ${formato(valores[sobre])}` : ""}</span>
{/if}

<style>
  .spark {
    position: relative;
    width: 100%;
    min-width: 60px;
    border-radius: 2px;
    cursor: crosshair;
    touch-action: pan-y;
  }
  .spark:focus-visible {
    outline-offset: 4px;
  }
  svg {
    display: block;
    width: 100%;
    height: 100%;
    overflow: visible;
  }
  .linea {
    fill: none;
    stroke: var(--graf-linea);
    stroke-width: 1.5;
    stroke-linejoin: round;
    stroke-linecap: round;
    vector-effect: non-scaling-stroke;
    filter: var(--graf-brillo);
  }
  .area-arriba {
    stop-color: var(--graf-area);
    stop-opacity: var(--graf-area-alfa);
  }
  .area-abajo {
    stop-color: var(--graf-area);
    stop-opacity: 0;
  }
  .guia {
    stroke: var(--graf-guia);
    stroke-width: 1;
    stroke-dasharray: 2 2;
    vector-effect: non-scaling-stroke;
  }
  .punto {
    position: absolute;
    width: 6px;
    height: 6px;
    margin: -3px 0 0 -3px;
    border-radius: 999px;
    background: var(--text-1);
    box-shadow: 0 0 0 2px var(--surface);
    pointer-events: none;
  }
  .punto.fin {
    background: var(--accent);
    /* El punto de hoy brilla un poco (más en oscuro: --graf-brillo-px). */
    box-shadow:
      0 0 0 2px var(--surface),
      0 0 calc(var(--graf-brillo-px) * 1px) color-mix(in srgb, var(--accent) 70%, transparent);
  }
  /* El globo es .graf-tip (ui/estilos.css); aquí solo dónde va. */
  .tip {
    bottom: calc(100% + 6px);
    translate: -10% 0;
  }
  .tip.izq {
    translate: -90% 0;
  }
</style>
