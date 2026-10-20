<script lang="ts">
  import PruebasEspejo from "$lib/componentes/pruebas/PruebasEspejo.svelte";
  import { listaFuncionales } from "$lib/funcionales.svelte";
  import { tip } from "$lib/tooltip";
  // Un repositorio en detalle, como en la app de escritorio: la frase que lo
  // resume, el camino de los datos, la salud de la protección, las cifras,
  // los cuadros de 60 días, las gráficas, las versiones y su historia.
  // Todo sale del resumen del equipo y de su último informe (v1.7 §6).
  import IndicePagina from "$lib/componentes/IndicePagina.svelte";
  import DondeEsta from "$lib/componentes/DondeEsta.svelte";
  import Migas from "$lib/componentes/Migas.svelte";
  import { page } from "$app/state";
  import { untrack } from "svelte";
  import { seguirCambios, tocaEquipo } from "$lib/vivo.svelte";
  import { ArrowRightLeft, Database, History, Pencil, Play, ShieldCheck, TriangleAlert } from "@lucide/svelte";
  import { destinoEspejo, espejosDelRepo, nombreEspejo, sinActualizar, textoSinActualizar } from "$lib/espejosRepo";
  import { hrefEspejo } from "$lib/espejoTrabajos";
  import * as api from "$lib/api";
  import { actual, puede, reloj } from "$lib/estado.svelte";
  import { bytes, fechaLarga, numero, relativo } from "$lib/formato";
  import {
    anadidoDe,
    bytesRepo,
    destinoDe,
    duracion,
    duracionMedia,
    fraseRepo,
    informeDe,
    nVersiones,
    proteccion,
    pruebaRestauracion,
    lineaEnDisco,
    TEXTO_RESULTADO,
    TEXTO_TAREA,
    TONO_RESULTADO,
    TONO_TAREA,
    ultimaEjecucion,
    verificacion,
    versionesDe,
  } from "$lib/repo";
  import type { Equipo, EquipoDetalle, VersionInforme } from "$lib/tipos";
  import { almacenDe, textoHorario, textoRegla } from "$lib/retencion";
  import { ordenQuitarExterna } from "$lib/copiaExterna";
  import { textoDestinoAusente } from "$lib/datosEquipo";
  import { admiteVerificacion, fraseVerificacion } from "$lib/verificacion";
  import RetencionAlmacen from "$lib/componentes/RetencionAlmacen.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import OrdenDialog from "$lib/componentes/OrdenDialog.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import AnilloProteccion from "$lib/componentes/repo/AnilloProteccion.svelte";
  import FlujoRepo from "$lib/componentes/repo/FlujoRepo.svelte";
  import GraficaBarras from "$lib/componentes/repo/GraficaBarras.svelte";
  import HistorialVersiones from "$lib/componentes/repo/HistorialVersiones.svelte";
  import { usarHistorialEquipo } from "$lib/historialEquipo.svelte";
  import SaludProteccion from "$lib/componentes/repo/SaludProteccion.svelte";
  import MenuAcciones from "$lib/componentes/MenuAcciones.svelte";
  import TraerHistorial from "$lib/componentes/TraerHistorial.svelte";
  import PasosRepo from "$lib/componentes/PasosRepo.svelte";
  import { reglaEfectiva } from "$lib/lineaTiempo";
  import { saludCompleta } from "$lib/saludCompleta";
  import { caminoDeCopia, hrefDerivada } from "$lib/camino";
  import { enVivoCamino } from "$lib/caminoVivo.svelte";
  import { reglaDeCopia } from "$lib/regla321";
  import { catalogoDe, cargarCatalogo } from "$lib/catalogoDestinos.svelte";
  // «Pulsar para ver más»: el panel de detalle (versión, qué cambió, espacio…) según la URL.
  import PanelDetalle from "$lib/componentes/detalle/PanelDetalle.svelte";
  import { abrirEspacio, abrirEstado, abrirVersion, abrirVuelta, elegirDia, elegirFechas, ir } from "$lib/componentes/detalle/navegar";
  import { leerSeleccion } from "$lib/detalle";
  import "$lib/componentes/detalle/pulsable.css";
  import Observaciones from "$lib/componentes/notas/Observaciones.svelte";
  import Comentarios from "$lib/componentes/notas/Comentarios.svelte";
  import { objetoDe } from "$lib/notas.svelte";
  // v1.41: dónde se guarda, el aviso «copias en el mismo equipo» y «Mover a otro sitio…».
  import { comprobacionLugar, lugarRepo, riesgoMismoEquipo } from "$lib/dondeGuarda";
  import SeGuardaEn from "$lib/componentes/SeGuardaEn.svelte";
  import AvisoMismoEquipo from "$lib/componentes/AvisoMismoEquipo.svelte";
  import MoverRepositorio from "$lib/componentes/MoverRepositorio.svelte";
  // v1.47: lo que está en marcha en él (también un «Mover a otro sitio…» que empezó otra consola).
  import EnMarcha from "$lib/componentes/EnMarcha.svelte";
  import MoviendoseAviso from "$lib/componentes/MoviendoseAviso.svelte";
  import { tareasDe } from "$lib/progreso.svelte";
  import { hayPlanMover, moviendoDe } from "$lib/mover";
  // Plan 0.8.0, B1: cambiar el nombre (todas las consolas) y ordenar su carpeta en el almacén.
  import RenombrarRepositorio from "$lib/componentes/RenombrarRepositorio.svelte";
  import OrdenarCarpetas from "$lib/componentes/OrdenarCarpetas.svelte";
  import { admiteNombres, porOrdenar as porOrdenarEn } from "$lib/nombres";
  import AvisoOrdenar from "$lib/componentes/almacen/AvisoOrdenar.svelte";

  const c = $derived(page.params.c ?? "");
  const e = $derived(page.params.e ?? "");
  const rid = $derived(page.params.r ?? "");
  const sel = $derived(leerSeleccion(page.url.searchParams));

  let equipo = $state<EquipoDetalle | null>(null);
  let error = $state("");
  $effect(() => {
    void e;
    equipo = null;
    api
      .equipo(c, e)
      .then((x) => ((equipo = x), (error = "")))
      .catch((x) => (error = (x as Error).message));
  });

  // v1.23: el historial que guarda el propio equipo (lo de antes, verificaciones, subidas…), por páginas y en vivo.
  const historia = usarHistorialEquipo(() => c, () => e);
  // Al día sin recargar: el equipo (versiones, espacio…).
  $effect(() => {
    const [cc, ee] = [c, e];
    return untrack(() =>
      seguirCambios(() => api.equipo(cc, ee).then((x) => ee === e && (equipo = x), () => {}), {
        ms: 0,
        toca: (x) => (x.t === "informe" || x.t === "config" || x.t === "equipo" || (x.t === "progreso" && x.estado === "termina")) && tocaEquipo(x, ee),
      }),
    );
  });

  const repo = $derived(equipo?.resumen?.repositorios?.find((r) => r.id === rid));
  const inf = $derived(informeDe(equipo?.ultimo_informe, rid));
  const copias = $derived(equipo?.resumen?.copias ?? []);
  const suyas = $derived(copias.filter((k) => k.repo === rid));
  const destinos = $derived(equipo?.resumen?.destinos ?? []);
  const destino = $derived(destinoDe(destinos, repo));
  const lugar = $derived(equipo && repo ? lugarRepo(repo, equipo, actual.equipos) : null);
  const riesgo = $derived(equipo && repo ? riesgoMismoEquipo(repo, equipo, actual.equipos) : null);
  // 14w: la salud del agente, completada con lo que sabe la consola (espejos del almacén y del
  // equipo, derivadas, lo marcado en cada destino, la retención del almacén): lo mismo que la tira 3-2-1.
  $effect(() => {
    const cc = c;
    if (cc) untrack(() => void cargarCatalogo(cc));
  });
  const reglaRepo = $derived.by(() => {
    if (!equipo || !repo) return null;
    const eq = equipo;
    const k = copias.find((x) => x.repo === repo!.id && x.activa !== false);
    return k ? reglaDeCopia(eq, k, actual.equipos.map((x) => (x.id === eq.id ? eq : x)), eq.ultimo_informe ?? null, catalogoDe(c), reloj.ahora, actual.organizacion, listaFuncionales(c), actual.infra) : null;
  });
  // 15g: el mismo camino que la página de la copia (espejos y copias derivadas, con su estado).
  const pasosRepo = $derived(caminoDeCopia(reglaRepo, { cliente: c, ahora: reloj.ahora, enVivo: enVivoCamino, equipos: actual.equipos, informe: equipo?.ultimo_informe ?? null }).filter((x) => x.clase !== "copia"));
  const prot = $derived(saludCompleta(inf, { extra: equipo && repo ? comprobacionLugar(repo, equipo, actual.equipos) : null, rc: reglaRepo, retencionAlmacen: reglaEfectiva(repo, destino, actual.equipos)?.quien ?? null, ahora: reloj.ahora }));
  // 15m: los espejos que lo incluyen o lo incluyeron (con su última subida) y los que dejó.
  const espejosRepo = $derived(equipo && repo ? espejosDelRepo(equipo, repo, actual.equipos.map((x) => (x.id === equipo!.id ? equipo! : x))) : []);
  const fuera = $derived(sinActualizar(espejosRepo));
  const ej = $derived(ultimaEjecucion(inf));
  const verif = $derived(repo ? verificacion(repo, inf) : null);
  const prueba = $derived(repo ? pruebaRestauracion(repo, inf) : null);
  const media = $derived(duracionMedia(inf));
  const versiones = $derived(versionesDe(inf));
  const rol = $derived(equipo?.modo === "trasladado" ? "lectura" : actual.cliente?.rol);
  const estado = $derived(
    ej ? { tono: TONO_RESULTADO[ej.resultado], texto: ej.resultado === "ok" ? "Al día" : TEXTO_RESULTADO[ej.resultado] } : repo && nVersiones(repo, inf) ? { tono: "ok" as const, texto: "Con versiones" } : { tono: "neutral" as const, texto: "Sin versiones todavía" },
  );
  // La retención que se le aplica (las versiones guardadas la simulan).
  const retencionLinea = $derived(reglaEfectiva(repo, destino, actual.equipos));
  const enlace = (r: string, v: VersionInforme, todo: boolean) => `/c/${c}/recuperar?${new URLSearchParams({ equipo: e, repo: r, version: v.id, ...(todo ? { todo: "1" } : {}) })}`;

  // Órdenes desde aquí (con su diálogo de siempre). `para`: a otro equipo (el almacén).
  let dialogo = $state<{ tipo: string; cuerpo: Record<string, unknown>; descripcion: string; accion?: string; titulo?: string; para?: Equipo; repo?: { id: string; nombre: string } } | null>(null);

  // v1.22: en un almacén (solo añadir), la retención la aplica el almacén.
  const enAlm = $derived(repo ? almacenDe(repo, destino, actual.equipos) : null);
  // «?retencion=1»: desde «Retención en el almacén…» de la página del equipo.
  let retAlm = $state(page.url.searchParams.get("retencion") === "1");
  function aplicarEnAlmacen() {
    if (!enAlm || !repo) return;
    dialogo = {
      tipo: "aplicar_retencion_almacen",
      cuerpo: { usuario: enAlm.usuario, repo: enAlm.carpeta, nombre: `${repo.nombre}, de ${equipo?.nombre ?? ""}` },
      descripcion: `${enAlm.almacen.nombre} borrará ahora de «${repo.nombre}» las versiones que ya no entren en su retención (lo hace él, en local) en cuanto pase la espera, sin aguardar a su próxima hora.`,
      para: enAlm.almacen,
    };
  }
  function dejarDeAplicar() {
    if (!enAlm || !repo) return;
    dialogo = {
      tipo: "retencion_almacen",
      cuerpo: { usuario: enAlm.usuario, repo: enAlm.carpeta, quitar: true, nombre: `${repo.nombre}, de ${equipo?.nombre ?? ""}` },
      titulo: "Dejar de aplicar la retención",
      descripcion: `${enAlm.almacen.nombre} dejará de podar «${repo.nombre}» y borrará del repositorio su clave (ya no podrá abrirlo). No borra versiones: el repositorio volverá a crecer sin límite.`,
      para: enAlm.almacen,
    };
  }
  // «?traer=1»: al terminar «Copiar en …» para traer el historial de otro repositorio.
  let traer = $state(page.url.searchParams.get("traer") === "1");
  // «?mover=1»: desde el aviso de Estado («Mover a un almacén…»).
  let mover = $state(page.url.searchParams.get("mover") === "1");
  // Moviéndose ya (desde otra consola u otro navegador): aquí no se puede empezar otro ni llevar sus pasos.
  const moviendo = $derived(moviendoDe(tareasDe(e), rid));
  const planAqui = $derived(!!moviendo && hayPlanMover(c, e, moviendo.origen ?? rid));
  const moverBloqueado = $derived(!!moviendo && (moviendo.otra_consola || !planAqui));
  // 14nn: «Quitar la copia externa» también aquí, con la misma orden que la ficha del equipo.
  const puedeQuitarExterna = $derived(!!repo?.externa && !repo.solo_lectura && puede.ordenar(rol));
  function quitarExterna() {
    if (repo) dialogo = ordenQuitarExterna(repo, destinos);
  }
  let renombrar = $state(false);
  let ordenar = $state(false);
  /** B1 y 15h: ¿su carpeta en el almacén aún no es legible? Con lo que volverían a subir sus espejos. */
  const porOrdenar = $derived(enAlm && repo && !repo.solo_lectura ? porOrdenarEn(enAlm.almacen, actual.equipos, e, rid) : []);
  // 14bbb: «Comprobar la contraseña» (el equipo la prueba de verdad contra el repositorio).
  const admiteComprobar = $derived(!!equipo?.resumen?.admite?.includes("comprobar_contrasena"));
  const comprobada = $derived(repo?.contrasena_comprobada ?? null);
  let comprobar = $state(false);
  const TEXTO_COMPROBADA = { abre: "La abre", no_abre: "No la abre", sin_respuesta: "Sin respuesta" } as const;
  const TONO_COMPROBADA = { abre: "ok", no_abre: "bad", sin_respuesta: "warn" } as const;
  /** 14nn: quitar una copia derivada cuyo destino ya no existe (o cualquiera), también desde aquí. */
  function quitarDerivada(id: string, destino: string | null) {
    if (!repo) return;
    dialogo = {
      tipo: "quitar_derivada",
      cuerpo: { repo: rid, id },
      titulo: "Quitar la copia derivada",
      descripcion: `«${repo.nombre}» dejará de copiarse a «${destino ?? "su destino"}». Lo ya copiado allí se queda.`,
      repo: { id: rid, nombre: repo.nombre },
    };
  }
  const copiarAhora = (k: (typeof suyas)[number]) =>
    (dialogo = { tipo: "copiar_ahora", cuerpo: { repo: k.repo, copia: k.id }, descripcion: `Se hará ahora la copia «${k.nombre}», sin esperar a su hora. No borra nada.`, accion: "Copiar ahora" });
