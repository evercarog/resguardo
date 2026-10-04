<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { ArrowRightLeft, BellRing, Building2, ChevronRight, Gauge, Monitor, Plus } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { app, cargarClientes, NOMBRE_ROL } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { plural } from "$lib/formato";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import RecibirCliente from "$lib/componentes/RecibirCliente.svelte";
  import DarCodigoConexion from "$lib/componentes/DarCodigoConexion.svelte";
  import MarcaCliente from "$lib/componentes/MarcaCliente.svelte";
  import CampoObservaciones from "$lib/componentes/notas/CampoObservaciones.svelte";
  import { errorTextoNota, guardarObservacion } from "$lib/notas.svelte";

  let cargado = $state(false);
  let nuevo = $state(page.url.searchParams.get("nuevo") === "1");
  let recibir = $state(false);
  let tambien = $state(false);
  let nombre = $state("");
  let espera = $state(24);
  let observaciones = $state("");
  let ocupado = $state(false);
  let error = $state("");

  onMount(async () => {
    await cargarClientes();
    cargado = true;
  });

  async function crear(e: SubmitEvent) {
    e.preventDefault();
    ocupado = true;
    error = "";
    try {
      const c = await api.crearCliente(nombre.trim(), espera);
      if (observaciones.trim()) await guardarObservacion(c.id, "cliente", c.id, observaciones).catch(() => avisar("El cliente se creó, pero sus observaciones no se guardaron.", "warn"));
      await cargarClientes();
      nuevo = false;
      avisar(`Cliente «${c.nombre}» creado. Empieza añadiendo su primer equipo.`);
      await goto(`/c/${c.id}/emparejar`);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      ocupado = false;
    }
  }
</script>

<svelte:head><title>Clientes · Resguardo Server</title></svelte:head>

<div class="page">
  <div class="page-top">
    <div>
      <h1>Clientes</h1>
      <p>Cada cliente tiene sus equipos, sus personas y su propia clave de administración.</p>
    </div>
    {#if app.cuenta?.superusuario}
      <div class="page-actions">
        <a class="btn" href="/servidor/clientes"><Gauge size={16} />Clientes del servidor</a>
        <button class="btn" onclick={() => (recibir = true)}><ArrowRightLeft size={16} />Recibir un cliente</button>
        <button class="btn btn-primary" onclick={() => (nuevo = true)}><Plus size={16} />Nuevo cliente</button>
      </div>
    {/if}
  </div>

  {#if !cargado}
    <Cargando />
  {:else if !app.clientes.length}
    <div class="card">
      <Vacio icono={Building2} ilustracion="bienvenida" titulo="Todavía no hay clientes" texto={app.cuenta?.superusuario ? "Crea el primero: por ejemplo, tu empresa o la de un cliente." : "Pide a quien administra el servidor que te invite a un cliente."}>
        {#if app.cuenta?.superusuario}<button class="btn btn-primary" onclick={() => (nuevo = true)}><Plus size={16} />Crear el primer cliente</button>{/if}
      </Vacio>
    </div>
  {:else}
    <div class="rejilla">
      {#each app.clientes as c (c.id)}
        <a class="card cliente" href="/c/{c.id}">
          <MarcaCliente nombre={c.nombre} marca={c.marca} tam={40} />
          <span class="texto">
            <strong title={c.nombre}>{c.nombre}</strong>
            <span class="meta">
              <span>{NOMBRE_ROL[c.rol]}</span>
              <span class="equipos"><Monitor size={13} />{plural(c.equipos, "equipo", "equipos")}</span>
              {#if c.avisos}<span class="badge badge-sm tone-bad"><BellRing size={11} />{plural(c.avisos, "aviso", "avisos")}</span>{/if}
            </span>
          </span>
          <ChevronRight size={16} />
        </a>
      {/each}
    </div>
  {/if}
</div>

{#if recibir}<RecibirCliente onclose={() => (recibir = false)} alTambien={() => ((recibir = false), (tambien = true))} />{/if}
{#if tambien}<DarCodigoConexion onclose={() => (tambien = false)} />{/if}

{#if nuevo}
  <Modal labelledby="t-nuevo" onclose={() => (nuevo = false)} width={480} dismissible={false}>
    <form class="form" onsubmit={crear}>
      <div class="dlg-title">
        <span class="ticon"><Building2 size={18} /></span>
        <div>
          <h2 id="t-nuevo">Nuevo cliente</h2>
          <p>Una empresa u oficina con sus equipos.</p>
        </div>
      </div>
      <div class="field">
        <label class="field-label" for="nombre-cliente">Nombre</label>
        <input id="nombre-cliente" class="input" bind:value={nombre} placeholder="Por ejemplo: Ferretería Altamar" aria-required="true" />
      </div>
      <div class="field">
        <div class="label-row"><label class="field-label" for="espera">Espera antes de borrar</label><Ayuda id="espera" /></div>
        <select id="espera" class="input" bind:value={espera}>
          <option value={1}>1 hora (mínimo)</option>
          <option value={12}>12 horas</option>
          <option value={24}>24 horas (recomendado)</option>
          <option value={48}>48 horas</option>
          <option value={72}>3 días</option>
          <option value={168}>7 días</option>
        </select>
        <span class="field-hint">Cuánto esperan las órdenes que pueden borrar copias antes de aplicarse. Mientras tanto se pueden cancelar.</span>
      </div>
      <CampoObservaciones id="obs-cliente" bind:valor={observaciones} filas={2} />
      {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
      <footer>
        <button type="button" class="btn btn-ghost" onclick={() => (nuevo = false)}>Cancelar</button>
        <button class="btn btn-primary" disabled={ocupado || !nombre.trim() || !!errorTextoNota(observaciones)}>{ocupado ? "Creando…" : "Crear cliente"}</button>
      </footer>
    </form>
  </Modal>
{/if}

<style>
  .cliente {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-4) var(--sp-5);
    color: inherit;
    text-decoration: none;
  }
  .cliente:hover {
    border-color: var(--border-strong);
  }
  .cliente > :global(svg) {
    color: var(--text-3);
  }
  .texto {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
    min-width: 0;
  }
  /* En tres columnas el nombre tiene poco sitio: hasta dos líneas antes de cortarse. */
  .texto strong {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
    overflow-wrap: anywhere;
    line-height: 1.3;
  }
  /* Debajo del nombre: el papel, los equipos y los avisos. */
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .equipos {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
</style>
