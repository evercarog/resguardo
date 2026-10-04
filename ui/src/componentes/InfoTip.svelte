<script lang="ts" module>
  /** Una entrada del glosario: qué es y qué hacer. */
  export interface EntradaGlosario {
    title: string;
    text: string;
    todo?: string;
  }
</script>

<script lang="ts">
  import { tick } from "svelte";
  import { CircleHelp } from "@lucide/svelte";

  // «¿Qué significa?»: un «?» pequeño junto a un estado, una cifra o una
  // comprobación. Al pulsarlo (o con Intro) abre una explicación corta con
  // qué hacer y un enlace a la ayuda. La app de escritorio y la consola le
  // pasan su propia entrada del glosario.
  interface Props {
    entry: EntradaGlosario | undefined;
    /** Enlace a la ayuda completa (si lo hay). */
    href?: string;
  }
  let { entry, href }: Props = $props();

  let open = $state(false);
  let button = $state<HTMLButtonElement>();
  let pop = $state<HTMLDivElement>();
  let pos = $state({ top: 0, left: 0, above: false });
  const uid = $props.id();
  const popId = `tip-${uid}`;
  const WIDTH = 300;

  async function toggle(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    if (open) return close(false);
    const r = button!.getBoundingClientRect();
    const width = Math.min(WIDTH, window.innerWidth - 24);
    const left = Math.max(12, Math.min(window.innerWidth - width - 12, r.left + r.width / 2 - width / 2));
    const above = r.bottom + 200 > window.innerHeight && r.top > 220;
    pos = { top: above ? r.top - 8 : r.bottom + 8, left, above };
    open = true;
    await tick();
    pop?.focus();
  }

  /** El globo va al final de la página: así ningún contenedor (con transform o recorte) lo descoloca ni lo corta. */
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  function close(focusBack = true) {
    open = false;
    if (focusBack) button?.focus();
  }

  /**
   * El globo está al final de la página: al salir de él con Tab se cierra y el
   * foco vuelve al «?», y desde ahí Tab sigue por la página en su orden.
   */
  function salirConTab(e: KeyboardEvent) {
    if (e.key !== "Tab" || !pop) return;
    const xs = Array.from(pop.querySelectorAll<HTMLElement>("a[href], button"));
    const sale = e.shiftKey ? !xs.length || document.activeElement === pop || document.activeElement === xs[0] : !xs.length || document.activeElement === xs.at(-1);
    if (!sale) return;
    close(false);
    button?.focus();
    // Con Mayús+Tab, el foco se queda en el «?» (lo anterior a lo que se leía).
    if (e.shiftKey) e.preventDefault();
  }

  function onwindow(e: Event) {
    if (!open) return;
    if (e.type === "keydown") {
      if ((e as KeyboardEvent).key === "Escape") {
        e.stopPropagation();
        close();
      }
      return;
    }
    if (e.type === "pointerdown" && (pop?.contains(e.target as Node) || button?.contains(e.target as Node))) return;
    close(false);
  }
</script>

<svelte:window onkeydowncapture={onwindow} onpointerdown={onwindow} onresize={onwindow} />
<svelte:document onscrollcapture={(e) => open && !pop?.contains(e.target as Node) && close(false)} />

{#if entry}
  <button
    bind:this={button}
    type="button"
    class="tip"
    class:on={open}
    aria-label="¿Qué significa «{entry.title}»?"
    title="¿Qué significa?"
    aria-expanded={open}
    aria-controls={open ? popId : undefined}
    onclick={toggle}
  >
    <CircleHelp size={13} />
  </button>
  {#if open}
    <div
      bind:this={pop}
      use:portal
      id={popId}
      class="pop"
      class:above={pos.above}
      role="dialog"
      aria-label={entry.title}
      tabindex="-1"
      onkeydown={salirConTab}
      style:top="{pos.top}px"
      style:left="{pos.left}px"
      style:width="min({WIDTH}px, calc(100vw - 24px))"
    >
      <strong>{entry.title}</strong>
      <p>{entry.text}</p>
      {#if entry.todo}<p class="todo"><span>Qué hacer:</span> {entry.todo}</p>{/if}
      {#if href}<a class="link" {href} onclick={() => close(false)}>Más en la ayuda</a>{/if}
    </div>
  {/if}
{/if}

<style>
  .tip {
    display: inline-grid;
    place-items: center;
    /* Objetivo de 24 px (WCAG 2.5.8) que ocupa en la línea lo mismo que el icono de 18. */
    width: 24px;
    height: 24px;
    margin: -5px -3px -5px -1px;
    padding: 0;
    vertical-align: middle;
    color: var(--text-3);
    background: none;
    border: none;
    border-radius: 50%;
    cursor: pointer;
    flex: none;
  }
  .tip:hover,
  .tip.on {
    color: var(--accent-text);
  }
  /* El contorno de foco de siempre (el anillo difuso no llega a 3:1). */
  .tip:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 0;
  }
  .pop {
    position: fixed;
    z-index: 40;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 14px;
    font-size: var(--fs-sm);
    font-weight: 400;
    line-height: 1.5;
    text-align: left;
    text-transform: none;
    letter-spacing: normal;
    white-space: normal;
    color: var(--text-1);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg);
  }
  .pop.above {
    transform: translateY(-100%);
  }
  .pop:focus {
    outline: none;
  }
  .pop strong {
    font-weight: 650;
  }
  .pop p {
    margin: 0;
    color: var(--text-2);
  }
  .todo span {
    font-weight: 600;
    color: var(--text-1);
  }
  .link {
    align-self: flex-start;
  }
</style>
