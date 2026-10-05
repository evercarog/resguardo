<script lang="ts">
  // «Buscar archivos»: un archivo por su nombre en TODAS las versiones de un
  // repositorio (o de todos los de un equipo, uno tras otro), con unas fechas
  // opcionales. Cada repositorio se abre con su contraseña en una sesión
  // cifrada `explorar` y el equipo busca con `buscar_todas` (api-servidor.md
  // §7): los nombres llegan cifrados a este navegador y el servidor no los ve.
  // En la URL solo van el equipo, el repositorio y las fechas (nunca el texto
  // ni nombres de archivo); el detalle («Qué cambió») usa los parámetros del
  // panel de detalle (`v`, `vista`…), que solo llevan ids de versión.
  import { onDestroy, untrack } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { ArrowDownUp, CalendarRange, CircleCheck, Database, FileSearch, Filter, LoaderCircle, Monitor, RotateCcw, Search, SkipForward, TriangleAlert, X } from "@lucide/svelte";
  import { AccesoRepo } from "$lib/accesoRepo.svelte";
  import {
    conFiltros,
    diaDeHace,
    errorTexto,
    filtrarEncontrados,
    fraseRecorte,
    leerFiltros,
    MAX_COINCIDENCIAS,
    OP_BUSCAR,
    ordenarArchivos,
    rangoIso,
    TEXTO_MAX,
    unirBusqueda,
    type ArchivoEncontrado,
    type OrdenBusqueda,
    type PaginaBusqueda,
    type ResultadoRepo,
  } from "$lib/buscarArchivos";
  import { anteriorDeLaCopia, leerSeleccion, migasDe, partesRuta, versionPorId } from "$lib/detalle";
  import { actual, puede, reloj } from "$lib/estado.svelte";
  import { fechaCorta, fechaLarga, numero, plural } from "$lib/formato";
  import { cargarInformes, ultimos } from "$lib/informes.svelte";
  import { informeDe, versionesDe } from "$lib/repo";
  import type { MensajeEquipo } from "$lib/sesion";
  import type { Equipo, RepositorioResumen } from "$lib/tipos";
  import { tip } from "$lib/tooltip";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import Cajon from "$lib/componentes/detalle/Cajon.svelte";
  import CambiosVersion from "$lib/componentes/detalle/CambiosVersion.svelte";
  import DesbloquearRepo from "$lib/componentes/detalle/DesbloquearRepo.svelte";
  import VersionesArchivo from "$lib/componentes/detalle/VersionesArchivo.svelte";
  import { atras, cerrar, ir } from "$lib/componentes/detalle/navegar";
  import TarjetaArchivo from "$lib/componentes/buscar/TarjetaArchivo.svelte";

  const filtros = $derived(leerFiltros(page.url.searchParams));
  const puedeOrdenar = $derived(puede.ordenar(actual.cliente?.rol));

  /** Los equipos donde buscar: con algún repositorio y no trasladados. */
  const equipos = $derived(actual.equipos.filter((e) => e.confirmado && e.modo !== "trasladado" && (e.resumen?.repositorios ?? []).length));
  const equipo = $derived<Equipo | undefined>(equipos.find((e) => e.id === filtros.equipo) ?? (equipos.length === 1 ? equipos[0] : undefined));
  const repos = $derived<RepositorioResumen[]>(equipo?.resumen?.repositorios ?? []);
  /** El repositorio elegido o «todos» (con uno solo, ese). */
  const eleccion = $derived(repos.length === 1 ? repos[0].id : filtros.repo === "todos" || (filtros.repo && repos.some((r) => r.id === filtros.repo)) ? filtros.repo : repos.length ? "todos" : null);
  const objetivo = $derived(eleccion === "todos" ? repos : repos.filter((r) => r.id === eleccion));
  const nombreRepo = (id: string) => repos.find((r) => r.id === id)?.nombre ?? id;

  $effect(() => {
    // Los informes (para «Qué cambió»), sin seguir lo que leen.
    const [cc, ids] = [actual.id, actual.equipos.map((e) => e.id)];
    untrack(() => void cargarInformes(cc, ids));
  });

  /** Cambia los filtros de la URL (sin añadir pasos al historial). */
  function poner(f: Parameters<typeof conFiltros>[1]) {
    void goto(page.url.pathname + conFiltros(page.url.searchParams, f), { replaceState: true, keepFocus: true, noScroll: true });
  }

  // --- Sesiones: una por repositorio, mientras dure la página ---------------
  let accesos = $state<Record<string, AccesoRepo>>({});
  function accesoDe(repo: string): AccesoRepo | null {
    if (!equipo || !actual.cliente) return null;
    const a = accesos[repo];
    if (a && a.equipo.id === equipo.id) return a;
    a?.cerrar();
    const nuevo = new AccesoRepo(actual.cliente, equipo, repo);
    accesos[repo] = nuevo;
    return nuevo;
  }
  function cerrarAccesos() {
    for (const a of Object.values(accesos)) a.cerrar();
    accesos = {};
  }
  // Otro equipo: se cierran las sesiones del anterior y se olvida lo buscado.
  let equipoVisto = "";
  $effect(() => {
    const id = equipo?.id ?? "";
    if (id === equipoVisto) return;
    equipoVisto = id;
    untrack(() => {
      cerrarAccesos();
      reiniciar();
    });
  });
  onDestroy(cerrarAccesos);

  // --- La búsqueda ----------------------------------------------------------
  type Estado = "esperando" | "clave" | "abriendo" | "buscando" | "listo" | "actualizar" | "error" | "saltado";
  interface PorRepo {
    estado: Estado;
    mensaje?: string;
    progreso?: string;
    resultado?: ResultadoRepo;
    versiones?: { id: string; cuando: string }[];
  }
  let texto = $state("");
  let errorForm = $state("");
  let busqueda = $state<{ n: number; texto: string; desde: string | null; hasta: string | null; repos: string[] } | null>(null);
  let porRepo = $state<Record<string, PorRepo>>({});
  let contador = 0;

  function reiniciar() {
    busqueda = null;
    porRepo = {};
  }

  function buscar(e?: SubmitEvent) {
    e?.preventDefault();
    const mal = errorTexto(texto);
    errorForm = mal ?? "";
    if (mal || !objetivo.length) return;
    busqueda = { n: ++contador, texto: texto.trim(), desde: filtros.desde, hasta: filtros.hasta, repos: objetivo.map((r) => r.id) };
    porRepo = Object.fromEntries(objetivo.map((r) => [r.id, { estado: "esperando" as Estado }]));
    refinar = "";
    limite = PASO;
    void seguir();
  }

  /** Sigue con el siguiente repositorio pendiente (uno tras otro: una sesión cada vez). */
  async function seguir() {
    const b = busqueda;
    if (!b) return;
    for (const id of b.repos) {
      const x = porRepo[id];
      if (!x || !["esperando", "clave"].includes(x.estado)) continue;
      const a = accesoDe(id);
      if (!a) return;
      if (!a.abierta) {
        // Hace falta su contraseña: se pide y se espera (el efecto de abajo sigue al abrirse).
        if (x.estado !== "clave") porRepo[id] = { estado: "clave" };
        return;
      }
      await buscarEn(b, id, a);
      if (busqueda?.n !== b.n) return;
    }
  }

  // Al abrir un repositorio que esperaba su contraseña, se sigue buscando.
  $effect(() => {
    const b = busqueda;
    if (!b) return;
    const listos = b.repos.filter((id) => porRepo[id]?.estado === "clave" && accesos[id]?.abierta);
    if (listos.length) untrack(() => void seguir());
  });

  async function buscarEn(b: NonNullable<typeof busqueda>, id: string, a: AccesoRepo) {
    if (!a.admite(OP_BUSCAR)) {
      porRepo[id] = { estado: "actualizar" };
      return;
    }
    porRepo[id] = { estado: "buscando", progreso: `${a.equipo.nombre} lee las versiones de «${nombreRepo(id)}»…` };
    try {
      const vs = await a.pedir<MensajeEquipo & { versiones?: { id: string; cuando: string }[] }>("versiones", {}, "versiones");
      if (busqueda?.n !== b.n) return;
      const rango = rangoIso(b.desde, b.hasta);
      const enRango = (vs.versiones ?? []).filter((v) => (!rango.desde || Date.parse(v.cuando) >= Date.parse(rango.desde)) && (!rango.hasta || Date.parse(v.cuando) <= Date.parse(rango.hasta)));
      porRepo[id] = { estado: "buscando", versiones: enRango, progreso: `${a.equipo.nombre} busca «${b.texto}» en ${plural(enRango.length, "versión", "versiones")} de «${nombreRepo(id)}»…` };
      const paginas: PaginaBusqueda[] = [];
      let indice: number | null = 0;
      while (indice != null) {
        const p: MensajeEquipo & PaginaBusqueda = await a.pedir<MensajeEquipo & PaginaBusqueda>(OP_BUSCAR, { texto: b.texto, ...rango, max: MAX_COINCIDENCIAS, indice });
        if (busqueda?.n !== b.n) return;
        paginas.push(p);
        const siguiente: number | null | undefined = p.siguiente;
        indice = typeof siguiente === "number" && siguiente > indice ? siguiente : null;
        if (indice != null) porRepo[id] = { ...porRepo[id], progreso: `Recibiendo lo encontrado en «${nombreRepo(id)}»… ${numero(indice)} de ${numero(p.total_archivos ?? 0)}` };
      }
      porRepo[id] = { estado: "listo", versiones: enRango, resultado: unirBusqueda(id, paginas) };
    } catch (e) {
      if (busqueda?.n === b.n) porRepo[id] = { estado: "error", mensaje: (e as Error).message };
    }
  }

  function saltar(id: string) {
    porRepo[id] = { estado: "saltado" };
    void seguir();
  }
  function reintentar(id: string) {
    porRepo[id] = { estado: "esperando" };
    void seguir();
  }

  // --- Resultados -----------------------------------------------------------
  const PASO = 40;
  let limite = $state(PASO);
  let refinar = $state("");
  let orden = $state<OrdenBusqueda>("reciente");
  const todosLosArchivos = $derived(Object.values(porRepo).flatMap((x) => x.resultado?.archivos ?? []));
  const lista = $derived(ordenarArchivos(filtrarEncontrados(todosLosArchivos, refinar), orden));
  const visibles = $derived(lista.slice(0, limite));
  const enCurso = $derived(Object.values(porRepo).some((x) => x.estado === "esperando" || x.estado === "buscando" || x.estado === "abriendo"));
  const pideClave = $derived(busqueda?.repos.find((id) => porRepo[id]?.estado === "clave") ?? null);
  const terminada = $derived(!!busqueda && !enCurso && !pideClave);
  const varios = $derived((busqueda?.repos.length ?? 0) > 1);
  const recortes = $derived(
    Object.entries(porRepo)
      .map(([id, x]) => (x.resultado ? { id, frase: fraseRecorte(x.resultado) } : null))
      .filter((x): x is { id: string; frase: string } => !!x?.frase),
  );
  const totalVersiones = $derived(Object.values(porRepo).reduce((n, x) => n + (x.resultado?.versionesBuscadas ?? 0), 0));

  const enlaceRestaurar = (repo: string) => (version: string) => `/c/${actual.id}/restaurar?${new URLSearchParams({ equipo: equipo?.id ?? "", repo, version })}`;

  // --- Fechas ---------------------------------------------------------------
  const RAPIDAS = [
    { texto: "Cualquier fecha", dias: null },
    { texto: "Última semana", dias: 7 },
    { texto: "Último mes", dias: 30 },
    { texto: "Últimos 3 meses", dias: 90 },
  ] as const;
  const rapidaActiva = (dias: number | null) => (dias == null ? !filtros.desde && !filtros.hasta : filtros.desde === diaDeHace(dias, reloj.ahora) && !filtros.hasta);

  // --- Panel de detalle: «Ver todas sus versiones» y «Qué cambió» -----------
  const sel = $derived(leerSeleccion(page.url.searchParams));
  /** De qué repositorio es lo que se ve en el panel (y, sin URL, el archivo). */
  let cajon = $state<{ repo: string; archivo: string | null } | null>(null);
  const verCambios = $derived(!!cajon && !!sel.version && (sel.vista === "cambios" || sel.vista === "version"));
  const accesoCajon = $derived(cajon ? (accesos[cajon.repo] ?? null) : null);
  const infCajon = $derived(cajon && equipo ? informeDe(ultimos.porEquipo[equipo.id], cajon.repo) : null);
  const versionesCajon = $derived(versionesDe(infCajon));
  const versionCajon = $derived(versionPorId(versionesCajon, sel.version));
  // Al cambiar de versión en el panel (un enlace de «Versiones de este archivo»), se deja de ver el archivo.
  let selVista = "";
  $effect(() => {
    const k = `${sel.vista}|${sel.version}|${sel.con}`;
    if (k === selVista) return;
    selVista = k;
    untrack(() => {
      if (cajon?.archivo && sel.version) cajon = { ...cajon, archivo: null };
    });
  });

  function verVersiones(a: ArchivoEncontrado) {
    cajon = { repo: a.repo, archivo: a.ruta };
  }
  function queCambio(a: ArchivoEncontrado, version: string) {
    cajon = { repo: a.repo, archivo: null };
    ir({ vista: "cambios", version, con: null, filtro: "todos", vuelta: null });
  }
  function cerrarCajon() {
    cajon = null;
    if (sel.vista) cerrar();
  }
  function alAtras() {
    if (cajon?.archivo && verCambios) cajon = { ...cajon, archivo: null };
    else if (verCambios) atras(cerrarCajon);
    else cerrarCajon();
  }
  const nombreVersion = (id: string) => {
    const v = versionPorId(versionesCajon, id);
    return v ? `Versión del ${fechaCorta(v.hora)}` : `Versión ${id.slice(0, 8)}`;
  };
  const migasCajon = $derived([
    ...(verCambios ? migasDe({ ...sel, vista: "cambios" }, nombreVersion).slice(1) : []).map((m) => ({ texto: m.texto, ir: () => ir(m.sel) })),
    ...(cajon?.archivo ? [{ texto: partesRuta(cajon.archivo).nombre, ir: undefined }] : []),
  ]);
  /** La hora de una versión (de lo encontrado), si no está en el informe. */
  const horaDe = (repo: string, id: string) => porRepo[repo]?.versiones?.find((v) => v.id === id || v.id.startsWith(id) || id.startsWith(v.id))?.cuando;
