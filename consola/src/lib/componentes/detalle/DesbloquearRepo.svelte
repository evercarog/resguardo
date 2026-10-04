<script lang="ts">
  // Los nombres de los archivos solo se ven con la contraseña del repositorio
  // (los lee el equipo y llegan cifrados a este navegador). Se pide aquí una
  // vez por página; luego valen para todos los detalles hasta salir.
  import { onMount } from "svelte";
  import { LoaderCircle, LockKeyhole, TriangleAlert } from "@lucide/svelte";
  import type { AccesoRepo } from "$lib/accesoRepo.svelte";
  import Ayuda from "../Ayuda.svelte";
  import AlertaLlaves from "../AlertaLlaves.svelte";
  import CampoClave from "../CampoClave.svelte";

  let { acceso, nombreRepo, para }: { acceso: AccesoRepo; nombreRepo: string; /** «ver qué cambió», «ver lo que más ocupa»… */ para: string } = $props();

  let contrasena = $state("");
  let claveAdmin = $state("");
  onMount(() => {
    void acceso.comprobar();
    return () => (contrasena = claveAdmin = "");
  });

  async function enviar(e: SubmitEvent) {
    e.preventDefault();
    try {
      await acceso.abrir(contrasena, acceso.pideAdmin ? claveAdmin : undefined);
      contrasena = claveAdmin = "";
    } catch {
      /* el error queda en `acceso.error` */
    }
  }
</script>

<form class="form desbloquear" onsubmit={enviar}>
  <p class="intro"><LockKeyhole size={15} />Para {para} hace falta la contraseña de «{nombreRepo}». Lo lee {acceso.equipo.nombre} y llega cifrado solo a este navegador: el servidor no ve los nombres de los archivos. Se olvida al salir de esta página.</p>
  <CampoClave requerido id="detalle-clave-repo" etiqueta="Contraseña del repositorio" bind:value={contrasena} autofocus ayuda="Está en su kit de recuperación (el que se imprimió al crearlo).">
    {#snippet extra()}<Ayuda id="contrasena-repo" />{/snippet}
  </CampoClave>
  {#if acceso.pideAdmin}
    <p class="faint pequeno">Es la primera vez que este navegador manda una contraseña a {acceso.equipo.nombre}: con la clave de administración se comprueba que sus llaves son las auténticas.</p>
    <CampoClave requerido id="detalle-clave-admin" etiqueta="Clave de administración" bind:value={claveAdmin}>
      {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
    </CampoClave>
  {/if}
  {#if acceso.llavesCambiadas}<AlertaLlaves equipo={acceso.equipo} cliente={acceso.cliente.id} />{/if}
  {#if !acceso.equipo.conectado}<div class="notice notice-warn"><TriangleAlert size={16} /><p>{acceso.equipo.nombre} no está conectado ahora: responderá cuando vuelva.</p></div>{/if}
  {#if acceso.error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{acceso.error}</p></div>{/if}
  <div class="fin">
    {#if acceso.abriendo}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{acceso.paso || "Abriendo…"}</span>{/if}
    <button class="btn btn-primary" disabled={!contrasena || (acceso.pideAdmin && !claveAdmin) || acceso.abriendo || acceso.llavesCambiadas}>
      <LockKeyhole size={15} />{acceso.pideAdmin ? "Abrir con las dos claves" : "Abrir"}
    </button>
  </div>
</form>

<style>
  .desbloquear {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-4);
    background: var(--surface-2, var(--bg-subtle));
    border-radius: var(--radius-lg);
  }
  .intro {
    display: flex;
    gap: 8px;
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .intro :global(svg) {
    flex: none;
    margin-top: 2px;
  }
  .pequeno {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .fin {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: var(--sp-3);
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
</style>
