<script lang="ts">
  // «Lo que más ocupa» de una versión (como en la app de escritorio): las
  // carpetas y los archivos más grandes, con barras proporcionales. Lo calcula
  // el equipo recorriendo la versión (`restic ls`) y llega cifrado.
  import { untrack } from "svelte";
  import { FileClock, Folder, File, LoaderCircle, TriangleAlert } from "@lucide/svelte";
  import type { AccesoRepo } from "$lib/accesoRepo.svelte";
  import type { MensajeEquipo } from "$lib/sesion";
  import { partesRuta, prefijoComun, rutaLegible } from "$lib/detalle";
  import { bytes, numero, plural } from "$lib/formato";
  import { tip } from "$lib/tooltip";
  import DesbloquearRepo from "./DesbloquearRepo.svelte";

  interface Item {
    ruta: string;
    bytes: number;
    archivos?: number;
  }
  interface Ocupa {
    total_bytes: number;
    total_archivos: number;
    carpetas: Item[];
    archivos: Item[];
  }

  let { acceso, nombreRepo, version, alVerArchivo }: { acceso: AccesoRepo; nombreRepo: string; version: string; alVerArchivo: (ruta: string) => void } = $props();

  let datos = $state<Ocupa | null>(null);
  let error = $state("");
  let que = $state<"carpetas" | "archivos">("carpetas");
  const puede = $derived(acceso.abierta && acceso.admite("ocupa"));
  $effect(() => {
    const [v, listo] = [version, puede];
    if (!listo) return;
    untrack(() => {
      datos = null;
      error = "";
      acceso
        .pedir<MensajeEquipo & Ocupa>("ocupa", { version: v }, `ocupa|${v}`)
        .then((m) => v === version && (datos = { total_bytes: m.total_bytes ?? 0, total_archivos: m.total_archivos ?? 0, carpetas: m.carpetas ?? [], archivos: m.archivos ?? [] }))
        .catch((e) => v === version && (error = (e as Error).message));
    });
  });
  const items = $derived(datos ? (que === "carpetas" ? datos.carpetas : datos.archivos) : []);
  const max = $derived(Math.max(1, ...items.map((x) => x.bytes)));
  const comun = $derived(datos ? prefijoComun([...datos.carpetas, ...datos.archivos].map((x) => `${x.ruta}/x`)) : "/");
  const corta = (r: string) => (comun !== "/" && r.startsWith(comun) ? r.slice(comun.length).replace(/^\//, "") || partesRuta(r).nombre : rutaLegible(r));
</script>

<section class="ocupa">
  {#if !acceso.abierta}
    <DesbloquearRepo {acceso} {nombreRepo} para="ver lo que más ocupa" />
  {:else if !acceso.admite("ocupa")}
    <div class="notice notice-info"><p>El agente de {acceso.equipo.nombre} todavía no sabe calcularlo. Actualízalo para ver aquí lo que más ocupa.</p></div>
  {:else if error}
    <div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>
  {:else if !datos}
    <p class="espera" role="status"><LoaderCircle size={15} class="spin" />{acceso.equipo.nombre} recorre la versión (en una grande, puede tardar un par de minutos)…</p>
  {:else}
    <p class="total"><strong class="num">{bytes(datos.total_bytes)}</strong> en {plural(datos.total_archivos, "archivo", "archivos")}{#if comun !== "/"}<span class="faint">{" "}· en <span class="selectable">{rutaLegible(comun)}</span></span>{/if}</p>
    <div class="segmented inline" role="group" aria-label="Qué ver">
      <button class:on={que === "carpetas"} aria-pressed={que === "carpetas"} onclick={() => (que = "carpetas")}>Carpetas</button>
      <button class:on={que === "archivos"} aria-pressed={que === "archivos"} onclick={() => (que = "archivos")}>Archivos</button>
    </div>
    <ol class="lista">
      {#each items as x (x.ruta)}
        <li>
          <span class="ic">{#if que === "carpetas"}<Folder size={14} />{:else}<File size={14} />{/if}</span>
          <span class="nombre selectable" title={rutaLegible(x.ruta)}>{corta(x.ruta)}</span>
          <span class="tam num">{bytes(x.bytes)}<small class="faint">{Math.round((x.bytes / Math.max(1, datos.total_bytes)) * 100)} %{#if que === "carpetas" && x.archivos}{" "}· {numero(x.archivos)}{/if}</small></span>
          {#if que === "archivos"}
            <button class="icon-btn" use:tip={"Ver versiones de este archivo"} aria-label="Ver las versiones de {partesRuta(x.ruta).nombre}" onclick={() => alVerArchivo(x.ruta)}><FileClock size={15} /></button>
          {:else}<span></span>{/if}
          <span class="barra" aria-hidden="true"><span style:width="{(x.bytes / max) * 100}%"></span></span>
        </li>
      {/each}
    </ol>
  {/if}
</section>

<style>
  .ocupa {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .total {
    margin: 0;
    font-size: var(--fs-sm);
    overflow-wrap: anywhere;
  }
  .total strong {
    font-size: 18px;
  }
  .lista {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) auto 32px;
    align-items: center;
    gap: 2px var(--sp-2);
    padding: 6px 0;
    border-top: 1px solid var(--border);
    font-size: var(--fs-sm);
  }
  .ic {
    display: inline-flex;
    color: var(--text-3);
  }
  .nombre {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .tam {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
  }
  .tam small {
    font-size: var(--fs-xs);
  }
  .barra {
    grid-column: 2 / -1;
    height: 4px;
    background: var(--border);
    border-radius: 2px;
    overflow: hidden;
  }
  .barra span {
    display: block;
    height: 100%;
    background: color-mix(in srgb, var(--text-3) 75%, transparent);
    border-radius: 2px;
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
</style>
