<script lang="ts">
  import { copiar as alPortapapeles } from "$lib/portapapeles";
  import { tip } from "$lib/tooltip";
  // Un código, una ruta o una huella con su botón de copiar al lado. Al
  // copiar, el icono pasa a ✓ un momento y se anuncia «Copiado».
  import { Check, Copy } from "@lucide/svelte";

  let { texto, mostrar, que = "el texto", mono = true }: { texto: string; mostrar?: string; que?: string; mono?: boolean } = $props();
  let hecho = $state(false);
  let visible = $state<HTMLElement>();
  let t: ReturnType<typeof setTimeout> | undefined;

  async function copiar(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    // Sin portapapeles (http sin TLS, permiso denegado): lo dice y deja el texto seleccionado.
    if (!(await alPortapapeles(texto, mostrar ? null : visible))) return;
    hecho = true;
    clearTimeout(t);
    t = setTimeout(() => (hecho = false), 1500);
  }
</script>

<span class="copiable">
  {#if mono}<code class="selectable" bind:this={visible}>{mostrar ?? texto}</code>{:else}<span class="selectable" bind:this={visible}>{mostrar ?? texto}</span>{/if}
  <button type="button" class:hecho onclick={copiar} use:tip={hecho ? "Copiado" : `Copiar ${que}`} aria-label={hecho ? "Copiado" : `Copiar ${que}`}>
    {#if hecho}<Check size={13} />{:else}<Copy size={13} />{/if}
  </button>
  <span class="sr-only" aria-live="polite">{hecho ? "Copiado" : ""}</span>
</span>
