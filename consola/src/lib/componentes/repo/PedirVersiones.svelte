<script lang="ts">
  // 14d: el resumen del equipo trae las versiones de los últimos 60 días. Para
  // ver todas, se piden al equipo como en Restaurar: una sesión cifrada
  // `explorar` abierta con la contraseña del repositorio (la lee el equipo y
  // llega cifrado solo a este navegador). Se cierra en cuanto llegan; lo traído
  // vale mientras la página esté abierta.
  import { onDestroy, untrack } from "svelte";
  import { History, LoaderCircle, TriangleAlert, X } from "@lucide/svelte";
  import { AccesoRepo } from "$lib/accesoRepo.svelte";
  import type { VersionSesion } from "$lib/historial";
  import type { MensajeEquipo } from "$lib/sesion";
  import type { Cliente, Equipo, RepositorioResumen } from "$lib/tipos";
  import { numero } from "$lib/formato";
  import DesbloquearRepo from "../detalle/DesbloquearRepo.svelte";

  let {
    cliente,
    equipo,
    repo,
    conocidas,
    total,
    alTraer,
  }: {
    cliente: Cliente;
    equipo: Equipo;
    repo: RepositorioResumen;
    /** Las que ya se ven (las del resumen). */
    conocidas: number;
    total: number;
    alTraer: (vs: VersionSesion[]) => void;
  } = $props();

  let acceso = $state<AccesoRepo | null>(null);
  let trayendo = $state(false);
  let error = $state("");

  function empezar() {
    error = "";
    acceso?.cerrar();
    acceso = new AccesoRepo(cliente, equipo, repo.id);
  }
  function cancelar() {
    acceso?.cerrar();
    acceso = null;
  }
  // Abierta la sesión, se piden las versiones una vez y se cierra.
  $effect(() => {
    const a = acceso;
    if (!a?.abierta) return;
    untrack(() => void traer(a));
  });
  async function traer(a: AccesoRepo) {
    if (trayendo) return;
    trayendo = true;
    try {
      const r = await a.pedir<MensajeEquipo & { versiones?: VersionSesion[] }>("versiones", { repo: repo.id });
      alTraer(Array.isArray(r.versiones) ? r.versiones : []);
      a.cerrar();
      acceso = null;
    } catch (err) {
      error = (err as Error).message;
      a.cerrar();
      acceso = null;
    } finally {
      trayendo = false;
    }
  }
  onDestroy(() => acceso?.cerrar());
</script>

<div class="pedir">
  {#if !acceso}
    <p class="faint linea">
      <span>Aquí se ven las de los últimos 60 días ({numero(conocidas)} de {numero(total)}), las que trae el resumen del equipo.</span>
      <button type="button" class="btn btn-sm" onclick={empezar}><History size={14} />Pedir todas a {equipo.nombre}</button>
    </p>
    {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>No se pudieron traer: {error}</p></div>{/if}
  {:else if trayendo || acceso.abierta}
    <p class="faint linea" role="status"><LoaderCircle size={14} class="spin" />Leyendo todas las versiones de «{repo.nombre}» en {equipo.nombre}…</p>
  {:else}
    <div class="desbloquear">
      <button type="button" class="icon-btn cerrar" aria-label="No pedirlas" onclick={cancelar}><X size={15} /></button>
      <DesbloquearRepo {acceso} nombreRepo={repo.nombre} para="ver todas sus versiones" />
    </div>
  {/if}
</div>

<style>
  .pedir {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .linea {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    margin: 0;
    font-size: var(--fs-sm);
  }
  .desbloquear {
    position: relative;
  }
  .cerrar {
    position: absolute;
    top: 6px;
    right: 6px;
  }
</style>
