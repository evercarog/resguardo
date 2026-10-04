<script lang="ts">
  // Botón que lanza un trabajo: mientras dura, muestra que está en ello
  // (icono girando y, si se da, otro texto) y no se puede volver a pulsar.
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import { LoaderCircle } from "@lucide/svelte";

  let { cargando = false, textoCargando, children, disabled, class: clase = "btn", ...resto }: HTMLButtonAttributes & { cargando?: boolean; textoCargando?: string; children: Snippet } = $props();
</script>

<button {...resto} class={clase} disabled={disabled || cargando} aria-busy={cargando}>
  {#if cargando}<LoaderCircle size={15} class="spin" />{#if textoCargando}{textoCargando}{:else}{@render children()}{/if}{:else}{@render children()}{/if}
</button>
