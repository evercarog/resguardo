<script lang="ts">
  // Botón que lanza un trabajo: mientras dura, muestra que está en ello
  // (icono girando y, si se da, otro texto) y no se puede volver a pulsar.
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import { LoaderCircle } from "@lucide/svelte";
  import { tip as globo } from "$lib/tooltip";

  // `tip`: el globo de siempre (en la consola no se usa `title` nativo).
  let { cargando = false, textoCargando, children, disabled, tip = null, class: clase = "btn", ...resto }: HTMLButtonAttributes & { cargando?: boolean; textoCargando?: string; tip?: string | null; children: Snippet } = $props();
</script>

<button {...resto} class={clase} disabled={disabled || cargando} aria-busy={cargando} use:globo={tip}>
  {#if cargando}<LoaderCircle size={15} class="spin" />{#if textoCargando}{textoCargando}{:else}{@render children()}{/if}{:else}{@render children()}{/if}
</button>
