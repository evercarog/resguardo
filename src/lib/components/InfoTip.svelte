<script lang="ts">
  import { tick } from "svelte";
  import { CircleHelp } from "@lucide/svelte";
  import { GLOSSARY } from "$lib/glossary";
  import { openHelp } from "$lib/help.svelte";

  // «¿Qué significa?»: un «?» pequeño junto a un estado, una cifra o una
  // comprobación. Al pulsarlo (o con Intro) abre una explicación corta con
  // qué hacer y un enlace a la ayuda. Los textos vienen de glossary.ts, que
  // también genera la sección de la ayuda.
  interface Props {
    /** Clave en GLOSSARY. */
    id: string;
  }
  let { id }: Props = $props();

  const entry = $derived(GLOSSARY[id]);
  let open = $state(false);
  let button = $state<HTMLButtonElement>();
  let pop = $state<HTMLDivElement>();
  let pos = $state({ top: 0, left: 0, above: false });
  const popId = `tip-${Math.random().toString(36).slice(2, 10)}`;
  const WIDTH = 300;

  async function toggle(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    if (open) return close(false);
    const r = button!.getBoundingClientRect();
    const left = Math.max(12, Math.min(window.innerWidth - WIDTH - 12, r.left + r.width / 2 - WIDTH / 2));
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
    aria-controls={popId}
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
      style:top="{pos.top}px"
      style:left="{pos.left}px"
      style:width="{WIDTH}px"
    >
      <strong>{entry.title}</strong>
      <p>{entry.text}</p>
      {#if entry.todo}<p class="todo"><span>Qué hacer:</span> {entry.todo}</p>{/if}
      <button
        type="button"
        class="link"
        onclick={() => {
          close(false);
          openHelp(entry.topic ?? `glosario-${id}`);
        }}>Más en la ayuda</button
      >
    </div>
  {/if}
{/if}

<style>
  .tip {
    display: inline-grid;
    place-items: center;
    width: 18px;
    height: 18px;
    margin: -2px 0 -2px 2px;
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
  .tip:focus-visible {
    outline: none;
    box-shadow: var(--focus);
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
