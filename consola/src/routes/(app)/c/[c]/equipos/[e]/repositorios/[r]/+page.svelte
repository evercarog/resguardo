<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Un repositorio en detalle, como en la app de escritorio: la frase que lo
  // resume, el camino de los datos, la salud de la protección, las cifras,
  // los cuadros de 60 días, las gráficas, las versiones y su historia.
  // Todo sale del resumen del equipo y de su último informe (v1.7 §6).
  import IndicePagina from "$lib/componentes/IndicePagina.svelte";
  import Migas from "$lib/componentes/Migas.svelte";
  import { page } from "$app/state";
  import { Cloud, Database, GitCompareArrows, HardDrive, History, Play, Server, ShieldCheck } from "@lucide/svelte";
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
    ratioTexto,
    TEXTO_RESULTADO,
    TEXTO_TAREA,
    TONO_RESULTADO,
    TONO_TAREA,
    ultimaEjecucion,
    verificacion,
    versionesDe,
  } from "$lib/repo";
  import type { EntradaHistorial, Equipo, EquipoDetalle, VersionInforme } from "$lib/tipos";
  import { almacenDe, textoHorario, textoRegla } from "$lib/retencion";
  import { admiteVerificacion, fraseVerificacion } from "$lib/verificacion";
  import RetencionAlmacen from "$lib/componentes/RetencionAlmacen.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import BotonCargando from "$lib/componentes/BotonCargando.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import OrdenDialog from "$lib/componentes/OrdenDialog.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import AnilloProteccion from "$lib/componentes/repo/AnilloProteccion.svelte";
  import FlujoRepo from "$lib/componentes/repo/FlujoRepo.svelte";
  import GraficaBarras from "$lib/componentes/repo/GraficaBarras.svelte";
  import HistoriaRepo from "$lib/componentes/repo/HistoriaRepo.svelte";
  import ListaVersiones from "$lib/componentes/repo/ListaVersiones.svelte";
  import SaludProteccion from "$lib/componentes/repo/SaludProteccion.svelte";
  import MenuAcciones from "$lib/componentes/MenuAcciones.svelte";
  import TraerHistorial from "$lib/componentes/TraerHistorial.svelte";
  import LineaTiempoVersiones from "$lib/componentes/repo/LineaTiempoVersiones.svelte";
  import { reglaEfectiva } from "$lib/lineaTiempo";
  // «Pulsar para ver más»: el panel de detalle (versión, qué cambió, espacio…) según la URL.
  import PanelDetalle from "$lib/componentes/detalle/PanelDetalle.svelte";
  import { abrirEspacio, abrirEstado, abrirVersion, abrirVuelta, elegirDia, ir } from "$lib/componentes/detalle/navegar";
  import { leerSeleccion } from "$lib/detalle";
  import "$lib/componentes/detalle/pulsable.css";

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

  // v1.23: el historial que guarda el propio equipo (para la Historia; vacío con un servidor anterior).
  // v1.26: por páginas, lo más reciente primero; «Cargar más» pide la siguiente.
  let historial = $state<EntradaHistorial[]>([]);
  let hayMas = $state(false);
  let cargandoMas = $state(false);
  $effect(() => {
    const [cc, ee] = [c, e];
    historial = [];
    hayMas = false;
    api
      .historialEquipo(cc, ee)
      .then((h) => {
        if (cc !== c || ee !== e) return;
        historial = h;
        hayMas = h.length >= api.HISTORIAL_POR_PAGINA;
      })
      .catch(() => {});
  });
  async function cargarMas() {
    const [cc, ee, ultima] = [c, e, historial.at(-1)];
    if (!ultima) return;
    cargandoMas = true;
    try {
      const h = await api.historialEquipo(cc, ee, { antes: ultima.id });
      if (cc !== c || ee !== e) return;
      // Sin repetir (un servidor anterior a v1.26 no entiende «antes» y da otra vez lo mismo).
      const ya = new Set(historial.map((x) => x.id));
      const nuevas = h.filter((x) => !ya.has(x.id));
      historial = [...historial, ...nuevas];
      hayMas = nuevas.length > 0 && h.length >= api.HISTORIAL_POR_PAGINA;
    } catch (x) {
      avisar((x as Error).message, "bad");
    } finally {
      cargandoMas = false;
    }
  }

  const repo = $derived(equipo?.resumen?.repositorios?.find((r) => r.id === rid));
  const inf = $derived(informeDe(equipo?.ultimo_informe, rid));
  const copias = $derived(equipo?.resumen?.copias ?? []);
  const suyas = $derived(copias.filter((k) => k.repo === rid));
  const destinos = $derived(equipo?.resumen?.destinos ?? []);
  const destino = $derived(destinoDe(destinos, repo));
  const prot = $derived(proteccion(inf));
  const ej = $derived(ultimaEjecucion(inf));
  const verif = $derived(repo ? verificacion(repo, inf) : null);
  const prueba = $derived(repo ? pruebaRestauracion(repo, inf) : null);
  const media = $derived(duracionMedia(inf));
  const versiones = $derived(versionesDe(inf));
  const rol = $derived(equipo?.modo === "trasladado" ? "lectura" : actual.cliente?.rol);
  const Icono = $derived(destino?.tipo === "local" ? HardDrive : destino?.tipo === "rest" ? Server : Cloud);
  const estado = $derived(
    ej ? { tono: TONO_RESULTADO[ej.resultado], texto: ej.resultado === "ok" ? "Al día" : TEXTO_RESULTADO[ej.resultado] } : repo && nVersiones(repo, inf) ? { tono: "ok" as const, texto: "Con versiones" } : { tono: "neutral" as const, texto: "Sin versiones todavía" },
  );
  // La línea de tiempo: la versión elegida en ella y la retención que se le aplica.
  let elegida = $state<string | null>(null);
  const vElegida = $derived(versiones.find((v) => v.id === elegida) ?? null);
  const retencionLinea = $derived(reglaEfectiva(repo, destino, actual.equipos));
  const enlace = (v: VersionInforme, todo: boolean) => `/c/${c}/restaurar?${new URLSearchParams({ equipo: e, repo: rid, version: v.id, ...(todo ? { todo: "1" } : {}) })}`;

  // Órdenes desde aquí (con su diálogo de siempre). `para`: a otro equipo (el almacén).
  let dialogo = $state<{ tipo: string; cuerpo: Record<string, unknown>; descripcion: string; accion?: string; titulo?: string; para?: Equipo } | null>(null);

  // v1.22: en un almacén (solo añadir), la retención la aplica el almacén.
  const enAlm = $derived(repo ? almacenDe(repo, destino, actual.equipos) : null);
  // «?retencion=1»: desde «Retención en el almacén…» de la página del equipo.
  let retAlm = $state(page.url.searchParams.get("retencion") === "1");
  function aplicarEnAlmacen() {
    if (!enAlm || !repo) return;
    dialogo = {
      tipo: "aplicar_retencion_almacen",
      cuerpo: { usuario: enAlm.usuario, repo: enAlm.carpeta },
      descripcion: `${enAlm.almacen.nombre} borrará ahora de «${repo.nombre}» las versiones que ya no entren en su retención (restic forget --prune, en local) en cuanto pase la espera, sin aguardar a su próxima hora.`,
      para: enAlm.almacen,
    };
  }
  function dejarDeAplicar() {
    if (!enAlm || !repo) return;
    dialogo = {
      tipo: "retencion_almacen",
      cuerpo: { usuario: enAlm.usuario, repo: enAlm.carpeta, quitar: true },
      titulo: "Dejar de aplicar la retención",
      descripcion: `${enAlm.almacen.nombre} dejará de podar «${repo.nombre}» y borrará del repositorio su clave (ya no podrá abrirlo). No borra versiones: el repositorio volverá a crecer sin límite.`,
      para: enAlm.almacen,
    };
  }
  // «?traer=1»: al terminar «Copiar en …» para traer el historial de otro repositorio.
  let traer = $state(page.url.searchParams.get("traer") === "1");
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
          <button class="pulsable-bloque chip-pulsable" use:tip={"¿Por qué? Ver detalle"} onclick={abrirEstado}><Chip tono={estado.tono} texto={estado.texto} /></button>
          {#if repo.solo_lectura}<span class="badge badge-sm tone-neutral">Solo lectura</span>{/if}
        </div>
        <p class="sub"><Icono size={14} />{destino?.nombre ?? repo.destino} · {equipo.nombre}</p>
      </div>
      {#if puede.ordenar(rol)}
        <div class="page-actions">
          {#if nVersiones(repo, inf) || repo.solo_lectura}<a class="btn btn-primary" href="/c/{c}/restaurar?equipo={e}&repo={rid}"><History size={16} />Restaurar</a>{/if}
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
          {/if}
        </div>
      {/if}
    </header>

    <IndicePagina
      items={[
        { id: "sec-resumen", texto: "Resumen" },
        { id: "t-flujo", texto: "Cómo se protege" },
        { id: "sec-proteccion", texto: "Protección" },
        ...(versiones.length >= 2 ? [{ id: "sec-graficas", texto: "Gráficas" }] : []),
        { id: "t-versiones", texto: "Versiones" },
        ...(inf ? [{ id: "t-historia", texto: "Historia" }] : []),
      ]}
    />

    <p class="frase" id="sec-resumen">{fraseRepo(repo, inf, copias, reloj.ahora)}</p>

    <div class="cifras">
      <div class="cifra">
        <span class="k">Versiones</span>
        <strong class="num">{numero(nVersiones(repo, inf))}</strong>
        <span class="faint">{repo.ultima_version || versiones[0] ? `la última ${relativo(repo.ultima_version ?? versiones[0].hora, reloj.ahora)}` : "todavía ninguna"}</span>
      </div>
      <button class="cifra pulsable-bloque" use:tip={"Ver de dónde sale el espacio"} onclick={abrirEspacio}>
        <span class="k">Protegido</span>
        <strong class="num">{bytes(bytesRepo(repo, inf) ?? inf?.espacio?.sin_comprimir)}</strong>
        <span class="faint">lo que ocupan tus archivos</span>
      </button>
      <button class="cifra pulsable-bloque" use:tip={"Ver de dónde sale el espacio"} onclick={abrirEspacio}>
        <span class="k">En disco</span>
        <strong class="num">{bytes(inf?.espacio?.en_disco_bytes)}</strong>
        <span class="faint">{inf?.espacio?.ratio ? `${ratioTexto(inf.espacio.ratio)} menos: comprimido y sin duplicados` : "comprimido y sin duplicados"}</span>
      </button>
      <div class="cifra">
        <span class="k">Duración media</span>
        <strong class="num">{duracion(media)}</strong>
        <span class="faint">{versiones.length ? `de las últimas ${numero(versiones.length)} copias` : "sin datos todavía"}</span>
      </div>
    </div>

    <FlujoRepo {repo} {inf} {copias} {destinos} equipos={actual.equipos} ahora={reloj.ahora} />

    <div class="dos" id="sec-proteccion">
      {#if prot}
        <SaludProteccion proteccion={prot} nombre={repo.nombre} />
      {:else}
        <section class="card p sin-salud">
          <h2 class="section-title">Salud de la protección <Ayuda id="salud-proteccion" /></h2>
          <p class="faint">Llegará con el próximo informe de {equipo.nombre}: copias automáticas, protección contra borrado, copia externa, verificación, prueba de restauración, kit y retención.</p>
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
          <div>
            <dt>Copia externa</dt>
            <dd>
              {#if inf?.externa?.ultima}<Chip pequeno tono={TONO_TAREA[inf.externa.resultado]} texto={TEXTO_TAREA[inf.externa.resultado]} /> <Tiempo iso={inf.externa.ultima} />
              {:else if repo.externa}<span class="faint">A «{repo.externa.destino}» cada día a las {repo.externa.hora}, todavía sin ninguna</span>
              {:else}<span class="faint">No tiene</span>{/if}
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
              </dd>
            </div>
          {:else if repo.retencion}<div><dt>Retención</dt><dd>{repo.retencion}{#if repo.solo_anadir}<span class="faint msg">Se aplica en el servidor (es de solo añadir).</span>{/if}</dd></div>
          {:else if repo.solo_anadir}<div><dt>Retención</dt><dd><span class="faint">En el servidor: es de solo añadir y desde aquí no se borra nada.</span></dd></div>{/if}
        </dl>
        {#if prot}<p class="faint pequeno"><AnilloProteccion proteccion={prot} tamano={14} /> Protección {prot.puntuacion} de {prot.total}</p>{/if}
      </section>
    </div>

    {#if versiones.length >= 2}
      <section class="card p graficas" id="sec-graficas" aria-label="Gráficas por versión">
        <GraficaBarras titulo="Datos añadidos por versión" datos={versiones} valor={(v) => anadidoDe(v)} formato={(x) => bytes(x)} alElegir={(v) => abrirVersion(v.id)} elegido={sel.version} clave={(v) => v.id} />
        <GraficaBarras titulo="Duración por versión" datos={versiones} valor={(v) => v.duracion_s} formato={(x) => duracion(x)} alElegir={(v) => abrirVersion(v.id)} elegido={sel.version} clave={(v) => v.id} />
      </section>
    {/if}

    {#if versiones.length}
      <section class="card p linea-tiempo" aria-labelledby="t-linea">
        <h2 class="section-title" id="t-linea">Línea de tiempo <span class="count">· {numero(versiones.length)} versiones de 60 días</span></h2>
        <LineaTiempoVersiones
          versiones={versiones.map((v) => ({ id: v.id, hora: v.hora, copia: v.copia, bytes: v.total_bytes, anadido: anadidoDe(v), archivos: null }))}
          {copias}
          regla={retencionLinea?.regla ?? null}
          quien={retencionLinea?.quien ?? null}
          ahora={reloj.ahora}
          seleccion={elegida}
          alElegir={(id) => (elegida = id)}
          verbo="Ver"
          etiqueta="Versiones de «{repo.nombre}» en el tiempo"
        >
          {#snippet acciones(id)}
            <!-- En la ficha de la versión elegida. -->
            <button type="button" class="btn btn-sm" onclick={() => ir({ vista: "cambios", version: id, con: null, filtro: "todos", vuelta: null })}><GitCompareArrows size={14} />Qué cambió</button>
            {#if puede.ordenar(rol) && vElegida}<a class="btn btn-sm btn-primary" href={enlace(vElegida, false)}><History size={14} />Explorar y restaurar</a>{/if}
          {/snippet}
        </LineaTiempoVersiones>
      </section>
    {/if}

    <ListaVersiones
      {repo}
      {inf}
      {copias}
      {enlace}
      ahora={reloj.ahora}
      puedeRestaurar={puede.ordenar(rol)}
      vacio={suyas.length ? "Cada vez que se haga una copia se guardará aquí una versión que podrás explorar y restaurar." : "Ninguna copia de este equipo guarda en este repositorio."}
      alAbrir={(v, f) => abrirVersion(v.id, f)}
      elegida={sel.version}
      dia={sel.dia}
      alDia={elegirDia}
    />

    {#if inf}
      <HistoriaRepo {repo} {inf} {copias} ultimas={equipo.ultimo_informe?.datos.copias ?? []} {historial} alAbrirVersion={(id) => abrirVersion(id)} alAbrirVuelta={abrirVuelta} dia={sel.dia} />
      {#if hayMas}
        <p class="faint pie">
          Se ha leído lo más reciente del historial del equipo ({numero(historial.length)} entradas).
          <BotonCargando class="btn btn-ghost btn-sm" cargando={cargandoMas} textoCargando="Cargando…" onclick={cargarMas}>Cargar más</BotonCargando>
        </p>
      {/if}
      <p class="faint pie">
        Datos del informe de {equipo.nombre} {relativo(equipo.ultimo_informe?.recibido, reloj.ahora)}{inf.versiones_leidas ? ` · versiones leídas ${relativo(inf.versiones_leidas, reloj.ahora)}` : ""}{inf.espacio?.leido ? ` · espacio medido ${relativo(inf.espacio.leido, reloj.ahora)}` : ""}.
        <span use:tip={fechaLarga(equipo.ultimo_informe?.recibido)}>Sin rutas ni nombres de archivos.</span>
      </p>
    {/if}
  {/if}
</div>

{#if dialogo && equipo && actual.cliente}
  {#key dialogo}
  <OrdenDialog cliente={actual.cliente} equipo={dialogo.para ?? equipo} tipo={dialogo.tipo} cuerpo={dialogo.cuerpo} titulo={dialogo.titulo} descripcion={dialogo.descripcion} accion={dialogo.accion} onclose={() => (dialogo = null)} />
  {/key}
{/if}

{#if retAlm && enAlm?.admite && equipo && repo && actual.cliente}
  <RetencionAlmacen cliente={actual.cliente} {equipo} {repo} en={enAlm} onclose={() => (retAlm = false)} />
{/if}

{#if equipo && repo && actual.cliente}
  <PanelDetalle cliente={actual.cliente} {equipo} {repo} {inf} {copias} {historial} puedeRestaurar={puede.ordenar(rol)} ahora={reloj.ahora} />
{/if}

{#if traer && equipo && repo && actual.cliente}
  <TraerHistorial cliente={actual.cliente} {equipo} {repo} onclose={() => (traer = false)} />
{/if}

<style>
  .linea-tiempo {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .linea-tiempo h2 {
    margin: 0;
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
  }
</style>
