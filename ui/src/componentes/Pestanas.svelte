<script lang="ts" module>
  import type { Component } from "svelte";

  /** Una pestaña con su dirección (plan 0.8.1, B1). */
  export interface Pestana {
    /** Adónde lleva (cada pestaña tiene su dirección). */
    href: string;
    texto: string;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    icono?: Component<any>;
    /** Un número al lado (avisos, órdenes en espera…). */
    cuenta?: number;
    /** El tono del número, si es de un estado. */
    tono?: "warn" | "bad";
    /** ¿Es la de ahora? */
    activa: boolean;
  }
</script>

<script lang="ts">
  // Pestañas de una sección (docs/diseno.md §4, «Pestañas»): subrayado de 2 px
  // del acento bajo la activa, sin fondos. Cada pestaña es un enlace con su
  // dirección; el grupo es un `tablist` con un solo punto de parada (la activa)
  // y las flechas (←→, Inicio y Fin) mueven el foco entre ellas; Intro la abre.
  // El panel (`role="tabpanel"`) lo pone quien la usa, con `idPanel`.
  interface Props {
    pestanas: Pestana[];
    /** Nombre del grupo para los lectores de pantalla («Recuperar»). */
    etiqueta: string;
    /** Id del panel que controlan (el contenido de la pestaña activa). */
    idPanel?: string;
    /** Prefijo de los ids de cada pestaña (`<prefijo>-<n>`), para `aria-labelledby` del panel. */
    prefijo?: string;
  }
  let { pestanas, etiqueta, idPanel, prefijo = "pestana" }: Props = $props();

  let lista = $state<HTMLDivElement>();
  const activa = $derived(Math.max(0, pestanas.findIndex((p) => p.activa)));

  function teclas(e: KeyboardEvent) {
    const xs = Array.from(lista?.querySelectorAll<HTMLAnchorElement>('[role="tab"]') ?? []);
    const i = xs.indexOf(document.activeElement as HTMLAnchorElement);
    if (i < 0) return;
    let j = -1;
    if (e.key === "ArrowRight") j = (i + 1) % xs.length;
    else if (e.key === "ArrowLeft") j = (i - 1 + xs.length) % xs.length;
    else if (e.key === "Home") j = 0;
    else if (e.key === "End") j = xs.length - 1;
    else if (e.key === " ") {
      // Espacio también la abre (como un botón de pestaña).
      e.preventDefault();
      xs[i].click();
      return;
    }
    if (j < 0) return;
    e.preventDefault();
    xs[j].focus();
    xs[j].scrollIntoView({ block: "nearest", inline: "nearest" });
  }
</script>

<!-- svelte-ignore a11y_interactive_supports_focus -->
<div class="pestanas" role="tablist" aria-label={etiqueta} bind:this={lista} onkeydown={teclas}>
  {#each pestanas as p, i (p.href)}
    <a
      role="tab"
      id="{prefijo}-{i}"
      href={p.href}
      aria-selected={i === activa}
      aria-controls={i === activa ? idPanel : undefined}
      aria-current={i === activa ? "page" : undefined}
      tabindex={i === activa ? 0 : -1}
      class:on={i === activa}
    >
      {#if p.icono}<p.icono size={15} aria-hidden="true" />{/if}
      <span>{p.texto}</span>
      {#if p.cuenta}<span class="cuenta num" class:warn={p.tono === "warn"} class:bad={p.tono === "bad"}>{p.cuenta}</span>{/if}
    </a>
  {/each}
</div>

<style>
  .pestanas {
    display: flex;
    gap: var(--sp-1);
    max-width: 100%;
    overflow-x: auto;
    scrollbar-width: none;
    border-bottom: 1px solid var(--border);
  }
  .pestanas::-webkit-scrollbar {
    display: none;
  }
  a {
    position: relative;
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 6px;
    min-height: 40px;
    padding: 0 var(--sp-3);
    font-size: var(--fs-body);
    font-weight: 500;
    color: var(--text-2);
    text-decoration: none;
    white-space: nowrap;
    border-radius: var(--radius-sm) var(--radius-sm) 0 0;
    transition: color var(--dur-fast) var(--ease);
  }
  a:hover {
    color: var(--text-1);
  }
  a.on {
    color: var(--text-1);
  }
  /* El subrayado de la activa, encima del borde de la tira. */
  a.on::after {
    content: "";
    position: absolute;
    left: var(--sp-2);
    right: var(--sp-2);
    bottom: -1px;
    height: 2px;
    background: var(--accent);
    border-radius: 2px;
  }
  a:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  a :global(svg) {
    color: var(--text-3);
  }
  a.on :global(svg) {
    color: var(--accent-text);
  }
  .cuenta {
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    font-size: var(--fs-xs);
    font-weight: 600;
    line-height: 18px;
    text-align: center;
    color: var(--text-2);
    background: var(--surface-3);
    border-radius: 999px;
  }
  .cuenta.warn {
    color: var(--warn);
    background: var(--warn-soft);
  }
  .cuenta.bad {
    color: var(--bad);
    background: var(--bad-soft);
  }
  @media (max-width: 640px) {
    a {
      min-height: 44px;
      padding: 0 var(--sp-2);
    }
  }
</style>
