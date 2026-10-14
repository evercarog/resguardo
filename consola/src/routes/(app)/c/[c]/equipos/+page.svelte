<script lang="ts">
  import { tip } from "$lib/tooltip";
  import { ArrowDown, ArrowUp, Building2, ChevronRight, CirclePause, CirclePlay, Clock, GripVertical, LayoutTemplate, ListChecks, Monitor, Play, Plus, Search, ShieldCheck, TriangleAlert, X } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import Anuncio from "$lib/componentes/Anuncio.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import OrganizarEquipos from "$lib/componentes/OrganizarEquipos.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { agruparPorNiveles, enOrdenManual, equiposSinSede, leerNiveles, moverAntesDe, moverEnGrupo, nivelesDisponibles, sedeDeEquipo, sedePorDefecto, type GrupoNivel, type Nivel } from "$lib/sedes";
  import { ponerOrden, ponerSedeEquipos } from "$lib/organizacion.svelte";
  import type { Equipo } from "$lib/tipos";
  import AccionesEnBloque from "$lib/componentes/AccionesEnBloque.svelte";
  import AplicarPlantillaEnBloque from "$lib/componentes/AplicarPlantillaEnBloque.svelte";
  import EtiquetaChip from "$lib/componentes/EtiquetaChip.svelte";
  import * as api from "$lib/api";
  import { enFondo } from "$lib/actividad.svelte";
  import { untrack } from "svelte";
  import { seguirCambios } from "$lib/vivo.svelte";
  import { actual, app, puede, reloj } from "$lib/estado.svelte";
  import { lista as lista_, plural } from "$lib/formato";
  import type { Preparado } from "$lib/tipos";
  import { PESO, saludEquipo, sinDuplicados, type Tono } from "$lib/salud";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import FilaEquipo from "$lib/componentes/FilaEquipo.svelte";
  import FiltroEtiquetas from "$lib/componentes/FiltroEtiquetas.svelte";
  import { agruparPorEtiqueta, ajusteDe, etiquetasDe, filtroEtiqueta, mismaEtiqueta, pasaFiltro } from "$lib/etiquetas.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import { cargarInformes, ultimos } from "$lib/informes.svelte";
  import { guardar, leer, leerTexto } from "$lib/recordar";
  import { datosBasicos, grupoSistema, textoBuscable } from "$lib/datosBasicos";

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

  // 14uu: filtrar por sistema («Windows 11», «Windows Server 2016», «Ubuntu 22.04»…).
  let sistema = $state("");
  const sistemas = $derived([...new Set(actual.equipos.map((e) => grupoSistema(e)))].sort((a, b) => a.localeCompare(b, "es", { numeric: true })));
  // Un sistema que ya no tiene ningún equipo deja de filtrar.
  const sistemaActivo = $derived(sistemas.includes(sistema) ? sistema : "");

  const lista = $derived(
    actual.equipos
      .filter((e) => {
        const t = buscar.trim().toLowerCase();
        if (t && !`${e.nombre} ${textoBuscable(e)} ${(e.etiquetas ?? []).join(" ")}`.toLowerCase().includes(t)) return false;
        if (sistemaActivo && grupoSistema(e) !== sistemaActivo) return false;
        if (!pasaFiltro(e, filtroEtiqueta.valor)) return false;
        const tono: Tono = saludEquipo(e, reloj.ahora).tono;
        if (filtro === "atencion") return tono === "bad" || tono === "warn";
        if (filtro === "almacen") return e.rol === "almacenamiento";
        return true;
      })
      .sort((a, b) => PESO[saludEquipo(a, reloj.ahora).tono] - PESO[saludEquipo(b, reloj.ahora).tono] || a.nombre.localeCompare(b.nombre)),
  );

  // --- Plan 0.8.0, D2 (14ss): agrupar por niveles, orden y sedes ---------------
  // Lo elegido se recuerda por persona (su cuenta) y cliente, en este navegador.
  const yo = $derived(app.cuenta?.id ?? "");
  const org = $derived(actual.organizacion);
  const conSistema = $derived(sistemas.length > 1 || actual.equipos.some((e) => !!datosBasicos(e)));
  const disponibles = $derived(nivelesDisponibles(org, conSistema));
  let niveles = $state<Nivel[]>([]);
  $effect.pre(() => {
    if (!actual.id) return;
    const guardado = leerTexto(`equipos.agrupar.${yo}.${actual.id}`);
    // Quien tenía «Agrupar por etiqueta» (antes de la 0.8.0) sigue agrupado por etiquetas.
    niveles = leerNiveles(guardado || (agruparPorEtiqueta.valor ? "etiqueta" : ""), disponibles);
  });
  function ponerNivel(i: number, v: string) {
    const ns = [...niveles];
    if (!v) ns.splice(i);
    else ns[i] = v as Nivel;
    niveles = leerNiveles(ns.join(">"), disponibles);
    guardar(`equipos.agrupar.${yo}.${actual.id}`, niveles.join(">") || "nada");
  }
  const ORDENES = ["atencion", "nombre", "manual"] as const;
  let orden = $state<(typeof ORDENES)[number]>("atencion");
  $effect.pre(() => {
    if (actual.id) orden = leer(`equipos.orden.${yo}.${actual.id}`, ORDENES, "atencion");
  });
  function ponerOrdenVista(o: (typeof ORDENES)[number]) {
    orden = o;
    guardar(`equipos.orden.${yo}.${actual.id}`, o === "atencion" ? "" : o);
  }
  const ordenada = $derived(
    orden === "manual" ? enOrdenManual(lista, org.orden) : orden === "nombre" ? [...lista].sort((a, b) => a.nombre.localeCompare(b.nombre, "es", { numeric: true })) : lista,
  );
  const arbol = $derived(niveles.length && !filtroEtiqueta.valor ? agruparPorNiveles(ordenada, niveles, { organizacion: org, sistema: grupoSistema }) : null);
  /** La sede en cada fila (si hay sedes y no se agrupa por ella). */
  const sedeFila = (e: Equipo) => {
    if (!org.sedes.length || niveles.includes("sede")) return null;
    const s = sedeDeEquipo(org, e.id);
    return s.sede ? (s.asignada ? s.sede.nombre : `${s.sede.nombre} (sin asignar)`) : null;
  };
  const sinSede = $derived(equiposSinSede(org, contablesSinTrasladar()));
  function contablesSinTrasladar() {
    return sinDuplicados(actual.equipos).filter((e) => e.modo !== "trasladado" && e.confirmado);
  }

  // Grupos plegados (por cliente, en este navegador).
  let plegados = $state<Set<string>>(new Set());
  $effect.pre(() => {
    if (!actual.id) return;
    try {
      plegados = new Set(JSON.parse(leerTexto(`equipos.plegados.${actual.id}`) || "[]") as string[]);
    } catch {
      plegados = new Set();
    }
  });
  function plegar(clave: string) {
    const s = new Set(plegados);
    if (s.has(clave)) s.delete(clave);
    else s.add(clave);
    plegados = s;
    guardar(`equipos.plegados.${actual.id}`, s.size ? JSON.stringify([...s].slice(-200)) : "");
  }

  // Orden a mano: flechas (teclado) o arrastrar, dentro de cada grupo; igual en todas las consolas.
  const ordenEditable = $derived(orden === "manual" && puede.administrar(actual.cliente?.rol));
  let anuncio = $state("");
  let arrastrando = $state<string | null>(null);
  async function aplicarOrden(nuevo: string[] | null, e: Equipo, grupo: Equipo[]) {
    if (!nuevo) return;
    const antes = org.orden;
    actual.organizacion = { ...org, orden: nuevo };
    const pos = enOrdenManual(grupo, nuevo).findIndex((x) => x.id === e.id) + 1;
    anuncio = `${e.nombre}: posición ${pos} de ${grupo.length}.`;
    try {
      await ponerOrden(actual.id, nuevo);
    } catch (err) {
      actual.organizacion = { ...actual.organizacion, orden: antes };
      fallo(err);
    }
  }
  const idsEnOrden = () => enOrdenManual(actual.equipos, org.orden).map((x) => x.id);
  const mover = (e: Equipo, grupo: Equipo[], paso: number) =>
    aplicarOrden(
      moverEnGrupo(
        idsEnOrden(),
        grupo.map((x) => x.id),
        e.id,
        paso,
      ),
      e,
      grupo,
    );
  function soltar(destino: Equipo | null, grupo: Equipo[]) {
    const id = arrastrando;
    arrastrando = null;
    const e = actual.equipos.find((x) => x.id === id);
    if (!e || !grupo.some((x) => x.id === e.id)) return;
    void aplicarOrden(
      moverAntesDe(
        idsEnOrden(),
        grupo.map((x) => x.id),
        e.id,
        destino?.id ?? null,
      ),
      e,
      grupo,
    );
  }
  let organizar = $state(false);
  // «Varios a la vez» → poner sede.
  let ponerSede = $state(false);
  let sedeElegida = $state("");
  let poniendoSede = $state(false);
  async function aplicarSede() {
    poniendoSede = true;
    try {
      await ponerSedeEquipos(
        actual.id,
        seleccion.map((e) => e.id),
        sedeElegida || null,
      );
      const s = org.sedes.find((x) => x.id === sedeElegida);
      avisar(`${plural(seleccion.length, "equipo", "equipos")} ${s ? `en «${s.nombre}»` : "sin sede asignada (la principal)"}. Para que lo vean las demás consolas, repártelo con la clave de administración desde el aviso de datos comunes.`);
      ponerSede = false;
    } catch (err) {
      fallo(err);
    } finally {
      poniendoSede = false;
    }
  }
  // Las cuentas, sin los duplicados sin confirmar (salen en la lista para quitarlos, pero no cuentan).
  const contables = $derived(sinDuplicados(actual.equipos));
  const conectados = $derived(contables.filter((e) => e.conectado).length);

  // Equipos preparados (instalador listo o línea de Linux, v1.17): los que esperan o ya se unieron.
  let preparados = $state<Preparado[]>([]);
  $effect(() => {
    const c = actual.id;
    if (!c || !puede.administrar(actual.cliente?.rol)) return;
    const cargar = () => api.preparados(c).then((x) => (preparados = x), () => {});
    untrack(() => void cargar());
    return untrack(() => seguirCambios(() => enFondo(cargar), { ms: 10_000, toca: (x) => x.t === "equipo" }));
  });


  // --- Acciones en bloque: copiar ahora, verificar, reanudar (inofensivas), pausar
  // (con la clave, y espera) y aplicar una plantilla (con la clave; v1.52) -------
  let seleccionando = $state(false);
  let elegidos = $state<Set<string>>(new Set());
  let enBloque = $state<"copiar" | "verificar" | "pausar" | "reanudar" | null>(null);
  let aplicarPlantilla = $state(false);
  /** Los que pueden recibir órdenes de aquí (confirmados y no trasladados). */
  const elegible = (e: { confirmado: boolean; modo: string }) => e.confirmado && e.modo !== "trasladado";
  const elegiblesVisibles = $derived(ordenada.filter(elegible));
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
      <p>{plural(contables.length, "equipo", "equipos")} · {plural(conectados, "conectado ahora", "conectados ahora")}</p>
    </div>
    {#if puede.administrar(actual.cliente?.rol)}
      <div class="acciones-cab">
        <button class="btn" onclick={() => (organizar = true)}><Building2 size={16} />Sedes y categorías</button>
        <a class="btn btn-primary" href="/c/{actual.id}/emparejar"><Plus size={16} />Añadir equipo</a>
      </div>
    {/if}
  </div>

  {#if org.sedes.length && sinSede.length}
    <div class="notice notice-info">
      <Building2 size={16} />
      <p>
        {plural(sinSede.length, "equipo no tiene sede", "equipos no tienen sede")}: {sinSede.length === 1 ? "cuenta" : "cuentan"} en «{sedePorDefecto(org)?.nombre}», la principal. Ponle la suya en su ficha o con «Varios a la vez». <Ayuda id="sedes" />
      </p>
    </div>
  {/if}

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
      {#if sistemas.length > 1}
        <select class="input sistema" bind:value={sistema} aria-label="Filtrar por sistema">
          <option value="">Todos los sistemas</option>
          {#each sistemas as s (s)}<option value={s}>{s} ({actual.equipos.filter((e) => grupoSistema(e) === s).length})</option>{/each}
        </select>
      {/if}
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
    <div class="vista" role="group" aria-label="Cómo ver la lista">
      {#if !filtroEtiqueta.valor}
        <label class="agrupar" for="ag-1">Agrupar por</label>
        <select id="ag-1" class="input input-sm" value={niveles[0] ?? ""} onchange={(e) => ponerNivel(0, e.currentTarget.value)}>
          <option value="">Nada</option>
          {#each disponibles as d (d.nivel)}<option value={d.nivel}>{d.nombre}</option>{/each}
        </select>
        {#if niveles[0]}
          <label class="agrupar" for="ag-2">luego</label>
          <select id="ag-2" class="input input-sm" value={niveles[1] ?? ""} onchange={(e) => ponerNivel(1, e.currentTarget.value)}>
            <option value="">Nada</option>
            {#each disponibles.filter((d) => d.nivel !== niveles[0]) as d (d.nivel)}<option value={d.nivel}>{d.nombre}</option>{/each}
          </select>
        {/if}
      {/if}
      <label class="agrupar" for="orden">Orden</label>
      <select id="orden" class="input input-sm" value={orden} onchange={(e) => ponerOrdenVista(e.currentTarget.value as (typeof ORDENES)[number])}>
        <option value="atencion">Lo que necesita atención primero</option>
        <option value="nombre">Por nombre</option>
        <option value="manual">A mano</option>
      </select>
      {#if orden === "manual"}<Ayuda id="orden-a-mano" />{/if}
    </div>
    <Anuncio texto={anuncio} />
    {#if seleccionando && etiquetasVisibles.length}
      <div class="por-etiqueta" role="group" aria-label="Elegir los equipos de una etiqueta">
        <span class="faint">Elegir todos los de:</span>
        {#each etiquetasVisibles as t (t.nombre)}
          {@const es = deEtiqueta(t.nombre).filter(elegible)}
          <EtiquetaChip nombre={t.nombre} n={es.length} activa={es.length > 0 && es.every((e) => elegidos.has(e.id))} onclick={() => alternarGrupo(es, !es.every((e) => elegidos.has(e.id)))} />
        {/each}
      </div>
    {/if}
    {#snippet filas(es: Equipo[])}
      {#each es as e, i (e.id)}
        {#if seleccionando}
          <div class="con-sel" class:on={elegidos.has(e.id)}>
            <input type="checkbox" class="sel" aria-label="Elegir {e.nombre}" disabled={!elegible(e)} checked={elegidos.has(e.id)} onchange={(ev) => alternar(e.id, ev.currentTarget.checked)} />
            <FilaEquipo equipo={e} cliente={actual.id} informe={informeDe(e.id)} sede={sedeFila(e)} />
          </div>
        {:else if ordenEditable}
          <div
            class="con-orden"
            class:arrastrando={arrastrando === e.id}
            role="group"
            aria-label={e.nombre}
            draggable="true"
            ondragstart={(ev) => {
              arrastrando = e.id;
              ev.dataTransfer?.setData("text/plain", e.id);
            }}
            ondragend={() => (arrastrando = null)}
            ondragover={(ev) => arrastrando && es.some((x) => x.id === arrastrando) && ev.preventDefault()}
            ondrop={(ev) => {
              ev.preventDefault();
              soltar(e, es);
            }}
          >
            <span class="asa" aria-hidden="true"><GripVertical size={14} /></span>
            <span class="flechas">
              <button class="icon-btn sm" aria-label="Subir {e.nombre}" disabled={i === 0} onclick={() => mover(e, es, -1)}><ArrowUp size={14} /></button>
              <button class="icon-btn sm" aria-label="Bajar {e.nombre}" disabled={i === es.length - 1} onclick={() => mover(e, es, 1)}><ArrowDown size={14} /></button>
            </span>
            <FilaEquipo equipo={e} cliente={actual.id} informe={informeDe(e.id)} sede={sedeFila(e)} acciones />
          </div>
        {:else}
          <FilaEquipo equipo={e} cliente={actual.id} informe={informeDe(e.id)} sede={sedeFila(e)} acciones />
        {/if}
      {/each}
      {#if ordenEditable && arrastrando && es.some((x) => x.id === arrastrando)}
        <div
          class="soltar-final"
          role="group"
          aria-label="Soltar al final"
          ondragover={(ev) => ev.preventDefault()}
          ondrop={(ev) => {
            ev.preventDefault();
            soltar(null, es);
          }}
        >
          Soltar al final
        </div>
      {/if}
    {/snippet}
    {#snippet grupo(g: GrupoNivel<Equipo>, hondo: number)}
      {@const eg = g.equipos.filter(elegible)}
      {@const plegado = plegados.has(g.clave)}
      {@const atencion = g.equipos.filter((e) => ["bad", "warn"].includes(saludEquipo(e, reloj.ahora).tono)).length}
      <section class={hondo ? "subgrupo" : "card lista grupo"} aria-label="{g.titulo}: {plural(g.equipos.length, 'equipo', 'equipos')}">
        <div class="fila cab-grupo" class:sub={hondo > 0}>
          {#if seleccionando}
            <input type="checkbox" aria-label="Elegir los de {g.titulo}" disabled={!eg.length} checked={eg.length > 0 && eg.every((e) => elegidos.has(e.id))} onchange={(ev) => alternarGrupo(g.equipos, ev.currentTarget.checked)} />
          {/if}
          <button class="plegar" aria-expanded={!plegado} onclick={() => plegar(g.clave)}>
            <span class="chev" class:abierto={!plegado}><ChevronRight size={15} /></span>
            {#if g.sin}<span class="sin-et">{g.titulo}</span>
            {:else if g.nivel === "sede"}<span class="titulo-g"><Building2 size={14} />{g.titulo}</span>
            {:else if g.nivel === "sistema"}<span class="titulo-g"><Monitor size={14} />{g.titulo}</span>
            {:else if g.nivel === "etiqueta"}<span class="titulo-g">{#each g.titulo.split(" · ") as t (t)}<EtiquetaChip nombre={t} />{/each}</span>
            {:else}<span class="titulo-g"><EtiquetaChip nombre={g.titulo} /></span>{/if}
          </button>
          <span class="faint resumen-g">{plural(g.equipos.filter((e) => contables.includes(e)).length, "equipo", "equipos")}{#if atencion}{" · "}<span class="atencion">{plural(atencion, "necesita atención", "necesitan atención")}</span>{/if}</span>
        </div>
        {#if !plegado}
          {#if g.hijos.length}
            {#each g.hijos as h (h.clave)}{@render grupo(h, hondo + 1)}{/each}
          {:else}
            {@render filas(g.equipos)}
          {/if}
        {/if}
      </section>
    {/snippet}
    {#if arbol && ordenada.length}
      <div class="grupos">
        {#each arbol as g (g.clave)}{@render grupo(g, 0)}{/each}
      </div>
    {:else if ordenada.length}
      <div class="card lista">
        {#if seleccionando}
          <label class="fila sel-todos">
            <input type="checkbox" checked={todosElegidos} indeterminate={!todosElegidos && elegiblesVisibles.some((e) => elegidos.has(e.id))} onchange={(ev) => alternarTodos(ev.currentTarget.checked)} />
            <span>Todos los que se ven{filtroEtiqueta.valor ? ` con «${filtroEtiqueta.valor}»` : ""} ({elegiblesVisibles.length})</span>
          </label>
          {@render filas(ordenada)}
        {:else}
          {@render filas(ordenada)}
        {/if}
      </div>
    {:else}
      <div class="card">
        <Vacio icono={Search} ilustracion="sin-resultados" titulo="Ningún equipo coincide" texto="Prueba con otro nombre o quita los filtros.">
          <button
            class="btn btn-sm"
            onclick={() => {
              buscar = "";
              sistema = "";
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
        {#if puede.administrar(actual.cliente.rol) && org.sedes.length}<button class="btn btn-sm" disabled={!seleccion.length} onclick={() => ((sedeElegida = ""), (ponerSede = true))}><Building2 size={14} />Poner sede</button>{/if}
        <button class="btn btn-sm btn-ghost" onclick={salirDeSeleccion}><X size={14} />Terminar</button>
      </div>
    {/if}
  {/if}
</div>

{#if organizar}<OrganizarEquipos onclose={() => (organizar = false)} />{/if}

{#if ponerSede}
  <Modal labelledby="t-poner-sede" onclose={() => (ponerSede = false)} width={440}>
    <div class="dlg-title">
      <span class="ticon"><Building2 size={18} /></span>
      <div>
        <h2 id="t-poner-sede">Sede de {plural(seleccion.length, "equipo", "equipos")}</h2>
        <p>Cuenta en la regla 3-2-1-1-0: un destino en otra sede está fuera del sitio. <Ayuda id="sedes" /></p>
      </div>
    </div>
    <div class="field">
      <label class="field-label" for="sede-bloque">Sede</label>
      <select id="sede-bloque" class="input" bind:value={sedeElegida}>
        <option value="">Sin asignar: la principal ({sedePorDefecto(org)?.nombre ?? "—"})</option>
        {#each [...org.sedes].sort((a, b) => a.nombre.localeCompare(b.nombre, "es")) as s (s.id)}<option value={s.id}>{s.nombre}</option>{/each}
      </select>
      <span class="field-hint">{seleccion.map((e) => e.nombre).join(", ")}</span>
    </div>
    <footer class="pie">
      <button class="btn btn-ghost" onclick={() => (ponerSede = false)}>Cancelar</button>
      <button class="btn btn-primary" disabled={poniendoSede} onclick={aplicarSede}>{poniendoSede ? "Guardando…" : "Guardar"}</button>
    </footer>
  </Modal>
{/if}

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
  }
  .vista {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .vista select {
    width: auto;
    max-width: 100%;
  }
  .acciones-cab {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .plegar {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 28px;
    padding: 0 4px;
    border: none;
    background: none;
    color: var(--text-1);
    font: inherit;
    font-weight: 550;
    cursor: pointer;
    border-radius: var(--radius-sm);
  }
  .plegar:hover {
    background: var(--surface-3);
  }
  .chev {
    display: inline-flex;
    color: var(--text-3);
    transition: transform var(--dur) var(--ease);
  }
  .chev.abierto {
    transform: rotate(90deg);
  }
  .titulo-g {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .titulo-g :global(svg) {
    color: var(--text-3);
  }
  .atencion {
    color: var(--warn);
  }
  .subgrupo {
    border-top: 1px solid var(--border);
  }
  .cab-grupo.sub {
    padding-left: var(--sp-8);
    background: color-mix(in srgb, var(--surface-2) 55%, var(--surface));
  }
  .con-orden {
    display: flex;
    align-items: center;
    gap: 2px;
    padding-left: 6px;
    border-top: 1px solid var(--border);
  }
  .con-orden :global(.fila-eq) {
    flex: 1;
    min-width: 0;
    border-top: none;
  }
  .con-orden.arrastrando {
    opacity: 0.5;
  }
  .asa {
    display: inline-flex;
    color: var(--text-3);
    cursor: grab;
  }
  .flechas {
    display: inline-flex;
    flex-direction: column;
  }
  .icon-btn.sm {
    width: 24px;
    height: 24px;
  }
  .soltar-final {
    padding: 8px var(--sp-4);
    border-top: 1px dashed var(--border-strong);
    font-size: var(--fs-sm);
    color: var(--text-3);
    text-align: center;
  }
  .pie {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: var(--sp-4);
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
  .sistema {
    width: auto;
    max-width: 100%;
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
