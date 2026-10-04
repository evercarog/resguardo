<script lang="ts">
  // «Ver versiones de este archivo»: en qué versiones del repositorio está,
  // con su tamaño y su fecha de modificación, marcando en cuáles cambió. Desde
  // cada una se puede restaurar o ver qué más cambió en esa versión.
  import { untrack } from "svelte";
  import { FileClock, GitCompareArrows, History, LoaderCircle, TriangleAlert } from "@lucide/svelte";
  import type { AccesoRepo } from "$lib/accesoRepo.svelte";
  import type { MensajeEquipo } from "$lib/sesion";
  import { marcarCambiosArchivo, partesRuta, rutaLegible, type ArchivoEnVersion } from "$lib/detalle";
  import { bytes, fechaCorta, fechaLarga, plural } from "$lib/formato";
  import { tip } from "$lib/tooltip";
  import RestaurarArchivo from "./RestaurarArchivo.svelte";
  import { abrirVersion } from "./navegar";

  let { acceso, ruta, puedeRestaurar }: { acceso: AccesoRepo; ruta: string; puedeRestaurar: boolean } = $props();

  let lista = $state<ReturnType<typeof marcarCambiosArchivo> | null>(null);
  let error = $state("");
  let restaurando = $state<string | null>(null);
  $effect(() => {
    const r = ruta;
    untrack(() => {
      lista = null;
      error = "";
      acceso
        .pedir<MensajeEquipo & { versiones?: ArchivoEnVersion[] }>("historial_archivo", { ruta: r }, `historial|${r}`)
        .then((m) => r === ruta && (lista = marcarCambiosArchivo(m.versiones ?? [])))
        .catch((e) => r === ruta && (error = (e as Error).message));
    });
  });
  const distintas = $derived(lista?.filter((x) => x.cambio).length ?? 0);
</script>

<section class="versiones-archivo" aria-labelledby="t-va">
  <h3 id="t-va"><FileClock size={16} />{partesRuta(ruta).nombre}</h3>
  <p class="faint ruta selectable">{rutaLegible(ruta)}</p>
  {#if error}
    <div class="notice notice-danger" role="alert"><TriangleAlert size={15} /><p>{error}</p></div>
  {:else if !lista}
    <p class="espera" role="status"><LoaderCircle size={15} class="spin" />{acceso.equipo.nombre} busca el archivo en todas las versiones…</p>
  {:else if !lista.length}
    <p class="faint">No está en ninguna versión que quede en el repositorio.</p>
  {:else}
    <p class="faint resumen">Está en {plural(lista.length, "versión", "versiones")}; {plural(distintas, "es distinta", "son distintas")} de la anterior.</p>
    <ul>
      {#each lista as x (x.version)}
        <li class:cambio={x.cambio}>
          <span class="cuando num" use:tip={fechaLarga(x.cuando)}>{fechaCorta(x.cuando)}</span>
          <span class="tam num">{bytes(x.bytes ?? null)}</span>
          <span class="faint mod">{#if x.modificado}modificado {fechaCorta(x.modificado)}{/if}</span>
          <span class="marca">{#if x.cambio}<span class="badge badge-sm tone-info">Distinto</span>{:else}<span class="faint">igual</span>{/if}</span>
          <span class="acc">
            <button class="btn btn-sm btn-ghost" use:tip={"Qué más cambió en esa versión"} onclick={() => abrirVersion(x.version)}><GitCompareArrows size={14} />Versión</button>
            {#if puedeRestaurar}<button class="btn btn-sm btn-ghost" aria-expanded={restaurando === x.version} onclick={() => (restaurando = restaurando === x.version ? null : x.version)}><History size={14} />Restaurar</button>{/if}
          </span>
          {#if restaurando === x.version}
            <div class="rest"><RestaurarArchivo {acceso} version={x.version} cuando={x.cuando} {ruta} tamano={x.bytes} /></div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: var(--fs-body);
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .ruta {
    margin: 2px 0 var(--sp-3);
    font-size: var(--fs-xs);
    overflow-wrap: anywhere;
  }
  .resumen {
    margin: 0 0 var(--sp-2);
    font-size: var(--fs-sm);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: grid;
    grid-template-columns: 9rem 6rem minmax(0, 1fr) auto auto;
    align-items: center;
    gap: var(--sp-2);
    min-height: 44px;
    padding: 6px 0;
    border-top: 1px solid var(--border);
    font-size: var(--fs-sm);
  }
  li:not(.cambio) {
    color: var(--text-2);
  }
  .tam {
    text-align: right;
  }
  .mod {
    font-size: var(--fs-xs);
  }
  .acc {
    display: flex;
    gap: 2px;
  }
  .rest {
    grid-column: 1 / -1;
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  @media (max-width: 640px) {
    li {
      grid-template-columns: minmax(0, 1fr) auto;
    }
    .mod,
    .marca {
      display: none;
    }
    .acc {
      grid-column: 1 / -1;
    }
  }
</style>
