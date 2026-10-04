<script lang="ts" generics="T extends { hora: string }">
  // Una serie por gráfica (sin doble eje): barras finas en tinta neutra
  // ancladas a la base, con dos líneas de referencia (la mitad y el máximo)
  // rotuladas a la izquierda y las fechas abajo. Al pasar el ratón (o con
  // las flechas, tras enfocar la gráfica) una guía vertical marca la barra,
  // que pasa al color de acento, y su dato va encima. Con `alElegir`, cada
  // barra es un botón que abre el detalle de su versión (y la elegida se queda
  // marcada); las flechas van de una a otra.
  import { fechaCorta } from "$lib/formato";

  interface Props {
    titulo: string;
    /** De más reciente a más antiguo; se muestran las últimas `cuantas`. */
    datos: T[];
    valor: (x: T) => number | null | undefined;
    formato: (v: number) => string;
    cuantas?: number;
    /** Pulsar una barra (o Intro sobre ella): abre su detalle. */
    alElegir?: (x: T) => void;
    /** La barra elegida (su `clave`), que se queda marcada. */
    elegido?: string | null;
    clave?: (x: T) => string;
  }
  let { titulo, datos, valor, formato, cuantas = 40, alElegir, elegido = null, clave = (x: T) => x.hora }: Props = $props();

  const puntos = $derived(
    datos
      .filter((x) => valor(x) != null)
      .slice(0, cuantas)
      .reverse()
      .map((x) => ({ hora: x.hora, v: valor(x) ?? 0, x, k: clave(x) })),
  );
  const max = $derived(Math.max(1, ...puntos.map((p) => p.v)));
  const media = $derived(puntos.length ? puntos.reduce((a, p) => a + p.v, 0) / puntos.length : 0);
  let sobre = $state<number | null>(null);
  const centro = (i: number) => ((i + 0.5) / puntos.length) * 100;

  /** En los extremos, el dato se ancla al borde para no salirse. */
  const tip = $derived.by(() => {
    if (sobre === null) return null;
    const f = (sobre + 0.5) / puntos.length;
    if (f < 0.25) return { lado: "inicio", left: "0", right: "auto" };
    if (f > 0.75) return { lado: "fin", left: "auto", right: "0" };
    return { lado: "centro", left: `${f * 100}%`, right: "auto" };
  });
  function tecla(e: KeyboardEvent) {
    if (!["ArrowLeft", "ArrowRight", "Home", "End", "Escape"].includes(e.key)) return;
    e.preventDefault();
    if (e.key === "Escape") return void (sobre = null);
    if (e.key === "Home") return void (sobre = 0);
    if (e.key === "End") return void (sobre = puntos.length - 1);
    const i = sobre ?? puntos.length - 1;
    sobre = Math.min(puntos.length - 1, Math.max(0, i + (e.key === "ArrowLeft" ? -1 : 1)));
  }
  const medio = $derived(puntos.length > 4 ? puntos[Math.floor((puntos.length - 1) / 2)] : null);

  // Con `alElegir`: un solo punto de parada para el tabulador (la elegida o la última).
  let plot = $state<HTMLElement>();
  const activa = $derived(Math.max(0, elegido ? puntos.findIndex((p) => p.k === elegido) : puntos.length - 1));
  function teclaBotones(e: KeyboardEvent) {
    const i = Number((e.target as HTMLElement).dataset.i ?? "-1");
    if (i < 0) return;
    const j = e.key === "ArrowRight" ? i + 1 : e.key === "ArrowLeft" ? i - 1 : e.key === "Home" ? 0 : e.key === "End" ? puntos.length - 1 : null;
    if (j == null) return;
    e.preventDefault();
    const k = Math.max(0, Math.min(puntos.length - 1, j));
    sobre = k;
    plot?.querySelector<HTMLElement>(`[data-i="${k}"]`)?.focus();
  }
</script>

