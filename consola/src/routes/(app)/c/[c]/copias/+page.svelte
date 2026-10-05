<script lang="ts">
  import { untrack } from "svelte";
  // Copias del cliente (docs/diseno.md §5): todas las copias de todos sus
  // equipos en una lista, como Equipos y Repositorios: qué copia, de qué
  // equipo, a qué repositorio y destino, cuándo (en palabras), cómo fue la
  // última y cuándo toca la próxima, lo que protege, lo que está en marcha y
  // sus observaciones. Filtros (equipo, estado, repositorio), búsqueda y
  // orden; cada fila lleva a la página de la copia y tiene «Copiar ahora»
  // (el diálogo de siempre) y «Ver versiones». «Varias a la vez» copia ahora
  // las elegidas. Todo sale del resumen de cada equipo y de su último informe.
  import { tip } from "$lib/tooltip";
  import { CalendarClock, ChevronRight, FolderSync, HardDrive, History, ListChecks, Play, Search, TriangleAlert, X } from "@lucide/svelte";
  import { actual, puede, reloj } from "$lib/estado.svelte";
  import { bytes, horarioEnFrase, numero, plural, relativo, cuandoFrase } from "$lib/formato";
  import { cargarInformes, ultimos } from "$lib/informes.svelte";
  import { ESTADOS_FILTRO, filasCopias, filtrarCopias, ORDENES, ordenarCopias, type EstadoFiltro, type FilaCopia, type Orden } from "$lib/copiasCliente";
  import { guardar, leer } from "$lib/recordar";
  import { objetoDe } from "$lib/notas.svelte";
  import AccionesEnBloque from "$lib/componentes/AccionesEnBloque.svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import Cifra from "$lib/componentes/Cifra.svelte";
  import EnMarcha from "$lib/componentes/EnMarcha.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import OrdenDialog from "$lib/componentes/OrdenDialog.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import ContadorNotas from "$lib/componentes/notas/ContadorNotas.svelte";
  // v1.41: dónde se guarda cada copia (y si se queda en el mismo equipo).
  import { lugarRepo, riesgoMismoEquipo } from "$lib/dondeGuarda";
  import SeGuardaEn from "$lib/componentes/SeGuardaEn.svelte";

  // Los informes (la última vuelta de cada copia, lo que protege, «sin cambios»).
  $effect(() => {
    // Las cargas, sin seguir lo que leen (si no, cada respuesta podría volver a lanzar el efecto).
    const [cc, ids] = [actual.id, actual.equipos.map((e) => e.id)];
    untrack(() => void cargarInformes(cc, ids));
  });
  const informes = $derived(ultimos.cliente === actual.id ? ultimos.porEquipo : {});
  const filas = $derived(filasCopias(actual.equipos, informes, reloj.ahora));

  // Filtros: el estado y el orden se recuerdan en este navegador (por cliente).
  let buscar = $state("");
  let estado = $state<EstadoFiltro>("todos");
  let orden = $state<Orden>("estado");
  let equipo = $state("");
  let repo = $state("");
  const ID_ESTADOS = ESTADOS_FILTRO.map((x) => x.id);
  const ID_ORDENES = ORDENES.map((x) => x.id);
  $effect.pre(() => {
    if (!actual.id) return;
    estado = leer(`copias.estado.${actual.id}`, ID_ESTADOS, "todos");
    orden = leer(`copias.orden.${actual.id}`, ID_ORDENES, "estado");
  });
  function ponerEstado(x: EstadoFiltro) {
    estado = x;
    guardar(`copias.estado.${actual.id}`, x === "todos" ? "" : x);
  }
  function ponerOrden(x: Orden) {
    orden = x;
    guardar(`copias.orden.${actual.id}`, x === "estado" ? "" : x);
  }
  function quitarFiltros() {
    buscar = "";
    equipo = "";
    repo = "";
    ponerEstado("todos");
  }
  const vistas = $derived(ordenarCopias(filtrarCopias(filas, { buscar, estado, equipo: equipo || null, repo: repo || null }), orden));
  const equiposConCopias = $derived(actual.equipos.filter((e) => e.resumen?.copias?.length).sort((a, b) => a.nombre.localeCompare(b.nombre, "es")));
  const repos = $derived(
    actual.equipos
      .filter((e) => !equipo || e.id === equipo)
      .flatMap((e) => (e.resumen?.repositorios ?? []).filter((r) => e.resumen?.copias?.some((k) => k.repo === r.id)).map((r) => ({ clave: `${e.id}|${r.id}`, texto: `${r.nombre} · ${e.nombre}` })))
      .sort((a, b) => a.texto.localeCompare(b.texto, "es")),
  );
  $effect(() => {
    if (repo && !repos.some((r) => r.clave === repo)) repo = "";
  });
  const hayFiltro = $derived(!!buscar.trim() || estado !== "todos" || !!equipo || !!repo);

  // Cifras.
  const atencion = $derived(filas.filter((x) => x.estado.tono === "bad" || x.estado.tono === "warn").length);
  const alDia = $derived(filas.filter((x) => x.estado.tono === "ok").length);
  const protegido = $derived(filas.reduce((n, x) => n + (x.protegido ?? 0), 0));
  const proxima = $derived(
    filas
      .map((x) => x.proxima)
      .filter((x): x is string => !!x && Date.parse(x) > reloj.ahora)
      .sort()
      .at(0) ?? null,
  );
  const equiposN = $derived(new Set(filas.map((x) => x.equipo.id)).size);

  // «Copiar ahora» de una fila: el diálogo de siempre.
  let copiar = $state<FilaCopia | null>(null);
  const puedeOrdenar = $derived(puede.ordenar(actual.cliente?.rol));

  // Varias a la vez (solo «Copiar ahora», inofensivo).
  let seleccionando = $state(false);
  let elegidas = $state<Set<string>>(new Set());
  let enBloque = $state(false);
  const elegiblesVisibles = $derived(vistas.filter((x) => x.ordenable));
  const todasElegidas = $derived(elegiblesVisibles.length > 0 && elegiblesVisibles.every((x) => elegidas.has(x.clave)));
  const seleccion = $derived(filas.filter((x) => elegidas.has(x.clave) && x.ordenable));
  const equiposSeleccion = $derived(actual.equipos.filter((e) => seleccion.some((x) => x.equipo.id === e.id)));
  function alternar(clave: string, on: boolean) {
    const s = new Set(elegidas);
    if (on) s.add(clave);
    else s.delete(clave);
    elegidas = s;
  }
  function alternarTodas(on: boolean) {
    const s = new Set(elegidas);
    for (const x of elegiblesVisibles) {
      if (on) s.add(x.clave);
      else s.delete(x.clave);
    }
    elegidas = s;
  }
  function salir() {
    seleccionando = false;
    elegidas = new Set();
  }

  const href = (x: FilaCopia, ancla = "") => `/c/${actual.id}/equipos/${x.equipo.id}/copias/${encodeURIComponent(x.copia.id)}${ancla}`;
  const TEXTO_ULTIMA = { ok: "correcta", sin_cambios: "sin cambios", aviso: "con avisos", fallo: "falló" } as const;
