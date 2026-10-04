<script lang="ts">
  // Una abreviatura o un dato con su explicación en un tooltip (lib/tooltip.ts):
  // subrayado de puntos, se puede enfocar con el teclado y tocar en el móvil.
  import type { Snippet } from "svelte";
  import { tip } from "$lib/tooltip";
  let { texto, children }: { texto: string; children: Snippet } = $props();
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<span class="tip-texto" tabindex="0" use:tip={texto}>{@render children()}</span>