</script>

<svelte:head><title>Buscar archivos · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <CabeceraPagina
    titulo="Buscar archivos"
    icono={FileSearch}
    migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${actual.id}` }, { texto: "Buscar archivos" }]}
    resumen="Encuentra un archivo por su nombre en todas las versiones guardadas: en cuáles está, cuándo cambió y cuánto ocupaba. Lo busca el equipo y llega cifrado: el servidor no ve los nombres."
  />

  {#if !puedeOrdenar && actual.cliente}
    <div class="notice notice-info"><p>Tu papel en este cliente es de solo lectura: no puede abrir los repositorios para buscar en ellos.</p></div>
  {:else if !actual.cargado}
    <Esqueleto forma="filas" n={3} />
  {:else if !equipos.length}
    <div class="card">
      <Vacio icono={FileSearch} ilustracion="sin-versiones" titulo="Todavía no hay dónde buscar" texto="Cuando un equipo tenga su primera copia, podrás buscar sus archivos en todas las versiones.">
        <a class="btn btn-primary" href="/c/{actual.id}/equipos">Ver los equipos</a>
      </Vacio>
    </div>
  {:else}
    <form class="card p buscador" onsubmit={buscar} role="search" aria-label="Buscar archivos en todas las versiones">
      <div class="fila-campos">
        <label class="campo">
          <span class="etq"><Monitor size={14} />Equipo</span>
          <select class="input" value={equipo?.id ?? ""} onchange={(e) => poner({ equipo: e.currentTarget.value || null, repo: null })}>
            {#if !equipo}<option value="">Elige un equipo…</option>{/if}
            {#each equipos as e (e.id)}<option value={e.id}>{e.nombre}{e.conectado ? "" : " (sin conexión)"}</option>{/each}
          </select>
        </label>
        <label class="campo">
          <span class="etq"><Database size={14} />Repositorio</span>
          <select class="input" value={eleccion ?? ""} disabled={!equipo || repos.length < 2} onchange={(e) => poner({ repo: e.currentTarget.value || null })}>
            {#if repos.length > 1}<option value="todos">Todos los repositorios del equipo ({repos.length})</option>{/if}
            {#each repos as r (r.id)}<option value={r.id}>{r.nombre}</option>{/each}
          </select>
        </label>
      </div>
      <div class="bq-texto">
        <label class="campo crece">
          <span class="etq"><Search size={14} />Nombre del archivo (o parte)</span>
          <input class="input grande" type="search" bind:value={texto} maxlength={TEXTO_MAX} placeholder="Por ejemplo: factura, contrato 2025, .xlsx" autocomplete="off" spellcheck="false" aria-describedby="ayuda-texto" aria-invalid={!!errorForm} />
        </label>
        <button class="btn btn-primary btn-buscar" disabled={!equipo || enCurso}>
          {#if enCurso}<LoaderCircle size={16} class="spin" />Buscando…{:else}<Search size={16} />Buscar{/if}
        </button>
      </div>
      <p id="ayuda-texto" class="faint pequeno">Sin distinguir mayúsculas. Se busca en el nombre, no en la carpeta ni dentro del archivo.</p>
      {#if errorForm}<p class="error-campo" role="alert"><TriangleAlert size={14} />{errorForm}</p>{/if}
      <fieldset class="fechas">
        <legend class="etq"><CalendarRange size={14} />Versiones de</legend>
        <div class="rapidas" role="group" aria-label="Fechas rápidas">
          {#each RAPIDAS as r (r.texto)}
            <button type="button" class="chip-filtro" class:on={rapidaActiva(r.dias)} aria-pressed={rapidaActiva(r.dias)} onclick={() => poner(r.dias == null ? { desde: null, hasta: null } : { desde: diaDeHace(r.dias, reloj.ahora), hasta: null })}>{r.texto}</button>
          {/each}
        </div>
        <div class="entre">
          <label><span class="faint">Desde</span><input class="input" type="date" value={filtros.desde ?? ""} max={filtros.hasta ?? undefined} onchange={(e) => poner({ desde: e.currentTarget.value || null })} /></label>
          <label><span class="faint">Hasta</span><input class="input" type="date" value={filtros.hasta ?? ""} min={filtros.desde ?? undefined} onchange={(e) => poner({ hasta: e.currentTarget.value || null })} /></label>
        </div>
      </fieldset>
    </form>

    {#if busqueda}
      <section class="resultados" aria-labelledby="t-resultados" aria-busy={enCurso}>
        <h2 id="t-resultados" class="sr-only">Resultados</h2>
        <!-- Cómo va cada repositorio -->
        <ul class="repos" aria-label="Repositorios">
          {#each busqueda.repos as id (id)}
            {@const x = porRepo[id]}
            <li class="repo-estado e-{x?.estado}">
              <Database size={14} />
              <span class="rn">{nombreRepo(id)}</span>
              {#if x?.estado === "buscando" || x?.estado === "esperando"}
                <span class="espera" role="status"><LoaderCircle size={14} class="spin" />{x.progreso ?? "En espera…"}</span>
              {:else if x?.estado === "clave"}
                <span class="faint">Hace falta su contraseña</span>
              {:else if x?.estado === "listo" && x.resultado}
                <span class="ok"><CircleCheck size={14} />{plural(x.resultado.archivos.length, "archivo", "archivos")} en {plural(x.resultado.versionesBuscadas, "versión", "versiones")}</span>
              {:else if x?.estado === "actualizar"}
                <span class="aviso"><TriangleAlert size={14} />Actualiza el agente de {equipo?.nombre} para buscar en todas las versiones. Mientras, puedes buscar dentro de una versión en&nbsp;<a href="/c/{actual.id}/restaurar?{new URLSearchParams({ equipo: equipo?.id ?? '', repo: id })}">Restaurar</a>.</span>
              {:else if x?.estado === "error"}
                <span class="mal"><TriangleAlert size={14} />{x.mensaje}</span>
                <button class="btn btn-sm btn-ghost" onclick={() => reintentar(id)}><RotateCcw size={14} />Reintentar</button>
              {:else if x?.estado === "saltado"}
                <span class="faint">Saltado</span>
              {/if}
            </li>
          {/each}
        </ul>

        {#if pideClave && accesos[pideClave]}
          <div class="desbloquear">
            {#key pideClave}
              <DesbloquearRepo acceso={accesos[pideClave]} nombreRepo={nombreRepo(pideClave)} para="buscar en sus versiones" />
            {/key}
            {#if varios}<button class="btn btn-sm btn-ghost bq-saltar" onclick={() => saltar(pideClave!)}><SkipForward size={14} />Saltar este repositorio</button>{/if}
          </div>
        {/if}

        {#if recortes.length}
          {#each recortes as r (r.id)}<div class="notice notice-info"><p>{#if varios}<strong>{nombreRepo(r.id)}:</strong>{" "}{/if}{r.frase}</p></div>{/each}
        {/if}

        {#if todosLosArchivos.length}
          <div class="barra-res">
            <p class="cuenta" aria-live="polite">
              <strong>{plural(todosLosArchivos.length, "archivo", "archivos")}</strong> con «{busqueda.texto}»{#if totalVersiones}{" "}en {plural(totalVersiones, "versión", "versiones")}{/if}{#if busqueda.desde || busqueda.hasta}{" "}<span class="faint">({busqueda.desde ? `desde ${fechaCorta(`${busqueda.desde}T12:00:00`)}` : ""}{busqueda.desde && busqueda.hasta ? " " : ""}{busqueda.hasta ? `hasta ${fechaCorta(`${busqueda.hasta}T12:00:00`)}` : ""})</span>{/if}
            </p>
            <div class="herramientas">
              {#if todosLosArchivos.length > 5}
                <label class="buscar">
                  <Filter size={14} />
                  <span class="sr-only">Filtrar lo encontrado por nombre o carpeta</span>
                  <input class="input" type="search" placeholder="Filtrar por nombre o carpeta" bind:value={refinar} oninput={() => (limite = PASO)} />
                </label>
              {/if}
              <label class="ordenar">
                <ArrowDownUp size={14} />
                <span class="sr-only">Ordenar</span>
                <select class="input" bind:value={orden}>
                  <option value="reciente">Los más recientes primero</option>
                  <option value="nombre">Por nombre</option>
                  <option value="tamano">Por tamaño</option>
                  <option value="cambios">Los que más cambiaron</option>
                </select>
              </label>
            </div>
          </div>
          {#if lista.length}
            <ul class="archivos">
              {#each visibles as a (`${a.repo}|${a.ruta}`)}
                <TarjetaArchivo
                  archivo={a}
                  texto={busqueda.texto}
                  versionesRepo={porRepo[a.repo]?.versiones ?? []}
                  nombreRepo={varios ? nombreRepo(a.repo) : null}
                  acceso={accesos[a.repo]?.abierta ? accesos[a.repo] : null}
                  puedeRestaurar={puedeOrdenar}
                  enlaceRestaurar={enlaceRestaurar(a.repo)}
                  alVerVersiones={() => verVersiones(a)}
                  alQueCambio={(v) => queCambio(a, v)}
                />
              {/each}
            </ul>
            {#if lista.length > limite}
              <button class="btn btn-ghost mas" onclick={() => (limite += PASO)}>Mostrar más ({numero(lista.length - limite)} más)</button>
            {/if}
          {:else}
            <p class="faint vacio-filtro">Nada de lo encontrado tiene «{refinar}» en el nombre o la carpeta. <button class="btn btn-sm btn-ghost" onclick={() => (refinar = "")}><X size={14} />Quitar el filtro</button></p>
          {/if}
        {:else if enCurso}
          <Esqueleto forma="filas" n={3} etiqueta="Buscando en las versiones…" />
        {:else if terminada && Object.values(porRepo).some((x) => x.estado === "listo")}
          <div class="card">
            <Vacio icono={FileSearch} titulo="Ningún archivo se llama así" texto={`No hay ningún archivo con «${busqueda.texto}» en el nombre en ${plural(totalVersiones, "versión", "versiones")}${busqueda.desde || busqueda.hasta ? " de esas fechas" : ""}. Prueba con una parte del nombre${busqueda.desde || busqueda.hasta ? " o con cualquier fecha" : ""}.`}>
              {#if busqueda.desde || busqueda.hasta}<button class="btn" onclick={() => poner({ desde: null, hasta: null })}>Buscar en cualquier fecha</button>{/if}
            </Vacio>
          </div>
        {/if}
        {#if terminada && todosLosArchivos.length}<p class="faint pie" use:tip={fechaLarga(new Date(reloj.ahora).toISOString())}>Leído de {equipo?.nombre}, cifrado de extremo a extremo.</p>{/if}
      </section>
    {:else}
      <div class="card consejo">
        <Vacio icono={FileSearch} titulo="¿Qué archivo buscas?" texto="Escribe su nombre, o una parte, y elige dónde. Verás en qué versiones está, cuándo cambió por última vez y podrás recuperar la que quieras." />
      </div>
    {/if}
  {/if}
</div>

{#if cajon && accesoCajon && (cajon.archivo || verCambios)}
  <Cajon titulo={cajon.archivo ? "Versiones de este archivo" : "Qué cambió"} migas={migasCajon} {alAtras} alCerrar={cerrarCajon}>
    {#if cajon.archivo}
      <VersionesArchivo acceso={accesoCajon} ruta={cajon.archivo} puedeRestaurar={puedeOrdenar} />
    {:else if verCambios && sel.version}
      <CambiosVersion
        acceso={accesoCajon}
        nombreRepo={nombreRepo(cajon.repo)}
        version={versionCajon ?? { id: sel.version, hora: horaDe(cajon.repo, sel.version) }}
        anterior={versionCajon ? anteriorDeLaCopia(versionesCajon, versionCajon) : null}
        con={sel.con}
        filtro={sel.filtro}
        versiones={versionesCajon}
        puedeRestaurar={puedeOrdenar}
        alVerArchivo={(r) => cajon && (cajon = { ...cajon, archivo: r })}
      />
    {/if}
  </Cajon>
{/if}

<style>
  .buscador {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .fila-campos {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-3);
  }
  .bq-texto {
    display: flex;
    align-items: flex-end;
    gap: var(--sp-2);
  }
  .campo {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .crece {
    flex: 1;
  }
  .etq {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--text-2);
  }
  .grande {
    font-size: var(--fs-body);
    min-height: 44px;
  }
  .btn-buscar {
    min-height: 44px;
  }
  .pequeno {
    margin: -4px 0 0;
    font-size: var(--fs-xs);
  }
  .error-campo {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--bad);
  }
  .fechas {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-4);
    margin: 0;
    padding: 0;
    border: 0;
  }
  .fechas legend {
    float: left;
    margin-right: var(--sp-2);
  }
  .rapidas,
  .entre {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .entre label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .entre input {
    width: auto;
  }
  .chip-filtro {
    min-height: 32px;
    padding: 4px 12px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface);
    color: var(--text-2);
    font: inherit;
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  .chip-filtro.on {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent-text, var(--accent));
    font-weight: 600;
  }
  .chip-filtro:focus-visible {
    outline: 2px solid var(--focus, var(--accent));
    outline-offset: 2px;
  }
  .resultados {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    margin-top: var(--sp-4);
  }
  .repos {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .repo-estado {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px var(--sp-2);
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .rn {
    font-weight: 600;
    color: var(--text-1);
  }
  .espera,
  .ok,
  .aviso,
  .mal {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .ok {
    color: var(--ok);
  }
  .aviso {
    color: var(--warn);
  }
  .mal {
    color: var(--bad);
  }
  .desbloquear {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    max-width: 40rem;
  }
  .bq-saltar {
    align-self: flex-start;
  }
  .barra-res {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2) var(--sp-4);
  }
  .cuenta {
    margin: 0;
  }
  .herramientas {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }
  .herramientas label {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .archivos {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin: 0;
    padding: 0;
  }
  .mas {
    align-self: center;
  }
  .vacio-filtro,
  .pie {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .pie {
    font-size: var(--fs-xs);
  }
  .consejo {
    margin-top: var(--sp-4);
  }
  @media (max-width: 640px) {
    .fila-campos {
      grid-template-columns: minmax(0, 1fr);
    }
    .bq-texto {
      flex-direction: column;
      align-items: stretch;
    }
    .fechas legend {
      float: none;
    }
    .barra-res,
    .herramientas,
    .herramientas label,
    .herramientas select,
    .herramientas input {
      width: 100%;
    }
  }
</style>
