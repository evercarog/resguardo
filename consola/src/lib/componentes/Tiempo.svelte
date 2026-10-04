<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Tiempo relativo que se actualiza solo, con la fecha exacta en el tooltip.
  import { fechaLarga, relativo } from "$lib/formato";
  import { reloj } from "$lib/estado.svelte";
  let { iso, nada = "nunca" }: { iso: string | null | undefined; nada?: string } = $props();
</script>

{#if iso}<time datetime={iso} use:tip={fechaLarga(iso)}>{relativo(iso, reloj.ahora)}</time>{:else}{nada}{/if}
