<script lang="ts">
  // Franja discreta cuando la consola no llega al servidor: lo que se ve puede
  // no estar al día. Se reintenta sola; también se puede probar ya.
  import { slide } from "svelte/transition";
  import { RefreshCw, WifiOff } from "@lucide/svelte";
  import { dur } from "$ui/movimiento";
  import { actividad, conexionOk } from "$lib/actividad.svelte";

  let probando = $state(false);
  async function probar() {
    probando = true;
    try {
      const r = await fetch("/api/servidor", { cache: "no-store", credentials: "same-origin" });
      if (r.status > 0) conexionOk();
    } catch {
      /* sigue sin conexión */
    } finally {
      probando = false;
    }
  }
</script>

{#if actividad.sinConexion}
  <div class="sin" role="alert" transition:slide={{ duration: dur(180) }}>
    <WifiOff size={15} />
    <p><strong>Sin conexión con el servidor.</strong> Lo que ves puede no estar al día; se vuelve a intentar solo.</p>
    <button class="btn btn-sm btn-ghost" disabled={probando} onclick={probar}><RefreshCw size={14} class={probando ? "spin" : ""} />Reintentar</button>
  </div>
{/if}

<style>
  .sin {
    position: sticky;
    top: 0;
    z-index: 6;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px var(--sp-4);
    font-size: var(--fs-sm);
    color: var(--text-1);
    background: color-mix(in srgb, var(--warn) 12%, var(--bg));
    border-bottom: 1px solid color-mix(in srgb, var(--warn) 35%, var(--border));
  }
  .sin :global(svg) {
    flex: none;
    color: var(--warn);
  }
  .sin p {
    flex: 1;
    margin: 0;
  }
  @media (max-width: 860px) {
    .sin {
      top: 52px;
    }
  }
</style>
