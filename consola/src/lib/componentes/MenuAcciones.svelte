<script lang="ts" module>
  import { Ellipsis } from "@lucide/svelte";
  export interface AccionMenu {
    texto: string;
    onclick: () => void;
    /** En rojo: lo que puede dejar de proteger o borrar. */
    peligro?: boolean;
    disabled?: boolean;
    /** Icono de 14 a la izquierda (el mismo que en el resto de la consola). */
    icono?: typeof Ellipsis;
  }
</script>

<script lang="ts">
  // Menú «Más…» para las acciones secundarias: grupos separados por una
  // línea; las peligrosas, al final y en rojo. Teclado: flechas, Inicio/Fin,
  // Escape cierra y devuelve el foco al botón.
  import { tick } from "svelte";
  import { ChevronDown } from "@lucide/svelte";

  let { grupos, texto = "Más…", etiqueta, primario = false }: { grupos: AccionMenu[][]; texto?: string; etiqueta?: string; primario?: boolean } = $props();

  let abierto = $state(false);
  let raiz = $state<HTMLDivElement>();
  let boton = $state<HTMLButtonElement>();
  const visibles = $derived(grupos.filter((g) => g.length));

  const items = () => Array.from(raiz?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not([disabled])') ?? []);

  async function abrir() {
    abierto = true;
    await tick();
    items()[0]?.focus();
  }
  function cerrar(devolverFoco = true) {
    abierto = false;
    if (devolverFoco) boton?.focus();
  }
  function elegir(a: AccionMenu) {
    cerrar(false);
    a.onclick();
  }
  function onkeydown(e: KeyboardEvent) {
    if (!abierto) return;
    const xs = items();
    const i = xs.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      cerrar();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      xs[(i + 1) % xs.length]?.focus();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      xs[(i - 1 + xs.length) % xs.length]?.focus();
    } else if (e.key === "Home") {
      e.preventDefault();
      xs[0]?.focus();
    } else if (e.key === "End") {
      e.preventDefault();
      xs.at(-1)?.focus();
    } else if (e.key === "Tab") {
      cerrar(false);
    }
  }
  function fuera(e: PointerEvent) {
    if (abierto && raiz && !raiz.contains(e.target as Node)) cerrar(false);
  }
</script>

<svelte:window onpointerdown={fuera} />

{#if visibles.length}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="menu-acciones" bind:this={raiz} {onkeydown}>
    <button
      bind:this={boton}
      type="button"
      class={primario ? "btn btn-primary" : "btn btn-sm btn-ghost"}
      aria-haspopup="menu"
      aria-expanded={abierto}
      aria-label={etiqueta}
      onclick={() => (abierto ? cerrar() : abrir())}>{#if !primario}<Ellipsis size={14} />{/if}{texto}<ChevronDown size={13} /></button
    >
    {#if abierto}
      <div class="lista card" role="menu">
        {#each visibles as g, i (i)}
          {#if i > 0}<div class="sep" role="separator"></div>{/if}
          {#each g as a (a.texto)}
            <button type="button" role="menuitem" class:peligro={a.peligro} disabled={a.disabled} onclick={() => elegir(a)}>{#if a.icono}<a.icono size={14} />{/if}{a.texto}</button>
          {/each}
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  .menu-acciones {
    position: relative;
    display: inline-block;
  }
  .lista {
    position: absolute;
    z-index: 5;
    top: calc(100% + 4px);
    /* Hacia la izquierda desde el botón: no se sale de la tarjeta ni de la página. */
    right: 0;
    min-width: 220px;
    padding: 4px;
    box-shadow: var(--shadow-lg, 0 10px 30px rgb(0 0 0 / 0.18));
  }
  [role="menuitem"] {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 10px;
    font: inherit;
    font-size: var(--fs-sm);
    text-align: left;
    color: var(--text-1);
    background: none;
    border: none;
    border-radius: calc(var(--radius) - 2px);
    cursor: pointer;
  }
  [role="menuitem"]:hover,
  [role="menuitem"]:focus-visible {
    background: var(--surface-2);
  }
  /* Con el teclado, además del fondo, el contorno de foco (el fondo solo no llega a 3:1). */
  [role="menuitem"]:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  [role="menuitem"] :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .peligro :global(svg) {
    color: inherit;
  }
  [role="menuitem"]:disabled {
    color: var(--text-3);
    cursor: default;
  }
  .peligro {
    color: var(--bad, #c62828);
  }
  .sep {
    height: 1px;
    margin: 4px 2px;
    background: var(--border);
  }
</style>
