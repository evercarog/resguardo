<script lang="ts" module>
  /** Lo que falta para poder pulsar el botón: la frase y, si es un campo, su `id`. */
  export interface Falta {
    texto: string;
    campo?: string;
    /** El motivo ya se ve encima (un error del formulario): junto al botón solo para lectores de pantalla. */
    yaVisible?: boolean;
  }
</script>

<script lang="ts">
  // 14aa: el botón principal de un formulario que aún no se puede usar dice qué
  // falta (una línea corta a su lado, enlazada con `aria-describedby`) y, al
  // pulsarlo, lleva al campo que falta y lo marca un momento. No se desactiva
  // con `disabled` (no se podría enfocar ni pulsar para saber qué pasa): va
  // con `aria-disabled` y el mismo aspecto apagado de siempre.
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import { LoaderCircle } from "@lucide/svelte";
  import { marcarFalta } from "$lib/falta";

  let {
    falta = null,
    cargando = false,
    textoCargando,
    children,
    disabled,
    onclick,
    class: clase = "btn btn-primary",
    ...resto
  }: HTMLButtonAttributes & { falta?: Falta | null; cargando?: boolean; textoCargando?: string; children: Snippet } = $props();

  const id = `falta-${Math.random().toString(36).slice(2, 8)}`;
  /** Ya se pulsó con algo pendiente: la línea pasa a color de aviso. */
  let insistido = $state(false);
  $effect(() => {
    if (!falta) insistido = false;
  });

  function pulsar(e: MouseEvent & { currentTarget: EventTarget & HTMLButtonElement }) {
    if (falta) {
      e.preventDefault();
      e.stopPropagation();
      insistido = true;
      if (falta.campo) marcarFalta(falta.campo);
      return;
    }
    onclick?.(e);
  }
</script>

<span class="con-falta">
  {#if falta && !cargando}<span class="falta" class:insistido class:sr-only={falta.yaVisible} {id} role={insistido && !falta.yaVisible ? "status" : undefined}>{falta.texto}</span>{/if}
  <button
    {...resto}
    class={clase}
    class:apagado={!!falta && !cargando}
    disabled={disabled || cargando}
    aria-disabled={falta ? true : undefined}
    aria-describedby={falta ? id : undefined}
    aria-busy={cargando}
    onclick={pulsar}
  >
    {#if cargando}<LoaderCircle size={15} class="spin" />{#if textoCargando}{textoCargando}{:else}{@render children()}{/if}{:else}{@render children()}{/if}
  </button>
</span>

<style>
  .con-falta {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
  }
  .falta {
    font-size: var(--fs-xs);
    color: var(--text-3);
    text-align: right;
  }
  .falta.insistido {
    color: var(--warn);
  }
  .apagado {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .apagado:active {
    transform: none;
  }
</style>
