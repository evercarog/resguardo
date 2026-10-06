<script lang="ts">
  import { tip } from "$lib/tooltip";
  import { CirclePause, CirclePlay, Clock, LayoutTemplate, ListChecks, Monitor, Play, Plus, Search, ShieldCheck, TriangleAlert, X } from "@lucide/svelte";
  import AccionesEnBloque from "$lib/componentes/AccionesEnBloque.svelte";
  import AplicarPlantillaEnBloque from "$lib/componentes/AplicarPlantillaEnBloque.svelte";
  import EtiquetaChip from "$lib/componentes/EtiquetaChip.svelte";
  import * as api from "$lib/api";
  import { enFondo } from "$lib/actividad.svelte";
  import { untrack } from "svelte";
  import { seguirCambios } from "$lib/vivo.svelte";
  import { actual, puede, reloj } from "$lib/estado.svelte";
  import { lista as lista_, plural } from "$lib/formato";
  import type { Preparado } from "$lib/tipos";
  import { PESO, saludEquipo, type Tono } from "$lib/salud";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import FilaEquipo from "$lib/componentes/FilaEquipo.svelte";
  import FiltroEtiquetas from "$lib/componentes/FiltroEtiquetas.svelte";
  import { agruparPorEtiqueta, ajusteDe, etiquetasDe, filtroEtiqueta, gruposPorEtiqueta, mismaEtiqueta, pasaFiltro } from "$lib/etiquetas.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import { cargarInformes, ultimos } from "$lib/informes.svelte";
  import { guardar, leer } from "$lib/recordar";

  let buscar = $state("");
  // El filtro se recuerda en este navegador (por cliente).
  const FILTROS = ["todos", "atencion", "almacen"] as const;
  let filtro = $state<(typeof FILTROS)[number]>("todos");
  $effect.pre(() => {
    if (actual.id) filtro = leer(`equipos.filtro.${actual.id}`, FILTROS, "todos");
  });
  function ponerFiltro(f: (typeof FILTROS)[number]) {
    filtro = f;
    guardar(`equipos.filtro.${actual.id}`, f === "todos" ? "" : f);
  }
  // Los informes, para los cuadros de 14 días de cada fila.
  $effect(() => {
    // Las cargas, sin seguir lo que leen (si no, cada respuesta podría volver a lanzar el efecto).
    const [cc, ids] = [actual.id, actual.equipos.map((e) => e.id)];
    untrack(() => void cargarInformes(cc, ids));
  });
  const informeDe = (id: string) => (ultimos.cliente === actual.id ? ultimos.porEquipo[id] : undefined);

  const lista = $derived(
    actual.equipos
      .filter((e) => {
        const t = buscar.trim().toLowerCase();
        if (t && !`${e.nombre} ${e.so} ${(e.etiquetas ?? []).join(" ")}`.toLowerCase().includes(t)) return false;
        if (!pasaFiltro(e, filtroEtiqueta.valor)) return false;
        const tono: Tono = saludEquipo(e, reloj.ahora).tono;
        if (filtro === "atencion") return tono === "bad" || tono === "warn";
        if (filtro === "almacen") return e.rol === "almacenamiento";
        return true;
      })
      .sort((a, b) => PESO[saludEquipo(a, reloj.ahora).tono] - PESO[saludEquipo(b, reloj.ahora).tono] || a.nombre.localeCompare(b.nombre)),
  );
  const conectados = $derived(actual.equipos.filter((e) => e.conectado).length);

  // Equipos preparados (instalador listo o línea de Linux, v1.17): los que esperan o ya se unieron.
  let preparados = $state<Preparado[]>([]);
  $effect(() => {
    const c = actual.id;
    if (!c || !puede.administrar(actual.cliente?.rol)) return;
    const cargar = () => api.preparados(c).then((x) => (preparados = x), () => {});
    untrack(() => void cargar());
    return untrack(() => seguirCambios(() => enFondo(cargar), { ms: 10_000, toca: (x) => x.t === "equipo" }));
  });
  // v1.4x: agrupados por etiqueta (un equipo con dos sale en las dos).
  const grupos = $derived(agruparPorEtiqueta.valor && !filtroEtiqueta.valor ? gruposPorEtiqueta(lista) : null);
  const hayEtiquetas = $derived(actual.equipos.some((e) => e.etiquetas?.length));

  // --- Acciones en bloque: copiar ahora, verificar, reanudar (inofensivas), pausar
  // (con la clave, y espera) y aplicar una plantilla (con la clave; v1.4x) -------
  let seleccionando = $state(false);
  let elegidos = $state<Set<string>>(new Set());
  let enBloque = $state<"copiar" | "verificar" | "pausar" | "reanudar" | null>(null);
  let aplicarPlantilla = $state(false);
  /** Los que pueden recibir órdenes de aquí (confirmados y no trasladados). */
  const elegible = (e: { confirmado: boolean; modo: string }) => e.confirmado && e.modo !== "trasladado";
  const elegiblesVisibles = $derived(lista.filter(elegible));
  const todosElegidos = $derived(elegiblesVisibles.length > 0 && elegiblesVisibles.every((e) => elegidos.has(e.id)));
  const seleccion = $derived(actual.equipos.filter((e) => elegidos.has(e.id) && elegible(e)));
  function alternar(id: string, on: boolean) {
    const s = new Set(elegidos);
    if (on) s.add(id);
    else s.delete(id);
    elegidos = s;
  }
  function alternarTodos(on: boolean) {
    const s = new Set(elegidos);
    for (const e of elegiblesVisibles) {
      if (on) s.add(e.id);
      else s.delete(e.id);
    }
    elegidos = s;
  }
  /** Elegir (o dejar) todos los de un grupo o una etiqueta. */
  function alternarGrupo(es: { id: string; confirmado: boolean; modo: string }[], on: boolean) {
    const s = new Set(elegidos);
    for (const e of es.filter(elegible)) {
      if (on) s.add(e.id);
      else s.delete(e.id);
    }
    elegidos = s;
  }
  const etiquetasVisibles = $derived(etiquetasDe(lista));
  const deEtiqueta = (t: string) => lista.filter((e) => e.etiquetas?.some((x) => mismaEtiqueta(x, t)));
  /** La plantilla de la etiqueta común a todos los elegidos (si la hay), para proponerla. */
  const plantillaComun = $derived.by(() => {
    if (!seleccion.length) return null;
    for (const t of seleccion[0].etiquetas ?? []) {
      const p = ajusteDe(t, actual.etiquetas)?.plantilla;
      if (p && seleccion.every((e) => e.etiquetas?.some((x) => mismaEtiqueta(x, t)))) return p;
    }
    return null;
  });
  function salirDeSeleccion() {
    seleccionando = false;
    elegidos = new Set();
  }

  const unidos = $derived(preparados.filter((p) => p.estado === "unido"));
  const esperando = $derived(preparados.filter((p) => p.estado === "abierto"));
