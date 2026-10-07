<script lang="ts">
  // El tipo y las marcas de un destino (0.7.26, bloque 2; lib/tipoDestino.ts).
  // Icono + etiqueta corta; nunca solo el color. «compacta» (cadenas, mapa,
  // selectores): el icono del tipo y su nombre, y las marcas solo con icono
  // (con su nombre para lectores de pantalla y en el globo). «completa»
  // (página y tarjetas del destino): todo con palabra, los días del bloqueo y,
  // si lo marcó una persona, «marcado por una persona».
  import { Building2, Cloud, HardDrive, Lock, Unplug } from "@lucide/svelte";
  import { TEXTO_TIPO, textoClasificacion } from "$lib/tipoDestino";
  import type { TipoDestino } from "$lib/tipos";

  let {
    tipo,
    inmutable = false,
    aislado = false,
    bloqueoDias = null,
    porPersona = false,
    variante = "compacta",
    soloIcono = false,
  }: {
    tipo: TipoDestino;
    inmutable?: boolean;
    aislado?: boolean;
    bloqueoDias?: number | null;
    porPersona?: boolean;
    variante?: "compacta" | "completa";
    /** Solo los iconos (dentro de un texto que ya nombra el destino). */
    soloIcono?: boolean;
  } = $props();

  const ICONO = { local: HardDrive, fuera: Building2, nube: Cloud } as const;
  const Icono = $derived(ICONO[tipo]);
  const texto = $derived(textoClasificacion({ tipo, inmutable, aislado, bloqueoDias }) + (porPersona ? " (marcado por una persona)" : ""));
  const tam = $derived(variante === "completa" ? 15 : 13);
</script>

<span class="td" class:completa={variante === "completa"} title={texto} data-tipo={tipo}>
  <span class="sr-only">{texto}</span>
  <span class="pieza" aria-hidden="true"><Icono size={tam} />{#if !soloIcono}<span>{TEXTO_TIPO[tipo]}</span>{/if}</span>
  {#if inmutable}
    <span class="pieza marca" aria-hidden="true"><Lock size={tam} />{#if variante === "completa"}<span>Inmutable{bloqueoDias ? ` · ${bloqueoDias} días` : ""}</span>{/if}</span>
  {/if}
  {#if aislado}
    <span class="pieza marca" aria-hidden="true"><Unplug size={tam} />{#if variante === "completa"}<span>Aislado</span>{/if}</span>
  {/if}
  {#if porPersona && variante === "completa"}<span class="persona" aria-hidden="true">marcado por una persona</span>{/if}
</span>

<style>
  .td {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    color: var(--text-2);
    font-size: var(--fs-xs, 0.75rem);
    line-height: 1.2;
    vertical-align: middle;
    min-width: 0;
  }
  /* La compacta no se parte: el tipo y sus marcas, siempre juntos. */
  .td:not(.completa) {
    flex-wrap: nowrap;
    gap: 4px;
  }
  .completa {
    font-size: var(--fs-sm, 0.8125rem);
    gap: 6px 8px;
  }
  .pieza {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    white-space: nowrap;
  }
  .completa .pieza {
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface);
    color: var(--text-1);
  }
  .marca {
    color: var(--text-1);
  }
  .persona {
    color: var(--text-3);
    font-style: italic;
  }
  .pieza :global(svg) {
    flex: none;
  }
</style>