{#if puntos.length >= 2}
  <figure>
    <figcaption>
      <span>{titulo}</span>
      <span class="faint">media {formato(media)}</span>
    </figcaption>
    <div class="marco">
      <div class="eje-y faint num" aria-hidden="true">
        <span style:bottom="100%">{formato(max)}</span>
        <span style:bottom="50%">{formato(max / 2)}</span>
        <span style:bottom="0%">0</span>
      </div>
      {#if alElegir}
        <!-- svelte-ignore a11y_no_static_element_interactions, a11y_no_noninteractive_element_interactions -->
        <div
          class="plot pulsable-plot"
          role="group"
          aria-label="{titulo}, últimas {puntos.length} versiones: máximo {formato(max)}, media {formato(media)}. Pulsa una barra para ver su detalle; las flechas van de una a otra."
          bind:this={plot}
          onmouseleave={() => (sobre = null)}
          onkeydown={teclaBotones}
        >
          <span class="rejilla" style:bottom="50%" aria-hidden="true"></span>
          <span class="rejilla" style:bottom="100%" aria-hidden="true"></span>
          {#if sobre !== null}<span class="guia" style:left="{centro(sobre)}%" aria-hidden="true"></span>{/if}
          {#each puntos as p, i (p.k + i)}
            <button
              type="button"
              class="col"
              class:on={sobre === i}
              class:elegida={elegido != null && p.k === elegido}
              data-i={i}
              tabindex={i === activa ? 0 : -1}
              aria-label="{fechaCorta(p.hora)}: {formato(p.v)}. Ver detalle"
              aria-pressed={elegido != null && p.k === elegido}
              onmouseenter={() => (sobre = i)}
              onfocus={() => (sobre = i)}
              onblur={() => (sobre = null)}
              onclick={() => alElegir(p.x)}
            >
              <span class="bar" style:height="{Math.max(2, (p.v / max) * 100)}%" style:--i={i}></span>
            </button>
          {/each}
          {#if sobre !== null && tip}
            <div class="tip {tip.lado}" style:left={tip.left} style:right={tip.right} aria-hidden="true">
              <strong class="num">{formato(puntos[sobre].v)}</strong>
              <span>{fechaCorta(puntos[sobre].hora)} · Ver detalle</span>
            </div>
          {/if}
        </div>
      {:else}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <div
        class="plot"
        role="img"
        tabindex="0"
        aria-label="{titulo}, últimas {puntos.length} versiones: máximo {formato(max)}, media {formato(media)}. Usa las flechas para recorrerlas."
        onmouseleave={() => (sobre = null)}
        onblur={() => (sobre = null)}
        onkeydown={tecla}
      >
        <span class="rejilla" style:bottom="50%" aria-hidden="true"></span>
        <span class="rejilla" style:bottom="100%" aria-hidden="true"></span>
        {#if sobre !== null}<span class="guia" style:left="{centro(sobre)}%" aria-hidden="true"></span>{/if}
        {#each puntos as p, i (p.hora + i)}
          <span class="col" class:on={sobre === i} onmouseenter={() => (sobre = i)} role="presentation">
            <span class="bar" style:height="{Math.max(2, (p.v / max) * 100)}%" style:--i={i}></span>
          </span>
        {/each}
        {#if sobre !== null && tip}
          <div class="tip {tip.lado}" style:left={tip.left} style:right={tip.right} aria-hidden="true">
            <strong class="num">{formato(puntos[sobre].v)}</strong>
            <span>{fechaCorta(puntos[sobre].hora)}</span>
          </div>
        {/if}
      </div>
      {/if}
    </div>
    <!-- Fuera del role="img" (sus hijos no se leen): el dato elegido con las flechas. -->
    <span class="sr-only" aria-live="polite" aria-atomic="true">{sobre !== null && puntos[sobre] ? `${fechaCorta(puntos[sobre].hora)}: ${formato(puntos[sobre].v)}` : ""}</span>
    <div class="eje faint" aria-hidden="true">
      <span>{fechaCorta(puntos[0].hora)}</span>
      {#if medio}<span>{fechaCorta(medio.hora)}</span>{/if}
      <span>{fechaCorta(puntos.at(-1)!.hora)}</span>
    </div>
    <!-- Alternativa en texto: los mismos datos en una tabla. -->
    <details class="datos">
      <summary>Ver los datos</summary>
      <div class="desplazable">
        <table class="tabla">
          <caption class="sr-only">{titulo}</caption>
          <thead><tr><th scope="col">Versión</th><th scope="col" class="der">{titulo}</th></tr></thead>
          <tbody>
            {#each [...puntos].reverse() as p, i (p.hora + i)}
              <tr>
                <td class="num">{#if alElegir}<button class="enlace-dato" onclick={() => alElegir(p.x)}>{fechaCorta(p.hora)}</button>{:else}{fechaCorta(p.hora)}{/if}</td>
                <td class="num der">{formato(p.v)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </details>
  </figure>
{/if}

<style>
  figure {
    margin: 0;
    min-width: 0;
  }
  figcaption {
    display: flex;
    justify-content: space-between;
    gap: var(--sp-2);
    margin-bottom: 10px;
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    font-weight: 500;
    color: var(--text-2);
  }
  figcaption .faint {
    font-weight: 400;
  }
  .marco {
    display: flex;
    gap: 6px;
  }
  /* Rótulos del eje vertical: tinta tenue, alineados con su línea de referencia. */
  .eje-y {
    position: relative;
    flex: none;
    width: 48px;
    height: 96px;
    font-size: 10.5px;
    line-height: 12px;
    text-align: right;
  }
  .eje-y span {
    position: absolute;
    right: 0;
    translate: 0 50%;
    white-space: nowrap;
  }
  .plot {
    position: relative;
    display: flex;
    flex: 1;
    align-items: flex-end;
    gap: 2px;
    min-width: 0;
    height: 96px;
    border-bottom: 1px solid var(--border-strong);
    cursor: crosshair;
  }
  .plot:focus-visible {
    outline-offset: 4px;
  }
  .rejilla {
    position: absolute;
    left: 0;
    right: 0;
    height: 0;
    border-top: 1px dashed var(--border);
    pointer-events: none;
  }
  .guia {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 0;
    border-left: 1px dashed var(--text-3);
    pointer-events: none;
  }
  .col {
    position: relative;
    display: flex;
    flex: 1;
    align-items: flex-end;
    height: 100%;
  }
  .bar {
    width: 100%;
    /* Marcas finas: con pocas versiones, barras y no bloques. */
    max-width: 10px;
    margin: 0 auto;
    border-radius: 2px 2px 0 0;
    /* 75 %: las marcas llegan a 3:1 sobre la tarjeta (WCAG 1.4.11). */
    background: color-mix(in srgb, var(--text-3) 75%, transparent);
    transform-origin: bottom;
    animation: crecer 0.45s cubic-bezier(0.2, 0.8, 0.2, 1) both;
    animation-delay: calc(var(--i) * 10ms);
  }
  .col.on .bar {
    background: var(--accent);
  }
  /* Barras que abren su detalle: botones sin aspecto de botón. */
  .pulsable-plot {
    cursor: pointer;
  }
  button.col {
    padding: 0;
    font: inherit;
    background: none;
    border: none;
    border-radius: 2px;
    cursor: pointer;
  }
  button.col:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .col.elegida .bar {
    background: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 30%, transparent);
  }
  .enlace-dato {
    padding: 0;
    font: inherit;
    color: var(--accent-text);
    background: none;
    border: none;
    cursor: pointer;
    text-decoration: underline dotted;
    text-underline-offset: 3px;
  }
  @keyframes crecer {
    from {
      transform: scaleY(0);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .bar {
      animation: none;
    }
  }
  .tip {
    position: absolute;
    bottom: calc(100% + 6px);
    z-index: 2;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 4px 8px;
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    white-space: nowrap;
    color: var(--bg);
    background: var(--text-1);
    border-radius: var(--radius-sm);
    pointer-events: none;
  }
  .tip.centro {
    translate: -50% 0;
  }
  .tip.inicio {
    align-items: flex-start;
  }
  .tip.fin {
    align-items: flex-end;
  }
  .tip span {
    opacity: 0.75;
  }
  .datos {
    margin-top: 8px;
    font-size: var(--fs-xs);
  }
  .datos summary {
    display: inline-flex;
    align-items: center;
    min-height: 24px;
    color: var(--text-2);
    cursor: pointer;
  }
  .datos .desplazable {
    max-height: 240px;
    overflow: auto;
  }
  .datos .der {
    text-align: right;
  }
  .eje {
    display: flex;
    justify-content: space-between;
    margin: 6px 0 0 54px;
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
  }
</style>
