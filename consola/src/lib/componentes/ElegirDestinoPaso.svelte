<script lang="ts" module>
  import type { Uso } from "$lib/cadenas";
  import type { TipoCorto } from "$lib/tipoDestino";
  export interface OpcionDestino {
    valor: string;
    nombre: string;
    /** Qué es, en pocas palabras («Dropbox · conectada en ALMACEN-SUR»). */
    detalle?: string;
    clase: "zona" | "nube" | "equipo" | "suelto" | "carpeta" | "nuevo";
    uso: Uso;
    /** 0.7.26: tipo y marcas del destino (sin ello no se enseñan). */
    tipoDestino?: TipoCorto;
  }
</script>

<script lang="ts">
  // Elegir el destino de un paso («Añadir paso», docs/editor-de-copias.md):
  // todos los destinos del cliente, cada uno con si se puede usar desde aquí y,
  // si no, por qué y qué hacer (conectar la nube en ese equipo, actualizar su
  // agente u otro tipo de paso). Nunca se esconde uno sin decir por qué.
  import { Cloud, Database, FolderPlus, HardDrive, KeyRound, Plus, Server } from "@lucide/svelte";
  import type { ZonaVista } from "$lib/destinos";
  import { untrack } from "svelte";
  import type { Equipo } from "$lib/tipos";
  import TipoDestino from "./TipoDestino.svelte";

  interface Props {
    id: string;
    etiqueta: string;
    opciones: OpcionDestino[];
    value: string;
    /** «Conectar … también en …» (ConectarNube en ese equipo). */
    alConectar?: (equipo: Equipo, nube: string, tipoNube: string) => void;
    /** «Usar … en su lugar» (otro tipo de paso). */
    alOtroPaso?: (uso: string) => void;
    /** Plan 0.8.0 (B3): «Dar acceso a … en este almacén» (sin crear repositorio). */
    alDarAcceso?: (equipo: Equipo, zona: ZonaVista) => void;
  }
  let { id, etiqueta, opciones, value = $bindable(), alConectar, alOtroPaso, alDarAcceso }: Props = $props();
  const ICONO = { zona: Server, nube: Cloud, equipo: Database, suelto: Database, carpeta: HardDrive, nuevo: Plus } as const;

  // 14ff: «Un destino nuevo…» siempre a la vista (bajo la lista, fuera de lo que
  // se desplaza) y, si la lista no cabe, una línea que dice que hay más.
  const lista = $derived(opciones.filter((o) => o.clase !== "nuevo"));
  const nuevos = $derived(opciones.filter((o) => o.clase === "nuevo"));
  let caja = $state<HTMLDivElement | null>(null);
  /** Cuántas opciones quedan por debajo de lo que se ve (0: se ven todas). */
  let ocultas = $state(0);
  function medir() {
    const el = caja;
    if (!el) return;
    if (el.scrollHeight - el.clientHeight - el.scrollTop <= 2) {
      ocultas = 0;
      return;
    }
    const fondo = el.getBoundingClientRect().bottom;
    ocultas = [...el.children].filter((c) => c.getBoundingClientRect().bottom > fondo + 2).length;
  }
  $effect(() => {
    void lista.length;
    const el = caja;
    if (!el) return;
    untrack(medir);
    const ro = new ResizeObserver(() => medir());
    ro.observe(el);
    return () => ro.disconnect();
  });
</script>

{#snippet fila(o: OpcionDestino)}
  {@const I = ICONO[o.clase] ?? FolderPlus}
  <div class="op" class:on={value === o.valor} class:no={!o.uso.ok}>
    <label class="op-fila">
      <input type="radio" name={id} value={o.valor} checked={value === o.valor} disabled={!o.uso.ok} onchange={() => (value = o.valor)} />
      <span class="ic"><I size={15} /></span>
      <span class="txt">
        <span class="nom">{o.nombre}</span>
        {#if o.detalle || o.uso.motivo || o.tipoDestino}<span class="det">{#if o.tipoDestino}<TipoDestino {...o.tipoDestino} />{#if o.detalle || o.uso.motivo}{" · "}{/if}{/if}{[o.detalle, o.uso.motivo].filter(Boolean).join(" · ")}</span>{/if}
      </span>
    </label>
    {#if !o.uso.ok && o.uso.accion?.tipo === "conectar_nube" && alConectar}
      {@const a = o.uso.accion}
      <button type="button" class="btn btn-sm" onclick={() => alConectar(a.equipo, a.nube, a.tipoNube)}><Cloud size={13} />{a.texto ?? `Conectar en ${a.equipo.nombre}`}</button>
    {:else if !o.uso.ok && o.uso.accion?.tipo === "dar_acceso" && alDarAcceso}
      {@const a = o.uso.accion}
      <button type="button" class="btn btn-sm" onclick={() => alDarAcceso(a.equipo, a.zona)}><KeyRound size={13} />{a.texto}</button>
    {:else if !o.uso.ok && o.uso.accion?.tipo === "otro_paso" && alOtroPaso}
      {@const a = o.uso.accion}
      <button type="button" class="btn btn-sm btn-ghost" onclick={() => alOtroPaso(a.uso)}>{a.uso === "derivada" ? "Nuevo repositorio a partir de esta" : a.uso === "espejo" ? "Espejo" : "Copia nueva"}</button>
    {/if}
  </div>
{/snippet}

<div class="field">
  <span class="field-label" id="{id}-l">{etiqueta}</span>
  <div class="grupo" role="radiogroup" aria-labelledby="{id}-l">
    {#if lista.length}
      <div class="marco" class:con-mas={ocultas > 0}>
        <div class="lista-dest" bind:this={caja} onscroll={medir}>
          {#each lista as o (o.valor)}{@render fila(o)}{/each}
        </div>
      </div>
      {#if ocultas > 0}<p class="hay-mas" aria-hidden="true">{ocultas === 1 ? "Hay 1 más" : `Hay ${ocultas} más`}: desliza la lista</p>{/if}
    {/if}
    {#each nuevos as o (o.valor)}<div class="aparte">{@render fila(o)}</div>{/each}
  </div>
</div>

<style>
  .grupo {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .marco {
    position: relative;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }
  /* Lo que queda por debajo se desvanece: se ve que la lista sigue. */
  .marco.con-mas::after {
    content: "";
    position: absolute;
    inset: auto 0 0 0;
    height: 28px;
    pointer-events: none;
    background: linear-gradient(to bottom, transparent, var(--surface));
  }
  .lista-dest {
    display: flex;
    flex-direction: column;
    max-height: 300px;
    overflow-y: auto;
  }
  .hay-mas {
    margin: 0;
    font-size: var(--fs-xs);
    color: var(--text-3);
    text-align: center;
  }
  .aparte {
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
  }
  .aparte .op {
    border-top: none;
  }
  .op {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
    padding: 8px 10px;
    border-top: 1px solid var(--border);
  }
  .op:first-child {
    border-top: none;
  }
  .op.on {
    background: var(--accent-soft);
  }
  .op-fila {
    display: flex;
    flex: 1 1 220px;
    align-items: flex-start;
    gap: 8px;
    min-width: 0;
    cursor: pointer;
  }
  .no .op-fila {
    cursor: default;
  }
  .op-fila input {
    margin-top: 3px;
    flex: none;
  }
  .ic {
    flex: none;
    margin-top: 1px;
    color: var(--text-3);
  }
  .txt {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .nom {
    font-size: var(--fs-sm);
    font-weight: 550;
    color: var(--text-1);
    overflow-wrap: anywhere;
  }
  .no .nom {
    color: var(--text-2);
    font-weight: 500;
  }
  .det {
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
</style>