</script>

<svelte:head><title>{repo?.nombre ?? "Repositorio"} · {equipo?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <Migas items={[{ texto: "Equipos", href: `/c/${c}/equipos` }, { texto: equipo?.nombre ?? "Equipo", href: `/c/${c}/equipos/${e}` }, { texto: repo?.nombre ?? "Repositorio" }]} />

  {#if error && !equipo}
    <div class="notice notice-danger"><p>{error}</p></div>
  {:else if !equipo}
    <Cargando forma="ficha" filas={4} />
  {:else if !repo}
    <div class="notice notice-info"><p>{equipo.nombre} ya no tiene ese repositorio.</p></div>
  {:else}
    <header class="cab">
      <span class="page-icon"><Database size={22} /></span>
      <div class="cab-texto">
        <div class="titulo">
          <h1>{repo.nombre}</h1>
          {#if admiteNombres(equipo) && puede.administrar(rol) && !repo.solo_lectura}<button class="btn btn-ghost btn-sm" use:tip={"Cambiar el nombre (en todas sus consolas)"} aria-label="Cambiar el nombre" onclick={() => (renombrar = true)}><Pencil size={14} /></button>{/if}
          <button class="pulsable-bloque chip-pulsable" use:tip={"¿Por qué? Ver detalle"} onclick={abrirEstado}><Chip tono={estado.tono} texto={estado.texto} /></button>
          {#if repo.solo_lectura}<span class="badge badge-sm tone-neutral">Solo lectura</span>{/if}
        </div>
        <p class="sub">Copias de <a class="link-suave" href="/c/{c}/equipos/{e}">{equipo.nombre}</a></p>
      </div>
      {#if puede.ordenar(rol)}
        <div class="page-actions">
          {#if nVersiones(repo, inf) || repo.solo_lectura}<a class="btn btn-primary" href="/c/{c}/recuperar?equipo={e}&repo={rid}"><History size={16} />Restaurar</a>{/if}
          {#if suyas.length === 1}
            <button class="btn" onclick={() => copiarAhora(suyas[0])}><Play size={16} />Copiar ahora</button>
          {:else if suyas.length > 1}
            <MenuAcciones texto="Copiar ahora" etiqueta="Elegir qué copia hacer ahora" grupos={[suyas.map((k) => ({ texto: k.nombre, onclick: () => copiarAhora(k) }))]} />
          {/if}
          {#if nVersiones(repo, inf)}
            <button class="btn btn-ghost" onclick={() => (dialogo = { tipo: "verificar_ahora", cuerpo: { repo: rid }, descripcion: `Se comprobará ahora una parte de «${repo!.nombre}» para confirmar que las copias se pueden leer.`, accion: "Verificar ahora" })}><ShieldCheck size={16} />Verificar</button>
          {/if}
          {#if !repo.solo_lectura && puede.administrar(rol)}
            <button class="btn btn-ghost" use:tip={"Copiar aquí las versiones de otro repositorio, p. ej. el de la app de escritorio"} onclick={() => (traer = true)}><History size={16} />Traer historial</button>
            <button
              class="btn btn-ghost"
              disabled={moverBloqueado}
              use:tip={moverBloqueado ? "Ya se está moviendo: los pasos los lleva quien lo empezó" : "Llevar este repositorio, con todo su historial, a otro destino (p. ej. el almacén de otro equipo)"}
              onclick={() => (mover = true)}><ArrowRightLeft size={16} />Mover a otro sitio…</button
            >
          {/if}
        </div>
      {/if}
    </header>
    {#if lugar}<p class="donde"><SeGuardaEn enlace {lugar} riesgo={!!riesgo} /></p>{/if}
    <!-- 15h: aviso suave, con lo que volverían a subir sus espejos. -->
    {#if porOrdenar.length && puede.administrar(rol)}<AvisoOrdenar items={porOrdenar} repo onordenar={() => (ordenar = true)} />{/if}
    <!-- Lo que se ajusta del repositorio, a la vista (docs/editor-de-copias.md). -->
    {#if actual.cliente && puede.ordenar(rol)}
      <PasosRepo cliente={actual.cliente} {equipo} {repo} equipos={actual.equipos} administra={puede.administrar(rol)} ordena={puede.ordenar(rol)} copia={suyas.length === 1 ? suyas[0].id : null} alCambiar={() => api.equipo(c, e).then((x) => (equipo = x)).catch(() => {})} />
    {/if}
    <MoviendoseAviso equipo={e} repo={rid} onseguir={planAqui && moviendo?.origen === rid ? () => (mover = true) : undefined} />
    <EnMarcha equipo={e} repo={rid} tipos={["verificar", "verificar_externa", "copia_externa", "prueba_restauracion", "historial", "retencion", "restauracion"]} sinMover marco />
    {#if riesgo}
      <AvisoMismoEquipo {riesgo} onmover={!repo.solo_lectura && puede.administrar(rol) && !moverBloqueado ? () => (mover = true) : undefined} hrefExterna={puede.ordenar(rol) && suyas.length ? `/c/${c}/equipos/${e}?externa=${encodeURIComponent(rid)}` : undefined} />
    {/if}
    <Observaciones tipo="repositorio" objeto={objetoDe(e, rid)} />
    {#each fuera as x (`${x.hace.id}|${x.trabajo.id}`)}
      <!-- 15m (2): salió de un espejo: su copia allí ya no se actualiza. -->
      <div class="notice notice-warn" role="note"><TriangleAlert size={16} /><p>{textoSinActualizar(x, reloj.ahora)} <a class="link" href={hrefEspejo(c, x.hace.id, x.trabajo.id)}>Ver el espejo</a></p></div>
    {/each}

    <IndicePagina
      items={[
        { id: "sec-resumen", texto: "Resumen" },
        { id: "t-flujo", texto: "Cómo se protege" },
        { id: "sec-donde", texto: "Dónde está" },
        { id: "sec-proteccion", texto: "Protección" },
        ...(versiones.length >= 2 ? [{ id: "sec-graficas", texto: "Gráficas" }] : []),
        { id: "t-historial", texto: "Historial y versiones" },
      ]}
    />

    <p class="frase" id="sec-resumen">{fraseRepo(repo, inf, copias, reloj.ahora, comprobacionLugar(repo, equipo, actual.equipos))}</p>

    <div class="cifras">
      <div class="cifra">
        <span class="k">Versiones</span>
        <strong class="num">{numero(nVersiones(repo, inf))}</strong>
        <span class="faint">{repo.ultima_version || versiones[0] ? `la última ${relativo(repo.ultima_version ?? versiones[0].hora, reloj.ahora)}` : "todavía ninguna"}</span>
      </div>
      <button class="cifra pulsable-bloque" use:tip={"Ver de dónde sale el espacio"} onclick={abrirEspacio}>
        <span class="k">Protegido</span>
        <strong class="num">{bytes(bytesRepo(repo, inf) ?? inf?.espacio?.sin_comprimir)}</strong>
        <span class="faint">tus archivos en la última versión</span>
      </button>
      <button class="cifra pulsable-bloque" use:tip={"Ver de dónde sale el espacio"} onclick={abrirEspacio}>
        <span class="k">En disco</span>
        <strong class="num">{bytes(inf?.espacio?.en_disco_bytes)}</strong>
        <span class="faint">{lineaEnDisco(nVersiones(repo, inf), versiones, inf?.espacio?.en_disco_bytes, !!inf?.recortado)}</span>
      </button>
      <div class="cifra">
        <span class="k">Duración media</span>
        <strong class="num">{duracion(media)}</strong>
        <span class="faint">{versiones.length ? `de las últimas ${numero(versiones.length)} copias` : "sin datos todavía"}</span>
      </div>
    </div>

    <FlujoRepo {repo} {inf} {copias} {destinos} {equipo} equipos={actual.equipos} ahora={reloj.ahora} alQuitarExterna={puedeQuitarExterna ? quitarExterna : undefined} pasos={reglaRepo ? pasosRepo : null} />
    <!-- 15m (3): los espejos que lo incluyen y los que lo incluyeron, con su última subida. -->
    {#if espejosRepo.length}
      <section class="card p espejos-repo" aria-labelledby="t-espejos">
        <h2 class="section-title" id="t-espejos">Sus espejos <Ayuda id="espejo" /></h2>
        <ul class="lista-espejos">
          {#each espejosRepo as x (`${x.hace.id}|${x.trabajo.id}`)}
            <li>
              <a class="link-suave" href={hrefEspejo(c, x.hace.id, x.trabajo.id)}>{nombreEspejo(x.trabajo)}</a>
              <span class="faint">→ {destinoEspejo(x.trabajo, x.hace)}{x.hace.id !== e ? ` · lo hace ${x.hace.nombre}` : ""}</span>
              {#if x.incluye}
                <Chip pequeno tono={x.trabajo.activo ? "ok" : "paused"} texto={x.trabajo.activo ? "Lo incluye" : "En pausa"} />
              {:else}
                <Chip pequeno tono="warn" texto="Ya no lo incluye" />
              {/if}
              <span class="faint">{x.ultima ? `última subida ${relativo(x.ultima, reloj.ahora)}` : "todavía sin subida"}</span>
            </li>
          {/each}
        </ul>
        {#if fuera.length}<p class="faint nota-espejos">Lo que ya no incluye se queda en su destino, sin actualizarse: la retención del espejo solo mira lo que incluye. Si ya no lo quieres allí, bórralo a mano.</p>{/if}
      </section>
    {/if}
    <!-- Plan 0.8.0, E4 (14j): probar a restaurar desde cada espejo de este repositorio. -->
    {#if actual.cliente}<PruebasEspejo cliente={actual.cliente} {equipo} rc={reglaRepo} equipos={actual.equipos} ordena={puede.ordenar(rol)} />{/if}
    <!-- Plan 0.8.1, C1 (15n): todos los sitios donde están sus datos, con su ruta real. -->
    <DondeEsta {equipo} {repo} rc={reglaRepo} informe={equipo.ultimo_informe} cliente={c} />

    <div class="dos" id="sec-proteccion">
      {#if prot}
        <SaludProteccion proteccion={prot} nombre={repo.nombre} />
      {:else}
        <section class="card p sin-salud">
          <h2 class="section-title">Salud de la protección <Ayuda id="salud-proteccion" /></h2>
          <p class="faint">Llegará con el próximo informe de {equipo.nombre}: copias automáticas, protección contra borrado, fuera del sitio, verificación, prueba de restauración, kit y retención.</p>
        </section>
      {/if}
      <section class="card p mantenimiento">
        <h2 class="section-title">Comprobaciones</h2>
        <dl>
          <div>
            <dt>Verificación</dt>
            <dd>
              {#if verif?.ultima}<Chip pequeno tono={TONO_TAREA[verif.resultado]} texto={TEXTO_TAREA[verif.resultado]} /> <Tiempo iso={verif.ultima} />{:else}<span class="faint">Todavía no</span>{/if}
              {#if verif?.mensaje_corto}<span class="faint msg">{verif.mensaje_corto}</span>{/if}
              <!-- v1.28: la automática del equipo (cada N días, rotativa) y, en un almacén, la del almacén. -->
              {#if repo.verificacion_auto}
                {@const va = repo.verificacion_auto}
                <span class="msg">En {equipo.nombre}: {fraseVerificacion(va).replace(/^./, (x) => x.toLowerCase())}</span>
                {#if va.proxima}<span class="faint msg">La próxima, <Tiempo iso={va.proxima} />.{#if va.todo_leido}{" "}Todo leído por última vez <Tiempo iso={va.todo_leido} />.{/if}</span>{/if}
              {:else if !repo.solo_lectura}
                <span class="faint msg">{admiteVerificacion(equipo) ? `Sin verificación automática en ${equipo.nombre}.` : `Para programarla, actualiza el agente de ${equipo.nombre}.`}</span>
              {/if}
              {#if enAlm?.retencion}
                <span class="faint msg">
                  {enAlm.retencion.verificar
                    ? `En ${enAlm.almacen.nombre}, con su clave: comprueba la estructura del repositorio después de aplicar la retención (${enAlm.retencion.horario_texto ?? textoHorario(enAlm.retencion.horario)}).`
                    : `En ${enAlm.almacen.nombre}: no comprueba el repositorio al aplicar la retención (se activa en «Retención en el almacén»).`}
                </span>
              {/if}
              {#if !repo.solo_lectura && admiteVerificacion(equipo) && puede.administrar(rol)}
                <span class="msg"><a class="btn btn-sm" href="/c/{c}/equipos/{e}/copias?verificacion={encodeURIComponent(rid)}">{repo.verificacion_auto ? "Cambiar…" : "Verificar automáticamente…"}</a></span>
              {/if}
            </dd>
          </div>
          <div>
            <dt>Prueba de restauración</dt>
            <dd>
              {#if prueba?.ultima}<Chip pequeno tono={TONO_TAREA[prueba.resultado]} texto={TEXTO_TAREA[prueba.resultado]} /> <Tiempo iso={prueba.ultima} />{:else}<span class="faint">Todavía no</span>{/if}
              {#if prueba?.mensaje_corto}<span class="faint msg">{prueba.mensaje_corto}</span>{/if}
            </dd>
          </div>
          <div id="kit">
            <dt>Kit y contraseña</dt>
            <dd>
              {#if comprobada?.ultima}
                <Chip pequeno tono={TONO_COMPROBADA[comprobada.resultado] ?? "neutral"} texto={TEXTO_COMPROBADA[comprobada.resultado] ?? comprobada.resultado} /> <Tiempo iso={comprobada.ultima} />
                {#if comprobada.bien && comprobada.bien !== comprobada.ultima}<span class="faint msg">La última vez que la abrió: <Tiempo iso={comprobada.bien} />.</span>{/if}
                {#if comprobada.resultado === "no_abre"}<span class="msg aviso-ret">La contraseña que tiene guardada {equipo.nombre} no abrió el repositorio: revísalo cuanto antes.</span>{/if}
              {:else}
                <span class="faint">Contraseña sin comprobar todavía</span>
              {/if}
              <span class="faint msg">Escribe la contraseña del kit y {equipo.nombre} prueba si abre el repositorio de verdad (y sus copias con la misma). Los fallos cuentan como intentos.</span>
              {#if !repo.solo_lectura && puede.ordenar(rol)}
                <span class="msg">
                  {#if admiteComprobar}
                    <button type="button" class="btn btn-sm" onclick={() => (comprobar = true)}>Comprobar la contraseña</button>
                  {:else}
                    <span class="faint">Para comprobarla, actualiza el agente de {equipo.nombre}.</span>
                  {/if}
                </span>
              {/if}
            </dd>
          </div>
          <div>
            <dt>Copias derivadas</dt>
            <dd>
              {#if repo.externa}
                <span class="msg">
                  {#if inf?.externa?.ultima}<Chip pequeno tono={TONO_TAREA[inf.externa.resultado]} texto={TEXTO_TAREA[inf.externa.resultado]} /> <Tiempo iso={inf.externa.ultima} />{" · "}{/if}
                  <a class="link-suave" href={hrefDerivada(c, e, rid, "externa")}>A «{repo.externa.destino}» cada día a las {repo.externa.hora}</a>{#if !inf?.externa?.ultima}<span class="faint">, todavía sin ninguna</span>{/if}
                </span>
                {#if inf?.externa?.resultado === "fallo" && inf.externa.mensaje_corto}<span class="faint msg">{inf.externa.mensaje_corto}</span>{/if}
                <!-- 14nn: también aquí (antes, solo en «Más…» de la ficha del equipo). -->
                {#if puedeQuitarExterna}<span class="msg"><button type="button" class="btn btn-sm" onclick={quitarExterna}>Quitar la copia derivada</button></span>{/if}
              {/if}
              <!-- 15g: también las demás copias derivadas (antes la página solo conocía la de cada día). -->
              {#each repo.derivadas ?? [] as dv (dv.id)}
                {@const u = inf?.derivadas?.find((x) => x.id === dv.id)}
                <span class="msg">
                  {#if u?.ultima && u.resultado}<Chip pequeno tono={TONO_TAREA[u.resultado]} texto={u.pausada ? "En pausa" : TEXTO_TAREA[u.resultado]} /> <Tiempo iso={u.ultima} />{" · "}{/if}
                  <a class="link-suave" href={hrefDerivada(c, e, rid, dv.id)}>A «{dv.destino ?? "otro destino"}»</a>{#if !u?.ultima}<span class="faint">, todavía sin ninguna</span>{/if}
                </span>
                {#if u?.motivo === "destino_ausente"}
                  <!-- 14nn: su destino ya no existe: «Quitar» también desde aquí. -->
                  <span class="msg aviso-ret">{textoDestinoAusente(u)}</span>
                  {#if puede.ordenar(rol) && !repo.solo_lectura}<span class="msg"><button type="button" class="btn btn-sm" onclick={() => quitarDerivada(dv.id, dv.destino ?? null)}>Quitar esta copia</button></span>{/if}
                {:else if u?.resultado === "fallo" && u.mensaje_corto}<span class="faint msg">{u.mensaje_corto}</span>{/if}
              {/each}
              {#if !repo.externa && !(repo.derivadas ?? []).length}<span class="faint">No tiene</span>{/if}
            </dd>
          </div>
          {#if enAlm}
            <div>
              <dt>Retención <Ayuda id="retencion-almacen" /></dt>
              <dd>
                {#if enAlm.retencion}
                  {@const ra = enAlm.retencion}
                  <span>{ra.texto ?? textoRegla(ra.retencion)}</span>
                  <span class="faint msg">La aplica el almacén {enAlm.almacen.nombre} {ra.horario_texto ?? textoHorario(ra.horario)}{ra.verificar ? " y después comprueba el repositorio" : ""}.</span>
                  {#if ra.clave === "pendiente"}<span class="msg aviso-ret">Su clave aún no abre el repositorio: vuelve a guardar la retención con {equipo.nombre} encendido.</span>{/if}
                  {#if ra.ultima}
                    <span class="msg"><Chip pequeno tono={ra.resultado === "ok" ? "ok" : "bad"} texto={ra.resultado === "ok" ? "Aplicada" : "Falló"} /> <Tiempo iso={ra.ultima} />{#if ra.mensaje}<span class="faint">{" · "}{ra.mensaje}</span>{/if}</span>
                  {:else}
                    <span class="faint msg">Todavía no se ha aplicado.</span>
                  {/if}
                  {#if ra.proxima}<span class="faint msg">La próxima, <Tiempo iso={ra.proxima} />.</span>{/if}
                {:else}
                  <span>{repo.retencion ?? "Sin regla todavía"}</span>
                  <span class="faint msg">
                    {enAlm.admite
                      ? `Es de solo añadir: desde ${equipo.nombre} no se borra nada. Que la aplique ${enAlm.almacen.nombre}, en local y a su hora.`
                      : `Es de solo añadir: desde ${equipo.nombre} no se borra nada. Actualiza el agente de ${enAlm.almacen.nombre} para que la aplique él.`}
                  </span>
                {/if}
                {#if enAlm.admite && puede.administrar(rol)}
                  <span class="msg acc-ret">
                    <button class="btn btn-sm" onclick={() => (retAlm = true)}>{enAlm.retencion ? "Cambiar…" : "Retención en el almacén…"}</button>
                    {#if enAlm.retencion}
                      <button class="btn btn-sm btn-ghost" onclick={aplicarEnAlmacen}>Aplicar ahora</button>
                      <button class="btn btn-sm btn-ghost" onclick={dejarDeAplicar}>Dejar de aplicarla</button>
                    {/if}
                  </span>
                {/if}
                <span class="msg"><a class="btn btn-sm btn-ghost" href="/c/{c}/equipos/{e}/repositorios/{encodeURIComponent(rid)}/retencion">Ver en detalle</a></span>
              </dd>
            </div>
          {:else if repo.retencion}<div><dt>Retención</dt><dd>{repo.retencion}{#if repo.solo_anadir}<span class="faint msg">Se aplica en el servidor (es de solo añadir).</span>{/if}<span class="msg"><a class="btn btn-sm btn-ghost" href="/c/{c}/equipos/{e}/repositorios/{encodeURIComponent(rid)}/retencion">Ver en detalle</a></span></dd></div>
          {:else if repo.solo_anadir}<div><dt>Retención</dt><dd><span class="faint">En el servidor: es de solo añadir y desde aquí no se borra nada.</span></dd></div>{/if}
        </dl>
        {#if prot}<p class="faint pequeno"><AnilloProteccion proteccion={prot} tamano={14} /> Protección {prot.puntuacion} de {prot.total}</p>{/if}
      </section>
    </div>

    {#if versiones.length >= 2}
      <section class="card p graficas" id="sec-graficas" aria-label="Gráficas por versión">
        <!-- Mismo orden que en la página de la copia (14vv): tamaño a la izquierda, actividad a la derecha. -->
        <div class="col-graf" aria-label="Tamaño">
          <GraficaBarras titulo="Datos añadidos" datos={versiones} valor={(v) => anadidoDe(v)} formato={(x) => bytes(x)} alElegir={(v) => abrirVersion(v.id)} elegido={sel.version} clave={(v) => v.id} />
          <GraficaBarras titulo="Tamaño de lo copiado" datos={versiones} valor={(v) => v.total_bytes} formato={(x) => bytes(x)} alElegir={(v) => abrirVersion(v.id)} elegido={sel.version} clave={(v) => v.id} />
        </div>
        <div class="col-graf" aria-label="Actividad">
          <GraficaBarras titulo="Archivos nuevos y cambiados" datos={versiones} valor={(v) => (v.archivos_nuevos == null && v.archivos_cambiados == null ? null : (v.archivos_nuevos ?? 0) + (v.archivos_cambiados ?? 0))} formato={(x) => numero(x)} alElegir={(v) => abrirVersion(v.id, "nuevos")} elegido={sel.version} clave={(v) => v.id} />
          <GraficaBarras titulo="Duración por versión" datos={versiones} valor={(v) => v.duracion_s} formato={(x) => duracion(x)} alElegir={(v) => abrirVersion(v.id)} elegido={sel.version} clave={(v) => v.id} />
        </div>
      </section>
    {/if}

    <HistorialVersiones
      fuentes={[{ repo, inf, regla: retencionLinea?.regla ?? null, quien: retencionLinea?.quien ?? null }]}
      {copias}
      historial={historia.entradas}
      ultimas={equipo.ultimo_informe?.datos.copias ?? []}
      {enlace}
      ahora={reloj.ahora}
      puedeRestaurar={puede.ordenar(rol)}
      vacio={suyas.length ? "Cada vez que se haga una copia se guardará aquí una versión que podrás explorar y restaurar." : "Ninguna copia de este equipo guarda en este repositorio."}
      alAbrir={(v, _r, f) => abrirVersion(v.id, f)}
      alAbrirVuelta={(h) => abrirVuelta(h)}
      elegida={sel.version}
      dia={sel.dia}
      alDia={elegirDia}
      fechas={sel.desde && sel.hasta ? { desde: sel.desde, hasta: sel.hasta } : null}
      alFechas={elegirFechas}
      hayMas={historia.hayMas}
      cargandoMas={historia.cargando}
      alCargarMas={historia.cargarMas}
      cliente={actual.cliente}
      delEquipo={equipo}
    />

    {#if inf}
      <p class="faint pie">
        Datos del informe de {equipo.nombre} {relativo(equipo.ultimo_informe?.recibido, reloj.ahora)}{inf.versiones_leidas ? ` · versiones leídas ${relativo(inf.versiones_leidas, reloj.ahora)}` : ""}{inf.espacio?.leido ? ` · espacio medido ${relativo(inf.espacio.leido, reloj.ahora)}` : ""}.
        <span use:tip={fechaLarga(equipo.ultimo_informe?.recibido)}>Sin rutas ni nombres de archivos.</span>
      </p>
    {/if}
    <Comentarios tipo="repositorio" objeto={objetoDe(e, rid)} />
  {/if}
</div>

{#if dialogo && equipo && actual.cliente}
  {#key dialogo}
  <OrdenDialog cliente={actual.cliente} equipo={dialogo.para ?? equipo} tipo={dialogo.tipo} cuerpo={dialogo.cuerpo} titulo={dialogo.titulo} descripcion={dialogo.descripcion} accion={dialogo.accion} repo={dialogo.repo} onclose={() => (dialogo = null)} />
  {/key}
{/if}

{#if comprobar && equipo && repo && actual.cliente}
  <OrdenDialog
    cliente={actual.cliente}
    {equipo}
    tipo="comprobar_contrasena"
    cuerpo={{ repo: repo.id }}
    titulo="Comprobar la contraseña de «{repo.nombre}»"
    descripcion="Escribe la contraseña de tu kit. {equipo.nombre} probará si abre «{repo.nombre}» de verdad, y sus copias derivadas con la misma contraseña. Los espejos se abren con la misma. No cambia nada ni guarda la contraseña."
    repo={{ id: repo.id, nombre: repo.nombre }}
    accion="Comprobar"
    alTerminar={() => api.equipo(c, e).then((x) => (equipo = x)).catch(() => {})}
    onclose={() => (comprobar = false)}
  />
{/if}

{#if retAlm && enAlm?.admite && equipo && repo && actual.cliente}
  <RetencionAlmacen cliente={actual.cliente} {equipo} {repo} en={enAlm} onclose={() => (retAlm = false)} />
{/if}

{#if equipo && repo && actual.cliente}
  <PanelDetalle cliente={actual.cliente} {equipo} {repo} {inf} {copias} historial={historia.entradas} puedeRestaurar={puede.ordenar(rol)} ahora={reloj.ahora} />
{/if}

{#if mover && equipo && repo && actual.cliente}
  <MoverRepositorio cliente={actual.cliente} {equipo} {repo} equipos={actual.equipos} onclose={() => (mover = false)} />
{/if}

{#if renombrar && equipo && repo && actual.cliente}
  <RenombrarRepositorio cliente={actual.cliente} {equipo} {repo} alCambiar={() => api.equipo(c, e).then((x) => (equipo = x)).catch(() => {})} onclose={() => (renombrar = false)} />
{/if}

{#if ordenar && enAlm && actual.cliente}
  <OrdenarCarpetas cliente={actual.cliente} almacen={enAlm.almacen} equipos={actual.equipos} equipoInicial={e} soloRepo={rid} onclose={() => (ordenar = false)} />
{/if}

{#if traer && equipo && repo && actual.cliente}
  <TraerHistorial cliente={actual.cliente} {equipo} {repo} onclose={() => (traer = false)} />
{/if}

<style>
  .espejos-repo {
    margin-top: var(--sp-5);
  }
  .lista-espejos {
    display: grid;
    gap: var(--sp-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .lista-espejos li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
    font-size: var(--fs-sm);
  }
  .nota-espejos {
    margin: var(--sp-3) 0 0;
    font-size: var(--fs-sm);
  }
  .cab {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--sp-4);
  }
  .cab-texto {
    flex: 1;
    min-width: 0;
  }
  .cab .page-actions {
    flex-wrap: wrap;
  }
  .titulo {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
  }
  h1 {
    margin: 0;
    font-size: var(--fs-title);
    line-height: var(--lh-title);
    font-weight: 650;
    letter-spacing: -0.015em;
    overflow-wrap: anywhere;
  }
  .sub {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px 6px;
    margin: 4px 0 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .donde {
    margin: calc(-1 * var(--sp-2)) 0 0;
  }
  .frase {
    margin: 0;
    max-width: 72ch;
    font-size: var(--fs-body);
    line-height: var(--lh-body);
    color: var(--text-2);
  }
  .cifras {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--sp-3);
  }
  .cifra {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    padding: var(--sp-4);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }
  button.cifra {
    text-align: left;
    transition: border-color var(--dur-fast) var(--ease);
  }
  button.cifra:hover {
    border-color: var(--text-3);
  }
  .chip-pulsable {
    width: auto;
    border-radius: 999px;
  }
  .cifra .k {
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--text-3);
  }
  .cifra strong {
    font-size: var(--fs-stat, 24px);
    line-height: 1.15;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .cifra .faint {
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
  }
  .dos {
    display: grid;
    grid-template-columns: minmax(0, 3fr) minmax(0, 2fr);
    gap: var(--sp-4);
    align-items: start;
  }
  .sin-salud p {
    margin: 6px 0 0;
    font-size: var(--fs-sm);
  }
  .mantenimiento dl {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    margin: var(--sp-3) 0 0;
  }
  .mantenimiento dl > div {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .mantenimiento dl > div:first-child {
    padding-top: 0;
    border-top: none;
  }
  dt {
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--text-3);
  }
  dd {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
  }
  .msg {
    flex-basis: 100%;
    font-size: var(--fs-xs);
  }
  .aviso-ret {
    color: var(--warn, var(--bad));
  }
  .acc-ret {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 4px;
  }
  .pequeno {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: var(--sp-4) 0 0;
    font-size: var(--fs-xs);
  }
  .graficas {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-6);
  }
  .col-graf {
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
    min-width: 0;
  }
  .pie {
    margin: 0;
    font-size: var(--fs-xs);
    text-align: center;
  }
  @media (max-width: 1000px) {
    .dos {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  @media (max-width: 760px) {
    .cab .page-actions {
      flex: 1 1 100%;
    }
    .cifras {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .graficas {
      grid-template-columns: minmax(0, 1fr);
    }
    .cifra strong {
      font-size: var(--fs-title);
    }
  }
</style>
