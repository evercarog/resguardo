<script lang="ts">
  // Un archivo encontrado en «Buscar archivos»: su nombre (con lo buscado
  // resaltado), la carpeta, su línea de tiempo en las versiones, cuándo
  // cambió por última vez y sus tamaños; y qué hacer con él: restaurar una de
  // sus versiones (junto al original o descargarla), ver todas sus versiones
  // o qué cambió en la versión en que cambió.
  import { FileClock, FileText, GitCompareArrows, History, MoveRight, TriangleAlert } from "@lucide/svelte";
  import type { AccesoRepo } from "$lib/accesoRepo.svelte";
  import { borradoDespues, lineaArchivo, trozosNombre, type ArchivoEncontrado } from "$lib/buscarArchivos";
  import { rutaLegible } from "$lib/detalle";
  import { bytes, fechaCorta, fechaLarga, plural } from "$lib/formato";
  import { tip } from "$lib/tooltip";
  import RestaurarArchivo from "../detalle/RestaurarArchivo.svelte";
  import LineaArchivo from "./LineaArchivo.svelte";

  let {
    archivo,
    texto,
    versionesRepo,
    nombreRepo = null,
    acceso,
    puedeRestaurar,
    enlaceRestaurar,
    alVerVersiones,
    alQueCambio,
  }: {
    archivo: ArchivoEncontrado;
    /** Lo que se buscó (para resaltarlo en el nombre). */
    texto: string;
    /** Las versiones del repositorio en las fechas de la búsqueda (para la línea). */
    versionesRepo: { id: string; cuando: string }[];
    /** Con varios repositorios, de cuál es. */
    nombreRepo?: string | null;
    acceso: AccesoRepo | null;
    puedeRestaurar: boolean;
    enlaceRestaurar: (version: string) => string;
    alVerVersiones: () => void;
    alQueCambio: (version: string) => void;
  } = $props();

  let restaurando = $state(false);
  let elegida = $state<string | null>(null);
  const version = $derived(archivo.versiones.find((v) => v.version === elegida) ?? archivo.ultima);
  const marcas = $derived(lineaArchivo(archivo, versionesRepo));
  const borrado = $derived(borradoDespues(archivo, versionesRepo));
  const varia = $derived(archivo.bytesMin != null && archivo.bytesMax != null && archivo.bytesMin !== archivo.bytesMax);
  const idTitulo = $derived(`a-${archivo.repo}-${archivo.ruta}`.replace(/[^A-Za-z0-9_-]/g, "_").slice(0, 200));
  const resumen = $derived(
    [
      `En ${plural(archivo.versiones.length, "versión", "versiones")}${archivo.recortado ? " (o más)" : ""}, de ${fechaCorta(archivo.primera.cuando)} a ${fechaCorta(archivo.ultima.cuando)}`,
      archivo.ultimoCambio ? `cambió por última vez en la versión del ${fechaCorta(archivo.ultimoCambio.cuando)}` : "igual desde que apareció",
      borrado ? "ya no está en la versión más reciente" : null,
    ]
      .filter(Boolean)
      .join("; "),
  );
</script>

