<script lang="ts">
  // Órdenes del cliente: las que esperan su turno (con «Cancelar») y las
  // últimas de todos los equipos, con su respuesta firmada, por páginas.
  import { onMount } from "svelte";
  import { ClipboardList, RefreshCw } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { enFondo } from "$lib/actividad.svelte";
  import { actual } from "$lib/estado.svelte";
  import { fallo } from "$lib/avisos.svelte";
  import type { Orden } from "$lib/tipos";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import ListaOrdenes from "$lib/componentes/ListaOrdenes.svelte";
  import Pendientes from "$lib/componentes/Pendientes.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";

  let ordenes = $state<Orden[] | null>(null);
  let siguiente = $state<string | null>(null);
  let filtro = $state<"todas" | "en_marcha" | "fallida" | "rechazada">("todas");
  let equipo = $state("");
  let cargando = $state(false);
  let vuelta = 0;

  async function cargar(mas = false) {
    if (!actual.id) return;
    const mia = ++vuelta;
    cargando = true;
    try {
      const r = await api.ordenesCliente(actual.id, { limite: 50, antes: mas ? siguiente : null, equipo: equipo || undefined, estado: filtro === "todas" ? undefined : filtro });
      if (mia !== vuelta) return;
      ordenes = mas ? [...(ordenes ?? []), ...r.ordenes] : r.ordenes;
      siguiente = r.siguiente;
    } catch (e) {
      if (mia === vuelta) fallo(e);
    } finally {
      if (mia === vuelta) cargando = false;
    }
  }
  $effect(() => {
    void actual.id;
    void filtro;
    void equipo;
    void cargar();
  });
  onMount(() => {
    const t = setInterval(() => document.visibilityState === "visible" && !siguiente && enFondo(cargar), 10_000);
    return () => clearInterval(t);
  });
</script>

<svelte:head><title>Órdenes · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <CabeceraPagina titulo="Órdenes" icono={ClipboardList} migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${actual.id}` }, { texto: "Órdenes" }]}>
    {#snippet detalle()}Lo que se ha pedido a los equipos y lo que han respondido. Cada respuesta va firmada por el equipo. <Ayuda id="firma-equipo" />{/snippet}
    {#snippet acciones()}
      <button class="btn btn-ghost" onclick={() => cargar()} disabled={cargando}><RefreshCw size={15} class={cargando ? "spin" : ""} />{cargando ? "Actualizando…" : "Actualizar"}</button>
    {/snippet}
  </CabeceraPagina>

  <Pendientes />

  <section>
    <div class="section-head filtros">
      <h2>Últimas órdenes</h2>
      <div class="controles">
        <select class="input equipo" bind:value={equipo} aria-label="Equipo">
          <option value="">Todos los equipos</option>
          {#each actual.equipos as e (e.id)}<option value={e.id}>{e.nombre}</option>{/each}
        </select>
        <div class="segmented inline" role="group" aria-label="Filtrar por estado">
          <button class:on={filtro === "todas"} aria-pressed={filtro === "todas"} onclick={() => (filtro = "todas")}>Todas</button>
          <button class:on={filtro === "en_marcha"} aria-pressed={filtro === "en_marcha"} onclick={() => (filtro = "en_marcha")}>En marcha</button>
          <button class:on={filtro === "fallida"} aria-pressed={filtro === "fallida"} onclick={() => (filtro = "fallida")}>Fallidas</button>
          <button class:on={filtro === "rechazada"} aria-pressed={filtro === "rechazada"} onclick={() => (filtro = "rechazada")}>Rechazadas</button>
        </div>
      </div>
    </div>
    {#if ordenes === null}
      <Cargando />
    {:else if ordenes.length}
      <div class="card p-0"><ListaOrdenes {ordenes} equipos={actual.equipos} mostrarEquipo alCambiar={() => cargar()} /></div>
      {#if siguiente}<button class="btn mas" disabled={cargando} onclick={() => cargar(true)}>{cargando ? "Cargando…" : "Ver más antiguas"}</button>{/if}
    {:else}
      <div class="card">
        <Vacio icono={ClipboardList} titulo={filtro === "todas" && !equipo ? "Todavía no hay órdenes" : "Nada por aquí"} texto={filtro === "todas" && !equipo ? "Cuando se pida algo a un equipo (copiar ahora, cambiar sus copias…), aparecerá aquí con su respuesta." : "Ninguna orden coincide con el filtro."}>
          {#if filtro !== "todas" || equipo}
            <button
              class="btn btn-sm"
              onclick={() => {
                filtro = "todas";
                equipo = "";
              }}>Ver todas las órdenes</button
            >
          {/if}
        </Vacio>
      </div>
    {/if}
  </section>
</div>

<style>
  .filtros {
    flex-wrap: wrap;
  }
  .controles {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .equipo {
    width: 200px;
  }
  .mas {
    align-self: center;
    margin-top: var(--sp-3);
  }
  section {
    display: flex;
    flex-direction: column;
  }
</style>