</script>

<svelte:head><title>Equipos · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <div class="page-top">
    <div>
      <h1>Equipos</h1>
      <p>{plural(actual.equipos.length, "equipo", "equipos")} · {plural(conectados, "conectado ahora", "conectados ahora")}</p>
    </div>
    {#if puede.administrar(actual.cliente?.rol)}
      <a class="btn btn-primary" href="/c/{actual.id}/emparejar"><Plus size={16} />Añadir equipo</a>
    {/if}
  </div>

  {#if unidos.length}
    <div class="notice notice-warn">
      <TriangleAlert size={16} />
      <p>{lista_(unidos.map((p) => `«${p.nombre}»`))} {unidos.length === 1 ? "se ha unido y espera" : "se han unido y esperan"} que compruebes su número y le{unidos.length === 1 ? "" : "s"} des de alta. <a class="link" href="/c/{actual.id}/emparejar">Comprobar ahora</a></p>
    </div>
  {:else if esperando.length}
    <div class="notice notice-info">
      <Clock size={16} />
      <p>{plural(esperando.length, "equipo preparado espera", "equipos preparados esperan")} a que lo{esperando.length === 1 ? "" : "s"} instalen: {lista_(esperando.map((p) => `«${p.nombre}»`))}. <a class="link" href="/c/{actual.id}/emparejar">Ver</a></p>
    </div>
  {/if}

  {#if !actual.cargado || !actual.cliente}
    <Esqueleto forma="filas" n={5} etiqueta="Cargando los equipos…" />
  {:else if !actual.equipos.length}
    <div class="card">
      <Vacio icono={Monitor} ilustracion="sin-equipos" titulo="Todavía no hay equipos" texto="Descarga un instalador listo para el equipo (se vincula solo) o vincúlalo con un código. Tarda un par de minutos.">
        {#if puede.administrar(actual.cliente.rol)}<a class="btn btn-primary" href="/c/{actual.id}/emparejar"><Plus size={16} />Añadir el primero</a>{/if}
      </Vacio>
    </div>
  {:else}
    <div class="barra-filtros">
      <label class="buscar">
        <Search size={15} />
        <input class="input" type="search" placeholder="Buscar por nombre o sistema" bind:value={buscar} aria-label="Buscar equipos" />
      </label>
      <div class="segmented inline" role="group" aria-label="Filtrar">
        <button class:on={filtro === "todos"} aria-pressed={filtro === "todos"} onclick={() => ponerFiltro("todos")}>Todos</button>
        <button class:on={filtro === "atencion"} aria-pressed={filtro === "atencion"} onclick={() => ponerFiltro("atencion")}>Necesitan atención</button>
        <button class:on={filtro === "almacen"} aria-pressed={filtro === "almacen"} onclick={() => ponerFiltro("almacen")}>Guardan copias</button>
      </div>
      {#if puede.ordenar(actual.cliente.rol) && !seleccionando}
        <button class="btn" onclick={() => (seleccionando = true)} use:tip={"Copiar ahora o verificar varios equipos a la vez"}><ListChecks size={16} />Varios a la vez</button>
      {/if}
    </div>
    <FiltroEtiquetas />
    {#if hayEtiquetas && !filtroEtiqueta.valor}
      <label class="agrupar"><input type="checkbox" checked={agruparPorEtiqueta.valor} onchange={(e) => agruparPorEtiqueta.poner(e.currentTarget.checked)} />Agrupar por etiqueta</label>
    {/if}
    {#if seleccionando && etiquetasVisibles.length}
      <div class="por-etiqueta" role="group" aria-label="Elegir los equipos de una etiqueta">
        <span class="faint">Elegir todos los de:</span>
        {#each etiquetasVisibles as t (t.nombre)}
          {@const es = deEtiqueta(t.nombre).filter(elegible)}
          <EtiquetaChip nombre={t.nombre} n={es.length} activa={es.length > 0 && es.every((e) => elegidos.has(e.id))} onclick={() => alternarGrupo(es, !es.every((e) => elegidos.has(e.id)))} />
        {/each}
      </div>
    {/if}
    {#if grupos && lista.length}
      <div class="grupos">
        {#each grupos as g (g.etiqueta ?? "")}
          {@const eg = g.equipos.filter(elegible)}
          <section class="card lista grupo" aria-label={g.etiqueta ? `Equipos con «${g.etiqueta}»` : "Equipos sin etiqueta"}>
            <div class="fila cab-grupo">
              {#if seleccionando}
                <input type="checkbox" aria-label="Elegir los de {g.etiqueta ?? 'sin etiqueta'}" disabled={!eg.length} checked={eg.length > 0 && eg.every((e) => elegidos.has(e.id))} onchange={(ev) => alternarGrupo(g.equipos, ev.currentTarget.checked)} />
              {/if}
              {#if g.etiqueta}<EtiquetaChip nombre={g.etiqueta} />{:else}<span class="sin-et">Sin etiqueta</span>{/if}
              <span class="faint resumen-g">{plural(g.equipos.length, "equipo", "equipos")}{#if g.equipos.filter((e) => ["bad", "warn"].includes(saludEquipo(e, reloj.ahora).tono)).length}{" · "}{plural(g.equipos.filter((e) => ["bad", "warn"].includes(saludEquipo(e, reloj.ahora).tono)).length, "necesita atención", "necesitan atención")}{/if}</span>
            </div>
            {#each g.equipos as e (e.id)}
              {#if seleccionando}
                <div class="con-sel" class:on={elegidos.has(e.id)}>
                  <input type="checkbox" class="sel" aria-label="Elegir {e.nombre}" disabled={!elegible(e)} checked={elegidos.has(e.id)} onchange={(ev) => alternar(e.id, ev.currentTarget.checked)} />
                  <FilaEquipo equipo={e} cliente={actual.id} informe={informeDe(e.id)} />
                </div>
              {:else}
                <FilaEquipo equipo={e} cliente={actual.id} informe={informeDe(e.id)} acciones />
              {/if}
            {/each}
          </section>
        {/each}
      </div>
    {:else if lista.length}
      <div class="card lista">
        {#if seleccionando}
          <label class="fila sel-todos">
            <input type="checkbox" checked={todosElegidos} indeterminate={!todosElegidos && elegiblesVisibles.some((e) => elegidos.has(e.id))} onchange={(ev) => alternarTodos(ev.currentTarget.checked)} />
            <span>Todos los que se ven{filtroEtiqueta.valor ? ` con «${filtroEtiqueta.valor}»` : ""} ({elegiblesVisibles.length})</span>
          </label>
          {#each lista as e (e.id)}
            <div class="con-sel" class:on={elegidos.has(e.id)}>
              <input type="checkbox" class="sel" aria-label="Elegir {e.nombre}" disabled={!elegible(e)} checked={elegidos.has(e.id)} onchange={(ev) => alternar(e.id, ev.currentTarget.checked)} />
              <FilaEquipo equipo={e} cliente={actual.id} informe={informeDe(e.id)} />
            </div>
          {/each}
        {:else}
          {#each lista as e (e.id)}<FilaEquipo equipo={e} cliente={actual.id} informe={informeDe(e.id)} acciones />{/each}
        {/if}
      </div>
    {:else}
      <div class="card">
        <Vacio icono={Search} ilustracion="sin-resultados" titulo="Ningún equipo coincide" texto="Prueba con otro nombre o quita los filtros.">
          <button
            class="btn btn-sm"
            onclick={() => {
              buscar = "";
              ponerFiltro("todos");
              filtroEtiqueta.poner("");
            }}><X size={14} />Quitar los filtros</button
          >
        </Vacio>
      </div>
    {/if}
    {#if seleccionando && lista.length}
      <div class="barra-bloque" role="toolbar" aria-label="Acciones con los equipos elegidos">
        <span class="cuantos">{seleccion.length ? plural(seleccion.length, "equipo elegido", "equipos elegidos") : "Elige equipos"}</span>
        <button class="btn btn-sm btn-primary" disabled={!seleccion.length} onclick={() => (enBloque = "copiar")}><Play size={14} />Copiar ahora</button>
        <button class="btn btn-sm" disabled={!seleccion.length} onclick={() => (enBloque = "verificar")}><ShieldCheck size={14} />Verificar</button>
        {#if puede.administrar(actual.cliente.rol)}<button class="btn btn-sm" disabled={!seleccion.length} onclick={() => (enBloque = "pausar")}><CirclePause size={14} />Pausar</button>{/if}
        <button class="btn btn-sm" disabled={!seleccion.some((e) => e.resumen?.pausado_hasta)} onclick={() => (enBloque = "reanudar")}><CirclePlay size={14} />Reanudar</button>
        {#if puede.administrar(actual.cliente.rol)}<button class="btn btn-sm" disabled={!seleccion.length} onclick={() => (aplicarPlantilla = true)}><LayoutTemplate size={14} />Aplicar plantilla</button>{/if}
        <button class="btn btn-sm btn-ghost" onclick={salirDeSeleccion}><X size={14} />Terminar</button>
      </div>
    {/if}
  {/if}
</div>

{#if aplicarPlantilla && actual.cliente}
  <AplicarPlantillaEnBloque cliente={actual.cliente} equipos={seleccion} plantillaInicial={plantillaComun} onclose={() => (aplicarPlantilla = false)} />
{/if}

{#if enBloque && actual.cliente}
  <AccionesEnBloque cliente={actual.cliente} equipos={seleccion} accion={enBloque} onclose={() => (enBloque = null)} />
{/if}

<style>
  .agrupar {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
    color: var(--text-2);
    cursor: pointer;
  }
  .por-etiqueta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .grupos {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .cab-grupo {
    gap: 10px;
    background: var(--surface-2);
  }
  .sin-et {
    font-size: var(--fs-sm);
    font-weight: 500;
    color: var(--text-2);
  }
  .resumen-g {
    font-size: var(--fs-sm);
  }
  .sel-todos {
    gap: 10px;
    font-size: var(--fs-sm);
    color: var(--text-2);
    background: var(--surface-2);
    cursor: pointer;
  }
  .con-sel {
    display: flex;
    align-items: center;
    padding-left: var(--sp-4);
    border-top: 1px solid var(--border);
  }
  .con-sel :global(.fila-eq) {
    flex: 1;
    min-width: 0;
    border-top: none;
  }
  .con-sel.on {
    background: var(--accent-soft);
  }
  .sel {
    flex: none;
  }
  .barra-bloque {
    position: sticky;
    bottom: var(--sp-4);
    z-index: 4;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 10px var(--sp-4);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
  }
  .cuantos {
    margin-right: auto;
    font-size: var(--fs-sm);
    font-weight: 500;
  }
  .barra-filtros {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3);
  }
  .buscar {
    position: relative;
    flex: 1;
    min-width: 220px;
  }
  .buscar :global(svg) {
    position: absolute;
    top: 10px;
    left: 11px;
    color: var(--text-3);
  }
  .buscar .input {
    padding-left: 34px;
  }
  @media (max-width: 640px) {
    .segmented.inline {
      display: grid;
      width: 100%;
      grid-auto-columns: minmax(max-content, 1fr);
    }
    .segmented.inline > button {
      padding: 0 8px;
    }
  }
</style>
