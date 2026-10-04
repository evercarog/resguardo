<script lang="ts">
  // Barra fina arriba mientras se cambia de pantalla o se espera al servidor
  // (solo si tarda más de ~300 ms, para no parpadear) y un «Actualizando…»
  // discreto arriba a la derecha para los refrescos automáticos. No mueve nada de sitio.
  import { navigating } from "$app/state";
  import { LoaderCircle } from "@lucide/svelte";
  import { actividad } from "$lib/actividad.svelte";

  const ocupado = $derived(!!navigating.to || actividad.primerPlano > 0);
  const refrescando = $derived(actividad.fondo > 0);
  let verBarra = $state(false);
  let verRefresco = $state(false);

  // Con retraso al aparecer (lo rápido no se anuncia) y al instante al irse.
  function retrasar(activo: () => boolean, poner: (v: boolean) => void, ms: number) {
    if (!activo()) {
      poner(false);
      return;
    }
    const t = setTimeout(() => poner(true), ms);
    return () => clearTimeout(t);
  }
  $effect(() => retrasar(() => ocupado, (v) => (verBarra = v), 300));
  $effect(() => retrasar(() => refrescando && !ocupado, (v) => (verRefresco = v), 800));
</script>

<div class="barra" class:on={verBarra} role="progressbar" aria-label="Cargando" aria-hidden={!verBarra}><span></span></div>
{#if verRefresco}
  <!-- Solo visual: los refrescos automáticos no se anuncian (cada pocos segundos sería ruido). -->
  <div class="refresco" aria-hidden="true"><LoaderCircle size={12} class="spin" />Actualizando…</div>
{/if}

<style>
  .barra {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    z-index: 60;
    height: 2px;
    overflow: hidden;
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--dur) var(--ease);
  }
  .barra.on {
    opacity: 1;
  }
  .barra span {
    position: absolute;
    top: 0;
    bottom: 0;
    left: -40%;
    width: 40%;
    background: var(--accent);
    border-radius: 2px;
    animation: avance 1.1s var(--ease) infinite;
  }
  @keyframes avance {
    to {
      left: 100%;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .barra span {
      left: 0;
      width: 100%;
      animation: none;
      opacity: 0.6;
    }
  }
  /* Arriba a la derecha, en el margen de la página (no tapa botones ni la lista). */
  .refresco {
    position: fixed;
    top: 6px;
    right: var(--sp-4);
    z-index: 40;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 1px 8px;
    font-size: 11px;
    line-height: 16px;
    color: var(--text-3);
    background: color-mix(in srgb, var(--bg) 85%, transparent);
    border-radius: 999px;
    pointer-events: none;
  }
  @media (max-width: 860px) {
    .refresco {
      top: 56px;
    }
  }
</style>