</script>

<svelte:head><title>Copias · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <CabeceraPagina
    titulo="Copias"
    icono={FolderSync}
    migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${actual.id}` }, { texto: "Copias" }]}
    resumen={actual.cargado ? `${plural(filas.length, "copia", "copias")} en ${plural(equiposN, "equipo", "equipos")}${atencion ? ` · ${plural(atencion, "necesita atención", "necesitan atención")}` : filas.length ? " · todas al día o en su sitio" : ""}` : "Cargando…"}
  />

  {#if !actual.cargado || !actual.cliente}
    <Esqueleto forma="cifras" n={4} />
    <Esqueleto forma="filas" n={6} etiqueta="Cargando las copias…" />
  {:else if !filas.length}
    <div class="card">
      <Vacio icono={FolderSync} ilustracion="sin-repos" titulo="Todavía no hay copias" texto="Una copia dice qué carpetas de un equipo se guardan, dónde y cuándo. Se crean desde la ficha de cada equipo.">
        {#if actual.equipos.length}<a class="btn btn-primary" href="/c/{actual.id}/equipos">Ver los equipos</a>{:else if puede.administrar(actual.cliente.rol)}<a class="btn btn-primary" href="/c/{actual.id}/emparejar">Añadir equipo</a>{/if}
      </Vacio>
    </div>
  {:else}
    <div class="cifras" role="list" aria-label="Cifras de las copias">
      <Cifra icono={FolderSync} etiqueta="Copias" valor={numero(alDia)} de="de {filas.length}" sub="al día" />
      <Cifra icono={TriangleAlert} etiqueta="Necesitan atención" valor={numero(atencion)} sub={atencion ? "con error, atrasadas o con avisos" : "ninguna"} mal={atencion > 0} />
      <Cifra icono={HardDrive} etiqueta="Protegido" valor={protegido ? bytes(protegido) : "—"} sub="lo que copian, según su última versión" />
      <Cifra icono={CalendarClock} etiqueta="Próxima copia" valor={proxima ? relativo(proxima, reloj.ahora) : "—"} sub={proxima ? cuandoFrase(proxima, reloj.ahora) : "nada programado"} />
    </div>

    <div class="barra-filtros">
      <label class="buscar">
        <Search size={15} />
        <input class="input" type="search" placeholder="Buscar por copia, equipo, repositorio o destino" bind:value={buscar} aria-label="Buscar copias" />
      </label>
      <div class="segmented inline" role="group" aria-label="Filtrar por estado">
        {#each ESTADOS_FILTRO as f (f.id)}
          <button class:on={estado === f.id} aria-pressed={estado === f.id} onclick={() => ponerEstado(f.id)}>{f.texto}</button>
        {/each}
      </div>
    </div>
    <div class="barra-filtros segunda">
      <label class="sel">
        <span class="faint">Equipo</span>
        <select class="input" bind:value={equipo}>
          <option value="">Todos</option>
          {#each equiposConCopias as e (e.id)}<option value={e.id}>{e.nombre}</option>{/each}
        </select>
      </label>
      <label class="sel">
        <span class="faint">Repositorio</span>
        <select class="input" bind:value={repo}>
          <option value="">Todos</option>
          {#each repos as r (r.clave)}<option value={r.clave}>{r.texto}</option>{/each}
        </select>
      </label>
      <label class="sel">
        <span class="faint">Orden</span>
        <select class="input" value={orden} onchange={(ev) => ponerOrden(ev.currentTarget.value as Orden)}>
          {#each ORDENES as o (o.id)}<option value={o.id}>{o.texto}</option>{/each}
        </select>
      </label>
      <span class="hueco"></span>
      {#if puedeOrdenar && !seleccionando}
        <button class="btn" onclick={() => (seleccionando = true)} use:tip={"Copiar ahora varias copias a la vez"}><ListChecks size={16} />Varias a la vez</button>
      {/if}
    </div>

    {#if vistas.length}
      <p class="sr-only" aria-live="polite">{plural(vistas.length, "copia", "copias")}</p>
      <div class="card p-0 lista-copias">
        {#if seleccionando}
          <label class="fila sel-todas">
            <input type="checkbox" checked={todasElegidas} indeterminate={!todasElegidas && elegiblesVisibles.some((x) => elegidas.has(x.clave))} onchange={(ev) => alternarTodas(ev.currentTarget.checked)} />
            <span>Todas las que se ven ({elegiblesVisibles.length})</span>
          </label>
        {/if}
        {#each vistas as x (x.clave)}
          <div class="fila-copia" class:on={elegidas.has(x.clave)} class:con-sel={seleccionando}>
            {#if seleccionando}
              <input type="checkbox" class="sel-c" aria-label="Elegir «{x.copia.nombre}» de {x.equipo.nombre}" disabled={!x.ordenable} checked={elegidas.has(x.clave)} onchange={(ev) => alternar(x.clave, ev.currentTarget.checked)} />
            {/if}
            <div class="principal">
              <a class="nombre" href={href(x)}>
                <strong>{x.copia.nombre}</strong>
                <ChevronRight size={14} aria-hidden="true" />
              </a>
              <ContadorNotas tipo="copia" objeto={objetoDe(x.equipo.id, x.copia.id)} />
              <span class="sub">
                <a class="link-suave" href="/c/{actual.id}/equipos/{x.equipo.id}">{x.equipo.nombre}</a>{" · "}{#if x.repo}<a class="link-suave" href="/c/{actual.id}/equipos/{x.equipo.id}/repositorios/{encodeURIComponent(x.repo.id)}">{x.repo.nombre}</a>{:else}{x.copia.repo}{/if}
              </span>
              {#if x.repo}<span class="sub"><SeGuardaEn pequeno lugar={lugarRepo(x.repo, x.equipo, actual.equipos)} riesgo={!!riesgoMismoEquipo(x.repo, x.equipo, actual.equipos)} /></span>{/if}
              <span class="sub faint">{horarioEnFrase(x.copia.horario)}</span>
              {#if x.vuelta?.resultado === "fallo" && x.vuelta.mensaje}<span class="sub msg-fallo">{x.vuelta.mensaje}</span>{/if}
              <span class="vivo"><EnMarcha equipo={x.equipo.id} copia={x.copia.id} compacto /></span>
            </div>
            <div class="estado">
              <Chip tono={x.estado.tono} texto={x.estado.texto} />
            </div>
            <div class="cuando num">
              <span>{#if x.vuelta}Última <Tiempo iso={x.vuelta.cuando} /><span class="faint">{" · "}{TEXTO_ULTIMA[x.vuelta.resultado]}</span>{:else}<span class="faint">Ninguna todavía</span>{/if}</span>
              <span class="faint">{#if x.proxima && Date.parse(x.proxima) > reloj.ahora}Próxima <Tiempo iso={x.proxima} />{:else if x.copia.activa === false}Desactivada{:else if x.estado.tono === "paused"}En pausa{:else}Sin próxima a la vista{/if}</span>
            </div>
            <div class="protegido num" use:tip={"Lo que ocupan sus archivos, según su última versión"}><span class="et-movil">{"Protege "}</span>{x.protegido != null ? bytes(x.protegido) : "—"}</div>
            <div class="acciones">
              <a class="icon-btn" href={href(x, "#t-historial")} aria-label="Ver las versiones de «{x.copia.nombre}»"><History size={15} /></a>
              {#if puedeOrdenar && x.ordenable && !seleccionando}
                <button class="icon-btn" aria-label="Copiar ahora «{x.copia.nombre}» de {x.equipo.nombre}" onclick={() => (copiar = x)}><Play size={15} /></button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
      {#if seleccionando}
        <div class="barra-bloque" role="toolbar" aria-label="Acciones con las copias elegidas">
          <span class="cuantos">{seleccion.length ? plural(seleccion.length, "copia elegida", "copias elegidas") : "Elige copias"}</span>
          <button class="btn btn-sm btn-primary" disabled={!seleccion.length} onclick={() => (enBloque = true)}><Play size={14} />Copiar ahora</button>
          <button class="btn btn-sm btn-ghost" onclick={salir}><X size={14} />Terminar</button>
        </div>
      {/if}
    {:else}
      <div class="card">
        <Vacio icono={Search} ilustracion="sin-resultados" titulo="Ninguna copia coincide" texto="Prueba con otro nombre o quita los filtros.">
          {#if hayFiltro}<button class="btn btn-sm" onclick={quitarFiltros}><X size={14} />Quitar los filtros</button>{/if}
        </Vacio>
      </div>
    {/if}
  {/if}
</div>

{#if copiar && actual.cliente}
  {@const x = copiar}
  <OrdenDialog
    cliente={actual.cliente}
    equipo={x.equipo}
    tipo="copiar_ahora"
    cuerpo={{ repo: x.copia.repo, copia: x.copia.id }}
    descripcion="Se hará ahora la copia «{x.copia.nombre}» de {x.equipo.nombre}, sin esperar a su hora. No borra nada."
    accion="Copiar ahora"
    onclose={() => (copiar = null)}
  />
{/if}
{#if enBloque && actual.cliente}
  <AccionesEnBloque cliente={actual.cliente} equipos={equiposSeleccion} accion="copiar" copias={new Set(seleccion.map((x) => x.clave))} onclose={() => (enBloque = false)} />
{/if}

<style>
  .barra-filtros {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3);
  }
  .barra-filtros.segunda {
    margin-top: calc(-1 * var(--sp-2));
  }
  .buscar {
    position: relative;
    flex: 1;
    min-width: 240px;
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
  .sel {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .sel .input {
    width: auto;
    max-width: 16rem;
    height: 32px;
  }
  .hueco {
    flex: 1;
  }
  /* La fila se adapta al ancho de la lista (con la barra lateral, no al de la ventana). */
  .lista-copias {
    container-type: inline-size;
  }
  /* Cada copia: nombre y de dónde a dónde · estado · última y próxima · lo que protege · acciones. */
  .fila-copia {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 130px 210px 76px auto;
    align-items: center;
    gap: var(--sp-4);
    min-height: 56px;
    padding: 12px var(--sp-4);
    border-top: 1px solid var(--border);
  }
  .fila-copia:first-child {
    border-top: none;
  }
  .fila-copia.con-sel {
    grid-template-columns: 16px minmax(0, 1fr) 130px 210px 76px auto;
  }
  .fila-copia:hover {
    background: color-mix(in srgb, var(--surface-2) 70%, transparent);
  }
  .fila-copia.on {
    background: var(--accent-soft);
  }
  .principal {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px 8px;
    min-width: 0;
  }
  .nombre {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    color: var(--text-1);
    text-decoration: none;
  }
  .nombre strong {
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .nombre :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .nombre:hover strong {
    text-decoration: underline;
  }
  .sub {
    flex-basis: 100%;
    display: block;
    font-size: var(--fs-sm);
    color: var(--text-2);
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .sub.faint {
    font-size: var(--fs-xs);
  }
  .link-suave {
    color: inherit;
    text-decoration: none;
  }
  .link-suave:hover {
    color: var(--text-1);
    text-decoration: underline;
  }
  .msg-fallo {
    color: var(--bad);
    font-size: var(--fs-xs);
  }
  .vivo:empty {
    display: none;
  }
  .vivo {
    flex-basis: 100%;
  }
  .cuando {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: var(--fs-sm);
  }
  .cuando .faint {
    font-size: var(--fs-xs);
  }
  .protegido {
    font-size: var(--fs-sm);
    text-align: right;
    color: var(--text-2);
  }
  .et-movil {
    display: none;
  }
  .acciones {
    display: flex;
    gap: 2px;
    justify-content: flex-end;
  }
  .sel-todas {
    gap: 10px;
    font-size: var(--fs-sm);
    color: var(--text-2);
    background: var(--surface-2);
    cursor: pointer;
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
  @container (max-width: 860px) {
    .fila-copia {
      grid-template-columns: minmax(0, 1fr) auto auto;
      grid-template-areas: "p e a" "c c v";
      row-gap: 6px;
    }
    .fila-copia.con-sel {
      grid-template-columns: 16px minmax(0, 1fr) auto auto;
      grid-template-areas: "s p e a" ". c c v";
    }
    .sel-c {
      grid-area: s;
    }
    .principal {
      grid-area: p;
    }
    .estado {
      grid-area: e;
    }
    .acciones {
      grid-area: a;
    }
    .cuando {
      grid-area: c;
      flex-direction: row;
      flex-wrap: wrap;
      gap: 0 10px;
    }
    .protegido {
      grid-area: v;
      align-self: end;
    }
  }
  @media (max-width: 640px) {
    .segmented.inline {
      display: grid;
      width: 100%;
      grid-auto-columns: minmax(max-content, 1fr);
      overflow-x: auto;
    }
    .segmented.inline > button {
      padding: 0 8px;
    }
    .sel {
      flex: 1 1 100%;
      min-width: 0;
    }
    .sel .input {
      flex: 1;
      min-width: 0;
      max-width: none;
    }
    .sel > span {
      min-width: 5.5em;
    }
    .buscar {
      min-width: 0;
      flex-basis: 100%;
    }
  }
  @container (max-width: 480px) {
    .fila-copia,
    .fila-copia.con-sel {
      grid-template-columns: minmax(0, 1fr) auto;
      grid-template-areas: "p a" "e e" "c c" "v v";
    }
    .fila-copia.con-sel {
      grid-template-columns: 16px minmax(0, 1fr) auto;
      grid-template-areas: "s p a" ". e e" ". c c" ". v v";
    }
    .protegido {
      text-align: left;
    }
    .et-movil {
      display: inline;
      color: var(--text-3);
    }
  }
</style>
