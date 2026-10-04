<script lang="ts">
  // Panel lateral del detalle: entra por la derecha sobre la página, con las
  // migas de lo que se está viendo (cada una vuelve a ese punto), «Atrás» y
  // cerrar. Escape cierra; el foco queda dentro mientras está abierto y vuelve
  // a donde estaba al cerrarlo.
  import { onMount, tick, type Snippet } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { ArrowLeft, ChevronRight, X } from "@lucide/svelte";
  import { anyModalOpen } from "$ui/componentes/Modal.svelte";
  import { dur } from "$ui/movimiento";
  import "./pulsable.css";

  let {
    titulo,
    migas = [],
    alAtras,
    alCerrar,
    children,
  }: {
    titulo: string;
    /** De lo general a lo concreto; la última es lo que se ve (sin enlace). */
    migas?: { texto: string; ir?: () => void }[];
    alAtras?: () => void;
    alCerrar: () => void;
    children: Snippet;
  } = $props();

  let panel: HTMLElement;
  const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

  onMount(() => {
    const antes = document.activeElement as HTMLElement | null;
    void tick().then(() => {
      if (!panel?.contains(document.activeElement)) panel?.querySelector<HTMLElement>("h2")?.focus();
    });
    return () => {
      if (antes && document.contains(antes)) antes.focus({ preventScroll: true });
    };
  });

  function tecla(e: KeyboardEvent) {
    if (anyModalOpen()) return;
    if (e.key === "Escape") {
      e.preventDefault();
      alCerrar();
    } else if (e.key === "Tab" && panel) {
      const items = Array.from(panel.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((el) => el.offsetParent !== null);
      if (!items.length) return;
      const [primero, ultimo] = [items[0], items.at(-1)!];
      if (e.shiftKey && (document.activeElement === primero || !panel.contains(document.activeElement))) {
        e.preventDefault();
        ultimo.focus();
      } else if (!e.shiftKey && (document.activeElement === ultimo || !panel.contains(document.activeElement))) {
        e.preventDefault();
        primero.focus();
      }
    }
  }
</script>

<svelte:window onkeydown={tecla} />

<div class="fondo" transition:fade={{ duration: dur(150) }} onclick={alCerrar} role="presentation"></div>
<div bind:this={panel} class="cajon" role="dialog" aria-modal="true" aria-labelledby="cajon-titulo" transition:fly={{ x: 40, duration: dur(200) }}>
  <header class="cab">
    <div class="fila">
      {#if alAtras}
        <button class="icon-btn" aria-label="Atrás" onclick={alAtras}><ArrowLeft size={18} /></button>
      {/if}
      <nav class="migas" aria-label="Dónde estás en el detalle">
        <ol>
          {#each migas as m, i (i)}
            <li>
              {#if i > 0}<ChevronRight size={12} aria-hidden="true" />{/if}
              {#if m.ir && i < migas.length - 1}<button class="pulsable" onclick={m.ir}>{m.texto}</button>{:else}<span aria-current={i === migas.length - 1 ? "page" : undefined}>{m.texto}</span>{/if}
            </li>
          {/each}
        </ol>
      </nav>
      <button class="icon-btn cerrar" aria-label="Cerrar el detalle" onclick={alCerrar}><X size={18} /></button>
    </div>
    <h2 id="cajon-titulo" tabindex="-1">{titulo}</h2>
  </header>
  <div class="cuerpo">
    {@render children()}
  </div>
</div>

<style>
  .fondo {
    position: fixed;
    inset: 0;
    z-index: 8;
    background: var(--overlay);
  }
  .cajon {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 9;
    display: flex;
    flex-direction: column;
    width: min(760px, 100vw);
    background: var(--bg);
    border-left: 1px solid var(--border);
    box-shadow: var(--shadow-lg);
  }
  .cab {
    flex: none;
    padding: var(--sp-4) var(--sp-5) var(--sp-3);
    border-bottom: 1px solid var(--border);
  }
  .fila {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }
  .migas {
    flex: 1;
    min-width: 0;
  }
  .migas ol {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px 4px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .migas li {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .migas [aria-current] {
    color: var(--text-2);
  }
  .cerrar {
    margin-left: auto;
  }
  h2 {
    margin: var(--sp-2) 0 0;
    font-size: 18px;
    line-height: 24px;
    font-weight: 650;
    letter-spacing: -0.01em;
    overflow-wrap: anywhere;
  }
  h2:focus {
    outline: none;
  }
  .cuerpo {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: var(--sp-4) var(--sp-5) var(--sp-6);
  }
  @media (max-width: 640px) {
    .cab {
      padding: var(--sp-3) var(--sp-4) var(--sp-2);
    }
    .cuerpo {
      padding: var(--sp-3) var(--sp-4) var(--sp-5);
    }
  }
</style>
