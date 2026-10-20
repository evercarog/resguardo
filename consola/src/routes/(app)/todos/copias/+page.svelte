<script lang="ts">
  // «Todos los clientes» → Copias (plan 0.8.0, D4, 14xx): todas las copias de todos
  // los clientes de la cuenta con la lógica de «Copias» de cada cliente
  // (lib/copiasCliente.ts), con su cliente, «Necesitan atención» primero y los pasos
  // de su camino (espejos y copias derivadas). Cada fila lleva a su cliente.
  import { ChevronRight, FolderSync, Search, X } from "@lucide/svelte";
  import { reloj } from "$lib/estado.svelte";
  import { horarioEnFrase, plural } from "$lib/formato";
  import { cuentaCopias, ESTADOS_FILTRO, filtrarCopias, ordenarCopias, type EstadoFiltro } from "$lib/copiasCliente";
  import { copiasTodos } from "$lib/todosSecciones";
  import { cargarTodos, todos } from "$lib/todos.svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import CaminoCopia from "$lib/componentes/CaminoCopia.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import ErrorCarga from "$lib/componentes/ErrorCarga.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import MarcaCliente from "$lib/componentes/MarcaCliente.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import FiltroCliente from "$lib/componentes/todos/FiltroCliente.svelte";

  let cliente = $state("");
  let buscar = $state("");
  let estado = $state<EstadoFiltro>("todos");
  const filas = $derived(copiasTodos(todos.clientes, reloj.ahora));
  const delCliente = $derived(cliente ? filas.filter((x) => x.cliente.id === cliente) : filas);
  // La búsqueda, como en cada cliente y también por el nombre del cliente.
  const vistas = $derived(
    ordenarCopias(filtrarCopias(delCliente, { estado }), "estado").filter((x) => !buscar.trim() || x.cliente.nombre.toLowerCase().includes(buscar.trim().toLowerCase()) || filtrarCopias([x], { buscar }).length > 0),
  );
  const cuenta = $derived(cuentaCopias(delCliente));
  const TEXTO_ULTIMA = { ok: "correcta", sin_cambios: "sin cambios", aviso: "con avisos", fallo: "falló" } as const;
  const href = (x: (typeof filas)[number]) => `/c/${x.cliente.id}/equipos/${x.equipo.id}/copias/${encodeURIComponent(x.copia.id)}`;
</script>

<svelte:head><title>Copias · Todos los clientes · Resguardo Server</title></svelte:head>

<div class="page ancha">
  <CabeceraPagina titulo="Copias" icono={FolderSync} migas={[{ texto: "Todos los clientes", href: "/todos" }, { texto: "Copias" }]} resumen={todos.cargado ? `${plural(cuenta.total, "copia", "copias")} · ${cuenta.alDia} al día · ${plural(cuenta.atencion, "necesita atención", "necesitan atención")}` : undefined} />

  {#if todos.error && !todos.cargado}
    <ErrorCarga que="las copias de todos los clientes" error={todos.error} reintentar={() => cargarTodos()} />
  {:else if !todos.cargado}
    <Esqueleto forma="filas" n={6} etiqueta="Cargando las copias de todos los clientes…" />
  {:else if !filas.length}
    <div class="card"><Vacio icono={FolderSync} ilustracion="sin-repos" titulo="Todavía no hay copias" texto="Cuando un equipo de cualquier cliente tenga copias, salen aquí." /></div>
  {:else}
    <FiltroCliente seccion="copias" bind:cliente bind:buscar placeholder="Buscar copia, equipo, repositorio o cliente" />
    <div class="segmented inline" role="group" aria-label="Estado">
      {#each ESTADOS_FILTRO as f (f.id)}<button class:on={estado === f.id} aria-pressed={estado === f.id} onclick={() => (estado = f.id)}>{f.texto}</button>{/each}
    </div>
    {#if vistas.length}
      <p class="sr-only" aria-live="polite">{plural(vistas.length, "copia", "copias")}</p>
      <div class="card p-0">
        {#each vistas as x (x.clave)}
          <div class="fila-copia">
            <div class="principal">
              <span class="cli"><MarcaCliente nombre={x.cliente.nombre} marca={x.cliente.marca} tam={16} /><a class="link-suave" href="/c/{x.cliente.id}/copias">{x.cliente.nombre}</a></span>
              <a class="nombre" href={href(x)}><strong>{x.copia.nombre}</strong><ChevronRight size={14} aria-hidden="true" /></a>
              <span class="sub"><a class="link-suave" href="/c/{x.cliente.id}/equipos/{x.equipo.id}">{x.equipo.nombre}</a>{" · "}{x.repo?.nombre ?? x.copia.repo}{x.destino ? ` → ${x.destino.nombre}` : ""}</span>
              {#if x.pasos.length}<div class="pasos"><CaminoCopia eslabones={x.pasos} ahora={reloj.ahora} forma="lineas" etiqueta="Después de «{x.copia.nombre}»" /></div>{/if}
              <span class="sub faint">{horarioEnFrase(x.copia.horario)}</span>
              {#if x.vuelta?.resultado === "fallo" && x.vuelta.mensaje}<span class="sub fallo">{x.vuelta.mensaje}</span>{/if}
            </div>
            <div class="estado">
              <Chip tono={x.estado.tono} texto={x.estado.texto} />
              {#if x.estado.tono === "ok" && x.pasoMal}<Chip pequeno tono="warn" texto="Un paso mal" />{/if}
            </div>
            <div class="cuando num">
              <span>{#if x.vuelta}Última <Tiempo iso={x.vuelta.cuando} /><span class="faint">{" · "}{TEXTO_ULTIMA[x.vuelta.resultado]}</span>{:else}<span class="faint">Ninguna todavía</span>{/if}</span>
              <span class="faint">{#if x.proxima && Date.parse(x.proxima) > reloj.ahora}Próxima <Tiempo iso={x.proxima} />{:else if x.copia.activa === false}Desactivada{:else}Sin próxima a la vista{/if}</span>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <div class="card">
        <Vacio icono={Search} ilustracion="sin-resultados" titulo="Ninguna copia coincide" texto="Prueba con otro nombre o quita los filtros.">
          <button
            class="btn btn-sm"
            onclick={() => {
              buscar = "";
              estado = "todos";
              cliente = "";
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
  .fila-copia {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto 200px;
    gap: var(--sp-4);
    align-items: start;
    padding: 12px var(--sp-4);
    border-top: 1px solid var(--border);
  }
  .fila-copia:first-child {
    border-top: none;
  }
  .principal {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .cli {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .nombre {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    color: var(--text-1);
  }
  .sub {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .fallo {
    color: var(--bad);
  }
  .pasos {
    margin: 2px 0;
  }
  .estado {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
  }
  .cuando {
    display: flex;
    flex-direction: column;
    font-size: var(--fs-sm);
    text-align: right;
  }
  @media (max-width: 760px) {
    .fila-copia {
      grid-template-columns: 1fr;
    }
    .estado,
    .cuando {
      align-items: flex-start;
      text-align: left;
    }
  }
</style>
