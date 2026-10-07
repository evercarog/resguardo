<script lang="ts">
  // «Restaurar este archivo» desde «Qué cambió» o desde sus versiones: junto
  // al original (lo seguro: no pisa nada) o descargado a este navegador por el
  // relé cifrado. Usa la contraseña con la que se abrió el repositorio.
  import { onDestroy } from "svelte";
  import { CircleCheck, Download, History, LoaderCircle, TriangleAlert } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { enFondo } from "$lib/actividad.svelte";
  import type { AccesoRepo } from "$lib/accesoRepo.svelte";
  import { aB64, aleatorio, borrar } from "$lib/cripto/bytes";
  import { bajarRelevo, guardar } from "$lib/descarga";
  import { partesRuta, rutaLegible } from "$lib/detalle";
  import { bytes, fechaCorta } from "$lib/formato";
  import { mandarOrden } from "$lib/ordenar";
  import { ESTADO_ORDEN } from "$lib/salud";
  import { textoRestaurar } from "$lib/textosEquipo";
  import type { Orden } from "$lib/tipos";

  let { acceso, version, cuando, ruta, tamano }: { acceso: AccesoRepo; version: string; cuando?: string | null; ruta: string; tamano?: number | null } = $props();

  const MAX_RELEVO = 500 * 1000 * 1000;
  let ocupado = $state(false);
  let paso = $state("");
  let error = $state("");
  let orden = $state<Orden | null>(null);
  let hecho = $state<string | null>(null);
  const parar = new AbortController();
  let vigilar: ReturnType<typeof setInterval> | undefined;
  onDestroy(() => {
    parar.abort();
    clearInterval(vigilar);
  });

  const nombre = $derived(partesRuta(ruta).nombre || "archivo");

  async function junto() {
    const secreto = acceso.secretoRepo();
    if (!secreto) return void (error = "Vuelve a abrir el repositorio con su contraseña.");
    error = "";
    ocupado = true;
    try {
      orden = await mandarOrden({
        cliente: acceso.cliente,
        equipo: acceso.equipo,
        tipo: "restaurar",
        cuerpo: { repo: acceso.repo, version, rutas: [ruta], destino: "junto", reemplazar: false },
        secretos: { repo: secreto },
        alPaso: (t) => (paso = t),
      });
      paso = "Enviado: el equipo lo restaurará junto al original…";
      const id = orden.id;
      vigilar = setInterval(async () => {
        try {
          const o = (await enFondo(() => api.ordenesEquipo(acceso.cliente.id, acceso.equipo.id, 10))).find((x) => x.id === id);
          if (o) orden = o;
          if (o && ["hecha", "fallida", "rechazada", "cancelada", "caducada"].includes(o.estado)) {
            clearInterval(vigilar);
            ocupado = false;
            if (o.estado === "hecha") hecho = o.mensaje ? textoRestaurar(o.mensaje) : "Restaurado junto al original.";
            else error = o.mensaje ? textoRestaurar(o.mensaje) : "El equipo no pudo restaurarlo.";
          }
        } catch {
          /* se reintenta */
        }
      }, 1500);
    } catch (e) {
      error = (e as Error).message;
      ocupado = false;
    }
  }

  async function descargar() {
    const secreto = acceso.secretoRepo();
    if (!secreto) return void (error = "Vuelve a abrir el repositorio con su contraseña.");
    error = "";
    ocupado = true;
    const relevo = crypto.randomUUID();
    const clave = aleatorio(32);
    try {
      orden = await mandarOrden({
        cliente: acceso.cliente,
        equipo: acceso.equipo,
        tipo: "descargar",
        cuerpo: { repo: acceso.repo, version, rutas: [ruta], formato: "archivo", relevo: { id: relevo, clave: aB64(clave) } },
        secretos: { repo: secreto },
        relevo: { id: relevo, max_bytes: MAX_RELEVO },
        alPaso: (t) => (paso = t),
      });
      const blob = await bajarRelevo({
        cliente: acceso.cliente.id,
        relevo,
        clave,
        signal: parar.signal,
        alProgreso: (p) => (paso = p.fase === "bajando" ? `Bajando… ${bytes(p.bajados)}` : p.fase === "subiendo" ? `El equipo lo prepara… ${bytes(p.bytes)}` : "Esperando al equipo…"),
      });
      await guardar(blob, nombre);
      hecho = "Descargado.";
    } catch (e) {
      if (!parar.signal.aborted) error = (e as Error).message;
    } finally {
      borrar(clave);
      ocupado = false;
    }
  }
</script>

<div class="restaurar" aria-live="polite">
  <p class="que">
    Restaurar <strong class="selectable">{nombre}</strong>{#if tamano != null}{" "}<span class="faint num">({bytes(tamano)})</span>{/if} tal como estaba en la versión <span class="pastilla mono" title={version}>{version.slice(0, 8)}</span>{#if cuando}{" "}<span class="faint">({fechaCorta(cuando)})</span>{/if}.
  </p>
  <p class="faint pequeno">Junto al original: en una carpeta «Restaurado …» al lado, en {acceso.equipo.nombre}, sin pisar nada. Estaba en <span class="selectable">{rutaLegible(partesRuta(ruta).carpeta)}</span></p>
  {#if hecho}
    <p class="ok"><CircleCheck size={15} />{hecho}</p>
  {:else}
    <div class="acciones">
      <button class="btn btn-sm btn-primary" disabled={ocupado} onclick={junto}><History size={14} />Junto al original</button>
      <button class="btn btn-sm" disabled={ocupado} onclick={descargar}><Download size={14} />Descargar</button>
      {#if ocupado}<span class="espera" role="status"><LoaderCircle size={14} class="spin" />{paso || (orden ? ESTADO_ORDEN[orden.estado] : "Preparando…")}</span>{/if}
    </div>
  {/if}
  {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={15} /><p>{error}</p></div>{/if}
</div>

<style>
  .restaurar {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: var(--sp-3);
    background: var(--surface-2, var(--bg-subtle));
    border-radius: var(--radius);
  }
  .que,
  .pequeno,
  .ok {
    margin: 0;
    font-size: var(--fs-sm);
    overflow-wrap: anywhere;
  }
  .pequeno {
    font-size: var(--fs-xs);
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .espera,
  .ok {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .ok {
    color: var(--ok);
    font-size: var(--fs-sm);
  }
</style>
