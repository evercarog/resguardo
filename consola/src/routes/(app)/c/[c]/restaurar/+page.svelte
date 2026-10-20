<script lang="ts">
  import { preguntarAlSalir } from "$lib/salir.svelte";
  import MarcasVersion from "$lib/componentes/MarcasVersion.svelte";
  import Ilustracion from "$ui/componentes/Ilustracion.svelte";
  import { tip } from "$lib/tooltip";
  // Restaurar, como un asistente tranquilo: equipo → repositorio →
  // contraseña del repositorio (abre una sesión cifrada «explorar») → versión
  // (una línea de tiempo por días) → archivos (árbol, búsqueda y tamaños) →
  // dónde (junto al original, lo recomendado; en su sitio; o descargar por el
  // relé) → progreso → listo, con dónde quedó. La contraseña solo vive en
  // esta pantalla y se borra al salir; el servidor no ve ni rutas ni archivos.
  import { bytesRepo, destinoDe, informeDe, nVersiones, ultimaEjecucion, ultimaVersion } from "$lib/repo";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Pasos from "$lib/componentes/Pasos.svelte";
  import Copiable from "$lib/componentes/Copiable.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import { saludEquipo } from "$lib/salud";
  import { cargarInformes, ultimos } from "$lib/informes.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import { onDestroy, untrack } from "svelte";
  import { page } from "$app/state";
  import {
    ArrowLeft,
    ArrowRight,
    Calendar,
    Check,
    ChevronRight,
    Database,
    Download,
    FileArchive,
    File,
    Folder,
    FolderOpen,
    FolderPlus,
    HardDrive,
    History,
    LifeBuoy,
    LoaderCircle,
    LockKeyhole,
    Monitor,
    MonitorSmartphone,
    Search,
    Server,
    ShieldCheck,
    TriangleAlert,
    X,
  } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { enFondo } from "$lib/actividad.svelte";
  import { actual, app, puede, reloj } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { aB64, aleatorio, borrar } from "$lib/cripto/bytes";
  import { ErrorFaltaAdmin, ErrorLlavesCambiadas, mandarOrden } from "$lib/ordenar";
  import { comprobarLlaves } from "$lib/fijadas";
  import AlertaLlaves from "$lib/componentes/AlertaLlaves.svelte";
  import { Sesion, type MensajeEquipo } from "$lib/sesion";
  import { bajarRelevo, guardar, type Progreso } from "$lib/descarga";
  import { bytes, dia, fechaCorta, hora, plural } from "$lib/formato";
  import { ESTADO_ORDEN } from "$lib/salud";
  import { resultadoFirmado } from "$lib/cripto/claves";
  import type { Equipo, Orden, RepositorioResumen } from "$lib/tipos";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import CampoClave from "$lib/componentes/CampoClave.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import BotonCargando from "$lib/componentes/BotonCargando.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import RestaurarEnOtro from "$lib/componentes/RestaurarEnOtro.svelte";
  import LineaTiempoVersiones from "$lib/componentes/repo/LineaTiempoVersiones.svelte";
  import { reglaEfectiva } from "$lib/lineaTiempo";
  import { textoRestaurar } from "$lib/textosEquipo";
  // Plan 0.8.0, E1 y E2: otra carpeta, .zip, desde dónde, en vivo y motivo.
  import ElegirCarpetas from "$lib/componentes/ElegirCarpetas.svelte";
  import RestaurarDesdeEspejo from "$lib/componentes/RestaurarDesdeEspejo.svelte";
  import ProgresoCopia from "$lib/componentes/ProgresoCopia.svelte";
  import { tareasDe } from "$lib/progreso.svelte";
  import { espejoDelRepo, etiquetaCarpeta, nombreEnAlmacen } from "$lib/espejo";
  import { claveEspejo } from "$lib/cripto/ordenes";
  import { errorCarpetaRestaurar, errorContrasenaZip, errorNombreZip, MOTIVOS, recomendar, type OrigenRestauracion } from "$lib/restauraciones";
  import type { MotivoRestauracion } from "$lib/tipos";

  interface Version {
    id: string;
    cuando: string;
    archivos?: number;
    bytes?: number;
    etiquetas?: string[];
  }
  interface Entrada {
    nombre: string;
    tipo: "dir" | "archivo";
    bytes?: number;
    modificado?: string;
    ruta?: string;
  }

  type Paso = "origen" | "repo" | "desde" | "clave" | "version" | "archivos" | "destino" | "progreso";
  let paso = $state<Paso>("origen");
  // G2: con archivos elegidos y sin restaurar aún, salir pregunta antes.
  preguntarAlSalir(() => (paso === "archivos" || paso === "destino") && elegidas.size > 0, "Lo que has elegido para restaurar aún no se ha pedido al equipo, y se perderá.");
  let equipoId = $state(page.url.searchParams.get("equipo") ?? "");
  $effect(() => {
    // Las cargas, sin seguir lo que leen (si no, cada respuesta podría volver a lanzar el efecto).
    const [cc, ids] = [actual.id, actual.equipos.map((e) => e.id)];
    untrack(() => void cargarInformes(cc, ids));
  });
  let repoId = $state(page.url.searchParams.get("repo") ?? "");
  let contrasena = $state("");
  /** Primera vez que este navegador manda una contraseña a este equipo: también la clave de administración. */
  let claveAdmin = $state("");
  let pideAdmin = $state(false);
  let llavesCambiadas = $state(false);
  let error = $state("");
  let ocupado = $state(false);
  let pasoTxt = $state("");
  let sesion: Sesion | null = null;
  let versiones = $state<Version[]>([]);
  let version = $state<Version | null>(null);
  let hijos = $state<Record<string, Entrada[] | "cargando">>({});
  /** Carpetas que no se pudieron abrir: el motivo, en su sitio del árbol, con «Reintentar». */
  let fallosNivel = $state<Record<string, string>>({});
  let buscando = $state(false);
  let errorBusqueda = $state("");
  let abiertas = $state<Record<string, boolean>>({});
  let elegidas = $state<Map<string, Entrada>>(new Map());
  let buscar = $state("");
  let resultados = $state<Entrada[] | null>(null);
  let destino = $state<"junto" | "original" | "nueva" | "zip" | "descargar">("junto");
  let reemplazar = $state(false);
  let orden = $state<Orden | null>(null);
  let progreso = $state<Progreso | null>(null);
  let terminado = $state(false);
  const parar = new AbortController();

  const opciones = $derived(actual.equipos.filter((e) => e.modo !== "trasladado").flatMap((e) => (e.resumen?.repositorios ?? []).map((r) => ({ equipo: e, repo: r }))));
  const equipo = $derived<Equipo | undefined>(actual.equipos.find((e) => e.id === equipoId));
  // Recién importado de otro equipo (§10): puede que su informe aún no lo traiga; basta con su id y nombre.
  const repo = $derived<RepositorioResumen | undefined>(
    equipo?.resumen?.repositorios?.find((r) => r.id === repoId) ??
      (equipo && repoId && page.url.searchParams.get("nombre") ? { id: repoId, nombre: page.url.searchParams.get("nombre")!, destino: "", solo_lectura: true } : undefined),
  );
  /** «Restaurar en otro equipo»: abierto, y con qué repositorio de partida. */
  let enOtro = $state<{ equipo: string; repo: string } | null | undefined>(undefined);
  /** Rutas de una versión: las de restic, siempre con «/» (en Windows, `/C/Users/…`). */
  const unir = (ruta: string, nombre: string) => (ruta.endsWith("/") ? ruta + nombre : `${ruta}/${nombre}`);
  const totalElegido = $derived([...elegidas.values()].reduce((n, e) => n + (e.bytes ?? 0), 0));
  const hayCarpetas = $derived([...elegidas.values()].some((e) => e.tipo === "dir"));
  /** Lo más que puede ocupar una descarga por el relé (lo dice el servidor; uno anterior, 500 MB). */
  const MAX_RELEVO = $derived(app.servidor?.max_descarga ?? 500 * 1024 * 1024);

  // ---------- 14m: desde dónde ----------
  /** `""`: el repositorio; `externa` o el id de una copia derivada (las hace el propio equipo). */
  let desde = $state("");
  /** El origen elegido en el paso «Desde dónde» (también un espejo, que se abre aparte). */
  let origenElegido = $state("principal");
  let desdeEspejo = $state<{ almacen: Equipo; clave: string; repo: string } | null>(null);
  const informeRepo = $derived(equipo && repo ? informeDe(ultimos.porEquipo[equipo.id], repo.id) : null);
  const admiteDesde = $derived(!!equipo?.resumen?.admite?.includes("restaurar_desde"));
  const admiteCarpeta = $derived(!!equipo?.resumen?.admite?.includes("restaurar_carpeta"));
  const destinoRepo = $derived(equipo && repo ? destinoDe(equipo.resumen?.destinos, repo) : undefined);
  const almacenRepo = $derived(destinoRepo?.equipo_almacen ? actual.equipos.find((e) => e.id === destinoRepo.equipo_almacen) : undefined);
  const nombreEspejo = $derived(repo ? nombreEnAlmacen(destinoRepo?.donde, repo) : null);
  const espejoRepo = $derived(espejoDelRepo(almacenRepo?.resumen?.guarda_copias?.espejo ?? null, nombreEspejo));
  const estadoTarea = (t: { resultado?: string | null; ultima?: string | null } | null | undefined): OrigenRestauracion["estado"] =>
    !t?.ultima ? "desconocido" : t.resultado === "fallo" ? "fallo" : t.resultado === "aviso" ? "aviso" : "ok";
  /** Todos los sitios donde está este repositorio, de lo más cercano a lo más lejano, con el recomendado. */
  const origenes = $derived.by((): (OrigenRestauracion & { motivo?: string })[] => {
    if (!equipo || !repo) return [];
    const os: (OrigenRestauracion & { motivo?: string })[] = [];
    const ej = ultimaEjecucion(informeRepo);
    const sinEquipo = !equipo.conectado ? `${equipo.nombre} no está conectado ahora.` : undefined;
    const almacenCaido = almacenRepo && !almacenRepo.conectado ? `${almacenRepo.nombre} no responde.` : undefined;
    os.push({
      id: "principal",
      tipo: "principal",
      nombre: "Su repositorio",
      donde: destinoRepo?.nombre ?? repo.destino ?? "",
      estado: almacenCaido ? "fallo" : sinEquipo ? "aviso" : ej?.resultado === "fallo" ? "aviso" : "ok",
      ultima: ultimaVersion(repo, informeRepo),
      motivo: almacenCaido ?? sinEquipo,
    });
    const actualiza = `Para restaurar desde aquí, actualiza el agente de ${equipo.nombre}.`;
    if (repo.externa) {
      const t = informeRepo?.externa;
      os.push({ id: "externa", tipo: "externa", nombre: "Copia externa", donde: repo.externa.destino, estado: sinEquipo ? "aviso" : estadoTarea(t), ultima: t?.ultima ?? null, noSe: admiteDesde ? null : actualiza, motivo: sinEquipo });
    }
    for (const d of repo.derivadas ?? []) {
      const t = informeRepo?.derivadas?.find((x) => x.id === d.id);
      os.push({
        id: d.id,
        tipo: "derivada",
        nombre: "Copia derivada",
        donde: d.destino ?? "otro destino",
        estado: sinEquipo ? "aviso" : estadoTarea(t as { resultado?: string | null; ultima?: string | null } | undefined),
        ultima: t?.ultima ?? null,
        noSe: !admiteDesde ? actualiza : d.activa === false ? "Esta copia derivada no está activa en el equipo." : null,
        motivo: sinEquipo,
      });
    }
    for (const dd of espejoRepo?.destinos ?? []) {
      os.push({
        id: `espejo:${claveEspejo(dd)}`,
        tipo: "espejo",
        nombre: `Espejo de ${almacenRepo?.nombre ?? "el almacén"}`,
        donde: dd.tipo === "nube" ? (dd.nube ?? "nube") : etiquetaCarpeta(dd.carpeta),
        estado: dd.resultado === "fallo" ? "fallo" : dd.ultima ? "ok" : "desconocido",
        ultima: dd.ultima ?? null,
        noSe: nombreEspejo ? null : "No se sabe dónde está este repositorio en el espejo.",
      });
    }
    return recomendar(os);
  });
  const recomendado = $derived(origenes.find((o) => o.recomendado));
  const principalCaido = $derived(origenes[0]?.estado === "fallo" && !!recomendado && recomendado.id !== "principal");

  function seguirDesde() {
    const o = origenes.find((x) => x.id === origenElegido);
    if (!o || o.noSe) return;
    if (o.tipo === "espejo") {
      if (almacenRepo && nombreEspejo) desdeEspejo = { almacen: almacenRepo, clave: o.id.slice("espejo:".length), repo: nombreEspejo };
      return;
    }
    desde = o.tipo === "principal" ? "" : o.id;
    paso = "clave";
  }

  // ---------- 13a, 13b: otra carpeta y .zip ----------
  let carpetaNueva = $state("");
  let carpetaZip = $state("");
  let nombreZip = $state("");
  let contrasenaZip = $state("");
  let contrasenaZip2 = $state("");
  let claveAdminDestino = $state("");
  let explorarPara = $state<"nueva" | "zip" | null>(null);
  /** Lo que se mandó (para decir dónde quedó). */
  let enviado = $state<{ carpeta: string; nombreZip: string; cifrado: boolean } | null>(null);
  const errorDestino = $derived.by(() => {
    if (destino === "nueva") return errorCarpetaRestaurar(carpetaNueva, esWindows);
    if (destino === "zip") return errorCarpetaRestaurar(carpetaZip, esWindows) ?? errorNombreZip(nombreZip) ?? errorContrasenaZip(contrasenaZip, contrasenaZip2);
    if (destino === "descargar") return errorContrasenaZip(contrasenaZip, contrasenaZip2);
    return null;
  });
  const pideClaveAdmin = $derived(destino === "nueva" || destino === "zip");
  /** Dónde quedó el .zip (si ya había uno con ese nombre, el equipo le pone «(2)»). */
  const rutaZip = $derived.by(() => (enviado ? `${enviado.carpeta.replace(/[\\/]+$/, "")}${esWindows ? "\\" : "/"}${enviado.nombreZip}` : ""));

  // ---------- 14l: motivo ----------
  let motivo = $state<MotivoRestauracion | "">("");
  let notasMotivo = $state("");
  let motivoObligatorio = $state(false);
  $effect(() => {
    const [cc, eq] = [actual.id, equipoId];
    if (!cc || !eq) return;
    untrack(() =>
      void api
        .restauraciones(cc, eq)
        .then((r) => (motivoObligatorio = r.ajustes.motivo_obligatorio))
        .catch(() => {}),
    );
  });
  /** Qué falta para poder restaurar (el botón lo dice). */
  const falta = $derived.by(() => {
    if (errorDestino) return errorDestino;
    if (pideClaveAdmin && !claveAdminDestino) return "Escribe la clave de administración.";
    if (motivoObligatorio && !motivo) return "Elige el motivo de la restauración.";
    if (destino === "descargar" && totalElegido > MAX_RELEVO) return `Son más de ${bytes(MAX_RELEVO)}: guárdalo como .zip en el equipo.`;
    return "";
  });

  /** 13c: la restauración en vivo en el equipo (con el estilo de las copias). */
  const enVivo = $derived(equipo && repo ? tareasDe(equipo.id, { repo: repo.id, tipos: ["restauracion"] })[0] : undefined);

  // Si se llega con ?equipo=&repo= (o solo ?equipo=), se saltan los primeros pasos (una vez).
  let saltado = false;
  $effect(() => {
    if (saltado || paso !== "origen" || !equipo) return;
    if (page.url.searchParams.get("repo")) {
      if (repo) {
        paso = "clave";
        saltado = true;
      }
    } else if (page.url.searchParams.get("equipo")) {
      const rs = equipo.resumen?.repositorios ?? [];
      if (rs.length === 1) elegirRepo(rs[0].id);
      else paso = "repo";
      saltado = true;
    }
  });

  /** Los equipos que se pueden elegir: con algún repositorio y no trasladados. */
  const activos = $derived(actual.equipos.filter((e) => e.confirmado && e.modo !== "trasladado"));
  const conRepos = $derived(activos.filter((e) => (e.resumen?.repositorios ?? []).length));

  function elegirEquipo(e: Equipo) {
    equipoId = e.id;
    error = "";
    const rs = e.resumen?.repositorios ?? [];
    if (rs.length === 1) elegirRepo(rs[0].id);
    else paso = "repo";
  }
  function elegirRepo(id: string) {
    repoId = id;
    contrasena = "";
    desde = "";
    // 14m: si el repositorio está en más de un sitio, primero «Desde dónde» (con el recomendado ya elegido).
    if (origenes.length > 1) {
      origenElegido = recomendado?.id ?? "principal";
      paso = "desde";
    } else paso = "clave";
  }
  /** Volver a un paso ya hecho (desde los pasos de arriba). Antes de la versión, se cierra la sesión abierta. */
  function volverA(id: string): boolean {
    if (ocupado || paso === "progreso") return false;
    const destinoPaso = id as Paso;
    if (indice(destinoPaso) >= indice(paso)) return false;
    if (indice(destinoPaso) <= indice("clave")) {
      void sesion?.cerrar();
      sesion = null;
      versiones = [];
      version = null;
      elegidas = new Map();
      claveAdmin = "";
    }
    if (destinoPaso === "origen") repoId = "";
    if (indice(destinoPaso) <= indice("desde")) desde = "";
    error = "";
    paso = destinoPaso;
    return true;
  }

  onDestroy(() => {
    parar.abort();
    void sesion?.cerrar();
    contrasena = claveAdmin = "";
  });
  // Al llegar a la contraseña se mira si las llaves del equipo ya están fijadas aquí.
  $effect(() => {
    if (paso === "clave" && equipo) {
      const [cc, eq] = [actual.id, equipo];
      untrack(() => void comprobarLlaves(cc, eq).then((x) => {
        llavesCambiadas = x === "cambiada";
        pideAdmin = x === "sin_fijar";
      }));
    }
  });

  async function abrir(e: SubmitEvent) {
    e.preventDefault();
    if (!equipo || !repo || !actual.cliente) return;
    error = "";
    ocupado = true;
    try {
      sesion = await Sesion.abrir({ cliente: actual.cliente, equipo, tipo: "explorar", cuerpo: { repo: repo.id, ...(desde ? { desde } : {}) }, secretos: { repo: { repo: repo.id, contrasena }, claveAdmin: pideAdmin ? claveAdmin : undefined }, alPaso: (t) => (pasoTxt = t) });
      claveAdmin = "";
      pideAdmin = false;
      pasoTxt = "Esperando al equipo…";
      await sesion.lista;
      pasoTxt = "Leyendo las versiones…";
      const r = await sesion.pedir<MensajeEquipo & { versiones: Version[] }>("versiones", { repo: repo.id });
      versiones = (r.versiones ?? []).sort((a, b) => Date.parse(b.cuando) - Date.parse(a.cuando));
      paso = "version";
      // Desde la lista de versiones del repositorio: esa versión directamente
      // («Explorar»), o entera, lista para elegir dónde («Restaurar»).
      const pedida = page.url.searchParams.get("version");
      const v = pedida ? versiones.find((x) => x.id === pedida || x.id.startsWith(pedida)) : undefined;
      if (v) {
        elegirVersion(v);
        if (page.url.searchParams.get("todo") === "1") {
          pasoTxt = "Leyendo la versión…";
          await cargar("/");
          const raiz = hijos["/"];
          if (Array.isArray(raiz) && raiz.length) {
            elegidas = new Map(raiz.map((x) => [unir("/", x.nombre), x]));
            paso = "destino";
          }
        }
      }
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) llavesCambiadas = true;
      else if (err instanceof ErrorFaltaAdmin) pideAdmin = true;
      error = err instanceof ErrorLlavesCambiadas ? "" : (err as Error).message;
      void sesion?.cerrar();
      sesion = null;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }

  async function cargar(ruta: string) {
    if (!sesion || !version || hijos[ruta]) return;
    hijos[ruta] = "cargando";
    delete fallosNivel[ruta];
    try {
      const r = await sesion.pedir<MensajeEquipo & { entradas: Entrada[] }>("listar", { version: version.id, ruta });
      hijos[ruta] = [...(r.entradas ?? [])].sort((a, b) => (a.tipo === b.tipo ? a.nombre.localeCompare(b.nombre) : a.tipo === "dir" ? -1 : 1));
    } catch (err) {
      delete hijos[ruta];
      fallosNivel[ruta] = (err as Error).message || "El equipo no respondió.";
    }
  }

  function elegirVersion(v: Version) {
    version = v;
    hijos = {};
    fallosNivel = {};
    errorBusqueda = "";
    abiertas = {};
    elegidas = new Map();
    resultados = null;
    paso = "archivos";
    void cargar("/");
  }

  function marcar(ruta: string, e: Entrada, on: boolean) {
    const m = new Map(elegidas);
    if (on) m.set(ruta, e);
    else m.delete(ruta);
    elegidas = m;
  }

  async function hacerBusqueda(ev?: SubmitEvent) {
    ev?.preventDefault();
    if (!sesion || !version || !buscar.trim()) return;
    resultados = null;
    errorBusqueda = "";
    buscando = true;
    try {
      const r = await sesion.pedir<MensajeEquipo & { resultados: Entrada[] }>("buscar", { version: version.id, texto: buscar.trim() });
      resultados = r.resultados ?? [];
    } catch (err) {
      errorBusqueda = (err as Error).message || "El equipo no respondió.";
    } finally {
      buscando = false;
    }
  }

  // La línea de tiempo del paso «Versión»: la copia de cada versión sale del informe (sus ids son los 8 primeros).
  const versionesLinea = $derived.by(() => {
    const inf = equipo && repo ? informeDe(ultimos.porEquipo[equipo.id], repo.id) : null;
    return versiones.map((v) => {
      const x = inf?.versiones.find((i) => v.id.startsWith(i.id) || i.id.startsWith(v.id));
      return { id: v.id, hora: v.cuando, copia: x?.copia ?? null, bytes: v.bytes ?? x?.total_bytes ?? null, anadido: x ? (x.anadido_empaquetado ?? x.anadido) : null, archivos: v.archivos ?? null, etiquetas: v.etiquetas ?? [] };
    });
  });
  const retencionLinea = $derived(repo && equipo ? reglaEfectiva(repo, destinoDe(equipo.resumen?.destinos, repo), actual.equipos) : null);
  async function restaurar() {
    if (!equipo || !repo || !version || !actual.cliente) return;
    if (falta) {
      error = falta;
      return;
    }
    error = "";
    ocupado = true;
    const rutas = [...elegidas.keys()];
    carpetaFija = carpetaRestaurado;
    // 14l: el motivo va con la orden (el equipo lo anota en su registro, que ven todas las consolas).
    const conMotivo = motivo ? { motivo: { tipo: motivo, ...(notasMotivo.trim() ? { notas: notasMotivo.trim().slice(0, 500) } : {}) } } : {};
    const conDesde = desde ? { desde } : {};
    const zipCifrado = !!contrasenaZip && (destino === "zip" || destino === "descargar");
    try {
      if (destino === "descargar") {
        const relevo = crypto.randomUUID();
        const clave = aleatorio(32);
        // Con contraseña (13b), siempre un .zip en AES-256 (lo hace el equipo).
        const formato = rutas.length === 1 && !hayCarpetas && !zipCifrado ? "archivo" : "zip";
        orden = await mandarOrden({
          cliente: actual.cliente,
          equipo,
          tipo: "descargar",
          cuerpo: { repo: repo.id, version: version.id, rutas, formato, relevo: { id: relevo, clave: aB64(clave) }, ...conDesde, ...conMotivo, ...(zipCifrado ? { contrasena_zip: contrasenaZip } : {}) },
          secretos: { repo: { repo: repo.id, contrasena } },
          relevo: { id: relevo, max_bytes: MAX_RELEVO },
          alPaso: (t) => (pasoTxt = t),
        });
        contrasenaZip = contrasenaZip2 = "";
        void guardarMotivo(orden.id);
        paso = "progreso";
        const blob = await bajarRelevo({ cliente: actual.cliente.id, relevo, clave, signal: parar.signal, alProgreso: (p) => (progreso = p) });
        borrar(clave);
        const nombre = formato === "archivo" ? rutas[0].split(/[\\/]/).pop() || "archivo" : `restauracion-${equipo.nombre}-${new Date(version.cuando).toISOString().slice(0, 10)}.zip`;
        await guardar(blob, nombre);
        enviado = { carpeta: "", nombreZip: nombre, cifrado: zipCifrado };
        terminado = true;
        avisar("Descarga lista.");
      } else {
        const cuerpo: Record<string, unknown> = { repo: repo.id, version: version.id, rutas, ...conDesde, ...conMotivo };
        if (destino === "nueva") Object.assign(cuerpo, { destino: "nueva", carpeta: carpetaNueva.trim() });
        else if (destino === "zip")
          Object.assign(cuerpo, { destino: "zip", carpeta: carpetaZip.trim(), ...(nombreZip.trim() ? { nombre: nombreZip.trim() } : {}), ...(zipCifrado ? { contrasena_zip: contrasenaZip } : {}) });
        else Object.assign(cuerpo, { destino: destino === "original" ? "original" : "junto", reemplazar: destino === "original" && reemplazar });
        orden = await mandarOrden({
          cliente: actual.cliente,
          equipo,
          tipo: "restaurar",
          cuerpo,
          secretos: { repo: { repo: repo.id, contrasena }, ...(pideClaveAdmin ? { claveAdmin: claveAdminDestino } : {}) },
          alPaso: (t) => (pasoTxt = t),
        });
        enviado = { carpeta: destino === "nueva" ? carpetaNueva.trim() : destino === "zip" ? carpetaZip.trim() : "", nombreZip: destino === "zip" ? `${nombreZip.trim().replace(/\.zip$/i, "") || carpetaRestaurado}.zip` : "", cifrado: zipCifrado };
        claveAdminDestino = contrasenaZip = contrasenaZip2 = "";
        void guardarMotivo(orden.id);
        paso = "progreso";
        seguir(orden.id);
      }
    } catch (err) {
      error = err instanceof ErrorLlavesCambiadas ? err.message : (err as Error).message;
      if (paso !== "progreso") paso = "destino";
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }

  /** 14l: el motivo también en esta consola (se puede cambiar después en «Restauraciones»). */
  async function guardarMotivo(ordenId: string) {
    if (!motivo || !actual.cliente || !equipo) return;
    try {
      await api.ponerMotivoRestauracion(actual.cliente.id, equipo.id, `restauracion-${ordenId}`, motivo, notasMotivo.trim() || null);
    } catch {
      /* Con un servidor anterior no se guarda aquí; el equipo lo anota igual. */
    }
  }

  /** Tras un «en su sitio» rechazado: lo mismo, junto al original (un clic). */
  async function restaurarJunto() {
    destino = "junto";
    reemplazar = false;
    terminado = false;
    orden = null;
    await restaurar();
  }

  function seguir(id: string) {
    const t = setInterval(async () => {
      try {
        const o = (await enFondo(() => api.ordenesEquipo(actual.id, equipoId, 10))).find((x) => x.id === id);
        if (o) orden = o;
        if (o && ["hecha", "fallida", "rechazada", "cancelada", "caducada"].includes(o.estado)) {
          clearInterval(t);
          terminado = true;
        }
      } catch {
        /* se reintenta */
      }
    }, 1500);
    parar.signal.addEventListener("abort", () => clearInterval(t));
  }

  // «Junto al original», como lo hace el agente (sesiones_v2.rs, restaurar): en
  // la carpeta de cada elemento, una nueva «Restaurado AAAA-MM-DD HHMM» (su
  // hora local al restaurar) con el elemento dentro.
  const p2 = (n: number) => String(n).padStart(2, "0");
  const carpetaRestaurado = $derived.by(() => {
    const d = new Date(reloj.ahora);
    return `Restaurado ${d.getFullYear()}-${p2(d.getMonth() + 1)}-${p2(d.getDate())} ${p2(d.getHours())}${p2(d.getMinutes())}`;
  });
  /** Ruta de restic («/C/Users/…») a la del equipo («C:\Users\…» en Windows). */
  const rutaLocal = (r: string) => {
    if (!/windows/i.test(equipo?.so ?? "")) return r;
    const resto = r.replace(/^\/+/, "");
    const i = resto.indexOf("/");
    const unidad = i < 0 ? resto : resto.slice(0, i);
    return unidad.length === 1 ? `${unidad}:\\${i < 0 ? "" : resto.slice(i + 1).replaceAll("/", "\\")}` : r.replaceAll("/", "\\");
  };
  const esWindows = $derived(/windows/i.test(equipo?.so ?? ""));
  /** La carpeta de una ruta, como la ve el equipo («C:\Users\ana\Documentos»). */
  const carpetaDe = (r: string) => {
    const corte = r.replace(/\/+$/, "").lastIndexOf("/");
    return corte <= 0 ? "" : rutaLocal(r.slice(0, corte));
  };
  const ejemploJunto = $derived.by(() => {
    const primera = [...elegidas.keys()][0];
    if (!primera) return "";
    const corte = primera.replace(/\/+$/, "").lastIndexOf("/");
    if (corte <= 0) return "";
    const padre = primera.slice(0, corte);
    const nombre = primera.slice(corte + 1);
    const sep = /windows/i.test(equipo?.so ?? "") ? "\\" : "/";
    return `${rutaLocal(padre).replace(/[\\/]+$/, "")}${sep}${carpetaRestaurado}${sep}${nombre}`;
  });

  /** La carpeta «Restaurado …» junto al primer elemento, con la hora en que se pidió (para «Abrir la carpeta»). */
  let carpetaFija = $state("");
  const carpetaJunto = $derived.by(() => {
    const primera = [...elegidas.keys()][0];
    if (!primera || !carpetaFija) return "";
    const corte = primera.replace(/\/+$/, "").lastIndexOf("/");
    if (corte <= 0) return "";
    const sep = /windows/i.test(equipo?.so ?? "") ? "\\" : "/";
    return `${rutaLocal(primera.slice(0, corte)).replace(/[\\/]+$/, "")}${sep}${carpetaFija}`;
  });

  const PASOS: { id: Paso; texto: string }[] = [
    { id: "origen", texto: "Equipo" },
    { id: "repo", texto: "Repositorio" },
    { id: "desde", texto: "Desde dónde" },
    { id: "clave", texto: "Contraseña" },
    { id: "version", texto: "Versión" },
    { id: "archivos", texto: "Archivos" },
    { id: "destino", texto: "Dónde" },
    { id: "progreso", texto: "Listo" },
  ];
  const indice = (p: Paso) => PASOS.findIndex((x) => x.id === p);

  /** Cómo va una restauración en el equipo (la orden): enviada, recogida, restaurando, hecha. */
  const ETAPAS = [
    { id: "enviada", texto: "Enviada" },
    { id: "recogida", texto: "El equipo la recoge" },
    { id: "restaurando", texto: "Restaurando" },
    { id: "hecha", texto: "Hecho" },
  ];
  const etapa = $derived(!orden ? 0 : orden.estado === "pendiente" ? 1 : orden.estado === "entregada" || orden.estado === "en_marcha" ? 2 : 3);
</script>

<svelte:head><title>Restaurar · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <CabeceraPagina
    titulo="Restaurar archivos"
    icono={History}
    migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${actual.id}` }, { texto: "Restaurar" }]}
    resumen="Recupera archivos de cualquier versión, paso a paso. Llegan cifrados desde el equipo: el servidor no ve ni nombres ni contenido."
  >
    {#snippet acciones()}
      {#if puede.administrar(actual.cliente?.rol) && activos.length > 1 && paso !== "progreso"}
        <button class="btn" onclick={() => (enOtro = equipoId && repoId ? { equipo: equipoId, repo: repoId } : null)}><MonitorSmartphone size={16} />Restaurar en otro equipo</button>
      {/if}
    {/snippet}
  </CabeceraPagina>

  {#if !puede.ordenar(actual.cliente?.rol) && actual.cliente}
    <div class="notice notice-info"><p>Tu papel en este cliente es de solo lectura: no puede restaurar.</p></div>
  {:else}
    <Pasos pasos={PASOS} actual={paso} completo={paso === "progreso" && terminado && (destino === "descargar" || orden?.estado === "hecha")} alElegir={paso === "progreso" || ocupado ? undefined : volverA} />

    {#if paso === "origen"}
      <section class="paso-card" aria-labelledby="t-origen">
        <header class="paso-cab">
          <h2 id="t-origen">¿De qué equipo son los archivos?</h2>
          <p>Elige el equipo donde estaban. Después, su repositorio y la versión.</p>
        </header>
        {#if !actual.cargado}
          <Esqueleto forma="tarjetas" n={3} />
        {:else if conRepos.length}
          <div class="rejilla equipos-r">
            {#each conRepos as e (e.id)}
              {@const s = saludEquipo(e, reloj.ahora)}
              {@const rs = e.resumen?.repositorios ?? []}
              <button class="card tile elegible" onclick={() => elegirEquipo(e)}>
                <span class="tile-cab">
                  <span class="tile-ic">{#if e.rol === "almacenamiento"}<Server size={16} />{:else}<Monitor size={16} />{/if}</span>
                  <span class="tile-nombre"><strong>{e.nombre}</strong><span>{#if e.conectado}<span class="dot" style="--tone: var(--ok)" aria-hidden="true"></span> Conectado{:else}Visto <Tiempo iso={e.ultimo_contacto} />{/if}</span></span>
                  <Chip pequeno tono={s.tono} texto={s.texto} />
                </span>
                <span class="tile-linea">{plural(rs.length, "repositorio", "repositorios")}: {rs.map((r) => r.nombre).join(", ")}</span>
              </button>
            {/each}
          </div>
        {:else}
          <div class="card">
            <Vacio icono={History} ilustracion="sin-versiones" titulo="Nada que restaurar todavía" texto="Cuando un equipo tenga su primera copia, podrás recuperar sus archivos desde aquí.">
              <a class="btn btn-primary" href="/c/{actual.id}/equipos">Ver los equipos</a>
            </Vacio>
          </div>
        {/if}
      </section>
    {:else if paso === "repo" && equipo}
      <section class="paso-card" aria-labelledby="t-repo">
        <header class="paso-cab">
          <h2 id="t-repo">¿De qué repositorio, en {equipo.nombre}?</h2>
          <p>Cada repositorio guarda las versiones de unas copias.</p>
        </header>
        <div class="card p-0 lista">
          {#each equipo.resumen?.repositorios ?? [] as r (r.id)}
            {@const inf = informeDe(ultimos.porEquipo[equipo.id], r.id)}
            {@const copiasDe = (equipo.resumen?.copias ?? []).filter((k) => k.repo === r.id)}
            {@const ult = ultimaVersion(r, inf)}
            <button class="fila elegible-fila" onclick={() => elegirRepo(r.id)}>
              <span class="tile-ic"><Database size={16} /></span>
              <span class="fila-texto">
                <span class="fila-titulo">{r.nombre}{#if r.solo_lectura} <span class="badge badge-sm tone-neutral" use:tip={"Importado de otro equipo: se puede explorar y restaurar, pero ninguna copia escribe en él."}>Solo lectura</span>{/if}</span>
                <span class="fila-sub">{copiasDe.length ? `Copias: ${copiasDe.map((k) => k.nombre).join(", ")}` : "Sin copias que escriban en él"}</span>
              </span>
              <span class="fila-meta num">
                {plural(nVersiones(r, inf), "versión", "versiones")}{#if bytesRepo(r, inf) != null}{" · "}{bytes(bytesRepo(r, inf))}{/if}
                {#if ult}<span class="bloque">la última, <Tiempo iso={ult} /></span>{/if}
              </span>
              <ChevronRight size={16} />
            </button>
          {/each}
        </div>
        <div class="fin izq"><button class="btn btn-ghost" onclick={() => volverA("origen")}><ArrowLeft size={15} />Otro equipo</button></div>
      </section>
    {:else if paso === "desde" && equipo && repo}
      <section class="paso-card" aria-labelledby="t-desde">
        <header class="paso-cab">
          <h2 id="t-desde">¿Desde dónde restauras «{repo.nombre}»?</h2>
          <p>Este repositorio está en más de un sitio. Lo más cercano suele ser lo más rápido; si no responde, usa otro.</p>
        </header>
        {#if principalCaido && recomendado}
          <div class="notice notice-warn">
            <TriangleAlert size={16} />
            <p>{origenes[0].motivo ?? "Su repositorio no responde."} Te proponemos restaurar desde «{recomendado.donde}»{#if recomendado.ultima} (actualizada <Tiempo iso={recomendado.ultima} />){/if}.</p>
          </div>
        {/if}
        <div class="card p-0 lista" role="radiogroup" aria-labelledby="t-desde">
          {#each origenes as o (o.id)}
            <label class="fila elegible-fila origen" class:on={origenElegido === o.id} class:apagado={!!o.noSe}>
              <input type="radio" name="desde" value={o.id} bind:group={origenElegido} disabled={!!o.noSe} />
              <span class="tile-ic">{#if o.tipo === "espejo"}<LifeBuoy size={16} />{:else if o.tipo === "principal"}<Database size={16} />{:else}<Server size={16} />{/if}</span>
              <span class="fila-texto">
                <span class="fila-titulo">{o.nombre} · {o.donde}{#if o.recomendado} <span class="badge badge-sm tone-ok">Recomendado</span>{/if}</span>
                <span class="fila-sub">
                  {#if o.noSe}{o.noSe}{:else if o.motivo}{o.motivo}{:else if o.tipo === "espejo"}Se abre con la contraseña del repositorio, en el equipo que elijas.{:else if o.tipo === "principal"}Donde copia {equipo.nombre}.{:else}La hace {equipo.nombre}: se restaura con su acceso.{/if}
                </span>
              </span>
              <span class="fila-meta num">
                <Chip pequeno tono={o.estado === "ok" ? "ok" : o.estado === "fallo" ? "bad" : o.estado === "aviso" ? "warn" : "neutral"} texto={o.estado === "ok" ? "Al día" : o.estado === "fallo" ? "No responde" : o.estado === "aviso" ? "Con avisos" : "Sin datos"} />
                {#if o.ultima}<span class="bloque">actualizado <Tiempo iso={o.ultima} /></span>{/if}
              </span>
            </label>
          {/each}
        </div>
        <div class="fin">
          <button class="btn btn-ghost" onclick={() => volverA((equipo?.resumen?.repositorios?.length ?? 0) > 1 ? "repo" : "origen")}><ArrowLeft size={15} />Atrás</button>
          <button class="btn btn-primary" disabled={!!origenes.find((x) => x.id === origenElegido)?.noSe} onclick={seguirDesde}>Seguir<ArrowRight size={15} /></button>
        </div>
      </section>
    {:else if paso === "clave" && equipo && repo}
      <section class="card p estrecha">
        <form class="form" onsubmit={abrir}>
          <div class="dlg-title">
            <span class="ticon"><LockKeyhole size={18} /></span>
            <div>
              <h2>Abrir «{repo.nombre}» de {equipo.nombre}{#if desde}, desde «{origenes.find((o) => o.id === desde)?.donde ?? "otra copia"}»{/if}</h2>
              <p>Para ver sus versiones hace falta la contraseña del repositorio. Se usa solo en esta pantalla y se olvida al salir.</p>
            </div>
          </div>
          <CampoClave requerido id="clave-repo" etiqueta="Contraseña del repositorio" bind:value={contrasena} autofocus ayuda="Está en su kit de recuperación (el que se imprimió al crearlo).">
            {#snippet extra()}<Ayuda id="contrasena-repo" />{/snippet}
          </CampoClave>
          {#if pideAdmin}
            <p class="faint pequeno">Es la primera vez que este navegador manda una contraseña a {equipo.nombre}: con la clave de administración se comprueba que sus llaves son las auténticas y se recuerdan.</p>
            <CampoClave requerido id="clave-admin" etiqueta="Clave de administración" bind:value={claveAdmin}>
              {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
            </CampoClave>
          {/if}
          {#if llavesCambiadas}<AlertaLlaves {equipo} cliente={actual.id} />{/if}
          {#if !equipo.conectado}<div class="notice notice-warn"><TriangleAlert size={16} /><p>{equipo.nombre} no está conectado ahora: abrirá el repositorio cuando vuelva. Puedes esperar aquí.</p></div>{/if}
          {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
          <div class="fin">
            {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
            <button type="button" class="btn btn-ghost" onclick={() => volverA(origenes.length > 1 ? "desde" : (equipo?.resumen?.repositorios?.length ?? 0) > 1 ? "repo" : "origen")} disabled={ocupado}><ArrowLeft size={15} />Atrás</button>
            <button class="btn btn-primary" disabled={!contrasena || (pideAdmin && !claveAdmin) || ocupado || llavesCambiadas}><LockKeyhole size={15} />{pideAdmin ? "Abrir con las dos claves" : "Abrir el repositorio"}</button>
          </div>
        </form>
      </section>
    {:else if paso === "version"}
      <section class="paso-card" aria-labelledby="t-version">
        <header class="paso-cab">
          <h2 id="t-version">¿De qué momento?</h2>
          <p>{plural(versiones.length, "versión guardada", "versiones guardadas")} de «{repo?.nombre}». Elige la de antes del problema (si se borró o estropeó algo, la anterior a ese día).</p>
        </header>
        {#if versiones.length}
          <div class="card p">
            <LineaTiempoVersiones
              versiones={versionesLinea}
              marcas={{ equipo, repo: repo?.id ?? null }}
              copias={equipo?.resumen?.copias?.filter((k) => k.repo === repo?.id) ?? []}
              regla={retencionLinea?.regla ?? null}
              quien={retencionLinea?.quien ?? null}
              ahora={reloj.ahora}
              seleccion={version?.id ?? null}
              alElegir={(id) => {
                const v = versiones.find((x) => x.id === id);
                if (v) elegirVersion(v);
              }}
              etiqueta="Versiones de «{repo?.nombre ?? ''}» en el tiempo; elige una para ver sus archivos"
            />
          </div>
        {:else}
          <div class="card"><Vacio icono={Calendar} ilustracion="sin-versiones" titulo="Este repositorio aún no tiene versiones" texto="Cuando se haga su primera copia, aparecerá aquí." /></div>
        {/if}
      </section>
    {:else if paso === "archivos" && version}
      <section class="card p-0 archivos" aria-labelledby="t-archivos">
        <div class="cab-paso">
          <FolderOpen size={16} />
          <span class="cab-texto"><strong id="t-archivos">¿Qué archivos?</strong><span class="faint">Versión del {dia(version.cuando)}, {hora(version.cuando)} <MarcasVersion {equipo} repo={repo?.id ?? ""} version={version.id} /></span></span>
          <button class="link" onclick={() => volverA("version")}>Otra versión</button>
        </div>
        <form class="buscar" onsubmit={hacerBusqueda} role="search">
          <Search size={15} />
          <input class="input" type="search" placeholder="Busca por nombre: «factura», «.xlsx»…" bind:value={buscar} aria-label="Buscar en la versión" />
          <BotonCargando class="btn btn-sm" cargando={buscando} textoCargando="Buscando…" disabled={!buscar.trim()}>Buscar</BotonCargando>
        </form>
        {#if buscando}
          <p class="cargando" role="status"><LoaderCircle size={14} class="spin" />Buscando «{buscar.trim()}» en la versión… En carpetas grandes tarda un poco.</p>
        {:else if errorBusqueda}
          <div class="notice notice-danger fallo-arbol" role="alert">
            <TriangleAlert size={16} />
            <p>No se pudo buscar: {errorBusqueda} <button type="button" class="notice-action" onclick={() => hacerBusqueda()}>Reintentar</button> · <button type="button" class="link" onclick={() => (errorBusqueda = "")}>Volver a las carpetas</button></p>
          </div>
        {:else if resultados}
          <div class="resultados">
            <p class="faint pequeno res-cab">{resultados.length ? plural(resultados.length, "resultado", "resultados") : "Nada con ese nombre en esta versión"} · <button class="link" onclick={() => (resultados = null)}>Volver a las carpetas</button></p>
            {#each resultados as r (r.ruta)}
              <label class="nodo">
                <input type="checkbox" checked={elegidas.has(r.ruta!)} onchange={(e) => marcar(r.ruta!, r, e.currentTarget.checked)} />
                {#if r.tipo === "dir"}<Folder size={15} class="ic-dir" />{:else}<File size={15} />{/if}
                <span class="nombre res-nombre"><span>{r.ruta!.replace(/\/+$/, "").split("/").pop()}</span><span class="faint pequeno">{carpetaDe(r.ruta!)}</span></span>
                {#if r.tipo !== "dir"}{#if r.modificado}<span class="faint pequeno meta fecha num">{fechaCorta(r.modificado)}</span>{/if}<span class="faint pequeno meta num">{bytes(r.bytes)}</span>{/if}
              </label>
            {/each}
          </div>
        {:else}
          <div class="arbol" role="tree" aria-label="Archivos de la versión" aria-multiselectable="true">{@render nivel("/")}</div>
        {/if}
        <div class="pie-paso">
          <div class="elegidas">
            {#if elegidas.size}
              <strong class="num">{plural(elegidas.size, "elemento", "elementos")}{totalElegido ? ` · ${bytes(totalElegido)}` : ""}</strong>
              <span class="chips-elegidas">
                {#each [...elegidas.entries()].slice(0, 3) as [ruta, e] (ruta)}
                  <span class="chip-elegida">{#if e.tipo === "dir"}<Folder size={12} />{:else}<File size={12} />{/if}<span class="ce-nombre">{e.nombre}</span><button type="button" aria-label="Quitar {e.nombre}" onclick={() => marcar(ruta, e, false)}><X size={12} /></button></span>
                {/each}
                {#if elegidas.size > 3}<span class="faint pequeno">y {elegidas.size - 3} más</span>{/if}
              </span>
            {:else}
              <span class="faint">Marca los archivos o carpetas que quieras recuperar.</span>
            {/if}
          </div>
          <button class="btn btn-primary" disabled={!elegidas.size} onclick={() => (paso = "destino")}>Seguir<ArrowRight size={15} /></button>
        </div>
      </section>
    {:else if paso === "destino" && equipo && version}
      <section class="card p estrecha form" aria-labelledby="t-destino">
        <div class="resumen-rest">
          <span class="tile-ic"><History size={16} /></span>
          <p>
            <strong>{plural(elegidas.size, "elemento", "elementos")}{totalElegido ? ` · ${bytes(totalElegido)}` : ""}</strong> de «{repo?.nombre}», versión del {dia(version.cuando)}, {hora(version.cuando)}.
          </p>
        </div>
        <h2 class="section-title" id="t-destino">¿Dónde los quieres?</h2>
        <div class="opciones" role="radiogroup" aria-labelledby="t-destino">
          <label class="opcion" class:on={destino === "junto"}>
            <input type="radio" bind:group={destino} value="junto" />
            <FolderPlus size={18} />
            <span>
              <strong>En {equipo.nombre}, junto al original <span class="badge badge-sm tone-ok">Recomendado</span></strong>
              <span class="faint">Al lado de cada elemento se crea una carpeta «{carpetaRestaurado}» con lo restaurado dentro. No toca nada de lo que hay: compara y quédate con lo que quieras.</span>
              {#if ejemploJunto}<span class="ejemplo">Por ejemplo: <code>{ejemploJunto}</code></span>{/if}
            </span>
          </label>
          <label class="opcion" class:on={destino === "original"} class:apagado={!!desde}>
            <input type="radio" bind:group={destino} value="original" disabled={!!desde} />
            <HardDrive size={18} />
            <span>
              <strong>En {equipo.nombre}, en su sitio</strong>
              <span class="faint">{desde ? "Desde otra copia no se restaura en su sitio: elige junto al original u otra carpeta." : "Vuelve a donde estaba. Si ya existe, se deja como está salvo que marques «Reemplazar»."}</span>
            </span>
          </label>
          <label class="opcion" class:on={destino === "nueva"} class:apagado={!admiteCarpeta}>
            <input type="radio" bind:group={destino} value="nueva" disabled={!admiteCarpeta} />
            <FolderOpen size={18} />
            <span>
              <strong>En {equipo.nombre}, en otra carpeta</strong>
              <span class="faint">{admiteCarpeta ? "En una carpeta nueva o vacía que elijas (también una unidad entera). Nunca sobrescribe nada." : `Para elegir otra carpeta, actualiza el agente de ${equipo.nombre}.`}</span>
            </span>
          </label>
          <label class="opcion" class:on={destino === "zip"} class:apagado={!admiteCarpeta}>
            <input type="radio" bind:group={destino} value="zip" disabled={!admiteCarpeta} />
            <FileArchive size={18} />
            <span>
              <strong>En {equipo.nombre}, como .zip</strong>
              <span class="faint">{admiteCarpeta ? "Un .zip en la carpeta que elijas, sin límite de tamaño y con contraseña si quieres." : `Para guardarlo como .zip, actualiza el agente de ${equipo.nombre}.`}</span>
            </span>
          </label>
          <label class="opcion" class:on={destino === "descargar"}>
            <input type="radio" bind:group={destino} value="descargar" />
            <Download size={18} />
            <span><strong>Descargar en este navegador</strong><span class="faint">Hasta {bytes(MAX_RELEVO)}{admiteCarpeta ? ", como .zip con contraseña si quieres" : ""}. Llega cifrado por el servidor, que no puede leerlo. <Ayuda id="relevo" /></span></span>
          </label>
        </div>
        {#if destino === "original"}
          <label class="switch-row"><input type="checkbox" bind:checked={reemplazar} /><span><strong>Reemplazar</strong> los archivos que ya existan<span class="faint">Lo que haya ahora se perderá.</span></span></label>
        {/if}
        {#if destino === "nueva"}
          <div class="field">
            <label class="field-label" for="carpeta-nueva">Carpeta en {equipo.nombre}</label>
            <div class="con-boton">
              <input id="carpeta-nueva" class="input mono" bind:value={carpetaNueva} placeholder={esWindows ? "D:\\Restaurado" : "/srv/restaurado"} autocomplete="off" spellcheck="false" />
              <button type="button" class="btn" onclick={() => (explorarPara = "nueva")}><FolderOpen size={15} />Explorar…</button>
            </div>
            <span class="field-hint">Una carpeta nueva (se crea) o vacía. No vale una carpeta del sistema, de los programas ni de Resguardo.</span>
            {#if carpetaNueva.trim() && errorCarpetaRestaurar(carpetaNueva, esWindows)}<span class="field-error">{errorCarpetaRestaurar(carpetaNueva, esWindows)}</span>{/if}
          </div>
        {/if}
        {#if destino === "zip"}
          <div class="field">
            <label class="field-label" for="carpeta-zip">Carpeta donde guardar el .zip en {equipo.nombre}</label>
            <div class="con-boton">
              <input id="carpeta-zip" class="input mono" bind:value={carpetaZip} placeholder={esWindows ? "D:\\Restauraciones" : "/srv/restauraciones"} autocomplete="off" spellcheck="false" />
              <button type="button" class="btn" onclick={() => (explorarPara = "zip")}><FolderOpen size={15} />Explorar…</button>
            </div>
            {#if carpetaZip.trim() && errorCarpetaRestaurar(carpetaZip, esWindows)}<span class="field-error">{errorCarpetaRestaurar(carpetaZip, esWindows)}</span>{/if}
          </div>
          <div class="field">
            <label class="field-label" for="nombre-zip">Nombre del .zip <span class="faint">(opcional)</span></label>
            <input id="nombre-zip" class="input" bind:value={nombreZip} placeholder={carpetaRestaurado} autocomplete="off" />
            <span class="field-hint">Si ya hay uno con ese nombre, se guarda como «… (2).zip». Nunca se sobrescribe.</span>
            {#if errorNombreZip(nombreZip)}<span class="field-error">{errorNombreZip(nombreZip)}</span>{/if}
          </div>
        {/if}
        {#if (destino === "zip" || destino === "descargar") && admiteCarpeta}
          <fieldset class="zip-clave">
            <legend>Contraseña del .zip <span class="faint">(opcional)</span></legend>
            <CampoClave id="zip-clave" etiqueta="Contraseña" bind:value={contrasenaZip} ayuda="AES-256. Se escribe aquí y viaja sellada para el equipo: el servidor no la ve." />
            {#if contrasenaZip}<CampoClave id="zip-clave-2" etiqueta="Repítela" bind:value={contrasenaZip2} />{/if}
            {#if contrasenaZip}
              <div class="notice notice-info"><p>El Explorador de Windows no abre un .zip con AES: hace falta 7-Zip o un programa parecido. Guarda la contraseña: sin ella no se puede abrir.</p></div>
            {/if}
            {#if errorContrasenaZip(contrasenaZip, contrasenaZip2) && contrasenaZip2}<span class="field-error">{errorContrasenaZip(contrasenaZip, contrasenaZip2)}</span>{/if}
          </fieldset>
        {/if}
        {#if destino === "descargar" && totalElegido > MAX_RELEVO}
          <div class="notice notice-warn sugerencia">
            <p>Son más de {bytes(MAX_RELEVO)}: por el navegador no cabe. Guárdalo como .zip en {equipo.nombre}.</p>
            {#if admiteCarpeta}<button class="btn btn-sm" onclick={() => (destino = "zip")}><FileArchive size={14} />Guardar como .zip en el equipo</button>{/if}
          </div>
        {/if}
        {#if pideClaveAdmin}
          <CampoClave requerido id="clave-admin-destino" etiqueta="Clave de administración" bind:value={claveAdminDestino} ayuda="Restaurar en otra carpeta o en un .zip escribe donde elijas en el equipo: lo confirma la clave de administración.">
            {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
          </CampoClave>
        {/if}
        <fieldset class="motivo">
          <legend>¿Por qué restauras? <span class="faint">{motivoObligatorio ? "(obligatorio en este cliente)" : "(opcional)"}</span></legend>
          <div class="chips-motivo" role="radiogroup" aria-label="Motivo">
            {#each MOTIVOS as m (m.id)}
              <label class="chip-motivo" class:on={motivo === m.id}><input type="radio" name="motivo" value={m.id} bind:group={motivo} />{m.texto}</label>
            {/each}
            {#if motivo && !motivoObligatorio}<button type="button" class="link" onclick={() => (motivo = "")}>Quitar</button>{/if}
          </div>
          {#if motivo}<input class="input" bind:value={notasMotivo} maxlength={500} placeholder="Notas (opcional): qué pasó, quién lo pidió… Sin contraseñas." aria-label="Notas del motivo" />{/if}
        </fieldset>
        <p class="faint pequeno">¿Lo necesitas en otro equipo? <button class="link" onclick={() => (enOtro = { equipo: equipoId, repo: repoId })}>Restaurar en otro equipo</button></p>
        {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
        <div class="fin">
          {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{:else if falta}<span class="espera faint" role="status">{falta}</span>{/if}
          <button class="btn btn-ghost" onclick={() => (paso = "archivos")} disabled={ocupado}><ArrowLeft size={15} />Atrás</button>
          <button class="btn {reemplazar && destino === 'original' ? 'btn-danger' : 'btn-primary'}" disabled={ocupado || !!falta} onclick={restaurar}>
            {#if destino === "descargar"}<Download size={15} />Descargar{:else if destino === "zip"}<FileArchive size={15} />Guardar el .zip con{:else}<History size={15} />Restaurar{/if} {plural(elegidas.size, "elemento", "elementos")}
          </button>
        </div>
      </section>
    {:else if paso === "progreso"}
      <section class="card p estrecha final" aria-live="polite">
        {#if destino === "descargar"}
          {#if terminado}
            <Ilustracion nombre="restaurado" ancho={150} />
            <h2 class="section-title">Descarga terminada</h2>
            <p class="faint">Está en la carpeta de descargas de este navegador. Ya se ha borrado del servidor.</p>
            {#if enviado?.cifrado}<p class="faint pequeno">Es un .zip con contraseña (AES-256): ábrelo con 7-Zip o un programa parecido; el Explorador de Windows no lo abre.</p>{/if}
          {:else}
            <span class="gran-ic info"><Download size={26} /></span>
            <h2 class="section-title">{progreso?.fase === "bajando" ? "Descargando y descifrando…" : progreso?.fase === "subiendo" ? `${equipo?.nombre} está preparando los archivos…` : "Esperando al equipo…"}</h2>
            <div class="progress live barra"><div style:width="{progreso?.fase === 'bajando' ? Math.round((progreso.bajados / Math.max(progreso.bytes, 1)) * 100) : progreso?.trozos ? 35 : 8}%"></div></div>
            <p class="faint num">{progreso ? `${bytes(progreso.fase === "bajando" ? progreso.bajados : progreso.bytes)}${progreso.fase === "bajando" ? ` de ${bytes(progreso.bytes)}` : ""}` : "No cierres esta pestaña hasta que termine."}</p>
          {/if}
        {:else if orden}
          {#if terminado && orden.estado === "hecha"}
            <Ilustracion nombre="restaurado" ancho={150} />
            <h2 class="section-title">Listo: {plural(elegidas.size, "elemento restaurado", "elementos restaurados")} en {equipo?.nombre}</h2>
          {:else if terminado}
            <span class="gran-ic mal"><TriangleAlert size={28} /></span>
            <h2 class="section-title">No se pudo restaurar</h2>
          {:else}
            <span class="gran-ic info"><LoaderCircle size={28} class="spin" /></span>
            <h2 class="section-title">Restaurando en {equipo?.nombre}…</h2>
          {/if}
          {#if !terminado && enVivo && equipo}
            <!-- 13c: el detalle en vivo, con el estilo de las copias (y en todas las consolas del equipo). -->
            <div class="en-vivo"><ProgresoCopia tarea={enVivo} equipo={equipo.id} titulo={repo?.nombre} /></div>
          {/if}
          <ol class="etapas" aria-label="Cómo va">
            {#each ETAPAS as et, j (et.id)}
              {@const hecha = etapa > j || (terminado && orden.estado === "hecha")}
              {@const enCurso = etapa === j && !terminado}
              <li class:hecha class:ahora={enCurso}>
                <span class="e-ic">{#if hecha}<Check size={13} strokeWidth={3} />{:else if enCurso}<LoaderCircle size={13} class="spin" />{/if}</span>{et.texto}
              </li>
            {/each}
          </ol>
          <!-- Sin «[ruta]» (el equipo quita las rutas): la carpeta, si se sabe, va en el recuadro de abajo. -->
          <p>{orden.mensaje ? textoRestaurar(orden.mensaje, terminado && orden.estado === "hecha" && destino === "junto" && !!carpetaJunto) : (equipo?.conectado ? "Puedes cerrar esta pantalla: el equipo sigue y lo verás en sus órdenes." : `${equipo?.nombre} lo hará en cuanto se conecte.`)}</p>
          {#if orden.firma_agente && equipo}<p class="faint pequeno firma"><ShieldCheck size={13} />{resultadoFirmado(equipo.sign_pub, orden) ? "Respuesta firmada por el equipo." : "La firma de la respuesta no es válida."}</p>{/if}
          {#if terminado && orden.estado === "hecha"}
            <div class="donde-quedo">
              <span class="tile-ic"><FolderOpen size={16} /></span>
              <div>
                {#if destino === "junto" && carpetaJunto}
                  <strong>Abre la carpeta en {equipo?.nombre}</strong>
                  <p>En el Explorador de archivos, pega esta ruta en la barra de direcciones (la hora del nombre es la del equipo, puede variar un minuto):</p>
                  <Copiable texto={carpetaJunto} que="la ruta" />
                {:else if destino === "junto"}
                  <strong>Junto al original</strong>
                  <p>Al lado de cada elemento hay una carpeta «Restaurado …» con la fecha de hoy.</p>
                {:else if destino === "nueva" && enviado}
                  <strong>En la carpeta que elegiste</strong>
                  <p>Ábrela en {equipo?.nombre} con el Explorador de archivos:</p>
                  <Copiable texto={enviado.carpeta} que="la ruta" />
                {:else if destino === "zip" && enviado}
                  <strong>El .zip está en {equipo?.nombre}</strong>
                  <p>Si ya había uno con ese nombre, este se llama «… (2).zip».{enviado.cifrado ? " Tiene contraseña (AES-256): ábrelo con 7-Zip o un programa parecido." : ""}</p>
                  <Copiable texto={rutaZip} que="la ruta" />
                {:else}
                  <strong>En su sitio de siempre</strong>
                  <p>{reemplazar ? "Los archivos se han reemplazado por los de la versión elegida." : "Lo que ya existía se ha dejado como estaba; lo que faltaba ha vuelto a su carpeta."}</p>
                {/if}
              </div>
            </div>
          {/if}
          {#if terminado && orden.estado !== "hecha" && destino === "original" && /junto/i.test(orden.mensaje ?? "")}
            <!-- v1.10: «en su sitio» solo dentro de las carpetas que copia el equipo; el agente sugiere «junto». -->
            <div class="notice notice-info sugerencia">
              <p>«En su sitio» solo vale para archivos de las carpetas que copia {equipo?.nombre}. Puedes restaurarlos junto al original, sin tocar nada de lo que hay.</p>
              <button class="btn btn-primary btn-sm" disabled={ocupado} onclick={restaurarJunto}>Restaurar junto al original</button>
            </div>
          {/if}
        {/if}
        {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
        <div class="fin centro-fin">
          <button
            class="btn"
            onclick={() => {
              paso = "archivos";
              terminado = false;
              orden = null;
              progreso = null;
              error = "";
            }}>Restaurar más de esta versión</button
          >
          <a class="btn" href="/c/{actual.id}/restauraciones?equipo={encodeURIComponent(equipoId)}">Ver en Restauraciones</a>
          <a class="btn btn-primary" href="/c/{actual.id}/equipos/{equipoId}?tab=ordenes">Ver en el equipo</a>
        </div>
      </section>
    {/if}
  {/if}
</div>

{#snippet nivel(ruta: string)}
  {@const h = hijos[ruta]}
  {#if h === "cargando"}
    <div class="cargando"><LoaderCircle size={14} class="spin" />Abriendo la carpeta…</div>
  {:else if !h && fallosNivel[ruta]}
    <div class="notice notice-danger fallo-arbol" role="alert">
      <TriangleAlert size={16} />
      <p>{ruta === "/" ? "No se pudo abrir la versión" : "No se pudo abrir la carpeta"}: {fallosNivel[ruta]} <button type="button" class="notice-action" onclick={() => cargar(ruta)}>Reintentar</button></p>
    </div>
  {:else if h && !h.length}
    <div class="cargando">Carpeta vacía</div>
  {:else if h}
    <ul role="group">
      {#each h as e (e.nombre)}
        {@const r = unir(ruta, e.nombre)}
        <li role="treeitem" aria-selected={elegidas.has(r)} aria-expanded={e.tipo === "dir" ? !!abiertas[r] : undefined}>
          <div class="nodo" class:marcado={elegidas.has(r)}>
            {#if e.tipo === "dir"}
              <button
                type="button"
                class="icon-btn flecha"
                class:abierta={abiertas[r]}
                aria-label={abiertas[r] ? `Plegar ${e.nombre}` : `Abrir ${e.nombre}`}
                onclick={() => {
                  abiertas[r] = !abiertas[r];
                  if (abiertas[r]) void cargar(r);
                }}><ChevronRight size={14} /></button
              >
            {:else}<span class="hueco"></span>{/if}
            <input type="checkbox" checked={elegidas.has(r)} aria-label="Restaurar {r}" onchange={(ev) => marcar(r, e, ev.currentTarget.checked)} />
            {#if e.tipo === "dir"}<Folder size={15} class="ic-dir" />{:else}<File size={15} />{/if}
            <span class="nombre">{ruta === "/" && e.tipo === "dir" && /^[A-Za-z]$/.test(e.nombre) && esWindows ? `Disco ${e.nombre.toUpperCase()}:` : e.nombre}</span>
            {#if e.tipo === "archivo"}
              {#if e.modificado}<span class="faint pequeno meta fecha num">{fechaCorta(e.modificado)}</span>{/if}
              <span class="faint pequeno meta num">{bytes(e.bytes)}</span>
            {/if}
          </div>
          {#if abiertas[r]}{@render nivel(r)}{/if}
        </li>
      {/each}
    </ul>
  {/if}
{/snippet}

{#if explorarPara && equipo && actual.cliente}
  <ElegirCarpetas
    cliente={actual.cliente}
    {equipo}
    unica
    claveAdmin={claveAdminDestino || undefined}
    titulo={explorarPara === "zip" ? "¿Dónde guardar el .zip?" : "¿En qué carpeta restaurar?"}
    iniciales={[explorarPara === "zip" ? carpetaZip : carpetaNueva].filter(Boolean)}
    validar={(r) => errorCarpetaRestaurar(r, esWindows)}
    onclose={() => (explorarPara = null)}
    alElegir={(rutas) => {
      if (explorarPara === "zip") carpetaZip = rutas[0] ?? carpetaZip;
      else carpetaNueva = rutas[0] ?? carpetaNueva;
      explorarPara = null;
    }}
  />
{/if}

{#if desdeEspejo && actual.cliente}
  <RestaurarDesdeEspejo
    cliente={actual.cliente}
    almacen={desdeEspejo.almacen}
    equipos={actual.equipos}
    destinoInicial={desdeEspejo.clave}
    repoInicial={desdeEspejo.repo}
    nombre={(r) => (r === nombreEspejo && repo ? repo.nombre : r)}
    onclose={() => (desdeEspejo = null)}
  />
{/if}

{#if enOtro !== undefined && actual.cliente}
  <RestaurarEnOtro cliente={actual.cliente} equipos={actual.equipos} origenInicial={enOtro ?? undefined} onclose={() => (enOtro = undefined)} />
{/if}

<style>
  .origen input[type="radio"] {
    flex: none;
  }
  .origen.on {
    background: var(--accent-soft);
  }
  .apagado {
    opacity: 0.6;
  }
  .con-boton {
    display: flex;
    gap: 8px;
  }
  .con-boton .input {
    flex: 1;
    min-width: 0;
  }
  .mono {
    font-family: var(--mono);
  }
  .zip-clave,
  .motivo {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    margin: 0;
    padding: 0;
    border: 0;
  }
  .zip-clave legend,
  .motivo legend {
    margin-bottom: var(--sp-2);
    font-size: var(--fs-sm);
    font-weight: 600;
  }
  .chips-motivo {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .chip-motivo {
    display: inline-flex;
    align-items: center;
    padding: 4px 10px;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  .chip-motivo input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .chip-motivo.on {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .chip-motivo:focus-within {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .en-vivo {
    width: 100%;
    text-align: left;
  }
  .field-error {
    font-size: var(--fs-xs);
    color: var(--bad);
  }
  .paso-card {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    animation: rise var(--dur-slow) var(--ease-out) both;
  }
  .paso-cab h2 {
    margin: 0;
    font-size: var(--fs-h2);
    line-height: var(--lh-h2);
    font-weight: 600;
  }
  .paso-cab p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .equipos-r {
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 280px), 1fr));
  }
  .elegible {
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .elegible:hover,
  .elegible:focus-visible {
    border-color: var(--border-strong);
    box-shadow: var(--shadow-sm);
  }
  .elegible-fila {
    gap: var(--sp-3);
  }
  .elegible-fila :global(> svg) {
    flex: none;
    color: var(--text-3);
  }
  .bloque {
    display: block;
  }
  .sugerencia {
    flex-direction: column;
    align-items: center;
    gap: var(--sp-3);
    text-align: center;
  }
  .ejemplo {
    display: block;
    margin-top: 4px;
    font-size: var(--fs-xs);
    color: var(--text-2);
    word-break: break-all;
  }
  .estrecha {
    width: 100%;
    max-width: 660px;
    animation: rise var(--dur-slow) var(--ease-out) both;
  }
  .fin {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    align-items: center;
    gap: 8px;
  }
  .fin.izq {
    justify-content: flex-start;
  }
  .centro-fin {
    justify-content: center;
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }

  /* Archivos. */
  .archivos {
    animation: rise var(--dur-slow) var(--ease-out) both;
  }
  .cab-paso {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: var(--sp-4) var(--sp-5);
    border-bottom: 1px solid var(--border);
  }
  .cab-paso > :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .cab-texto {
    display: flex;
    flex: 1;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 10px;
    min-width: 0;
  }
  .cab-texto .faint {
    font-size: var(--fs-sm);
  }
  .fallo-arbol {
    margin: var(--sp-2) var(--sp-4);
  }
  .buscar {
    position: relative;
    display: flex;
    gap: 8px;
    padding: var(--sp-3) var(--sp-5) 0;
  }
  .buscar > :global(svg) {
    position: absolute;
    top: 22px;
    left: 31px;
    color: var(--text-3);
  }
  .buscar .input {
    flex: 1;
    padding-left: 34px;
  }
  .buscar .btn {
    align-self: center;
  }
  .arbol,
  .resultados {
    max-height: 50vh;
    overflow: auto;
    padding: var(--sp-3) var(--sp-4);
  }
  .res-nombre {
    display: flex;
    flex-direction: column;
    line-height: 1.25;
    padding: 3px 0;
  }
  .res-nombre span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .res-cab {
    margin: 0 0 6px 6px;
  }
  ul {
    margin: 0;
    padding: 0 0 0 18px;
    list-style: none;
  }
  .arbol > ul {
    padding-left: 0;
  }
  .nodo {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 32px;
    padding-right: 6px;
    border-radius: var(--radius-sm);
    transition: background var(--dur-fast) var(--ease);
  }
  .nodo:hover {
    background: var(--surface-2);
  }
  .nodo.marcado {
    background: var(--accent-soft);
  }
  .nodo :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .nodo :global(.ic-dir) {
    color: var(--accent-text);
  }
  .hueco {
    width: 28px;
    flex: none;
  }
  .flecha :global(svg) {
    transition: transform var(--dur) var(--ease);
  }
  .flecha.abierta :global(svg) {
    transform: rotate(90deg);
  }
  .nombre {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    flex: none;
    min-width: 56px;
    text-align: right;
  }
  .meta.fecha {
    min-width: 96px;
  }
  .pequeno {
    font-size: var(--fs-xs);
  }
  .cargando {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 28px;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .pie-paso {
    position: sticky;
    bottom: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-5);
    font-size: var(--fs-sm);
    background: var(--surface);
    border-top: 1px solid var(--border);
  }
  .elegidas {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    min-width: 0;
  }
  .chips-elegidas {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    min-width: 0;
  }
  .chip-elegida {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    max-width: 200px;
    height: 24px;
    padding: 0 2px 0 8px;
    font-size: var(--fs-xs);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
  }
  .chip-elegida :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .ce-nombre {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chip-elegida button {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    padding: 0;
    color: var(--text-3);
    background: none;
    border: none;
    border-radius: 999px;
    cursor: pointer;
  }
  .chip-elegida button:hover {
    color: var(--text-1);
    background: var(--surface-3);
  }

  /* Dónde. */
  .resumen-rest {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3);
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .resumen-rest p {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .resumen-rest strong {
    color: var(--text-1);
    font-weight: 600;
  }
  .opciones {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .opcion {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    transition:
      border-color var(--dur-fast) var(--ease),
      box-shadow var(--dur-fast) var(--ease);
  }
  .opcion:hover {
    border-color: var(--border-strong);
  }
  .opcion.on {
    border-color: var(--accent);
    box-shadow: var(--focus);
  }
  .opcion :global(svg) {
    flex: none;
    margin-top: 1px;
    color: var(--text-2);
  }
  .opcion > span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .opcion strong .badge {
    margin-left: 4px;
    vertical-align: 1px;
  }
  .opcion .faint,
  .switch-row .faint {
    display: block;
    font-size: var(--fs-sm);
  }

  /* Progreso y final. */
  .final {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-3);
    text-align: center;
  }
  .final p {
    margin: 0;
  }
  .gran-ic {
    display: grid;
    place-items: center;
    width: 56px;
    height: 56px;
    border-radius: 999px;
  }
  .gran-ic.info {
    color: var(--info);
    background: var(--info-soft);
  }
  .gran-ic.mal {
    color: var(--bad);
    background: var(--bad-soft);
  }
  .etapas {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 6px 16px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .etapas li {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .e-ic {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border: 1.5px solid var(--border-strong);
    border-radius: 999px;
  }
  .etapas .hecha {
    color: var(--text-2);
  }
  .etapas .hecha .e-ic {
    color: var(--accent-contrast);
    background: var(--accent);
    border-color: var(--accent);
  }
  .etapas .ahora {
    color: var(--text-1);
    font-weight: 550;
  }
  .etapas .ahora .e-ic {
    color: var(--info);
    border-color: transparent;
  }
  .firma {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .donde-quedo {
    display: flex;
    gap: var(--sp-3);
    width: 100%;
    padding: var(--sp-4);
    text-align: left;
    background: var(--surface-2);
    border-radius: var(--radius-lg);
  }
  .donde-quedo > div {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .donde-quedo p {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .donde-quedo :global(code) {
    word-break: break-all;
  }
  .barra {
    width: 100%;
    max-width: 380px;
  }
  @media (max-width: 640px) {
    .meta.fecha {
      display: none;
    }
    .pie-paso {
      flex-wrap: wrap;
    }
    .pie-paso .btn {
      width: 100%;
    }
  }
</style>
