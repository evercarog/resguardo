<script lang="ts">
  // «Todos los clientes» → Equipos (plan 0.8.0, D4, 14xx): los equipos de todos los
  // clientes de la cuenta, con su cliente, su sede (D1) y su sistema exacto (14uu); lo
  // que necesita atención primero. Cada fila lleva a su ficha en su cliente.
  import { Building2, Monitor, Search, X } from "@lucide/svelte";
  import { reloj } from "$lib/estado.svelte";
  import { plural } from "$lib/formato";
  import { sinTildes } from "$lib/global";
  import { equiposTodos } from "$lib/todosSecciones";
  import { cargarTodos, todos } from "$lib/todos.svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import ErrorCarga from "$lib/componentes/ErrorCarga.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import MarcaCliente from "$lib/componentes/MarcaCliente.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import FiltroCliente from "$lib/componentes/todos/FiltroCliente.svelte";

  let cliente = $state("");
  let buscar = $state("");
  let soloAtencion = $state(false);
  let sistema = $state("");
  const filas = $derived(equiposTodos(todos.clientes, reloj.ahora));
  const sistemas = $derived([...new Set(filas.map((x) => x.grupoSistema))].sort((a, b) => a.localeCompare(b, "es", { numeric: true })));
  const vistas = $derived(
    filas.filter((x) => {
      if (cliente && x.cliente.id !== cliente) return false;
      if (soloAtencion && x.tono !== "bad" && x.tono !== "warn") return false;
      if (sistema && x.grupoSistema !== sistema) return false;
      const p = sinTildes(buscar).split(/\s+/).filter(Boolean);
      const t = sinTildes([x.equipo.nombre, x.cliente.nombre, x.sede ?? "", x.sistema, ...(x.equipo.etiquetas ?? [])].join(" "));
      return p.every((w) => t.includes(w));
    }),
  );
  const atencion = $derived(filas.filter((x) => x.tono === "bad" || x.tono === "warn").length);
  const ultimaDe = (x: (typeof filas)[number]) =>
    (x.equipo.resumen?.copias ?? [])
      .map((k) => k.ultima?.cuando)
      .filter((y): y is string => !!y)
      .sort()
      .at(-1) ?? null;
</script>

<svelte:head><title>Equipos · Todos los clientes · Resguardo Server</title></svelte:head>

<div class="page ancha">
  <CabeceraPagina titulo="Equipos" icono={Monitor} migas={[{ texto: "Todos los clientes", href: "/todos" }, { texto: "Equipos" }]} resumen={todos.cargado ? `${plural(filas.length, "equipo", "equipos")} en ${plural(todos.clientes.length, "cliente", "clientes")} · ${plural(atencion, "necesita atención", "necesitan atención")}` : undefined} />

  {#if todos.error && !todos.cargado}
    <ErrorCarga que="los equipos de todos los clientes" error={todos.error} reintentar={() => cargarTodos()} />
  {:else if !todos.cargado}
    <Esqueleto forma="filas" n={6} etiqueta="Cargando los equipos de todos los clientes…" />
  {:else if !filas.length}
    <div class="card">
      <Vacio icono={Monitor} ilustracion="sin-equipos" titulo="Todavía no hay equipos" texto="Cuando un cliente tenga equipos, salen aquí. Se añaden desde cada cliente.">
        <a class="btn btn-primary" href="/clientes">Ver los clientes</a>
      </Vacio>
    </div>
  {:else}
    <FiltroCliente seccion="equipos" bind:cliente bind:buscar placeholder="Buscar equipo, cliente, sede o sistema" />
    <div class="filtros">
      <label class="check"><input type="checkbox" bind:checked={soloAtencion} />Solo los que necesitan atención</label>
      {#if sistemas.length > 1}
        <select class="input input-sm" bind:value={sistema} aria-label="Filtrar por sistema">
          <option value="">Todos los sistemas</option>
          {#each sistemas as s (s)}<option value={s}>{s}</option>{/each}
        </select>
      {/if}
    </div>
    {#if vistas.length}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div class="card p-0 desplazable" tabindex="0" role="region" aria-label="Equipos (se desliza a los lados)">
        <table class="tabla">
          <caption class="sr-only">Equipos de todos los clientes</caption>
          <thead>
            <tr><th scope="col">Equipo</th><th scope="col">Cliente</th><th scope="col">Sede</th><th scope="col">Sistema</th><th scope="col">Última copia</th><th scope="col">Estado</th></tr>
          </thead>
          <tbody>
            {#each vistas as x (x.clave)}
              {@const ultima = ultimaDe(x)}
              <tr>
                <td><a href="/c/{x.cliente.id}/equipos/{x.equipo.id}"><strong>{x.equipo.nombre}</strong></a></td>
                <td><span class="cli"><MarcaCliente nombre={x.cliente.nombre} marca={x.cliente.marca} tam={16} /><a class="link-suave" href="/c/{x.cliente.id}/equipos">{x.cliente.nombre}</a></span></td>
                <td>{#if x.sede}<span class="sede"><Building2 size={13} aria-hidden="true" />{x.sede}{#if !x.sedeAsignada}<span class="faint"> (sin asignar)</span>{/if}</span>{:else}<span class="faint">—</span>{/if}</td>
                <td>{x.sistema}</td>
                <td class="num">{#if ultima}<Tiempo iso={ultima} />{:else}<span class="faint">Sin copias</span>{/if}</td>
                <td><Chip pequeno tono={x.tono} texto={x.estado} /></td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else}
      <div class="card">
        <Vacio icono={Search} ilustracion="sin-resultados" titulo="Ningún equipo coincide" texto="Prueba con otro nombre o quita los filtros.">
          <button
            class="btn btn-sm"
            onclick={() => {
              buscar = "";
              cliente = "";
              sistema = "";
              soloAtencion = false;
            }}><X size={14} />Quitar los filtros</button
          >
        </Vacio>
      </div>
    {/if}
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .filtros {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-3);
    align-items: center;
  }
  .filtros select {
    width: auto;
  }
  .check {
    display: inline-flex;
    gap: 8px;
    align-items: center;
    font-size: var(--fs-sm);
  }
  .cli,
  .sede {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .sede :global(svg) {
    color: var(--text-3);
  }
</style>
