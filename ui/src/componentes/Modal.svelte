<script lang="ts" module>
  /** Modales abiertos, del más antiguo al más reciente. */
  const stack: symbol[] = [];

  /** true si hay algún modal abierto (para no disparar atajos de teclado debajo). */
  export const anyModalOpen = () => stack.length > 0;
</script>

<script lang="ts">
  import { onMount, tick, type Snippet } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { dur } from "../movimiento";

  interface Props {
    /** Se llama con Escape o al pulsar fuera (si `dismissible`). */
    onclose: () => void;
    labelledby: string;
    width?: number;
    /**
     * false: pulsar fuera no cierra, y Escape pregunta antes («¿Cerrar sin
     * terminar?»): formularios y asistentes que perderían lo escrito, códigos que
     * no se vuelven a enseñar.
     */
    dismissible?: boolean;
    /** Lo que se pierde al cerrar, en la pregunta de Escape (con `dismissible={false}`). */
    sePierde?: string;
    /** Mientras envía: ni Escape ni pulsar fuera cierran (los botones del diálogo mandan). */
    ocupado?: boolean;
    children: Snippet;
  }
  let { onclose, labelledby, width = 520, dismissible = true, sePierde = "Lo que has escrito se pierde.", ocupado = false, children }: Props = $props();

  const id = Symbol("modal");
  /** Un modal abierto encima de otro (p. ej. la ayuda desde un formulario) queda por delante, con su fondo. */
  const layer = 10 + stack.length * 2;
  let dialog: HTMLDivElement;
  /** Escape en un diálogo que no se cierra sin más: la pregunta, encima de todo. */
  let preguntando = $state(false);
  let quedarse = $state<HTMLButtonElement>();

  async function preguntar() {
    preguntando = true;
    await tick();
    quedarse?.focus();
  }
  function seguir() {
    preguntando = false;
    tick().then(() => dialog?.focus());
  }

  const FOCUSABLE =
    'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

  const focusables = () =>
    Array.from(dialog?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? []).filter((el) => el.offsetParent !== null || el === document.activeElement);

  onMount(() => {
    stack.push(id);
    // Se guarda quién tenía el foco para devolvérselo al cerrar.
    const previous = document.activeElement as HTMLElement | null;
    tick().then(() => {
      if (!dialog || dialog.contains(document.activeElement)) return; // el contenido ya enfocó algo (use:autofocus)
      const first =
        dialog.querySelector<HTMLElement>("[autofocus]") ??
        dialog.querySelector<HTMLElement>(
          "input:not([disabled]):not([type='hidden']):not([type='radio']):not([type='checkbox']), textarea:not([disabled]), select:not([disabled])",
        ) ?? dialog;
      first.focus();
    });
    return () => {
      stack.splice(stack.indexOf(id), 1);
      if (previous && document.contains(previous)) previous.focus();
    };
  });

  function onkeydown(e: KeyboardEvent) {
    if (stack.at(-1) !== id) return;
    if (e.key === "Escape") {
      e.stopPropagation();
      if (preguntando) seguir();
      else if (ocupado) return;
      else if (dismissible) onclose();
      else void preguntar();
    } else if (e.key === "Tab") {
      // Trampa de foco: Tab y Mayús+Tab dan la vuelta dentro del diálogo.
      const items = focusables();
      if (!items.length) {
        e.preventDefault();
        dialog.focus();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      if (e.shiftKey && (active === first || !dialog.contains(active))) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && (active === last || !dialog.contains(active))) {
        e.preventDefault();
        first.focus();
      }
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" style:z-index={layer} transition:fade={{ duration: dur(150) }} onclick={() => dismissible && !ocupado && !preguntando && onclose()} role="presentation"></div>

<div
  bind:this={dialog}
  class="dialog card"
  style:width="min({width}px, calc(100vw - 32px))"
  style:z-index={layer + 1}
  role="dialog"
  aria-modal="true"
  aria-labelledby={labelledby}
  tabindex="-1"
  transition:scale={{ duration: dur(180), start: 0.96 }}
>
  <div class="contenido" inert={preguntando}>{@render children()}</div>
  {#if preguntando}
    <div class="pregunta" role="alertdialog" aria-modal="true" aria-labelledby="{labelledby}-cerrar" aria-describedby="{labelledby}-pierde">
      <div class="pregunta-caja">
        <p class="pregunta-titulo" id="{labelledby}-cerrar">¿Cerrar sin terminar?</p>
        <p class="pregunta-texto" id="{labelledby}-pierde">{sePierde}</p>
        <div class="pregunta-botones">
          <button type="button" class="btn btn-ghost" onclick={() => ((preguntando = false), onclose())}>Cerrar</button>
          <button type="button" class="btn btn-primary" bind:this={quedarse} onclick={seguir}>Seguir aquí</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 10;
    background: var(--overlay);
    backdrop-filter: blur(2px);
  }
  .dialog {
    position: fixed;
    z-index: 11;
    top: 50%;
    left: 50%;
    translate: -50% -50%;
    max-height: calc(100vh - 48px);
    max-height: calc(100dvh - 48px);
    overflow: auto;
    padding: var(--sp-6);
    border-radius: var(--radius-xl);
    box-shadow: var(--shadow-lg);
  }
  /* Piezas comunes de los diálogos (docs/diseno.md): título, pie con las
   * acciones a la derecha. Cada diálogo puede añadir lo suyo. */
  .dialog :global(h2) {
    font-size: 17px;
    line-height: 24px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .dialog :global(footer) {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: var(--sp-2);
    margin-top: var(--sp-6);
    padding-top: var(--sp-4);
    border-top: 1px solid var(--border);
  }
  .dialog:focus {
    outline: none;
  }
  .contenido {
    display: contents;
  }
  /* La pregunta de Escape: tapa el diálogo (lo de debajo, inerte) sin moverlo. */
  .pregunta {
    position: sticky;
    bottom: 0;
    display: flex;
    justify-content: center;
    margin: var(--sp-4) calc(-1 * var(--sp-6)) calc(-1 * var(--sp-6));
    padding: var(--sp-4) var(--sp-6) var(--sp-6);
    background: linear-gradient(to bottom, transparent, var(--surface) var(--sp-4));
  }
  .pregunta-caja {
    width: 100%;
    padding: var(--sp-4);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-md);
  }
  .pregunta-titulo {
    margin: 0;
    font-size: var(--fs-body);
    font-weight: 600;
    color: var(--text-1);
  }
  .pregunta-texto {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .pregunta-botones {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--sp-2);
    margin-top: var(--sp-3);
  }
</style>
