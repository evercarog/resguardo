<script lang="ts">
  // Las nubes conectadas en un equipo, ahí mismo (plan 0.8.1, B3; plan-mejoras 15s): nombre,
  // tipo, bucket y carpeta base, si es inmutable, qué la usa, lo que ocupa y el estado de la
  // conexión, con enlace a su página. Sin «(ver abajo)». «Desconectar» solo si nadie la usa.
  import { ChevronRight, Cloud, Lock } from "@lucide/svelte";
  import { tip } from "$lib/tooltip";
  import { bytes } from "$lib/formato";
  import { hrefNube, type NubeVista } from "$lib/nubesAlmacen";
  import Chip from "../Chip.svelte";
  import Tiempo from "../Tiempo.svelte";

  let {
    cliente,
    equipo,
    nubes,
    administra = false,
    ondesconectar,
  }: {
    cliente: string;
    equipo: string;
    nubes: NubeVista[];
    administra?: boolean;
    /** Desconectar (la página abre su orden con la clave de administración). */
    ondesconectar?: (nombre: string) => void;
  } = $props();

  const TONO = { ok: "ok", bad: "bad", neutral: "neutral" } as const;
</script>

<ul class="nubes">
  {#each nubes as n (n.nombre)}
    <li class="nube">
      <span class="ic" aria-hidden="true"><Cloud size={15} /></span>
      <span class="txt">
        <a class="nombre" href={hrefNube(cliente, equipo, n.nombre)}>{n.nombre}<ChevronRight size={13} aria-hidden="true" /></a>
        <span class="faint">
          {n.tipoTexto}{#if n.bucket}{" · bucket "}<span class="pastilla mono">{n.bucket}</span>{/if}{#if n.carpetaBase}{" · carpeta base "}<span class="pastilla mono">{n.carpetaBase}</span>{/if}
          {#if n.inmutable}{" · "}<span class="inm" use:tip={"Bloqueo de objetos: lo subido no se puede borrar ni cambiar durante su plazo."}><Lock size={11} />inmutable</span>{:else}{" · no inmutable"}{/if}
        </span>
        <span class="faint">{n.usos.length ? `La usa ${n.usos.join(", ")}` : "Nadie la usa ahora"}{#if n.ocupa != null}{" · "}{bytes(n.ocupa)} allí{/if}{#if n.espacio}{" · "}{bytes(n.espacio.libre)} libres{/if}</span>
        {#if n.conexion.tono === "bad" && n.conexion.error}<span class="mal">{n.conexion.error}</span>{/if}
      </span>
      <span class="der">
        <Chip pequeno tono={TONO[n.conexion.tono]} texto={n.conexion.texto} />
        {#if n.conexion.cuando}<span class="faint cuando"><Tiempo iso={n.conexion.cuando} /></span>{/if}
        {#if administra && ondesconectar && !n.motivo}<button class="btn btn-sm btn-ghost" onclick={() => ondesconectar?.(n.nombre)} aria-label="Desconectar {n.nombre}">Desconectar</button>{/if}
      </span>
    </li>
  {/each}
</ul>

<style>
  .nubes {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
    border-top: 1px solid var(--border);
  }
  .nube {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--sp-3);
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
  }
  .ic {
    display: grid;
    flex: none;
    margin-top: 2px;
    color: var(--text-3);
  }
  .txt {
    display: flex;
    flex: 1 1 220px;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    font-size: var(--fs-sm);
  }
  .txt .faint,
  .mal {
    font-size: var(--fs-xs);
    overflow-wrap: anywhere;
  }
  .mal {
    color: var(--bad);
  }
  .nombre {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-weight: 500;
    color: var(--text-1);
    text-decoration: none;
    overflow-wrap: anywhere;
  }
  .nombre:hover {
    text-decoration: underline;
  }
  .inm {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .der {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-left: auto;
  }
  .cuando {
    font-size: var(--fs-xs);
  }
</style>
