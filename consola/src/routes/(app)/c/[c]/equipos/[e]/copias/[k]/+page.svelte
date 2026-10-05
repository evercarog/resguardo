<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Una copia en detalle (como la página de un repositorio): cómo está, qué
  // copia, cuándo y dónde, sus vueltas de 60 días en cuadros y gráficas, los
  // errores recientes explicados, sus versiones y su historial. Todo sale del
  // resumen del equipo y de su último informe; las carpetas, que van cifradas,
  // se ven con la clave de administración (y se olvidan al salir).
  import IndicePagina from "$lib/componentes/IndicePagina.svelte";
  import Copiable from "$lib/componentes/Copiable.svelte";
  import Migas from "$lib/componentes/Migas.svelte";
  import { onDestroy, untrack } from "svelte";
  import { seguirCambios, tocaEquipo } from "$lib/vivo.svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import {
    CalendarClock,
    CircleHelp,
    Database,
    Eye,
    FolderOpen,
    FolderSync,
    History,
    KeyRound,
    LoaderCircle,
    LockKeyhole,
    Pencil,
    Play,
    TriangleAlert,
  } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { ApiError } from "$lib/api";
  import { borrar, deB64 } from "$lib/cripto/bytes";
  import { descifrarConfig } from "$lib/cripto/simetrico";
  import { ErrorLlavesCambiadas, kcfgComprobada } from "$lib/ordenar";
  import { actual, puede, reloj } from "$lib/estado.svelte";
  import { bytes, cuandoFrase, fechaLarga, horarioEnFrase, numero, plural, relativo } from "$lib/formato";
  import { enPausa } from "$lib/salud";
  import { fraseGancho, ganchosDe, NOMBRE_GANCHO } from "$lib/ganchos";
  import { anadidoDe, destinoDe, duracion, informeDe } from "$lib/repo";
  // v1.41: dónde se guarda y el aviso «copias en el mismo equipo».
  import { lugarRepo, riesgoMismoEquipo } from "$lib/dondeGuarda";
  import SeGuardaEn from "$lib/componentes/SeGuardaEn.svelte";
  import AvisoMismoEquipo from "$lib/componentes/AvisoMismoEquipo.svelte";
  import { atrasada, cifrasCopia, estadoCopia, explicarError, filaInforme, fraseCopia, infCopia, proximaDe, ultimaProgramada, ultimaVuelta } from "$lib/copia";
  import type { Configuracion, CopiaConfig, EquipoDetalle, VersionInforme } from "$lib/tipos";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import CampoClave from "$lib/componentes/CampoClave.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import OrdenDialog from "$lib/componentes/OrdenDialog.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import EnMarcha from "$lib/componentes/EnMarcha.svelte";
  import GraficaBarras from "$lib/componentes/repo/GraficaBarras.svelte";
  import HistorialVersiones from "$lib/componentes/repo/HistorialVersiones.svelte";
  import { usarHistorialEquipo } from "$lib/historialEquipo.svelte";
  import { reglaEfectiva } from "$lib/lineaTiempo";
  // «Pulsar para ver más»: el panel de detalle (versión, qué cambió, vuelta…) según la URL.
  import PanelDetalle from "$lib/componentes/detalle/PanelDetalle.svelte";
  import { abrirEstado, abrirVersion, abrirVuelta, elegirDia, elegirFechas } from "$lib/componentes/detalle/navegar";
  import { leerSeleccion } from "$lib/detalle";
  import "$lib/componentes/detalle/pulsable.css";
  import Observaciones from "$lib/componentes/notas/Observaciones.svelte";
  import Comentarios from "$lib/componentes/notas/Comentarios.svelte";
  import { objetoDe } from "$lib/notas.svelte";

  const c = $derived(page.params.c ?? "");
  const e = $derived(page.params.e ?? "");
  const kid = $derived(page.params.k ?? "");
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
  // Al día sin recargar (sin vaciar la página): cuando el canal en vivo dice que algo de este equipo cambió.
  $effect(() => {
    const [cc, ee] = [c, e];
    return untrack(() =>
      seguirCambios(() => api.equipo(cc, ee).then((x) => ee === e && (equipo = x), () => {}), {
        ms: 0,
        toca: (x) => (x.t === "informe" || x.t === "config" || x.t === "equipo" || (x.t === "progreso" && x.estado === "termina")) && tocaEquipo(x, ee),
      }),
    );
  });

  const copias = $derived(equipo?.resumen?.copias ?? []);
  const k = $derived(copias.find((x) => x.id === kid));
  const informe = $derived(equipo?.ultimo_informe ?? null);
  const repo = $derived(equipo?.resumen?.repositorios?.find((r) => r.id === k?.repo));
  const destino = $derived(destinoDe(equipo?.resumen?.destinos, repo));
  const lugar = $derived(equipo && repo ? lugarRepo(repo, equipo, actual.equipos) : null);
  const riesgo = $derived(equipo && repo ? riesgoMismoEquipo(repo, equipo, actual.equipos) : null);
  const infRepo = $derived(k ? informeDe(informe, k.repo) : null);
  const inf = $derived(k ? infCopia(infRepo, k.id) : null);
  const fila = $derived(k ? filaInforme(informe, k) : null);
  const vuelta = $derived(k ? ultimaVuelta(k, informe) : null);
  const pausada = $derived(equipo ? enPausa(equipo, reloj.ahora) : false);
  const proxima = $derived(k ? proximaDe(k, informe, reloj.ahora) : null);
  const estado = $derived(k ? estadoCopia(k, vuelta, pausada, reloj.ahora) : null);
  const cifras = $derived(cifrasCopia(inf));
  const versiones = $derived(inf?.versiones ?? []);
  const ejecuciones = $derived(inf?.ejecuciones ?? []);
  const conDuracion = $derived(ejecuciones.filter((x) => x.duracion_s != null));
  const ganchosRes = $derived(fila?.ganchos ?? []);
  const rol = $derived(equipo?.modo === "trasladado" ? "lectura" : actual.cliente?.rol);
  const almacen = $derived(destino?.equipo_almacen ? actual.equipos.find((x) => x.id === destino.equipo_almacen) : null);
  const espejo = $derived(almacen?.resumen?.guarda_copias?.espejo ?? null);
  const enlace = (r: string, v: VersionInforme, todo: boolean) => `/c/${c}/restaurar?${new URLSearchParams({ equipo: e, repo: r, version: v.id, ...(todo ? { todo: "1" } : {}) })}`;
  // La retención que se le aplica a su repositorio (simulada con todas sus versiones, como en su página).
  const retencionLinea = $derived(reglaEfectiva(repo, destino, actual.equipos));
  // v1.23: el historial que guarda el propio equipo (lo de antes, verificaciones, subidas…), por páginas y en vivo.
  const historia = usarHistorialEquipo(() => c, () => e);
  /** Las vueltas de 60 días en una frase (antes, encima de sus cuadros). */
  const resumen60 = $derived(
    [
      cifras.correctas - cifras.sinCambios ? plural(cifras.correctas - cifras.sinCambios, "con versión nueva", "con versión nueva") : null,
      cifras.sinCambios ? plural(cifras.sinCambios, "sin cambios", "sin cambios") : null,
      cifras.conAvisos ? plural(cifras.conAvisos, "con avisos", "con avisos") : null,
      cifras.fallidas ? plural(cifras.fallidas, "falló", "fallaron") : null,
    ]
      .filter(Boolean)
      .join(" · ") + (cifras.anadido != null ? ` · añadió ${bytes(cifras.anadido)}` : ""),
  );

  /** Problemas de los últimos 60 días, agrupados por explicación (el más reciente primero). */
  const problemas = $derived.by(() => {
    const grupos = new Map<string, { exp: ReturnType<typeof explicarError>; n: number; ultima: string; mensaje: string | null; tono: "bad" | "warn" }>();
    for (const x of ejecuciones) {
      if (x.resultado !== "fallo" && x.resultado !== "aviso") continue;
      const exp = explicarError(x.mensaje_corto);
      const g = grupos.get(exp.titulo);
      if (g) g.n++;
      else grupos.set(exp.titulo, { exp, n: 1, ultima: x.hora, mensaje: x.mensaje_corto, tono: x.resultado === "fallo" ? "bad" : "warn" });
    }
    return [...grupos.values()];
  });
  const explicacion = $derived(vuelta && (vuelta.resultado === "fallo" || vuelta.resultado === "aviso") ? explicarError(vuelta.mensaje) : null);

  // --- Las carpetas (cifradas): con la clave de administración ------------
  let clave = $state("");
  let pidiendo = $state(false);
  let abriendo = $state(false);
  let errorClave = $state("");
  let detalle = $state<CopiaConfig | null>(null);
  async function verCarpetas(ev: SubmitEvent) {
    ev.preventDefault();
    if (!equipo || !actual.cliente) return;
    errorClave = "";
    abriendo = true;
    let kcfg: Uint8Array | null = null;
    try {
      kcfg = await kcfgComprobada(actual.cliente, equipo, clave);
      clave = "";
      const cifrada = await api.configEquipo(c, equipo.id);
      let cfg: Configuracion;
      try {
        cfg = descifrarConfig<Configuracion>(kcfg, equipo.id, cifrada.seq, deB64(cifrada.cifrado));
      } catch {
        throw new Error("No se pudo descifrar la configuración del equipo con esta clave.");
      }
      detalle = cfg.copias.find((x) => x.id === kid) ?? null;
      if (!detalle) errorClave = "La configuración del equipo ya no tiene esta copia.";
      pidiendo = false;
    } catch (err) {
      errorClave =
        err instanceof ApiError && err.codigo === "no_existe"
          ? "El equipo todavía no ha subido su configuración."
          : err instanceof ErrorLlavesCambiadas
            ? `${err.message} Revísalo en la ficha del equipo antes de seguir.`
            : (err as Error).message;
    } finally {
      borrar(kcfg);
      abriendo = false;
    }
  }
  onDestroy(() => {
    detalle = null;
    clave = "";
  });

  // --- Órdenes ---------------------------------------------------------------
  let dialogo = $state<{ tipo: string; cuerpo: Record<string, unknown>; descripcion: string; accion?: string } | null>(null);
  const copiarAhora = () =>
    k && (dialogo = { tipo: "copiar_ahora", cuerpo: { repo: k.repo, copia: k.id }, descripcion: `Se hará ahora la copia «${k.nombre}», sin esperar a su hora. No borra nada.`, accion: "Copiar ahora" });

  const tonoGancho = (s: string) => (s === "ok" ? "ok" : s === "aviso" ? "warn" : "bad") as "ok" | "warn" | "bad";
  const textoGancho = (s: string) => (s === "ok" ? "Correcto" : s === "aviso" ? "Con avisos" : "Falló");
