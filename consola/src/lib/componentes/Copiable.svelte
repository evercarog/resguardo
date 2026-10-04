<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Un código, una ruta o una huella con su botón de copiar al lado. Al
  // copiar, el icono pasa a ✓ un momento y se anuncia «Copiado».
  import { Check, Copy } from "@lucide/svelte";

  let { texto, mostrar, que = "el texto", mono = true }: { texto: string; mostrar?: string; que?: string; mono?: boolean } = $props();
  let hecho = $state(false);
  let t: ReturnType<typeof setTimeout> | undefined;

  async function copiar(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(texto);
      hecho = true;
      clearTimeout(t);
      t = setTimeout(() => (hecho = false), 1500);
    } catch {
      /* sin portapapeles (http sin TLS, permiso denegado): se puede seleccionar a mano */
    }
  }
</script>

<span class="copiable">
  {#if mono}<code class="selectable">{mostrar ?? texto}</code>{:else}<span class="selectable">{mostrar ?? texto}</span>{/if}
  <button type="button" class:hecho onclick={copiar} use:tip={hecho ? "Copiado" : `Copiar ${que}`} aria-label={hecho ? "Copiado" : `Copiar ${que}`}>
    {#if hecho}<Check size={13} />{:else}<Copy size={13} />{/if}
  </button>
  <span class="sr-only" aria-live="polite">{hecho ? "Copiado" : ""}</span>
</span>
