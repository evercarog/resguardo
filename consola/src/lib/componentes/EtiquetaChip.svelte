<script lang="ts">
  // Una etiqueta de equipo: punto de su color (siempre el mismo) y el texto en
  // tinta neutra. Con `onquitar`, una «x» para quitarla; con `onclick`, es un
  // botón (filtrar por ella), y `activa` la marca.
  import { X } from "@lucide/svelte";
  import { colorEtiqueta } from "$lib/etiquetas.svelte";

  let { nombre, onquitar, onclick, activa = false, n }: { nombre: string; onquitar?: () => void; onclick?: () => void; activa?: boolean; n?: number } = $props();
  const color = $derived(`var(--et-${colorEtiqueta(nombre)})`);
</script>

{#if onclick}
  <button type="button" class="et boton" class:activa aria-pressed={activa} style:--et={color} {onclick}><span class="punto" aria-hidden="true"></span>{nombre}{#if n !== undefined}<span class="n">{n}</span>{/if}</button>
{:else}
  <span class="et" style:--et={color}>
    <span class="punto" aria-hidden="true"></span>{nombre}
    {#if onquitar}<button type="button" class="quitar" aria-label="Quitar la etiqueta «{nombre}»" onclick={onquitar}><X size={12} /></button>{/if}
  </span>
{/if}

<style>
  .et {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    max-width: 100%;
    height: 22px;
    padding: 0 8px;
    font: inherit;
    font-size: var(--fs-xs);
    line-height: 1;
    white-space: nowrap;
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
  }
  .punto {
    flex: none;
    width: 8px;
    height: 8px;
    background: var(--et);
    border-radius: 999px;
  }
  .boton {
    height: 26px;
    padding: 0 10px;
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  .boton:hover {
    color: var(--text-1);
    border-color: var(--border-strong);
  }
  .boton.activa {
    color: var(--text-1);
    background: color-mix(in srgb, var(--et) 14%, var(--surface));
    border-color: color-mix(in srgb, var(--et) 60%, var(--border));
  }
  .n {
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
  }
  .quitar {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    margin-right: -4px;
    padding: 0;
    color: var(--text-3);
    background: none;
    border: none;
    border-radius: 999px;
    cursor: pointer;
  }
  .quitar:hover {
    color: var(--text-1);
    background: var(--surface-3);
  }
</style>