</script>

<svelte:head><title>{k?.nombre ?? "Copia"} · {equipo?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <Migas items={[{ texto: "Equipos", href: `/c/${c}/equipos` }, { texto: equipo?.nombre ?? "Equipo", href: `/c/${c}/equipos/${e}` }, { texto: k?.nombre ?? "Copia" }]} />

  {#if error && !equipo}
    <div class="notice notice-danger"><p>{error}</p></div>
  {:else if !equipo}
    <Cargando forma="ficha" filas={4} />
  {:else if !k || !estado}
    <div class="notice notice-info"><p>{equipo.nombre} ya no tiene esa copia. <a class="link" href="/c/{c}/equipos/{e}">Ver sus copias</a></p></div>
  {:else}
    <header class="cab">
      <span class="page-icon"><FolderSync size={22} /></span>
      <div class="cab-texto">
        <div class="titulo">
          <h1>{k.nombre}</h1>
          <button class="pulsable-bloque chip-pulsable" use:tip={"¿Por qué? Ver detalle"} onclick={abrirEstado}><Chip tono={estado.tono} texto={estado.texto} /></button>
        </div>
        <p class="sub">
          <Database size={14} />
          {#if repo}<a class="link" href="/c/{c}/equipos/{e}/repositorios/{encodeURIComponent(repo.id)}">{repo.nombre}</a>{:else}{k.repo}{/if}
          · {equipo.nombre}
        </p>
      </div>
      {#if puede.ordenar(rol)}
        <div class="page-actions">
          {#if k.activa !== false}<button class="btn btn-primary" onclick={copiarAhora}><Play size={16} />Copiar ahora</button>{/if}
          {#if puede.administrar(rol)}<a class="btn" href="/c/{c}/equipos/{e}/copias"><Pencil size={16} />Cambiar</a>{/if}
          {#if versiones.length || repo?.versiones}<a class="btn btn-ghost" href="/c/{c}/restaurar?equipo={e}&repo={encodeURIComponent(k.repo)}"><History size={16} />Restaurar</a>{/if}
        </div>
      {/if}
    </header>
    {#if lugar}<p class="donde"><SeGuardaEn {lugar} riesgo={!!riesgo} /></p>{/if}
    {#if riesgo && repo}
      <AvisoMismoEquipo
        {riesgo}
        onmover={puede.administrar(rol) ? () => goto(`/c/${c}/equipos/${e}/repositorios/${encodeURIComponent(repo!.id)}?mover=1`) : undefined}
        hrefExterna={puede.ordenar(rol) ? `/c/${c}/equipos/${e}?externa=${encodeURIComponent(repo.id)}` : undefined}
      />
    {/if}
    <Observaciones tipo="copia" objeto={objetoDe(e, kid)} />

    <EnMarcha equipo={e} copia={kid} marco alTerminar={() => api.equipo(c, e).then((x) => (equipo = x)).catch(() => {})} />

    <p class="frase">
      {fraseCopia(k, { equipo: equipo.nombre, repo: repo?.nombre ?? k.repo, vuelta, proxima, pausada, ganchos: ganchosRes.map((g) => (NOMBRE_GANCHO[g.tipo] ?? g.tipo).replace(/^./, (x) => x.toLowerCase())) }, reloj.ahora)}
    </p>

    {#if explicacion && vuelta}
      <div class="notice {vuelta.resultado === 'fallo' ? 'notice-danger' : 'notice-warn'} explicacion" role="status">
        <TriangleAlert size={16} />
        <div>
          <p><strong>{explicacion.titulo}.</strong> {explicacion.texto}</p>
          {#if vuelta.mensaje}<p class="msg-equipo">El equipo dijo: «{vuelta.mensaje.replace(/\.$/, "")}» (<span use:tip={fechaLarga(vuelta.cuando)}>{relativo(vuelta.cuando, reloj.ahora)}</span>).</p>{/if}
          <p><a class="link" href="/ayuda#{explicacion.ayuda}"><CircleHelp size={14} />¿Qué hago?</a></p>
        </div>
      </div>
    {:else if estado.texto === "Atrasada"}
      {@const slot = ultimaProgramada(k.horario, reloj.ahora)}
      <div class="notice notice-warn" role="status">
        <TriangleAlert size={16} />
        <p>
          Le tocaba {slot ? relativo(new Date(slot).toISOString(), reloj.ahora) : "hace un rato"} y no se ha hecho{vuelta ? `: la última fue ${relativo(vuelta.cuando, reloj.ahora)}` : ""}.
          {equipo.conectado ? "El equipo está conectado: puedes pulsar «Copiar ahora»." : `El equipo no se conecta desde ${relativo(equipo.ultimo_contacto, reloj.ahora)}: comprueba que está encendido.`}
        </p>
      </div>
    {/if}

    <IndicePagina
      items={[
        { id: "sec-resumen", texto: "Resumen" },
        { id: "t-que", texto: "Qué y cuándo" },
        ...(versiones.length >= 2 || conDuracion.length >= 2 ? [{ id: "sec-graficas", texto: "Gráficas" }] : []),
        ...(problemas.length ? [{ id: "t-errores", texto: "Errores" }] : []),
        { id: "t-historial", texto: "Historial y versiones" },
      ]}
    />

    <div class="cifras" id="sec-resumen">
      {#if ejecuciones[0]}
        <button class="cifra pulsable-bloque" use:tip={"Ver detalle de la última copia"} onclick={() => abrirVuelta(ejecuciones[0].hora)}>
          <span class="k">Última copia</span>
          <strong class="num">{vuelta ? relativo(vuelta.cuando, reloj.ahora) : "—"}</strong>
          <span class="faint">{vuelta ? (vuelta.resultado === "sin_cambios" ? "sin cambios" : vuelta.resultado === "ok" ? "correcta" : vuelta.resultado === "aviso" ? "con avisos" : "falló") : "todavía ninguna"}</span>
        </button>
      {:else}
      <div class="cifra">
        <span class="k">Última copia</span>
        <strong class="num">{vuelta ? relativo(vuelta.cuando, reloj.ahora) : "—"}</strong>
        <span class="faint">{vuelta ? (vuelta.resultado === "sin_cambios" ? "sin cambios" : vuelta.resultado === "ok" ? "correcta" : vuelta.resultado === "aviso" ? "con avisos" : "falló") : "todavía ninguna"}</span>
      </div>
      {/if}
      <div class="cifra">
        <span class="k">Próxima</span>
        <strong class="num">{proxima && Date.parse(proxima) > reloj.ahora ? relativo(proxima, reloj.ahora) : "—"}</strong>
        <span class="faint">{k.activa === false ? "desactivada" : pausada ? "en pausa" : proxima && Date.parse(proxima) > reloj.ahora ? cuandoFrase(proxima, reloj.ahora) : horarioEnFrase(k.horario)}</span>
      </div>
      <button class="cifra pulsable-bloque" use:tip={"Ver los problemas recientes"} onclick={abrirEstado}>
        <span class="k">Copias correctas</span>
        <strong class="num">{cifras.vueltas ? `${numero(Math.round((cifras.correctas / cifras.vueltas) * 100))} %` : "—"}</strong>
        <span class="faint">{cifras.vueltas ? `${numero(cifras.correctas)} de ${numero(cifras.vueltas)} en 60 días` : "sin datos todavía"}</span>
      </button>
      <div class="cifra">
        <span class="k">Lo que copia</span>
        <strong class="num">{bytes(cifras.tamano)}</strong>
        <span class="faint">{cifras.duracionMedia != null ? `unos ${duracion(cifras.duracionMedia)} por copia` : "según su última versión"}</span>
      </div>
    </div>

    <div class="dos">
      <section class="card p que" aria-labelledby="t-que">
        <div class="sec-cab">
          <h2 class="section-title" id="t-que">Qué copia</h2>
          {#if !detalle && puede.administrar(rol) && !pidiendo}
            <button class="btn btn-sm btn-ghost" onclick={() => (pidiendo = true)}><Eye size={14} />Ver las carpetas</button>
          {/if}
        </div>
        {#if detalle}
          <ul class="carpetas">
            {#each detalle.carpetas as p (p)}<li><FolderOpen size={14} /><Copiable texto={p} que="la ruta" /></li>{:else}<li class="faint">Sin carpetas.</li>{/each}
          </ul>
          {#if detalle.exclusiones.length}
            <p class="faint pequeno">Sin copiar: {#each detalle.exclusiones as x, i (i)}<code>{x}</code>{i < detalle.exclusiones.length - 1 ? " " : ""}{/each}</p>
          {:else}
            <p class="faint pequeno">Sin exclusiones: se copia todo lo de esas carpetas.</p>
          {/if}
        {:else}
          <p class="dato"><FolderOpen size={14} />{k.carpetas != null ? plural(k.carpetas, "carpeta", "carpetas") : "Sus carpetas"} de {equipo.nombre}</p>
          {#if pidiendo}
            <form class="form desbloquear" onsubmit={verCarpetas}>
              <p class="faint pequeno"><LockKeyhole size={13} />Las rutas viajan y se guardan cifradas: solo se ven con la clave de administración. Aquí no se cambia nada y se olvidan al salir.</p>
              <CampoClave requerido id="clave-ver" etiqueta="Clave de administración" bind:value={clave} autofocus>
                {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
              </CampoClave>
              {#if errorClave}<div class="notice notice-danger" role="alert"><p>{errorClave}</p></div>{/if}
              <div class="acciones">
                <button class="btn btn-sm btn-primary" disabled={!clave || abriendo}>{#if abriendo}<LoaderCircle size={14} class="spin" />Descifrando…{:else}<KeyRound size={14} />Ver{/if}</button>
                <button type="button" class="btn btn-sm btn-ghost" onclick={() => ((pidiendo = false), (clave = ""), (errorClave = ""))}>Cancelar</button>
              </div>
            </form>
          {:else}
            <p class="faint pequeno">Las rutas van cifradas: el servidor no las ve.</p>
          {/if}
        {/if}

        <h3 class="sub-titulo">Antes de copiar <Ayuda id="ganchos" /></h3>
        {#if detalle && ganchosDe(detalle.gancho).length}
          <ul class="ganchos">
            {#each ganchosDe(detalle.gancho) as g, i (i)}
              {@const r = ganchosRes.find((x) => x.tipo === g.tipo)}
              <li>
                <span>{fraseGancho(g)}</span>
                {#if r}<Chip pequeno tono={tonoGancho(r.estado)} texto={textoGancho(r.estado)} />{/if}
                {#if r?.mensaje}<span class="faint pequeno msg" class:msg-fallo={r.estado === "fallo"}>{r.mensaje}</span>{/if}
              </li>
            {/each}
          </ul>
        {:else if ganchosRes.length}
          <ul class="ganchos">
            {#each ganchosRes as r, i (i)}
              <li>
                <span>{NOMBRE_GANCHO[r.tipo] ?? r.tipo}</span>
                <Chip pequeno tono={tonoGancho(r.estado)} texto={textoGancho(r.estado)} />
                {#if r.mensaje}<span class="faint pequeno msg" class:msg-fallo={r.estado === "fallo"}>{r.mensaje}</span>{/if}
              </li>
            {/each}
          </ul>
          {#if fila?.cuando}<p class="faint pequeno">En la copia de {relativo(fila.cuando, reloj.ahora)}.</p>{/if}
        {:else}
          <p class="faint pequeno">{detalle ? "Nada: copia las carpetas tal cual." : "Nada, o todavía no se ha hecho ninguna copia con pasos previos."}</p>
        {/if}
      </section>

      <section class="card p cuando" aria-labelledby="t-cuando">
        <h2 class="section-title" id="t-cuando">Cuándo y dónde</h2>
        <dl>
          <div>
            <dt>Horario</dt>
            <dd><CalendarClock size={14} />{horarioEnFrase(k.horario)}{k.activa === false ? " (desactivada)" : ""}</dd>
          </div>
          <div>
            <dt>Próxima</dt>
            <dd>
              {#if k.activa === false}<span class="faint">Ninguna: está desactivada</span>
              {:else if pausada}<span class="faint">En pausa{equipo.resumen?.pausado_hasta && equipo.resumen.pausado_hasta !== "indefinido" ? ` hasta ${cuandoFrase(equipo.resumen.pausado_hasta, reloj.ahora)}` : " hasta que alguien la reanude"}</span>
              {:else if proxima && Date.parse(proxima) > reloj.ahora}{cuandoFrase(proxima, reloj.ahora)} <span class="faint">({relativo(proxima, reloj.ahora)})</span>
              {:else}<span class="faint">Según el horario (el equipo aún no la ha dicho)</span>{/if}
            </dd>
          </div>
          <div>
            <dt>Última</dt>
            <dd>{#if vuelta}<Tiempo iso={vuelta.cuando} />{#if atrasada(k, vuelta, pausada, reloj.ahora)} <Chip pequeno tono="warn" texto="Atrasada" />{/if}{:else}<span class="faint">Todavía ninguna</span>{/if}</dd>
          </div>
          <div>
            <dt>Si no hay cambios</dt>
            <dd class="faint">No guarda una versión nueva: la copia cuenta como hecha y no ocupa nada.</dd>
          </div>
          <div>
            <dt>Repositorio</dt>
            <dd>
              {#if repo}<a class="link" href="/c/{c}/equipos/{e}/repositorios/{encodeURIComponent(repo.id)}">{repo.nombre}</a>{:else}{k.repo}{/if}
              {#if lugar}<SeGuardaEn pequeno etiqueta="en" {lugar} riesgo={!!riesgo} />{#if destino?.inmutable}<span class="faint">(solo añadir)</span>{/if}{:else if repo?.destino}<span class="faint">en {repo.destino}</span>{/if}
            </dd>
          </div>
          <div>
            <dt>Copia externa <Ayuda id="copia-externa" /></dt>
            <dd>
              {#if repo?.externa}A «{repo.externa.destino}» cada día a las {repo.externa.hora}
              {:else}<span class="faint">No tiene: todas las versiones están en un solo sitio</span>{/if}
            </dd>
          </div>
          {#if espejo && almacen}
            <div>
              <dt>Espejo <Ayuda id="espejo" /></dt>
              <dd>{almacen.nombre} lo copia cada noche a las {espejo.hora}{#if espejo.ultima}<span class="faint">, la última {relativo(espejo.ultima, reloj.ahora)}</span>{/if}</dd>
            </div>
          {/if}
          <div>
            <dt>Retención <Ayuda id="retencion" /></dt>
            <dd>{#if repo?.retencion}Guarda {repo.retencion}{:else}<span class="faint">Todas las versiones (sin retención)</span>{/if}</dd>
          </div>
        </dl>
      </section>
    </div>

    {#if versiones.length >= 2 || conDuracion.length >= 2}
      <section class="card p graficas" id="sec-graficas" aria-label="Gráficas de la copia">
        {#if versiones.length >= 2}<GraficaBarras titulo="Datos nuevos por versión" datos={versiones} valor={(v) => anadidoDe(v)} formato={(x) => bytes(x)} alElegir={(v) => abrirVersion(v.id)} elegido={sel.version} clave={(v) => v.id} />{/if}
        {#if conDuracion.length >= 2}
          <GraficaBarras titulo="Duración de cada copia" datos={conDuracion} valor={(x) => x.duracion_s} formato={(x) => duracion(x)} alElegir={(x) => abrirVuelta(x.hora)} elegido={sel.vuelta} />
        {:else}
          <GraficaBarras titulo="Duración por versión" datos={versiones} valor={(v) => v.duracion_s} formato={(x) => duracion(x)} alElegir={(v) => abrirVersion(v.id)} elegido={sel.version} clave={(v) => v.id} />
        {/if}
        {#if versiones.length >= 2}
          <GraficaBarras titulo="Archivos nuevos y cambiados" datos={versiones} valor={(v) => (v.archivos_nuevos == null && v.archivos_cambiados == null ? null : (v.archivos_nuevos ?? 0) + (v.archivos_cambiados ?? 0))} formato={(x) => numero(x)} alElegir={(v) => abrirVersion(v.id, "nuevos")} elegido={sel.version} clave={(v) => v.id} />
          <GraficaBarras titulo="Tamaño de lo copiado" datos={versiones} valor={(v) => v.total_bytes} formato={(x) => bytes(x)} alElegir={(v) => abrirVersion(v.id)} elegido={sel.version} clave={(v) => v.id} />
        {/if}
      </section>
    {/if}

    {#if problemas.length}
      <section class="card p errores" aria-labelledby="t-errores">
        <h2 class="section-title" id="t-errores">Errores recientes <span class="count">· 60 días</span></h2>
        <ul>
          {#each problemas as p (p.exp.titulo)}
            <li class="tone-{p.tono}">
              <TriangleAlert size={16} />
              <div>
                <p class="linea"><button class="pulsable" use:tip={"Ver la última vez"} onclick={() => abrirVuelta(p.ultima)}><strong>{p.exp.titulo}</strong></button> <span class="faint num">{plural(p.n, "vez", "veces")} · la última {relativo(p.ultima, reloj.ahora)}</span></p>
                <p class="faint">{p.exp.texto}</p>
                {#if p.mensaje}<p class="faint pequeno">«{p.mensaje.replace(/\.$/, "")}»</p>{/if}
              </div>
              <a class="btn btn-sm btn-ghost" href="/ayuda#{p.exp.ayuda}"><CircleHelp size={14} />¿Qué hago?</a>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if repo}
      <HistorialVersiones
        fuentes={[{ repo: { ...repo, versiones: undefined }, inf: infRepo, regla: retencionLinea?.regla ?? null, quien: retencionLinea?.quien ?? null }]}
        {copias}
        soloCopia={k.id}
        historial={historia.entradas}
        ultimas={informe?.datos.copias ?? []}
        {enlace}
        ahora={reloj.ahora}
        puedeRestaurar={puede.ordenar(rol)}
        resumen={inf && cifras.vueltas ? `En 60 días: ${resumen60}` : null}
        vacio={infRepo ? "Cada vez que esta copia encuentre cambios se guardará aquí una versión que podrás explorar y restaurar." : `Llegarán con el próximo informe de ${equipo.nombre}.`}
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
      />
    {/if}

    {#if informe}
      <p class="faint pie">
        Datos del informe de {equipo.nombre} {relativo(informe.recibido, reloj.ahora)}{infRepo?.versiones_leidas ? ` · versiones leídas ${relativo(infRepo.versiones_leidas, reloj.ahora)}` : ""}.
        <span use:tip={fechaLarga(informe.recibido)}>El servidor no ve rutas ni nombres de archivos.</span>
      </p>
    {/if}
    <Comentarios tipo="copia" objeto={objetoDe(e, kid)} />
  {/if}
</div>

{#if equipo && repo && k && actual.cliente}
  <PanelDetalle cliente={actual.cliente} {equipo} {repo} inf={infRepo} {copias} historial={historia.entradas} soloCopia={k.id} puedeRestaurar={puede.ordenar(rol)} ahora={reloj.ahora} />
{/if}

{#if dialogo && equipo && actual.cliente}
  {#key dialogo}
    <OrdenDialog cliente={actual.cliente} {equipo} tipo={dialogo.tipo} cuerpo={dialogo.cuerpo} descripcion={dialogo.descripcion} accion={dialogo.accion} onclose={() => (dialogo = null)} />
  {/key}
{/if}

<style>
  .cab {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--sp-4);
  }
  .donde {
    margin: calc(-1 * var(--sp-2)) 0 0;
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
    gap: 6px;
    margin: 4px 0 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .frase {
    margin: 0;
    max-width: 72ch;
    font-size: var(--fs-body);
    line-height: var(--lh-body);
    color: var(--text-2);
  }
  .explicacion p {
    margin: 0;
  }
  .explicacion > div {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .explicacion a {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-weight: 500;
  }
  .msg-equipo {
    font-size: var(--fs-sm);
    overflow-wrap: anywhere;
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
    overflow-wrap: anywhere;
  }
  .cifra .faint {
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
  }
  .dos {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-4);
    align-items: start;
  }
  .sec-cab {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
  }
  .dato {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: var(--sp-3) 0 0;
    font-size: var(--fs-sm);
  }
  .pequeno {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 6px;
    margin: 6px 0 0;
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
  }
  .carpetas {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: var(--sp-3) 0 0;
    padding: 0;
    list-style: none;
  }
  .carpetas li {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    font-size: var(--fs-sm);
  }
  .carpetas :global(code) {
    overflow-wrap: anywhere;
  }
  .desbloquear {
    margin-top: var(--sp-3);
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }
  .sub-titulo {
    display: flex;
    align-items: center;
    gap: 4px;
    margin: var(--sp-5) 0 0;
    font-size: var(--fs-sm);
    font-weight: 600;
  }
  .ganchos {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin: var(--sp-2) 0 0;
    padding: 0;
    list-style: none;
  }
  .ganchos li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .msg {
    flex-basis: 100%;
    margin: 0;
    overflow-wrap: anywhere;
  }
  .msg-fallo {
    color: var(--bad);
  }
  .cuando dl {
    display: flex;
    flex-direction: column;
    margin: var(--sp-3) 0 0;
  }
  .cuando dl > div {
    display: grid;
    grid-template-columns: minmax(110px, 34%) minmax(0, 1fr);
    gap: var(--sp-3);
    padding: 10px 0;
    border-top: 1px solid var(--border);
  }
  .cuando dl > div:first-child {
    border-top: none;
    padding-top: 0;
  }
  dt {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--text-3);
  }
  dd {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 6px;
    margin: 0;
    font-size: var(--fs-sm);
    min-width: 0;
  }
  .graficas {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-6);
  }
  .errores ul {
    display: flex;
    flex-direction: column;
    margin: var(--sp-3) 0 0;
    padding: 0;
    list-style: none;
  }
  .errores li {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr) auto;
    align-items: start;
    gap: var(--sp-3);
    padding: var(--sp-3) 0;
    border-top: 1px solid var(--border);
  }
  .errores li:first-child {
    border-top: none;
  }
  .errores li > :global(svg) {
    margin-top: 2px;
  }
  .errores .tone-bad > :global(svg) {
    color: var(--bad);
  }
  .errores .tone-warn > :global(svg) {
    color: var(--warn);
  }
  .errores p {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .errores .linea {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 8px;
  }
  .count {
    font-weight: 400;
    color: var(--text-3);
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
      font-size: 20px;
    }
    .errores li {
      grid-template-columns: 16px minmax(0, 1fr);
    }
    .errores li > a {
      grid-column: 2;
      justify-self: start;
    }
    .cuando dl > div {
      grid-template-columns: minmax(0, 1fr);
      gap: 2px;
    }
  }
</style>
