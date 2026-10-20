<script lang="ts">
  // «Todos los clientes» → Órdenes en espera (plan 0.8.0, D4, 14xx): cualquier orden
  // que reduce la protección y espera su turno, en cualquier cliente, a la vista y
  // cancelable (importante para la seguridad). Las de esta consola, de
  // GET /api/panel/ordenes (se cancelan en su servidor); las de otras consolas, del
  // resumen de cada equipo (se cancelan con `cancelar_espera`, inofensiva y sin clave).
  import { onMount } from "svelte";
  import { Check, Clock, MonitorCheck, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { puede, reloj } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { cuentaAtras, fechaLarga, plural } from "$lib/formato";
  import { nombreOrden } from "$lib/salud";
  import { admiteCancelarEspera } from "$lib/espera";
  import { mandarOrden } from "$lib/ordenar";
  import { tip } from "$lib/tooltip";
  import { ordenesDeTodos } from "$lib/todosSecciones.svelte";
  import { esperaTodos, type EsperaTodos } from "$lib/todosSecciones";
  import { cargarTodos, todos } from "$lib/todos.svelte";
  import { seguirTodos } from "$lib/todosVivo";
  import type { Orden } from "$lib/tipos";
  import type { PanelDe } from "$lib/global";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import ErrorCarga from "$lib/componentes/ErrorCarga.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import MarcaCliente from "$lib/componentes/MarcaCliente.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import FiltroCliente from "$lib/componentes/todos/FiltroCliente.svelte";

  let propias = $state<PanelDe<Orden[]> | null>(null);
  let error = $state("");
  async function cargar() {
    try {
      propias = await ordenesDeTodos();
      error = "";
    } catch (e) {
      error = (e as Error).message;
    }
  }
  onMount(() => seguirTodos(cargar, 20_000));

  let cliente = $state("");
  let buscar = $state("");
  const filas = $derived(esperaTodos(todos.clientes, propias, reloj.ahora));
  const vistas = $derived(
    filas.filter((o) => {
      if (cliente && o.cliente.id !== cliente) return false;
      const t = buscar.trim().toLowerCase();
      return !t || [o.descripcion ?? nombreOrden(o.tipo), o.equipoNombre, o.cliente.nombre, o.otraConsola ?? "", o.por ?? ""].join(" ").toLowerCase().includes(t);
    }),
  );
  const equipoDe = (o: EsperaTodos) => todos.clientes.find((c) => c.id === o.cliente.id)?.equipos.find((e) => e.id === o.equipo);
  const puedeCancelar = (o: EsperaTodos) => puede.ordenar(o.cliente.rol) && (o.cancelar === "servidor" || admiteCancelarEspera(equipoDe(o)));

  let confirmar = $state<string | null>(null);
  let cancelando = $state<string | null>(null);
  async function cancelar(o: EsperaTodos) {
    cancelando = o.clave;
    try {
      if (o.cancelar === "servidor") {
        await api.cancelarOrden(o.cliente.id, o.id);
        avisar(`Cancelada: «${nombreOrden(o.tipo)}» en ${o.equipoNombre} (${o.cliente.nombre}).`);
      } else {
        const equipo = equipoDe(o);
        if (!equipo) return;
        // La orden la sella el navegador con lo del cliente (su sal): se pide entero.
        const c = await api.cliente(o.cliente.id);
        await mandarOrden({ cliente: c, equipo, tipo: "cancelar_espera", cuerpo: { id: o.id } });
        avisar(`Pedido a ${o.equipoNombre} (${o.cliente.nombre}): cancelar «${nombreOrden(o.tipo)}», de ${o.otraConsola}. Se quitará de la lista en cuanto el equipo conteste.`);
      }
      confirmar = null;
      await Promise.all([cargar(), cargarTodos()]);
    } catch (e) {
      fallo(e);
    } finally {
      cancelando = null;
    }
  }
</script>

<svelte:head><title>Órdenes en espera · Todos los clientes · Resguardo Server</title></svelte:head>

<div class="page ancha">
  <CabeceraPagina titulo="Órdenes en espera" icono={Clock} migas={[{ texto: "Todos los clientes", href: "/todos" }, { texto: "Órdenes en espera" }]}>
    {#snippet detalle()}Lo que reduce la protección y espera su turno, en cualquier cliente. Si no esperabas alguna, cancélala y cambia la clave de administración de ese cliente. <Ayuda id="espera" />{/snippet}
  </CabeceraPagina>

  {#if error && !propias}
    <ErrorCarga que="las órdenes en espera" {error} reintentar={cargar} />
  {:else if !propias || !todos.cargado}
    <Esqueleto forma="filas" n={3} etiqueta="Cargando las órdenes en espera…" />
  {:else if !filas.length}
    <div class="card"><Vacio icono={Clock} ilustracion="todo-en-orden" titulo="Ninguna orden en espera" texto="Ningún cliente tiene órdenes esperando su turno." /></div>
  {:else}
    <FiltroCliente seccion="ordenes" bind:cliente bind:buscar placeholder="Buscar orden, equipo o cliente" />
    <section class="card pendientes" aria-labelledby="t-pend">
      <h2 id="t-pend" class="section-title"><Clock size={16} />{plural(vistas.length, "orden esperando su turno", "órdenes esperando su turno")}</h2>
      <ul>
        {#each vistas as o (o.clave)}
          <li>
            <span class="texto">
              <span class="quien"><MarcaCliente nombre={o.cliente.nombre} marca={o.cliente.marca} tam={16} /><a class="link-suave" href="/c/{o.cliente.id}/ordenes">{o.cliente.nombre}</a></span>
              <span><strong>{o.descripcion ?? nombreOrden(o.tipo)}</strong> en <a class="link-suave" href="/c/{o.cliente.id}/equipos/{o.equipo}">{o.equipoNombre}</a></span>
              <span class="faint">
                {#if o.otraConsola}Desde otra consola: <strong>{o.otraConsola}</strong>{#if o.por}{" · "}pedida por {o.por}{/if}{:else}Pedida por {o.por ?? "?"}{/if}
                {#if o.aplica}{" · "}se aplica en <time datetime={o.aplica} use:tip={fechaLarga(o.aplica)}>{cuentaAtras(o.aplica, reloj.ahora)}</time>{/if}
                {#if o.clavesComprobadas}{" · "}<span class="ok"><Check size={12} />claves comprobadas</span>{/if}
                {#if o.aprobable}{" · "}<span><MonitorCheck size={12} />se puede aprobar en el equipo</span>{/if}
              </span>
            </span>
            {#if puedeCancelar(o)}
              {#if confirmar === o.clave}
                <span class="conf">
                  <button class="btn btn-sm btn-ghost" onclick={() => (confirmar = null)}>No</button>
                  <button class="btn btn-sm btn-danger" disabled={cancelando === o.clave} onclick={() => cancelar(o)}>{cancelando === o.clave ? "Cancelando…" : "Sí, cancelar"}</button>
                </span>
              {:else}
                <button class="btn btn-sm" onclick={() => (confirmar = o.clave)}><X size={14} />Cancelar</button>
              {/if}
            {/if}
          </li>
        {/each}
      </ul>
    </section>
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .pendientes {
    padding: var(--sp-4) var(--sp-5);
    border-color: color-mix(in srgb, var(--warn) 30%, var(--border));
    background: color-mix(in srgb, var(--warn) 4%, var(--surface));
  }
  .section-title {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .section-title :global(svg) {
    color: var(--warn);
  }
  ul {
    list-style: none;
    margin: var(--sp-3) 0 0;
    padding: 0;
  }
  li {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 10px 0;
    border-top: 1px solid var(--border);
  }
  .texto {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .quien {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .faint {
    font-size: var(--fs-sm);
  }
  .faint :global(svg) {
    vertical-align: -2px;
  }
  .ok {
    color: var(--ok);
  }
  .conf {
    display: inline-flex;
    gap: 4px;
  }
</style>