<li class="card archivo" aria-labelledby={idTitulo}>
  <div class="cab">
    <span class="ic" aria-hidden="true"><FileText size={18} /></span>
    <div class="titulo">
      <h3 id={idTitulo} class="nombre selectable">
        {#each trozosNombre(archivo.nombre, texto) as t, i (i)}{#if t.marca}<mark>{t.texto}</mark>{:else}{t.texto}{/if}{/each}
      </h3>
      <p class="carpeta faint selectable" title={rutaLegible(archivo.ruta)}>{rutaLegible(archivo.carpeta)}{#if nombreRepo}<span class="badge badge-sm tone-neutral repo">{nombreRepo}</span>{/if}</p>
    </div>
    <div class="tam num">
      <strong use:tip={"Lo que ocupa en la versión más reciente en la que está"}>{bytes(archivo.bytes)}</strong>
      {#if varia}<small class="faint">de {bytes(archivo.bytesMin)} a {bytes(archivo.bytesMax)}</small>{/if}
    </div>
  </div>

  <div class="linea-fila">
    <span class="extremo faint num">{fechaCorta(versionesRepo.length ? [...versionesRepo].sort((a, b) => Date.parse(a.cuando) - Date.parse(b.cuando))[0].cuando : archivo.primera.cuando)}</span>
    <LineaArchivo {marcas} elegida={restaurando ? version.version : null} />
    <span class="extremo faint num">{fechaCorta(versionesRepo.length ? [...versionesRepo].sort((a, b) => Date.parse(b.cuando) - Date.parse(a.cuando))[0].cuando : archivo.ultima.cuando)}</span>
  </div>
  <p class="sr-only">{resumen}.</p>

  <p class="datos" aria-hidden="true">
    <span>En <strong>{plural(archivo.versiones.length, "versión", "versiones")}</strong>{#if archivo.recortado}{" "}(o más){/if}</span>
    {#if archivo.ultimoCambio}
      <span use:tip={fechaLarga(archivo.ultimoCambio.cuando)}>Cambió por última vez: <strong>{fechaCorta(archivo.ultimoCambio.cuando)}</strong>{#if archivo.cambios > 1}{" "}<span class="faint">({plural(archivo.cambios, "cambio", "cambios")})</span>{/if}</span>
    {:else}
      <span>Igual desde que apareció ({fechaCorta(archivo.primera.cuando)})</span>
    {/if}
    {#if archivo.ultima.modificado}<span class="faint" use:tip={"La fecha de modificación del archivo en el equipo"}>modificado {fechaCorta(archivo.ultima.modificado)}</span>{/if}
    {#if borrado}<span class="badge badge-sm tone-warn"><TriangleAlert size={12} />Ya no está en la versión más reciente</span>{/if}
  </p>

  <div class="acciones">
    {#if puedeRestaurar && acceso}
      <button class="btn btn-sm" class:btn-primary={borrado} aria-expanded={restaurando} onclick={() => (restaurando = !restaurando)}><History size={14} />Restaurar esta versión</button>
    {/if}
    <button class="btn btn-sm btn-ghost" disabled={!acceso} onclick={alVerVersiones}><FileClock size={14} />Ver todas sus versiones</button>
    <button class="btn btn-sm btn-ghost" disabled={!acceso} use:tip={archivo.ultimoCambio ? `Qué más cambió en la versión del ${fechaCorta(archivo.ultimoCambio.cuando)}` : "Qué cambió en la versión en que apareció"} onclick={() => alQueCambio((archivo.ultimoCambio ?? archivo.primera).version)}><GitCompareArrows size={14} />Qué cambió</button>
  </div>

  {#if restaurando && acceso}
    <div class="restaurar">
      <label class="elegir">
        <span>Versión</span>
        <select class="input" value={version.version} onchange={(e) => (elegida = e.currentTarget.value)}>
          {#each archivo.versiones as v (v.version)}
            <option value={v.version}>{fechaCorta(v.cuando)} · {bytes(v.bytes ?? null)}{v.cambio ? (v === archivo.primera ? " · aparece" : " · cambió") : ""}</option>
          {/each}
        </select>
      </label>
      {#key version.version}
        <RestaurarArchivo {acceso} version={version.version} cuando={version.cuando} ruta={archivo.ruta} tamano={version.bytes} />
      {/key}
      <a class="mas faint" href={enlaceRestaurar(version.version)}>Otras formas (en su sitio, en otro equipo, varios archivos): en Restaurar<MoveRight size={13} /></a>
    </div>
  {/if}
</li>

<style>
  .archivo {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-4);
    list-style: none;
  }
  .cab {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: var(--sp-3);
    align-items: start;
  }
  .ic {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: var(--radius);
    background: var(--accent-soft);
    color: var(--accent-text, var(--accent));
  }
  .titulo {
    min-width: 0;
  }
  .nombre {
    margin: 0;
    font-size: var(--fs-body);
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  mark {
    background: var(--warn-soft);
    color: inherit;
    border-radius: 3px;
    padding: 0 1px;
  }
  .carpeta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin: 2px 0 0;
    font-size: var(--fs-xs);
    overflow-wrap: anywhere;
  }
  .tam {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    font-size: var(--fs-sm);
    white-space: nowrap;
  }
  .tam small {
    font-size: var(--fs-xs);
  }
  .linea-fila {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--sp-2);
  }
  .extremo {
    font-size: var(--fs-xs);
  }
  .datos {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px var(--sp-3);
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .restaurar {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding-top: var(--sp-2);
    border-top: 1px solid var(--border);
  }
  .elegir {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--fs-sm);
  }
  .elegir select {
    max-width: 100%;
    width: auto;
  }
  .mas {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--fs-xs);
  }
  @media (max-width: 640px) {
    .cab {
      grid-template-columns: auto minmax(0, 1fr);
    }
    .tam {
      grid-column: 2;
      align-items: flex-start;
      flex-direction: row;
      gap: 6px;
    }
    .extremo {
      display: none;
    }
    .linea-fila {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
