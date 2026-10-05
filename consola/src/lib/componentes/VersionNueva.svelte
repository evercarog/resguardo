<script lang="ts">
  // «Hay una versión nueva de la consola · Recargar»: el servidor se actualizó y
  // esta pestaña sigue con el código de antes (lib/version.svelte.ts).
  import { slide } from "svelte/transition";
  import { RefreshCw, Sparkles } from "@lucide/svelte";
  import { dur } from "$ui/movimiento";
  import { versionNueva } from "$lib/version.svelte";
</script>

{#if versionNueva.hay}
  <div class="nueva" role="status" transition:slide={{ duration: dur(180) }}>
    <Sparkles size={15} />
    <p><strong>Hay una versión nueva de la consola</strong> (Resguardo Server {versionNueva.version}). Esta pestaña sigue con la anterior: recárgala para usar la nueva.</p>
    <button class="btn btn-sm btn-primary" onclick={() => location.reload()}><RefreshCw size={14} />Recargar</button>
  </div>
{/if}

<style>
  .nueva {
    position: sticky;
    top: 0;
    z-index: 6;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px var(--sp-4);
    font-size: var(--fs-sm);
    color: var(--text);
    background: var(--accent-soft);
    border-bottom: 1px solid color-mix(in srgb, var(--accent) 40%, var(--border));
  }
  .nueva p {
    flex: 1;
    margin: 0;
  }
</style>
