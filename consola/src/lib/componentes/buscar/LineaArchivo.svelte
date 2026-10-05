<script lang="ts">
  // Línea de tiempo pequeña de un archivo encontrado: una marca por versión
  // del repositorio (en las fechas de la búsqueda), rellena donde el archivo
  // está, de color donde cambió respecto a la anterior y hueca donde falta.
  // Es un dibujo: lo mismo va en texto al lado (para lectores de pantalla).
  import { fechaCorta } from "$lib/formato";
  import type { MarcaLinea } from "$lib/buscarArchivos";

  let { marcas, elegida = null }: { marcas: MarcaLinea[]; elegida?: string | null } = $props();

  const TEXTO = { falta: "No está", igual: "Igual que la anterior", cambio: "Cambió", aparece: "Aparece" };
  // Con muchas versiones, las marcas se estrechan (sin dejar de verse).
  const r = $derived(marcas.length > 120 ? 2 : marcas.length > 50 ? 2.5 : 3.5);
  const presente = $derived(marcas.filter((m) => m.estado !== "falta"));
  const desde = $derived(presente.length ? presente[0].x : 0);
  const hasta = $derived(presente.length ? presente[presente.length - 1].x : 0);
  const X = (x: number) => 6 + x * 88;
</script>

<div class="caja">
  <svg class="linea" viewBox="0 0 100 14" preserveAspectRatio="none" aria-hidden="true" focusable="false">
  <line class="eje" x1="6" x2="94" y1="7" y2="7" />
  {#if presente.length}<line class="vida" x1={X(desde)} x2={X(hasta)} y1="7" y2="7" />{/if}
  </svg>
  <div class="marcas" aria-hidden="true">
    {#each marcas as m (m.version)}
      <span class="m {m.estado}" class:elegida={elegida != null && m.version === elegida} style:left="{X(m.x)}%" style:--r="{r}px" title="{fechaCorta(m.cuando)} · {TEXTO[m.estado]}"></span>
    {/each}
  </div>
</div>

<style>
  .caja {
    position: relative;
    height: 18px;
    min-width: 0;
  }
  .linea {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
  }
  .eje {
    stroke: var(--border);
    stroke-width: 1.5;
    vector-effect: non-scaling-stroke;
  }
  .vida {
    stroke: var(--text-3);
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
  }
  .marcas {
    position: absolute;
    inset: 0;
  }
  .m {
    position: absolute;
    top: 50%;
    width: calc(var(--r) * 2);
    height: calc(var(--r) * 2);
    border-radius: 50%;
    transform: translate(-50%, -50%);
    background: var(--text-3);
    border: 1px solid var(--surface);
  }
  .m.falta {
    background: var(--surface);
    border-color: var(--border-strong);
    width: calc(var(--r) * 1.4);
    height: calc(var(--r) * 1.4);
  }
  .m.cambio,
  .m.aparece {
    background: var(--accent);
    width: calc(var(--r) * 2.4);
    height: calc(var(--r) * 2.4);
  }
  .m.aparece {
    background: var(--ok);
  }
  .m.elegida {
    outline: 2px solid var(--focus, var(--accent));
    outline-offset: 1px;
    z-index: 1;
  }
</style>
