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
  import { Cloud, Database, FolderPlus, HardDrive, Plus, Server } from "@lucide/svelte";
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
  }
  let { id, etiqueta, opciones, value = $bindable(), alConectar, alOtroPaso }: Props = $props();
  const ICONO = { zona: Server, nube: Cloud, equipo: Database, suelto: Database, carpeta: HardDrive, nuevo: Plus } as const;
</script>

<div class="field">
  <span class="field-label" id="{id}-l">{etiqueta}</span>
  <div class="lista-dest" role="radiogroup" aria-labelledby="{id}-l">
    {#each opciones as o (o.valor)}
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
        {:else if !o.uso.ok && o.uso.accion?.tipo === "otro_paso" && alOtroPaso}
          {@const a = o.uso.accion}
          <button type="button" class="btn btn-sm btn-ghost" onclick={() => alOtroPaso(a.uso)}>{a.uso === "derivada" ? "Repositorio nuevo a partir de esta" : a.uso === "espejo" ? "Espejo" : "Copia nueva"}</button>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .lista-dest {
    display: flex;
    flex-direction: column;
    max-height: 300px;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius);
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
