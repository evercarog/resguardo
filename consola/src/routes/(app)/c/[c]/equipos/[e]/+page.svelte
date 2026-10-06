<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Ficha de un equipo: estado, copias, repositorios, órdenes, informes y detalles.
  import Copiable from "$lib/componentes/Copiable.svelte";
  import Migas from "$lib/componentes/Migas.svelte";
  import { bytesRepo, destinoDe, dias, diasCopia, informeDe, nVersiones, proteccion, TEXTO_RESULTADO, TONO_RESULTADO, ultimaEjecucion, ultimaVersion } from "$lib/repo";
  import AnilloProteccion from "$lib/componentes/repo/AnilloProteccion.svelte";
  import DiasCuadros from "$lib/componentes/repo/DiasCuadros.svelte";
  import { ultimas24h } from "$lib/panel";
  import HistorialVersiones from "$lib/componentes/repo/HistorialVersiones.svelte";
  import PanelDetalle from "$lib/componentes/detalle/PanelDetalle.svelte";
  import { abrirVersion, abrirVuelta, elegirDia, elegirFechas } from "$lib/componentes/detalle/navegar";
  import { leerSeleccion } from "$lib/detalle";
  import { usarHistorialEquipo } from "$lib/historialEquipo.svelte";
  import { reglaEfectiva } from "$lib/lineaTiempo";
  import { untrack } from "svelte";
  import { seguirCambios, tocaEquipo } from "$lib/vivo.svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import {
    BellRing,
    Check,
    Trash2,
    Plus,
    Cloud,
    CirclePause,
    CirclePlay,
    Copy,
    ChevronRight,
    Clock,
    CloudUpload,
    Database,
    FolderOpen,
    FolderSync,
    HardDrive,
    History,
    Info,
    Laptop,
    Monitor,
    Pencil,
    Play,
    RefreshCw,
    Server,
    ShieldCheck,
    Tag,
    Unlink,
    X,
  } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { enFondo } from "$lib/actividad.svelte";
  import { actual, cargarCliente, puede, reloj } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { bytes, cuandoFrase, fechaLarga, horarioEnFrase, numero, plural, relativo } from "$lib/formato";
  import { enPausa, nombreOrden, proximaCopia, resultadoConError, saludEquipo, type Tono } from "$lib/salud";
  import { huellaCorta } from "$lib/servidores";
  import { hostDe } from "$lib/conexion";
  import { claveEspejo } from "$lib/cripto/ordenes";
  import { admiteEspejoFlexible, conRepos, cuandoEspejo, textoVerificacion, textoRetencion, errorDiasRetencion, diaLegible, RETENCION_ESPEJO, destinoParaOrden, horaParaConsolasAnteriores, horarioDiario, nombresRepos, nuevosEn, textoRepos, type DestinoEspejoOrden, type DestinoEspejoResumen } from "$lib/espejo";
  import { errorReglas, reglasDe } from "$lib/horario";
  import EspejoOpciones from "$lib/componentes/EspejoOpciones.svelte";
  import { errorCarpetaDestino, errorCarpetaEspejo } from "$lib/ganchos";
  import { NOMBRE_GANCHO } from "$lib/ganchos";
  import { estadoCopia, proximaDe, ultimaVuelta } from "$lib/copia";
  import type { Equipo, EquipoDetalle, Horario, Informe, Orden, Regla, RepositorioResumen, RetencionAlmacen } from "$lib/tipos";
  import { admiteAlmacenPropio, admitePlazos, almacenDe, copiaRegla, errorRegla, esDeAlmacen, horarioDeCopias, REGLA_POR_DEFECTO, reglaDe, reglaParaOrden, repoDeRetencion, TEXTO_ALMACEN_PROPIO, textoHorario } from "$lib/retencion";
  import EditorRetencion from "$lib/componentes/EditorRetencion.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import BotonCargando from "$lib/componentes/BotonCargando.svelte";
  import EtiquetaChip from "$lib/componentes/EtiquetaChip.svelte";
  import EditorEtiquetas from "$lib/componentes/EditorEtiquetas.svelte";
  import { filtroEtiqueta } from "$lib/etiquetas.svelte";
  import PendienteItem from "$lib/componentes/PendienteItem.svelte";
  import { pendientesDe, terminada } from "$lib/pendientes.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import EstadoEquipo from "$lib/componentes/detalle/EstadoEquipo.svelte";
  import ListaOrdenes from "$lib/componentes/ListaOrdenes.svelte";
  import OrdenDialog from "$lib/componentes/OrdenDialog.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import EnMarcha from "$lib/componentes/EnMarcha.svelte";
  import CopiarEnAlmacen from "$lib/componentes/CopiarEnAlmacen.svelte";
  import MapaProteccion from "$lib/componentes/mapa/MapaProteccion.svelte";
  import { cargarInformes as cargarUltimos, ultimos } from "$lib/informes.svelte";
  import ElegirCarpetas from "$lib/componentes/ElegirCarpetas.svelte";
  import ConectarNube from "$lib/componentes/ConectarNube.svelte";
  import MenuAcciones, { type AccionMenu } from "$lib/componentes/MenuAcciones.svelte";
  import AlertaLlaves from "$lib/componentes/AlertaLlaves.svelte";
  import CampoClave from "$lib/componentes/CampoClave.svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { comprobarLlaves, fijadaEl, type EstadoLlaves } from "$lib/fijadas";
  import { ErrorEtiqueta, kcfgComprobada } from "$lib/ordenar";
  // v1.41: dónde se guarda cada copia y repositorio, el aviso «copias en el mismo equipo» y «Mover a otro sitio…».
  import { comprobacionLugar, lugarRepo, riesgoMismoEquipo } from "$lib/dondeGuarda";
  import SeGuardaEn from "$lib/componentes/SeGuardaEn.svelte";
  import AvisoMismoEquipo from "$lib/componentes/AvisoMismoEquipo.svelte";
  import MoverRepositorio from "$lib/componentes/MoverRepositorio.svelte";
  import FormRepoExistente from "$lib/componentes/FormRepoExistente.svelte";
  import { admiteExternaExistente, cuerpoBloqueo, cuerpoExistente, detallesExterna, diasBloqueo, errorBloqueo, existenteCompleto, externaExtraVacia, MAX_BLOQUEO, textoRetencionDestino } from "$lib/copiaExterna";
  // v1.47: un «Mover a otro sitio…» en marcha (también si lo empezó otra consola).
  import MoviendoseAviso from "$lib/componentes/MoviendoseAviso.svelte";
  import { tareasDe } from "$lib/progreso.svelte";
  import { hayPlanMover, moviendoDe } from "$lib/mover";
  import { borrar } from "$lib/cripto/bytes";
  import Observaciones from "$lib/componentes/notas/Observaciones.svelte";
  import Comentarios from "$lib/componentes/notas/Comentarios.svelte";
  import ContadorNotas from "$lib/componentes/notas/ContadorNotas.svelte";
  import NotasDialogo from "$lib/componentes/notas/NotasDialogo.svelte";
  import { objetoDe } from "$lib/notas.svelte";

  const c = $derived(page.params.c ?? "");
  // Los informes de los demás equipos (para el camino de las copias de un almacén).
  $effect(() => {
    const [cc, ids] = [c, actual.equipos.map((x) => x.id)];
    untrack(() => void cargarUltimos(cc, ids));
  });
  const id = $derived(page.params.e ?? "");
  const tab = $derived(page.url.searchParams.get("tab") ?? "resumen");

  let equipo = $state<EquipoDetalle | null>(null);
  let ordenes = $state<Orden[]>([]);
  let informes = $state<Informe[]>([]);
  let error = $state("");
  let renombrando = $state(false);
  /** Las notas de un destino (no tiene página propia). */
  let notasDestino = $state<{ id: string; nombre: string } | null>(null);
  let editarEtiquetas = $state(false);
  let nuevoNombre = $state("");

  let llaves = $state<EstadoLlaves | null>(null);
  let fijadaFecha = $state<string | null>(null);
  let comprobar = $state(false);
  let claveComprobar = $state("");
  let errorComprobar = $state("");
  let comprobando = $state(false);

  async function cargar() {
    try {
      const [e, o] = await Promise.all([api.equipo(c, id), api.ordenesEquipo(c, id, 30)]);
      equipo = e;
      ordenes = o;
      llaves = await comprobarLlaves(c, e);
      fijadaFecha = await fijadaEl(c, e.id);
      error = "";
    } catch (e) {
      error = (e as Error).message;
    }
  }
  async function cargarInformes() {
    try {
      informes = await api.informes(c, id, 20);
    } catch (e) {
      fallo(e);
    }
  }
  $effect(() => {
    void id;
    void c;
    equipo = null;
    untrack(() => void cargar());
  });
  $effect(() => {
    if (tab === "informes" && id) untrack(() => void cargarInformes());
  });
  // Al día sin recargar: con el canal en vivo, cuando cambia algo de este equipo; sin él, cada 8 s.
  $effect(() => {
    const eq = id;
    return untrack(() => {
      const dejar = seguirCambios(() => enFondo(cargar), { ms: 8000, toca: (x) => x.t !== "historial" && (x.t !== "progreso" || x.estado === "termina") && tocaEquipo(x, eq) });
      const dejarInformes = seguirCambios(() => tab === "informes" && cargarInformes(), { ms: 0, toca: (x) => x.t === "informe" && tocaEquipo(x, eq) });
      return () => {
        dejar();
        dejarInformes();
      };
    });
  });

  const salud = $derived(equipo ? saludEquipo(equipo, reloj.ahora) : null);
  const copias = $derived(equipo?.resumen?.copias ?? []);
  const repos = $derived(equipo?.resumen?.repositorios ?? []);
  const destinos = $derived(equipo?.resumen?.destinos ?? []);
  const pausado = $derived(!!equipo && enPausa(equipo, reloj.ahora));
  // Cifras del resumen: última vuelta y próxima de todas sus copias, lo protegido y los 60 días.
  const ultimaDeTodas = $derived(
    [...copias.map((k) => k.ultima?.cuando), ...(equipo?.ultimo_informe?.datos.copias ?? []).map((k) => k.cuando)]
      .filter((x): x is string => !!x)
      .sort()
      .at(-1),
  );
  const proximaDeTodas = $derived(pausado ? null : proximaCopia(copias, reloj.ahora));
  const protegidoEquipo = $derived(repos.reduce((n, r) => n + (bytesRepo(r, informeDe(equipo?.ultimo_informe, r.id)) ?? 0), 0));
  // «Historial y versiones» de todas sus copias y repositorios (el mismo de la página de cada uno).
  const historia = usarHistorialEquipo(() => c, () => id);
  const sel = $derived(leerSeleccion(page.url.searchParams));
  const fuentesHistorial = $derived(
    repos.map((r) => {
      const ret = reglaEfectiva(r, destinoDe(destinos, r), actual.equipos);
      return { repo: r, inf: informeDe(equipo?.ultimo_informe, r.id), regla: ret?.regla ?? null, quien: ret?.quien ?? null };
    }),
  );
  /** El repositorio de lo que se abre en el cajón (la versión o la vuelta de la URL). */
  let repoCajon = $state<string | null>(null);
  const repoDetalle = $derived.by(() => {
    if (!sel.version && !sel.vuelta) return null;
    const t = sel.vuelta ? Date.parse(sel.vuelta) : NaN;
    const de = (r: RepositorioResumen) => {
      const inf = informeDe(equipo?.ultimo_informe, r.id);
      return (!!sel.version && !!inf?.versiones.some((v) => v.id === sel.version)) || (!sel.version && !!inf?.ejecuciones.some((x) => Math.abs(Date.parse(x.hora) - t) < 60_000));
    };
    return repos.find((r) => r.id === repoCajon && de(r)) ?? repos.find(de) ?? null;
  });
  const recientesEquipo = $derived(equipo ? ultimas24h([equipo], { [equipo.id]: equipo.ultimo_informe ?? null }, reloj.ahora) : { versiones: 0, fallos: 0 });
  const rolCliente = $derived(actual.cliente?.rol);
  /** Un equipo trasladado a otro servidor ya no recibe órdenes de este: se ve, pero no se toca. */
  const trasladado = $derived(equipo?.modo === "trasladado");
  const rol = $derived(trasladado && rolCliente ? "lectura" : rolCliente);
  // v1.36: las otras consolas que gestionan este equipo y el último cambio, si vino de una de ellas.
  const otrasConsolas = $derived((equipo?.resumen?.consolas ?? []).filter((x) => !x.esta));
  const cambioDeOtra = $derived.by(() => {
    const c = equipo?.resumen?.cambio_config;
    const esta = equipo?.resumen?.consolas?.find((x) => x.esta);
    return c && esta && c.consola.identidad !== esta.identidad ? c : null;
  });
  const nombreConsola = (x: { nombre: string; url: string }) => (x.nombre && x.nombre !== hostDe(x.url) ? `${x.nombre} (${hostDe(x.url)})` : hostDe(x.url));
  const Icono = $derived(equipo?.rol === "almacenamiento" ? Server : /portatil|laptop/i.test(equipo?.nombre ?? "") ? Laptop : /linux|debian|ubuntu/i.test(equipo?.so ?? "") ? HardDrive : Monitor);
  // Órdenes en camino de este equipo, en la sección donde se verá su resultado.
  const pendCopias = $derived(pendientesDe("copias", id));
  const pendRepos = $derived(pendientesDe("repositorios", id));
  const pendDestinos = $derived(pendientesDe("destinos", id));
  const pendGuarda = $derived(pendientesDe(["guarda", "espejo", "nubes"], id));

  // --- Órdenes -------------------------------------------------------------
  type Dialogo = {
    tipo: string;
    cuerpo: Record<string, unknown>;
    titulo?: string;
    descripcion: string;
    repo?: { id: string; nombre: string };
    accion?: string;
    campos?: "pausar" | "retencion" | "desvincular" | "repo" | "guardar" | "quitar" | "destino" | "externa" | "espejo";
  };
  let dialogo = $state<Dialogo | null>(null);
  /** Equipos del cliente que guardan copias (para «Copiar en …»). */
  const almacenes = $derived(actual.equipos.filter((x) => x.id !== equipo?.id && x.confirmado && x.modo !== "trasladado" && x.resumen?.guarda_copias?.activo));
  let copiarEn = $state<Equipo | null>(null);
  /** v1.28: un almacén puede tener un repositorio en sí mismo (para lo suyo, p. ej. las copias de la consola). */
  const propioPosible = $derived(!!equipo && admiteAlmacenPropio(equipo) && !destinos.some((d) => esDeAlmacen(d, equipo!)));
  function abrirGuardar() {
    abrir({
      tipo: "guarda_copias",
      // v1.19: el agente dice un puerto libre (si no lo dice, 8000).
      cuerpo: { activo: true, carpeta: "", puerto: equipo?.resumen?.puerto_libre ?? 8000, solo_red_local: true },
      titulo: "Este equipo guarda copias",
      descripcion: "Convierte este equipo en un almacén de copias para los demás equipos de su red (rest-server en modo solo añadir, con TLS propio y su regla del cortafuegos). La consola (Resguardo Server) no guarda copias: solo coordina.",
      campos: "guardar",
    });
  }
  const abrir = (d: Dialogo) => (dialogo = { ...d, cuerpo: d.cuerpo });
  /** La regla que se edita en «Cambiar la retención» (EditorRetencion). */
  let reg = $state(copiaRegla(REGLA_POR_DEFECTO));
  // Desde «Vincular este servidor» (?guardar=1): se abre «Este equipo guarda copias» una vez.
  let guardarPedido = false;
  $effect(() => {
    if (guardarPedido || page.url.searchParams.get("guardar") !== "1" || !equipo || equipo.resumen?.guarda_copias?.activo || !puede.administrar(rol)) return;
    guardarPedido = true;
    abrirGuardar();
  });
  // v1.41: desde el aviso «copias en el mismo equipo» (?externa=<repo>): «Copia externa» de ese repositorio, una vez.
  let externaPedida = false;
  $effect(() => {
    const r = page.url.searchParams.get("externa");
    const repo = r ? equipo?.resumen?.repositorios?.find((x) => x.id === r) : undefined;
    if (externaPedida || !repo || !puede.ordenar(rol)) return;
    externaPedida = true;
    untrack(() => abrirExterna(repo));
  });

  // --- Espejo del Servidor de copias (v1.9) ------------------------------
  // La orden lleva la lista ENTERA de destinos: al añadir uno se reenvían los
  // que ya hay (con sus opciones); quitar uno (o todo el espejo) es
  // destructiva y espera. Con `admite: "espejo_flexible"` (docs/espejo.md),
  // cada destino lleva su horario y sus opciones.
  type DestinoEspejoUI = DestinoEspejoOrden;
  const espejoActual = $derived(equipo?.resumen?.guarda_copias?.espejo ?? null);
  const nubes = $derived(equipo?.resumen?.guarda_copias?.nubes ?? []);
  const flexible = $derived(admiteEspejoFlexible(equipo));
  /** Destinos actuales, en la forma de la orden (sin sus resultados, con sus opciones). */
  const destinosActuales = $derived<DestinoEspejoUI[]>((espejoActual?.destinos ?? []).map(destinoParaOrden));
  // `limite`: el campo es numérico (bind:value da número, o "" si se vacía).
  let esp = $state({
    tipo: "carpeta" as "carpeta" | "nube",
    carpeta: "",
    nube: "",
    carpetaNube: "",
    hora: "02:00",
    limite: "" as number | string,
    horario: horarioDiario("02:00") as Horario,
    trasCopia: false,
    /** §3f: todos los repositorios o solo `elegidos`. */
    todos: true,
    elegidos: [] as string[],
    /** §3d: % que se comprueba cada día. */
    verificarPct: 5,
    /** §3b: qué hacer con lo que ya no está en el almacén. */
    borrar: "nunca" as "nunca" | "retencion" | "bloqueo",
    dias: RETENCION_ESPEJO.defecto as number,
    /** Clave (claveEspejo) del destino que se cambia; null: uno nuevo. */
    editando: null as string | null,
  });
  const limiteTxt = $derived(String(esp.limite ?? "").trim());
  const nuevoDestino = $derived.by<DestinoEspejoUI>(() => {
    const d: DestinoEspejoUI = esp.tipo === "nube" ? { tipo: "nube", nube: esp.nube, carpeta: esp.carpetaNube.trim() } : { tipo: "carpeta", carpeta: esp.carpeta.trim() };
    if (!flexible) return d;
    return { ...d, horario: esp.horario, ...(esp.trasCopia ? { tras_copia: true } : {}), ...(esp.todos ? {} : { repos: [...esp.elegidos].sort(), vistos: reposAlmacen }), verificar_pct: Number(esp.verificarPct), ...(esp.borrar === "bloqueo" ? { bloqueo: true } : esp.borrar === "retencion" ? { retencion_dias: Number(esp.dias) } : {}) };
  });
  /** §3f: los repositorios del almacén, como los nombra el espejo. */
  const reposAlmacen = $derived(nombresRepos(equipo?.resumen?.guarda_copias?.repositorios));
  /** «Contabilidad, de RECEPCION» para `usuario/repo` (si se sabe de qué equipo es). */
  function nombreRepoAlmacen(r: string): string {
    if (!equipo) return r;
    const [usuario, repo] = r.includes("/") ? r.split("/") : [r, "."];
    const de = repoDeRetencion({ usuario, repo } as RetencionAlmacen, equipo, actual.equipos);
    return de ? `${de.repo.nombre}, de ${de.equipo.nombre}` : r;
  }
  /** Repositorios nuevos que no entran en algún destino con selección (la consola pregunta). */
  const nuevosEspejo = $derived([...new Set((espejoActual?.destinos ?? []).flatMap((d) => nuevosEn(d, reposAlmacen)))]);
  const conSeleccion = $derived((espejoActual?.destinos ?? []).filter((d) => nuevosEn(d, reposAlmacen).length));
  /** El espejo con un cambio en cada destino (los demás, tal cual). */
  const espejoCon = (f: (d: DestinoEspejoUI) => DestinoEspejoUI) => {
    const destinos = destinosActuales.map(conHorario).map(f);
    return { destinos, hora: horaParaConsolasAnteriores(destinos, espejoActual?.hora ?? "02:00"), ...(espejoActual?.limite_kib ? { limite_kib: espejoActual.limite_kib } : {}) };
  };
  /** §3b: el freno saltó en un destino; confirmarlo (espera) hace que se anote y se borre pasados sus días. */
  function confirmarFreno(d: DestinoEspejoResumen) {
    const de = destinoParaOrden(d);
    abrir({
      tipo: "guarda_copias",
      cuerpo: { espejo_freno: { tipo: de.tipo, carpeta: de.carpeta, ...(de.nube ? { nube: de.nube } : {}) } },
      titulo: "Confirmar lo que falta en el almacén",
      descripcion: `Hazlo solo si sabes por qué falta (una poda grande o un repositorio que quitaste). En la próxima vuelta se anotará y se borrará de ${de.tipo === "nube" ? `«${de.nube}»` : de.carpeta} pasados ${d.retencion_dias ?? RETENCION_ESPEJO.defecto} días. Si no lo sabes, revisa antes el almacén: podría estar dañado.`,
    });
  }
  function preguntarNuevos(anadir: boolean) {
    const nombres = nuevosEspejo.map(nombreRepoAlmacen).join(", ");
    abrir({
      tipo: "guarda_copias",
      cuerpo: { espejo: espejoCon((d) => (d.repos ? (anadir ? conRepos(d, nuevosEn(d, reposAlmacen), reposAlmacen) : { ...d, vistos: reposAlmacen }) : d)) },
      titulo: anadir ? "Añadir los repositorios nuevos al espejo" : "Dejar fuera los repositorios nuevos",
      descripcion: anadir
        ? `${nombres} se copiarán también a los destinos del espejo que tienen una selección.`
        : `${nombres} no irán a los destinos del espejo con una selección (sí a los de «todos»). No se volverá a preguntar por ellos.`,
    });
  }
  const repetido = $derived(!esp.editando && destinosActuales.some((d) => claveEspejo(d) === claveEspejo(nuevoDestino)));
  /** Con un agente que lo admite, cada destino con su horario explícito (el de antes, `hora`, pasa a «cada día a esa hora»). */
  const conHorario = (d: DestinoEspejoUI): DestinoEspejoUI => (flexible && !d.horario ? { ...d, horario: horarioDiario(espejoActual?.hora ?? "02:00") } : d);
  const destinosEspejo = $derived.by(() => {
    const otros = destinosActuales.map(conHorario);
    if (!esp.editando) return [...otros, nuevoDestino];
    return otros.map((d) => (claveEspejo(d) === esp.editando ? nuevoDestino : d));
  });
  const cuerpoEspejo = $derived({
    espejo: {
      destinos: destinosEspejo,
      hora: flexible ? horaParaConsolasAnteriores(destinosEspejo, espejoActual?.hora ?? "02:00") : esp.hora,
      ...(limiteTxt ? { limite_kib: Math.max(1, Math.round(Number(limiteTxt))) } : espejoActual?.limite_kib ? { limite_kib: espejoActual.limite_kib } : {}),
    },
  });
  /** El espejo sin `quitar` (o `null` si era el último): con los demás destinos tal cual. */
  const espejoSin = (quitar: (d: DestinoEspejoUI) => boolean) => {
    const quedan = destinosActuales.filter((d) => !quitar(d)).map(conHorario);
    return quedan.length
      ? { destinos: quedan, hora: espejoActual?.hora ?? "02:00", ...(espejoActual?.limite_kib ? { limite_kib: espejoActual.limite_kib } : {}) }
      : null;
  };
  const win = $derived(/windows/i.test(equipo?.so ?? ""));
  const errorEspejo = $derived(esp.tipo === "carpeta" && esp.carpeta.trim() ? errorCarpetaEspejo(esp.carpeta, win) : null);
  const errorGuardar = $derived(dialogo?.campos === "guardar" && String(dialogo.cuerpo.carpeta ?? "").trim() ? errorCarpetaDestino(String(dialogo.cuerpo.carpeta), win) : null);
  /** Carpeta en la nube: relativa a la carpeta de la app (Aplicaciones/Resguardo ya es la raíz). */
  const errorCarpetaNube = $derived.by(() => {
    if (esp.tipo !== "nube") return null;
    const t = esp.carpetaNube.trim();
    if (!t) return null;
    if (/^[\/]/.test(t) || /^(aplicaciones|apps)[\/]/i.test(t)) return "Escríbela relativa a Aplicaciones/Resguardo: por ejemplo, «Sur» (sin «/» al principio ni «Aplicaciones/Resguardo/»).";
    if (t.length > 200 || t.includes(":") || t.split("/").some((x) => x === ".." || x === "." || x === "")) return "Usa nombres de carpeta separados por «/», sin «..», «.», «:» ni barras dobles o al final (hasta 200 caracteres).";
    if (/[\u0000-\u001f:*?"<>|]/.test(t)) return "La carpeta no puede llevar : * ? \" < > |.";
    return null;
  });
  const espejoValido = $derived(
    !errorEspejo &&
    !errorCarpetaNube &&
    (flexible ? !!(esp.horario.reglas?.length || esp.horario.horas?.length) && !errorReglas(reglasDe(esp.horario)) : /^([01]\d|2[0-3]):[0-5]\d$/.test(esp.hora)) &&
      !repetido &&
      (!flexible || esp.todos || esp.elegidos.length > 0) &&
      (!flexible || esp.borrar !== "retencion" || !errorDiasRetencion(Number(esp.dias))) &&
      (esp.tipo === "carpeta" ? !!esp.carpeta.trim() : !!esp.nube && !!esp.carpetaNube.trim()) &&
      (!limiteTxt || Number(limiteTxt) > 0),
  );

  /** Añadir un destino al espejo o, con `editar`, cambiar las opciones de uno que ya tiene. */
  function abrirEspejo(tipo: "carpeta" | "nube" = "carpeta", editar?: DestinoEspejoResumen) {
    const hora = espejoActual?.hora ?? "02:00";
    const de = editar ? conHorario(destinoParaOrden(editar)) : null;
    esp = {
      tipo: de?.tipo ?? tipo,
      carpeta: de?.tipo === "carpeta" ? de.carpeta : "",
      nube: de?.nube ?? nubes[0]?.nombre ?? "",
      // Con el permiso «App folder», la raíz ya es Aplicaciones/Resguardo.
      carpetaNube: de?.tipo === "nube" ? de.carpeta : (equipo?.nombre ?? "copias").replace(/[\\/:*?"<>|]+/g, "-"),
      hora,
      limite: "",
      horario: de?.horario ?? horarioDiario(hora),
      trasCopia: !!de?.tras_copia,
      todos: !Array.isArray(de?.repos),
      elegidos: de?.repos ?? [],
      verificarPct: de?.verificar_pct ?? (( de?.tipo ?? tipo) === "nube" ? 0 : 5),
      borrar: de?.bloqueo ? "bloqueo" : de?.retencion_dias ? "retencion" : "nunca",
      dias: de?.retencion_dias ?? RETENCION_ESPEJO.defecto,
      editando: de ? claveEspejo(de) : null,
    };
    abrir({
      tipo: "guarda_copias",
      cuerpo: {},
      titulo: de ? `Cambiar el espejo en ${de.tipo === "nube" ? `«${de.nube}»` : de.carpeta}` : espejoActual ? "Añadir destino del espejo" : "Espejo de lo que guarda",
      descripcion: de
        ? "Cuándo y qué se copia a este destino. Lo que ya está allí no cambia."
        : flexible
          ? "Lo que guarda este equipo se copia a otro sitio: otra carpeta (mejor en otro disco) o una nube conectada en el equipo, cuando tú elijas. Solo añade: nunca borra allí, y deja fuera lo que se esté escribiendo."
          : "Cada noche se copia todo lo que guarda este equipo a otro sitio: otra carpeta (mejor en otro disco) o una nube conectada en el equipo. Solo añade: nunca borra allí, y deja fuera lo que se esté escribiendo.",
      campos: "espejo",
    });
  }
  /** Quitar un destino: se reenvía la lista sin él (o `espejo: null` si era el último). */
  function quitarDestinoEspejo(d: DestinoEspejoUI) {
    const nombre = d.tipo === "nube" ? `«${d.nube}» (${d.carpeta})` : `la carpeta ${d.carpeta}`;
    abrir({
      tipo: "guarda_copias",
      cuerpo: { espejo: espejoSin((x) => claveEspejo(x) === claveEspejo(d)) },
      titulo: "Quitar un destino del espejo",
      descripcion: `Dejará de copiarse a ${nombre}. Lo que ya está allí se queda.`,
    });
  }
  let conectarNube = $state(false);
  /** Desconectar una nube: espera si el espejo la usa (lo decide esDestructiva con el contexto). */
  function quitarNube(nombre: string) {
    const usada = destinosActuales.some((d) => d.tipo === "nube" && d.nube === nombre);
    abrir({
      tipo: "quitar_nube",
      cuerpo: { nombre },
      titulo: `Desconectar «${nombre}»`,
      descripcion: usada
        ? `El espejo dejará de subir a «${nombre}» y el equipo olvidará su permiso. Lo ya subido se queda en Dropbox. Revoca también el permiso en la web de Dropbox («Aplicaciones conectadas»).`
        : `${equipo!.nombre} olvidará el permiso de «${nombre}». Revoca también el permiso en la web de Dropbox («Aplicaciones conectadas»).`,
    });
  }

  function masAlmacen(conEspejo: boolean): AccionMenu[][] {
    return [
      [{ texto: "Conectar Dropbox…", onclick: () => (conectarNube = true) }, { texto: "Quitar el acceso de un equipo…", onclick: () => abrir({ tipo: "guarda_copias", cuerpo: { quitar: "" }, titulo: "Quitar el acceso de un equipo", descripcion: "Ese equipo dejará de poder copiar aquí. Lo que ya copió se queda.", campos: "quitar" }) }],
      [
        ...(conEspejo ? [{ texto: "Quitar todo el espejo", peligro: true, onclick: () => abrir({ tipo: "guarda_copias", cuerpo: { espejo: null }, titulo: "Quitar el espejo", descripcion: "Dejará de copiarse cada noche a todos sus destinos. Lo que ya está en ellos se queda." }) }] : []),
        { texto: "Dejar de guardar copias", peligro: true, onclick: () => abrir({ tipo: "guarda_copias", cuerpo: { activo: false }, titulo: "Dejar de guardar copias", descripcion: `${equipo!.nombre} dejará de recibir copias de los demás equipos. Lo ya guardado se queda en su disco.` }) },
      ],
    ];
  }

  const TIPO_DESTINO: Record<string, string> = { local: "Disco o carpeta del equipo", rest: "Servidor de copias", s3: "S3 compatible", b2: "Backblaze B2", sftp: "SFTP", otro: "Otro" };

  /** Una línea en vez de cuatro cifras vacías: cuándo llegará la primera versión. */
  function sinVersiones(r: RepositorioResumen) {
    const suyas = copias.filter((k) => k.repo === r.id);
    if (r.solo_lectura) return "Todavía sin versiones que mostrar.";
    if (!suyas.length) return "Todavía sin versiones: ninguna copia guarda aquí.";
    const p = proximaCopia(suyas, reloj.ahora);
    return p ? `Todavía sin versiones: la primera copia será ${cuandoFrase(p, reloj.ahora)}.` : "Todavía sin versiones: pulsa «Copiar ahora» para hacer la primera.";
  }

  /** «Más…» de un repositorio: protección, y aparte (en rojo) lo que deja de proteger. */
  function masRepo(r: RepositorioResumen): AccionMenu[][] {
    const conCopias = !r.solo_lectura && copias.some((k) => k.repo === r.id);
    const ref = { id: r.id, nombre: r.nombre };
    const conVersiones = nVersiones(r, informeDe(equipo?.ultimo_informe, r.id)) > 0;
    // v1.22: en un almacén (solo añadir) la retención la aplica el almacén, desde la página del repositorio.
    const enAlm = almacenDe(r, destinoDe(destinos, r), actual.equipos);
    const paginaRet = `/c/${c}/equipos/${id}/repositorios/${encodeURIComponent(r.id)}?retencion=1`;
    const proteccion: AccionMenu[] = [
      ...(conVersiones
        ? [{ texto: "Probar la restauración", onclick: () => abrir({ tipo: "probar_restauracion", cuerpo: { repo: r.id }, descripcion: `Se restaurarán unos archivos de «${r.nombre}» a una carpeta temporal y se compararán. No toca tus archivos.`, accion: "Probar la restauración" }) }]
        : []),
      ...(r.solo_lectura
        ? []
        : enAlm?.admite
          ? [{ texto: `Retención en ${enAlm.almacen.nombre}…`, onclick: () => goto(paginaRet) }]
          : [
            {
              texto: "Cambiar la retención",
              onclick: () => {
                // La que ya tiene (v1.28: la regla del resumen; antes, leída de su texto).
                reg = copiaRegla(reglaDe(r) ?? REGLA_POR_DEFECTO);
                abrir({
                  tipo: "cambiar_retencion",
                  cuerpo: { repo: r.id },
                  descripcion: `Cambia qué versiones guarda «${r.nombre}». Solo se guarda la regla: las versiones sobrantes se borran al «Aplicar retención».`,
                  repo: ref,
                  campos: "retencion",
                });
              },
            },
          ]),
      ...(conCopias ? [{ texto: r.externa ? "Cambiar la copia externa" : "Copia externa…", onclick: () => abrirExterna(r) }] : []),
      // v1.41: con todo su historial, a otro destino (p. ej. el almacén de otro equipo).
      // Si ya se está moviendo (desde otra consola u otro navegador), aquí no se puede empezar otro.
      ...(!r.solo_lectura && puede.administrar(rol) && !moverBloqueado(r.id) ? [{ texto: "Mover a otro sitio…", onclick: () => (mover = r) }] : []),
    ];
    const peligro: AccionMenu[] = [
      // En un servidor de solo añadir, desde el equipo no se puede (403).
      ...(conVersiones && !r.solo_lectura && !r.solo_anadir && !enAlm
        ? [{ texto: "Aplicar retención (borra versiones)", peligro: true, onclick: () => abrir({ tipo: "aplicar_retencion", cuerpo: { repo: r.id }, descripcion: `Se borrarán de «${r.nombre}» las versiones que ya no entren en su retención (restic forget --prune).`, repo: ref }) }]
        : []),
      ...(r.externa ? [{ texto: "Quitar la copia externa", peligro: true, onclick: () => quitarExterna(r) }] : []),
      ...(conCopias
        ? [{ texto: "Dejar de copiar", peligro: true, onclick: () => abrir({ tipo: "dejar_de_copiar", cuerpo: { repo: r.id }, descripcion: `${equipo!.nombre} dejará de copiar en «${r.nombre}». Lo ya guardado sigue en su destino.`, repo: ref }) }]
        : []),
      { texto: "Quitar el repositorio", peligro: true, onclick: () => abrir({ tipo: "quitar_repositorio", cuerpo: { repo: r.id }, descripcion: `${equipo!.nombre} dejará de copiar en «${r.nombre}» y lo olvidará. Lo guardado sigue en su destino.`, repo: ref }) },
    ];
    return [proteccion, peligro];
  }

  /** «Mover a otro sitio…» de un repositorio. */
  let mover = $state<RepositorioResumen | null>(null);
  /** v1.47: ¿se está moviendo y no lo lleva este navegador? (Lo lleva otra consola u otro navegador: solo se ve.) */
  function moverBloqueado(repo: string): boolean {
    if (!equipo) return false;
    const m = moviendoDe(tareasDe(equipo.id), repo);
    return !!m && (!!m.otra_consola || !hayPlanMover(c, equipo.id, m.origen ?? repo));
  }
  /** «Ver los pasos» del movimiento de `r`, si lo lleva este navegador. */
  function seguirMover(r: RepositorioResumen): (() => void) | undefined {
    if (!equipo) return undefined;
    const m = moviendoDe(tareasDe(equipo.id), r.id);
    return m && !m.otra_consola && m.origen === r.id && hayPlanMover(c, equipo.id, r.id) ? () => (mover = r) : undefined;
  }

  /** Elegir una carpeta del equipo (sesión elegir_carpetas) para un campo «donde». */
  let elegirCarpeta = $state<((ruta: string) => void) | null>(null);
  /** Qué reglas tiene que cumplir la carpeta que se elige (la del espejo o la del Servidor de copias). */
  let validarEleccion = $state<((ruta: string) => string | null) | null>(null);
  /** La carpeta que ya estaba escrita (se ve marcada al abrir). */
  let inicialEleccion = $state("");

  // --- Copia externa (cambiar_copia_externa) ------------------------------
  // Cada día, a la hora fijada, el equipo copia el repositorio (restic copy)
  // a otro destino: uno que ya tenga o uno nuevo (p. ej. una carpeta del
  // segundo disco). Va con la contraseña del repositorio.
  const RET_EXT: Regla = { diarias: 7, semanales: 4, mensuales: 12, anuales: 2 };
  type TipoDestino = "local" | "rest" | "s3" | "b2" | "sftp";
  const extVacia = () => ({
    repo: "",
    destinoRepo: "",
    destino: "",
    idNuevo: "",
    nombre: "Disco 2",
    tipo: "local" as TipoDestino,
    donde: "",
    usuario: "",
    secreto: "",
    hora: "21:00",
    conRetencion: false,
    retencion: { ...RET_EXT },
    otraContrasena: false,
    contrasenaDestino: "",
  });
  let ext = $state(extVacia());
  /** v1.46 (agente con `externa_existente`): «Usar uno que ya existe», bloqueo de objetos y «Probar». */
  let extX = $state(externaExtraVacia());
  const extNueva = $derived(admiteExternaExistente(equipo));
  /** Destinos a los que puede ir la copia externa: cualquiera menos el del propio repositorio. */
  const destinosExt = $derived(destinos.filter((d) => d.id !== ext.destinoRepo));
  const cuerpoExterna = $derived({
    repo: ext.repo,
    ...(extNueva && extX.modo === "existente"
      ? cuerpoExistente(extX, ext.idNuevo)
      : {
          destino:
            ext.destino === "nuevo"
              ? {
                  id: ext.idNuevo,
                  nombre: ext.nombre.trim(),
                  tipo: ext.tipo,
                  donde: ext.donde.trim(),
                  ...(ext.tipo !== "local" && ext.usuario ? { usuario: ext.usuario } : {}),
                  ...(ext.tipo !== "local" && ext.secreto ? { secreto: ext.secreto } : {}),
                }
              : { id: ext.destino },
          ...(ext.otraContrasena && ext.contrasenaDestino ? { contrasena_destino: ext.contrasenaDestino } : {}),
        }),
    hora: ext.hora,
    ...(ext.conRetencion ? { retencion: reglaParaOrden(ext.retencion) } : {}),
    // Con un agente que lo entiende, siempre (0: sin bloqueo); si no, nada.
    ...(extNueva ? { bloqueo_dias: 0, ...cuerpoBloqueo(extX) } : {}),
  });
  const externaValida = $derived(
    (!ext.conRetencion || !errorRegla(ext.retencion, admitePlazos(equipo))) &&
      /^([01]\d|2[0-3]):[0-5]\d$/.test(ext.hora) &&
      (extNueva && extX.modo === "existente"
        ? existenteCompleto(extX)
        : !!ext.destino && (ext.destino !== "nuevo" || (!!ext.nombre.trim() && !!ext.donde.trim())) && (!ext.otraContrasena || ext.contrasenaDestino.length >= 8)) &&
      !errorBloqueo(extX),
  );
  /** Qué pasa con la retención allí (lo mismo que dirá el equipo al guardarla). */
  const efectoExterna = $derived(textoRetencionDestino(ext.conRetencion, extX.conBloqueo ? diasBloqueo(extX.bloqueoDias) : null));
  /** Dónde quedará el repositorio en un destino local: «donde» + separador + id del repositorio. */
  const rutaExterna = $derived(ext.donde.trim().replace(/[\\/]+$/, "") + (/windows/i.test(equipo?.so ?? "") ? "\\" : "/") + ext.repo);
  const idDestinoExterna = (r: RepositorioResumen) => (r.externa ? (r.externa.destino_id ?? destinos.find((d) => d.nombre === r.externa!.destino)?.id) : undefined);
  function abrirExterna(r: RepositorioResumen) {
    const propio = destinoDe(destinos, r)?.id ?? r.destino;
    const otros = destinos.filter((d) => d.id !== propio);
    ext = {
      ...extVacia(),
      repo: r.id,
      destinoRepo: propio,
      destino: idDestinoExterna(r) ?? (otros.length ? otros[0].id : "nuevo"),
      idNuevo: `externa-${crypto.randomUUID().slice(0, 8)}`,
      hora: r.externa?.hora ?? "21:00",
    };
    // El bloqueo que ya tiene (si lo tiene), para no quitarlo al cambiar la hora.
    extX = { ...externaExtraVacia(), conBloqueo: !!r.externa?.bloqueo_dias, bloqueoDias: r.externa?.bloqueo_dias ?? 30 };
    abrir({
      tipo: "cambiar_copia_externa",
      cuerpo: {},
      titulo: r.externa ? "Cambiar la copia externa" : "Copia externa",
      descripcion: admiteExternaExistente(equipo)
        ? `Cada día, a la hora que elijas, ${equipo!.nombre} copiará «${r.nombre}» a otro destino: a un repositorio nuevo (lo crea allí, en una carpeta con su id) o a uno que ya existe. Si algo le pasa al destino principal, queda esta.`
        : `Cada día, a la hora que elijas, ${equipo!.nombre} copiará «${r.nombre}» a otro destino (la primera vez crea allí el repositorio, en una carpeta con su id). Si algo le pasa al destino principal, queda esta.`,
      repo: { id: r.id, nombre: r.nombre },
      campos: "externa",
    });
  }
  function quitarExterna(r: RepositorioResumen) {
    const id = idDestinoExterna(r);
    abrir({
      tipo: "cambiar_copia_externa",
      cuerpo: { repo: r.id, ...(id ? { destino: { id } } : {}), hora: null },
      titulo: "Quitar la copia externa",
      descripcion: `«${r.nombre}» dejará de copiarse cada día a «${r.externa?.destino ?? "su destino externo"}». Lo ya copiado allí se queda.`,
      repo: { id: r.id, nombre: r.nombre },
    });
  }



  async function renombrar(e: SubmitEvent) {
    e.preventDefault();
    if (!equipo || !nuevoNombre.trim()) return;
    try {
      await api.renombrarEquipo(c, equipo.id, nuevoNombre.trim());
      renombrando = false;
      avisar("Nombre cambiado.");
      await Promise.all([cargar(), cargarCliente(c, { silencioso: true })]);
    } catch (e) {
      fallo(e);
    }
  }

  /** Comprueba (o vuelve a comprobar) las llaves del equipo con la clave de administración y las fija aquí. */
  async function comprobarLlavesConClave(ev: SubmitEvent) {
    ev.preventDefault();
    if (!equipo || !actual.cliente) return;
    errorComprobar = "";
    comprobando = true;
    try {
      borrar(await kcfgComprobada(actual.cliente, equipo, claveComprobar, true));
      claveComprobar = "";
      comprobar = false;
      avisar("Llaves comprobadas: coinciden con las que se confirmaron al emparejar.");
      await cargar();
    } catch (err) {
      errorComprobar = err instanceof ErrorEtiqueta ? "No cuadran: o la clave no es correcta o las llaves no son las auténticas. No se ha fijado nada." : (err as Error).message;
    } finally {
      comprobando = false;
    }
  }

  let pidiendoAtencion = $state(false);
  async function atencion() {
    pidiendoAtencion = true;
    try {
      await api.pedirAtencion(c, id);
      avisar("Pedido: el equipo consultará cada 2 s durante 10 min.");
    } catch (e) {
      fallo(e);
    } finally {
      pidiendoAtencion = false;
    }
  }

  const tono = (estado?: string): Tono => (estado === "fallo" ? "bad" : estado === "aviso" ? "warn" : estado === "ok" ? "ok" : "neutral");
  const TEXTO_ESTADO: Record<string, string> = { ok: "Correcta", aviso: "Con avisos", fallo: "Fallida" };

  const cambiarTab = (t: string) => goto(`?tab=${t}`, { replaceState: true, noScroll: true, keepFocus: true });
</script>

<svelte:head><title>{equipo?.nombre ?? "Equipo"} · Resguardo Server</title></svelte:head>

<div class="page">
  <Migas items={[{ texto: "Equipos", href: `/c/${c}/equipos` }, { texto: equipo?.nombre ?? "Equipo" }]} />

  {#if error && !equipo}
    <div class="notice notice-danger"><p>{error}</p></div>
  {:else if !equipo || !salud}
    <Esqueleto forma="ficha" n={4} etiqueta="Cargando el equipo…" />
  {:else}
    <header class="page-head">
      <span class="page-icon"><Icono size={22} /></span>
      <div class="page-head-text">
        {#if renombrando}
          <form class="renombrar" onsubmit={renombrar}>
            <!-- svelte-ignore a11y_autofocus -->
            <input class="input" bind:value={nuevoNombre} aria-label="Nombre del equipo" autofocus />
            <button class="icon-btn" aria-label="Guardar"><Check size={16} /></button>
            <button type="button" class="icon-btn" aria-label="Cancelar" onclick={() => (renombrando = false)}><X size={16} /></button>
          </form>
        {:else}
          <div class="page-title-line">
            <h1 class="page-title">{equipo.nombre}</h1>
            <EstadoEquipo cliente={c} {equipo} ahora={reloj.ahora} />
            {#if puede.administrar(rol)}
              <button
                class="icon-btn"
                aria-label="Cambiar el nombre"
                use:tip={"Cambiar el nombre"}
                onclick={() => {
                  nuevoNombre = equipo!.nombre;
                  renombrando = true;
                }}><Pencil size={14} /></button
              >
            {/if}
          </div>
        {/if}
        <p class="sub">
          <span class="junto">{equipo.so} · agente <span class="pastilla mono">{equipo.version_agente}</span></span>
          {#if equipo.rol === "almacenamiento"}<span class="junto">· Guarda copias <Ayuda id="guarda-copias" /></span>{/if}
          <span class="junto">·
            {#if equipo.conectado}<span class="conn"><span class="dot" style="--tone: var(--ok)"></span>Conectado</span><Ayuda id="conectado" />{:else}visto <Tiempo iso={equipo.ultimo_contacto} />{/if}</span
          >
        </p>
        {#if equipo.etiquetas?.length || puede.ordenar(rol)}
          <div class="etiquetas-eq">
            {#each equipo.etiquetas ?? [] as t (t)}<a class="et-enlace" href="/c/{c}/equipos" onclick={() => filtroEtiqueta.poner(t)} use:tip={`Ver los equipos con «${t}»`}><EtiquetaChip nombre={t} /></a>{/each}
            {#if puede.ordenar(rol)}<button class="link mini-et" onclick={() => (editarEtiquetas = true)}><Tag size={12} />{equipo.etiquetas?.length ? "Cambiar etiquetas" : "Añadir etiquetas"}</button>{/if}
          </div>
        {/if}
      </div>
      {#if puede.ordenar(rol)}
        <div class="page-actions">
          {#if pausado}
            <button class="btn" onclick={() => abrir({ tipo: "reanudar", cuerpo: { repo: "" }, descripcion: "Las copias automáticas vuelven a su horario a partir de ahora.", accion: "Reanudar", campos: "repo" })}><CirclePlay size={16} />Reanudar</button>
          {/if}
          {#if copias.length > 1}
            <MenuAcciones
              texto="Copiar ahora"
              primario
              etiqueta="Elegir qué copia hacer ahora"
              grupos={[
                copias.map((k) => ({
                  texto: k.nombre,
                  onclick: () => abrir({ tipo: "copiar_ahora", cuerpo: { repo: k.repo, copia: k.id }, descripcion: `Se hará ahora la copia «${k.nombre}», sin esperar a su hora. No borra nada.`, accion: "Copiar ahora" }),
                })),
              ]}
            />
          {:else if copias.length === 1}
            <button
              class="btn btn-primary"
              use:tip={`Hace ahora la copia «${copias[0].nombre}»`}
              onclick={() =>
                abrir({
                  tipo: "copiar_ahora",
                  cuerpo: { repo: copias[0].repo, copia: copias[0].id },
                  descripcion: `Se hará ahora la copia «${copias[0].nombre}», sin esperar a su hora. No borra nada.`,
                  accion: "Copiar ahora",
                })}><Play size={16} />Copiar ahora</button
            >
          {/if}
        </div>
      {/if}
    </header>
    {#if trasladado}
      <div class="notice notice-info trasladado"><Info size={16} /><p>{equipo.nombre} se trasladó a otro servidor y ya no recibe órdenes desde aquí. Se conserva su historial.</p></div>
    {/if}
    <Observaciones tipo="equipo" objeto={equipo.id} />

    {#if otrasConsolas.length && !trasladado}
      <section class="consolas-eq" aria-label="Otras consolas">
        <span class="ico"><Monitor size={16} /></span>
        <div class="txt">
          <p class="titulo">También lo gestiona{otrasConsolas.length > 1 ? "n" : ""}:</p>
          <ul>
            {#each otrasConsolas as x (x.id)}
              <li>
                <span><strong>{nombreConsola(x)}</strong> <span class="faint">· identidad <code>{huellaCorta(x.identidad)}</code> · {x.ultimo_contacto ? `último contacto ${relativo(x.ultimo_contacto, reloj.ahora)}` : "aún sin contacto"}</span></span>
                {#if puede.administrar(rol)}
                  <button
                    class="btn btn-sm btn-ghost"
                    onclick={() =>
                      abrir({
                        tipo: "quitar_consola",
                        cuerpo: { identidad: x.identidad },
                        titulo: `Quitar ${x.nombre}`,
                        descripcion: `${equipo!.nombre} dejará de conectarse a ${x.nombre} (${x.url}, identidad ${huellaCorta(x.identidad)}): desde allí ya no se podrá gestionar. Esta consola y las demás siguen igual, y el equipo sigue copiando.`,
                        accion: "Quitar la consola",
                      })}>Quitar</button
                  >
                {/if}
              </li>
            {/each}
          </ul>
          {#if cambioDeOtra}
            <p class="faint cambio">Cambiado desde otra consola {relativo(cambioDeOtra.cuando, reloj.ahora)}: «{nombreOrden(cambioDeOtra.tipo)}», desde {nombreConsola(cambioDeOtra.consola)}.</p>
          {/if}
        </div>
      </section>
    {/if}

    {#if llaves === "cambiada"}
      <AlertaLlaves {equipo} cliente={c} />
    {/if}

    {#if salud.tono !== "ok"}
      <div class="notice {salud.tono === 'bad' ? 'notice-danger' : salud.tono === 'warn' ? 'notice-warn' : 'notice-info'}"><p>{salud.detalle}</p></div>
    {/if}

    <nav class="pestanas" aria-label="Secciones del equipo">
      {#each [["resumen", "Resumen"], ["ordenes", "Órdenes"], ["informes", "Informes"], ["detalles", "Detalles"]] as [t, txt] (t)}
        <button class:on={tab === t} aria-current={tab === t ? "page" : undefined} onclick={() => cambiarTab(t)}>{txt}{#if t === "ordenes" && ordenes.some((o) => o.estado === "pendiente")}<span class="punto" aria-hidden="true"></span><span class="sr-only"> (con pendientes)</span>{/if}</button>
      {/each}
    </nav>

    {#if tab === "resumen"}
      {#if copias.length || repos.length}
        <div class="stats">
          <div class="stat">
            <span class="stat-label">Última copia</span>
            <span class="stat-value">{ultimaDeTodas ? relativo(ultimaDeTodas, reloj.ahora) : "Todavía no"}</span>
            <span class="stat-sub">{ultimaDeTodas ? fechaLarga(ultimaDeTodas) : "sin copias todavía"}</span>
          </div>
          <div class="stat">
            <span class="stat-label">Próxima</span>
            <span class="stat-value">{proximaDeTodas ? relativo(proximaDeTodas, reloj.ahora) : "—"}</span>
            <span class="stat-sub">{pausado ? "en pausa" : proximaDeTodas ? cuandoFrase(proximaDeTodas, reloj.ahora) : "nada programado"}</span>
          </div>
          <div class="stat">
            <span class="stat-label">Protegido</span>
            <span class="stat-value">{protegidoEquipo ? bytes(protegidoEquipo) : "—"}</span>
            <span class="stat-sub">en {plural(repos.length, "repositorio", "repositorios")}</span>
          </div>
          <div class="stat">
            <span class="stat-label">Versiones en 24 h</span>
            <span class="stat-value">{numero(recientesEquipo.versiones)}</span>
            <span class="stat-sub">{recientesEquipo.fallos ? plural(recientesEquipo.fallos, "copia fallida", "copias fallidas") : "sin copias fallidas"}</span>
          </div>
        </div>
      {/if}

      <!-- El camino de sus datos (y, si guarda copias, lo de los demás que guarda). -->
      <MapaProteccion equipos={actual.equipos} informes={{ ...(ultimos.cliente === c ? ultimos.porEquipo : {}), [equipo.id]: equipo.ultimo_informe ?? null }} cliente={c} ahora={reloj.ahora} equipo={equipo.id} titulo="Camino de sus copias" />

      <section>
        <div class="section-head">
          <h2>Copias <span class="count">· {copias.length}</span></h2>
          {#if puede.administrar(rol)}<a class="btn btn-sm btn-ghost" href="/c/{c}/equipos/{equipo.id}/copias"><Pencil size={14} />Cambiar las copias</a>{/if}
        </div>
        {#each pendCopias as p (p.orden.id)}<div class="en-camino"><PendienteItem {p} /></div>{/each}
        {#if copias.length}
          <div class="card p-0 lista">
            {#each copias as k (k.id)}
              {@const vuelta = ultimaVuelta(k, equipo.ultimo_informe)}
              {@const est = estadoCopia(k, vuelta, pausado, reloj.ahora)}
              {@const prox = proximaDe(k, equipo.ultimo_informe, reloj.ahora)}
              {@const rk = repos.find((r) => r.id === k.repo)}
              <div class="fila">
                <span class="fila-texto">
                  <a class="fila-titulo enlace-copia" href="/c/{c}/equipos/{equipo.id}/copias/{encodeURIComponent(k.id)}" use:tip={`Ver el detalle de «${k.nombre}»`}>{k.nombre}<ContadorNotas tipo="copia" objeto={objetoDe(equipo.id, k.id)} /><ChevronRight size={14} /></a>
                  <span class="fila-sub">{horarioEnFrase(k.horario)}{k.carpetas !== undefined ? ` · ${plural(k.carpetas, "carpeta", "carpetas")}` : ""} → {rk?.nombre ?? k.repo}</span>
                  {#if rk}<span class="fila-sub"><SeGuardaEn pequeno lugar={lugarRepo(rk, equipo, actual.equipos)} riesgo={!!riesgoMismoEquipo(rk, equipo, actual.equipos)} /></span>{/if}
                  {#if vuelta?.resultado === "fallo" && vuelta.mensaje}<span class="fila-sub msg-fallo">{vuelta.mensaje}</span>{/if}
                  <EnMarcha equipo={equipo.id} copia={k.id} />
                  {#each equipo.ultimo_informe?.datos.copias?.find((x) => x.id === k.id)?.ganchos ?? [] as g, gi (gi)}
                    <span class="fila-sub gancho-res">
                      <Chip pequeno tono={g.estado === "ok" ? "ok" : g.estado === "aviso" ? "warn" : "bad"} texto={NOMBRE_GANCHO[g.tipo] ?? g.tipo} />
                      {#if g.mensaje}<span class:msg-fallo={g.estado === "fallo"}>{g.mensaje}</span>{/if}
                    </span>
                  {/each}
                  {#if informeDe(equipo.ultimo_informe, k.repo)}
                    <span class="mini-copia"><DiasCuadros dias={diasCopia(informeDe(equipo.ultimo_informe, k.repo), k.id, 14, reloj.ahora)} tamano="mini" etiqueta="«{k.nombre}» en los últimos 14 días" /></span>
                  {/if}
                </span>
                <span class="fila-meta solo-ancho">
                  {#if vuelta}Última <Tiempo iso={vuelta.cuando} />{:else}Sin copias todavía{/if}
                  {#if prox && k.activa !== false && !pausado && Date.parse(prox) > reloj.ahora}<br />Próxima <Tiempo iso={prox} />{/if}
                </span>
                <Chip tono={est.tono} texto={est.texto} />
                {#if puede.ordenar(rol)}
                  <button
                    class="icon-btn"
                    use:tip={"Copiar ahora"}
                    aria-label="Copiar ahora «{k.nombre}»"
                    onclick={() => abrir({ tipo: "copiar_ahora", cuerpo: { repo: k.repo, copia: k.id }, descripcion: `Se hará ahora la copia «${k.nombre}», sin esperar a su hora. No borra nada.`, accion: "Copiar ahora" })}
                    ><Play size={15} /></button
                  >
                {/if}
              </div>
            {/each}
          </div>
        {:else if !pendCopias.length}
          <div class="card">
            <Vacio icono={FolderSync} titulo="Este equipo aún no copia nada" texto="Elige qué carpetas copiar, dónde y cuándo.">
              {#if puede.administrar(rol)}<a class="btn btn-primary" href="/c/{c}/equipos/{equipo.id}/copias">Crear la primera copia</a>{/if}
            </Vacio>
          </div>
        {/if}
      </section>

      {#if repos.length || pendRepos.length}
        <section>
          <div class="section-head"><h2>Repositorios <span class="count">· {repos.length}</span></h2></div>
          {#each pendRepos as p (p.orden.id)}<div class="en-camino"><PendienteItem {p} /></div>{/each}
          <div class="rejilla repos">
            {#each repos as r (r.id)}
              {@const inf = informeDe(equipo.ultimo_informe, r.id)}
              {@const prot = proteccion(inf, comprobacionLugar(r, equipo, actual.equipos))}
              {@const ej = ultimaEjecucion(inf)}
              {@const riesgo = riesgoMismoEquipo(r, equipo, actual.equipos)}
              <article class="card repo">
                <a class="repo-cab enlace" href="/c/{c}/equipos/{equipo.id}/repositorios/{encodeURIComponent(r.id)}" use:tip={`Ver el detalle de «${r.nombre}»`}>
                  <span class="card-icon on"><Database size={18} /></span>
                  <div class="repo-nombre">
                    <h3>{r.nombre} <ContadorNotas tipo="repositorio" objeto={objetoDe(equipo.id, r.id)} />{#if r.solo_lectura} <span class="badge badge-sm tone-neutral" use:tip={"Importado de otro equipo: se puede explorar y restaurar, pero ninguna copia escribe en él."}>Solo lectura</span>{/if}</h3>
                    <p class="donde"><SeGuardaEn pequeno lugar={lugarRepo(r, equipo, actual.equipos)} riesgo={!!riesgo} /></p>
                  </div>
                  {#if ej}<Chip pequeno tono={TONO_RESULTADO[ej.resultado]} texto={ej.resultado === "ok" ? "Al día" : TEXTO_RESULTADO[ej.resultado]} />{/if}
                  <ChevronRight size={16} />
                </a>
                {#if nVersiones(r, inf)}
                  <div class="fact-boxes">
                    <div><span class="faint">Versiones</span><strong class="num">{numero(nVersiones(r, inf))}</strong></div>
                    <div><span class="faint">Tamaño</span><strong class="num">{bytes(bytesRepo(r, inf))}</strong></div>
                    <div><span class="faint">Última versión</span><span><Tiempo iso={ultimaVersion(r, inf)} nada="todavía no" /></span></div>
                    <div>
                      <span class="faint">Protección</span>
                      {#if prot}<a class="prot" href="/c/{c}/equipos/{equipo.id}/repositorios/{encodeURIComponent(r.id)}#proteccion"><AnilloProteccion proteccion={prot} tamano={16} /><span class="num">{prot.puntuacion} de {prot.total}</span></a>
                      {:else}<span><Tiempo iso={r.verificado} nada="sin datos" /></span>{/if}
                    </div>
                  </div>
                  {#if inf}
                    <div class="mini">
                      <DiasCuadros dias={dias(inf, 14, reloj.ahora)} tamano="mini" etiqueta="Resultado de las copias en los últimos 14 días" />
                      <span class="faint">14 días</span>
                    </div>
                  {/if}
                {:else}
                  <p class="sin-versiones"><Clock size={14} />{sinVersiones(r)}</p>
                {/if}
                <MoviendoseAviso equipo={equipo.id} repo={r.id} onseguir={seguirMover(r)} />
                <EnMarcha equipo={equipo.id} repo={r.id} tipos={["verificar", "verificar_externa", "copia_externa", "prueba_restauracion", "historial", "retencion", "restauracion"]} sinMover />
                {#if riesgo}<AvisoMismoEquipo compacto {riesgo} onmover={puede.administrar(rol) && !moverBloqueado(r.id) ? () => (mover = r) : undefined} hrefExterna={puede.ordenar(rol) && copias.some((k) => k.repo === r.id) ? `/c/${c}/equipos/${equipo.id}?externa=${encodeURIComponent(r.id)}` : undefined} />{/if}
                {#if r.retencion}<p class="faint retencion">Guarda {r.retencion} <Ayuda id="retencion" /></p>{/if}
                {#if r.externa}<p class="externa"><CloudUpload size={14} />Copia externa a «{r.externa.destino}» cada día a las {r.externa.hora}{#each detallesExterna(r.externa) as d (d)}{" · "}{d}{/each} <Ayuda id="copia-externa" /></p>{/if}
                {#if puede.ordenar(rol)}
                  <div class="acciones">
                    {#if nVersiones(r, inf) || r.solo_lectura}<a class="btn btn-sm btn-primary" href="/c/{c}/restaurar?equipo={equipo.id}&repo={r.id}"><History size={14} />Restaurar</a>{/if}
                    {#if nVersiones(r, inf)}
                      <button class="btn btn-sm" onclick={() => abrir({ tipo: "verificar_ahora", cuerpo: { repo: r.id }, descripcion: `Se comprobará ahora una parte de «${r.nombre}» para confirmar que las copias se pueden leer.`, accion: "Verificar ahora" })}><ShieldCheck size={14} />Verificar</button>
                    {/if}
                    <MenuAcciones etiqueta="Más acciones de «{r.nombre}»" grupos={masRepo(r)} />
                  </div>
                {/if}
              </article>
            {/each}
          </div>
        </section>
      {/if}

      {#if repos.length}
        <HistorialVersiones
          fuentes={fuentesHistorial}
          {copias}
          equipo
          historial={historia.entradas}
          ultimas={equipo.ultimo_informe?.datos.copias ?? []}
          enlace={(r, v, todo) => `/c/${c}/restaurar?${new URLSearchParams({ equipo: equipo!.id, repo: r, version: v.id, ...(todo ? { todo: "1" } : {}) })}`}
          ahora={reloj.ahora}
          puedeRestaurar={puede.ordenar(rol)}
          vacio="Cada vez que se haga una copia se guardará aquí una versión que podrás explorar y restaurar."
          alAbrir={(v, r, f) => ((repoCajon = r), abrirVersion(v.id, f))}
          alAbrirVuelta={(h, r) => ((repoCajon = r), abrirVuelta(h))}
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

      {#if destinos.length || pendDestinos.length}
        <section>
          <div class="section-head"><h2>Destinos <span class="count">· {destinos.length}</span></h2></div>
          {#each pendDestinos as p (p.orden.id)}<div class="en-camino"><PendienteItem {p} /></div>{/each}
          <div class="card p-0 lista">
            {#each destinos as d (d.id)}
              <div class="fila">
                <span class="fila-texto">
                  <span class="fila-titulo">{d.nombre} <ContadorNotas tipo="destino" objeto={d.id} /></span>
                  <span class="fila-sub">{TIPO_DESTINO[d.tipo] ?? d.tipo}{#if d.donde}{" · "}<span class="pastilla mono">{d.donde}</span>{/if}{d.inmutable ? " · solo añadir" : ""}</span>
                </span>
                <button class="btn btn-sm btn-ghost" onclick={() => (notasDestino = { id: d.id, nombre: d.nombre })}>Notas</button>
                {#if puede.administrar(rol) && d.tipo !== "local"}
                  <button
                    class="btn btn-sm btn-ghost"
                    onclick={() =>
                      abrir({
                        tipo: "cambiar_destino",
                        cuerpo: { destino: d.id, donde: "", usuario: "", secreto: "", ca_pem: "" },
                        titulo: "Cambiar las credenciales del destino",
                        descripcion: `Nuevos datos para «${d.nombre}». El equipo comprueba que cada repositorio se abre con ellos antes de guardarlos; si alguno no, no cambia nada. Deja en blanco lo que no cambie.`,
                        campos: "destino",
                      })}>Cambiar credenciales</button
                  >
                {/if}
              </div>
            {/each}
          </div>
        </section>
      {/if}

      {#if equipo.resumen?.guarda_copias?.activo}
        {@const g = equipo.resumen.guarda_copias}
        <section class="card p">
          <div class="repo-cab">
            <span class="card-icon on"><Server size={18} /></span>
            <div>
              <h3>Guarda copias <Ayuda id="guarda-copias" /></h3>
              <p class="faint">Puerto <span class="pastilla mono">{g.puerto}</span> · {g.solo_red_local ? "solo redes internas" : "abierto a otras sedes"} · {plural(g.usuarios ?? 0, "equipo copia aquí", "equipos copian aquí")}</p>
            </div>
          </div>
          {#each pendGuarda as p (p.orden.id)}<div class="en-camino dentro"><PendienteItem {p} /></div>{/each}
          {#if g.espejo}
            <div class="espejo">
              <p class="externa">
                <HardDrive size={14} /><span>{flexible ? "Espejo de lo que guarda" : `Espejo cada noche a las ${g.espejo.hora}`}{#if g.espejo.limite_kib}{" · "}subida limitada a {numero(g.espejo.limite_kib)} KiB/s{/if}{#if g.espejo.ultima}{" · "}la última subida <Tiempo iso={g.espejo.ultima} />{/if}</span>
                <Ayuda id="espejo" />
              </p>
              {#if g.espejo.destinos?.length}
                <ul class="destinos-espejo">
                  {#each g.espejo.destinos as d (d.tipo + (d.nube ?? "") + (d.carpeta ?? ""))}
                    <li>
                      <span class="ic-d">{#if d.tipo === "nube"}<Cloud size={14} />{:else}<HardDrive size={14} />{/if}</span>
                      <span class="d-texto">
                        <strong>{d.tipo === "nube" ? d.nube : d.carpeta}</strong>
                        <span class="faint">{d.tipo === "nube" ? `en la carpeta ${d.carpeta}` : "otra carpeta"}{#if d.ultima}{" · "}<Tiempo iso={d.ultima} />{/if}</span>
                        {#if flexible}<span class="faint">{cuandoEspejo(d, g.espejo.hora)}{#if d.proxima}{" · la próxima "}<Tiempo iso={d.proxima} />{/if}</span>
                          <span class="faint">{textoRepos(d, nombreRepoAlmacen)}{#if textoVerificacion(d)}{" · "}{textoVerificacion(d)}{/if}</span>
                          <span class="faint">{textoRetencion(d)}{#if d.por_borrar?.archivos}{" · "}{plural(d.por_borrar.archivos, "archivo espera", "archivos esperan")} para borrarse ({bytes(d.por_borrar.bytes)}){#if d.por_borrar.primero}, el primero el {diaLegible(d.por_borrar.primero)}{/if}{/if}</span>
                          {#if d.freno && puede.administrar(rol)}<span class="freno"><button class="btn btn-sm" onclick={() => confirmarFreno(d)}>Confirmar lo que falta…</button></span>{/if}{/if}
                        {#if resultadoConError(d.resultado)}<span class="msg-fallo">{d.resultado} <a href="/ayuda#{d.tipo === 'nube' && /permis|token|auth|401|403|expir|revoc/i.test(d.resultado ?? '') ? 'si-token' : 'si-espejo'}">Qué hacer</a></span>{/if}
                      </span>
                      {#if d.resultado}<Chip pequeno tono={resultadoConError(d.resultado) ? "bad" : "ok"} texto={resultadoConError(d.resultado) ? "Falló" : "Hecho"} />{:else}<Chip pequeno tono="neutral" texto="Todavía no" />{/if}
                      {#if puede.administrar(rol)}
                        {#if flexible}<button class="btn btn-sm btn-ghost" onclick={() => abrirEspejo(d.tipo, d)}>Cambiar</button>{/if}
                        <button class="icon-btn" use:tip={"Quitar este destino"} aria-label="Quitar este destino del espejo" onclick={() => quitarDestinoEspejo(destinoParaOrden(d))}><Trash2 size={14} /></button>
                      {/if}
                    </li>
                  {/each}
                </ul>
              {#if flexible && nuevosEspejo.length}
                <div class="notice notice-info nuevos-espejo">
                  <p>{nuevosEspejo.length === 1 ? "Hay un repositorio nuevo" : `Hay ${nuevosEspejo.length} repositorios nuevos`} ({nuevosEspejo.map(nombreRepoAlmacen).join(", ")}) que no {nuevosEspejo.length === 1 ? "entra" : "entran"} en {conSeleccion.length === 1 ? "un destino del espejo con selección" : `${conSeleccion.length} destinos del espejo con selección`}. Los de «todos» ya {nuevosEspejo.length === 1 ? "lo copian" : "los copian"}.</p>
                  {#if puede.administrar(rol)}
                    <div class="acciones-nuevos">
                      <button class="btn btn-sm btn-primary" onclick={() => preguntarNuevos(true)}>Añadir{nuevosEspejo.length === 1 ? "lo" : "los"}</button>
                      <button class="btn btn-sm btn-ghost" onclick={() => preguntarNuevos(false)}>Dejar{nuevosEspejo.length === 1 ? "lo" : "los"} fuera</button>
                    </div>
                  {/if}
                </div>
              {/if}
              {:else if g.espejo.resultado}
                <p class="faint pequeno-e">{#if resultadoConError(g.espejo.resultado)}<span class="msg-fallo">{g.espejo.resultado}</span>{:else}{g.espejo.resultado}{/if}</p>
              {/if}
            </div>
          {/if}
          {#if g.retenciones?.length}
            <div class="espejo">
              <p class="externa"><History size={14} /><span>Retención que aplica en local (los equipos no pueden borrar aquí)</span><Ayuda id="retencion-almacen" /></p>
              <ul class="destinos-espejo">
                {#each g.retenciones as ra (ra.usuario + "/" + ra.repo)}
                  {@const de = repoDeRetencion(ra, equipo, actual.equipos)}
                  <li>
                    <span class="ic-d"><Database size={14} /></span>
                    <span class="d-texto">
                      {#if de}<a class="link" href="/c/{c}/equipos/{de.equipo.id}/repositorios/{encodeURIComponent(de.repo.id)}"><strong>{de.repo.nombre}</strong></a> <span class="faint">de {de.equipo.nombre}</span>{:else}<strong>{ra.usuario}/{ra.repo}</strong>{/if}
                      <span class="faint">{ra.texto} · {ra.horario.reglas?.length ? horarioEnFrase({ dias: [], horas: [], reglas: ra.horario.reglas }).replace(/^./, (x) => x.toLowerCase()) : (ra.horario_texto ?? textoHorario(ra.horario))}{#if ra.ultima}{" · "}<Tiempo iso={ra.ultima} />{/if}</span>
                      {#if ra.resultado === "fallo" && ra.mensaje}<span class="msg-fallo">{ra.mensaje}</span>{:else if ra.clave === "pendiente"}<span class="msg-fallo">Su clave aún no abre el repositorio.</span>{/if}
                    </span>
                    {#if ra.resultado}<Chip pequeno tono={ra.resultado === "ok" ? "ok" : "bad"} texto={ra.resultado === "ok" ? "Aplicada" : "Falló"} />{:else}<Chip pequeno tono="neutral" texto="Todavía no" />{/if}
                  </li>
                {/each}
              </ul>
            </div>
          {/if}
          {#if g.nubes?.length}
            <p class="externa">
              <Cloud size={14} />Nubes conectadas: {#each g.nubes as n, ni (n.nombre)}{ni ? ", " : ""}<span class="nube-c">{n.nombre}{#if puede.administrar(rol)}<button class="link quitar-nube" onclick={() => quitarNube(n.nombre)} aria-label="Desconectar {n.nombre}">Desconectar</button>{/if}</span>{/each}
            </p>
          {/if}
          {#if puede.administrar(rol)}
            <div class="acciones">
              <button class="btn btn-sm" onclick={() => abrirEspejo()}><Plus size={14} />{g.espejo ? "Añadir destino del espejo" : "Espejo de lo que guarda"}</button>
              <MenuAcciones etiqueta="Más acciones de «Guarda copias»" grupos={masAlmacen(!!g.espejo)} />
            </div>
          {/if}
        </section>
      {:else if puede.administrar(rol)}
        <section class="card p">
          <div class="repo-cab">
            <span class="card-icon"><Server size={18} /></span>
            <div>
              <h3>Este equipo puede guardar copias <Ayuda id="guarda-copias" /></h3>
              <p class="faint">Convierte este equipo en un almacén de copias para los demás equipos de su red (rest-server en modo solo añadir): cada uno con su usuario y sin poder borrar lo ya copiado. La consola (Resguardo Server) no guarda copias: solo coordina. <a class="link" href="/ayuda#consola-y-almacen">¿Qué diferencia hay?</a></p>
            </div>
          </div>
          {#each pendGuarda as p (p.orden.id)}<div class="en-camino dentro"><PendienteItem {p} /></div>{/each}
          {#if !pendGuarda.some((p) => !terminada(p.orden))}<div class="acciones"><button class="btn btn-sm" onclick={abrirGuardar}><Server size={14} />Este equipo guarda copias</button></div>{/if}
        </section>
      {/if}

      {#if equipo.resumen?.escritorio}
        {@const esc = equipo.resumen.escritorio}
        <section class="card p" aria-labelledby="t-en-equipo">
          <h3 class="section-title" id="t-en-equipo">En el equipo</h3>
          <p class="faint nota-almacen">
            Ventana: {({ off: "sin ventana", siempre_disponible: "desde el icono", al_trabajar: "se abre sola al trabajar" } as const)[esc.ventana]} · Avisos:
            {({ off: "ninguno", errores: "al fallar y al recuperarse", todo: "todos (empezar, terminar, fallar)" } as const)[esc.avisos]}
            {#if equipo.resumen.escritorio_cambiado_en_equipo}<br />Cambiado en el equipo {relativo(equipo.resumen.escritorio_cambiado_en_equipo)}.{/if}
          </p>
          {#if puede.administrar(rol)}<div class="acciones"><a class="btn btn-sm" href={`/c/${c}/equipos/${equipo.id}/copias#escritorio`}>Cambiar</a></div>{/if}
        </section>
      {/if}

      {#if propioPosible && puede.administrar(rol)}
        <section class="card p">
          <h3 class="section-title">Copiar en este mismo almacén <Ayuda id="almacen-propio" /></h3>
          <p class="faint nota-almacen">{TEXTO_ALMACEN_PROPIO}</p>
          <div class="acciones"><button class="btn btn-sm" onclick={() => (copiarEn = equipo)}><Server size={14} />Copiar en este mismo almacén</button></div>
        </section>
      {/if}

      {#if almacenes.length && puede.administrar(rol)}
        <section class="card p">
          <h3 class="section-title">Copiar en un equipo de la oficina</h3>
          <p class="faint nota-almacen">Rápido para restaurar y sin tocar el router. Lo recomendado: además, una copia externa en la nube.</p>
          <div class="acciones">
            {#each almacenes as a (a.id)}<button class="btn btn-sm" onclick={() => (copiarEn = a)}><Server size={14} />Copiar en «{a.nombre}»</button>{/each}
          </div>
        </section>
      {/if}
      <Comentarios tipo="equipo" objeto={equipo.id} />
    {:else if tab === "ordenes"}
      <section>
        {#if ordenes.length}
          <div class="card p-0"><ListaOrdenes {ordenes} equipos={[equipo]} alCambiar={cargar} /></div>
        {:else}
          <div class="card"><Vacio icono={BellRing} titulo="Todavía no hay órdenes" texto="Aquí verás cada orden que se envíe a este equipo y su respuesta firmada." /></div>
        {/if}
      </section>
    {:else if tab === "informes"}
      <section>
        {#if !informes.length}
          <Cargando />
        {:else}
          <div class="card p-0 desplazable">
            <table class="tabla">
              <caption class="sr-only">Informes recibidos de {equipo.nombre}</caption>
              <thead><tr><th scope="col">Recibido</th><th scope="col">Copias</th><th scope="col" class="der">Datos nuevos</th><th scope="col" class="der">Archivos</th><th scope="col">Resultado</th></tr></thead>
              <tbody>
                {#each informes as inf (inf.recibido)}
                  {@const cs = inf.datos.copias ?? []}
                  {@const peor = cs.some((x) => x.estado === "fallo") ? "fallo" : cs.some((x) => x.estado === "aviso") ? "aviso" : "ok"}
                  <tr>
                    <td><time datetime={inf.recibido} use:tip={fechaLarga(inf.recibido)}>{fechaLarga(inf.recibido)}</time></td>
                    <td>{cs.map((x) => x.nombre ?? x.id).join(", ") || "—"}</td>
                    <td class="num der">{bytes(cs.reduce((n, x) => n + (x.bytes ?? 0), 0))}</td>
                    <td class="num der">{numero(cs.reduce((n, x) => n + (x.archivos ?? 0), 0))}</td>
                    <td>
                      <Chip pequeno tono={tono(peor)} texto={TEXTO_ESTADO[peor]} />
                      {#each cs.filter((x) => x.mensaje) as x (x.id)}<p class="msg faint">{x.mensaje}</p>{/each}
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
          <p class="faint nota">Los informes no llevan rutas ni nombres de archivos: solo cifras y estados.</p>
        {/if}
      </section>
    {:else}
      <section class="card p detalles">
        <dl>
          <div><dt>Identificador</dt><dd><Copiable texto={equipo.id} que="el identificador" /></dd></div>
          <div><dt>Modo</dt><dd>{equipo.modo === "gestionado" ? (otrasConsolas.length ? `Gestionado desde este servidor y ${plural(otrasConsolas.length, "consola más", "consolas más")}` : "Gestionado desde este servidor") : equipo.modo === "trasladado" ? "Trasladado a otro servidor" : "Local (sin servidor)"}</dd></div>
          <div><dt>Papel</dt><dd>{equipo.rol === "almacenamiento" ? "Guarda copias" : "Agente"}</dd></div>
          <div><dt>Siguiente orden</dt><dd class="num">N.º {equipo.siguiente_seq} <Ayuda id="seq" /></dd></div>
          <div><dt>Llave para recibir (X25519)</dt><dd class="llave"><Copiable texto={equipo.box_pub} que="la llave" /></dd></div>
          <div><dt>Llave para firmar (Ed25519)</dt><dd class="llave"><Copiable texto={equipo.sign_pub} que="la llave" /></dd></div>
          <div>
            <dt>En este navegador</dt>
            <dd>
              {#if llaves === "fijada"}<span class="badge badge-sm tone-ok"><ShieldCheck size={11} />Llaves comprobadas</span> <span class="faint">el {fechaLarga(fijadaFecha)}</span>
              {:else if llaves === "cambiada"}<span class="badge badge-sm tone-bad">Las llaves cambiaron</span>
              {:else}<span class="badge badge-sm tone-neutral">Llaves aún sin comprobar</span>{/if}
              {#if puede.ordenar(rol)}<button class="link" onclick={() => (comprobar = true)}>{llaves === "fijada" ? "Volver a comprobar" : "Comprobar con la clave de administración"}</button>{/if}
            </dd>
          </div>
          <div><dt>Etiqueta <Ayuda id="etiqueta" /></dt><dd>{#if equipo.etiqueta}<span class="llave"><Copiable texto={equipo.etiqueta} que="la etiqueta" /></span>{:else}<span class="faint">Sin confirmar</span>{/if}</dd></div>
        </dl>
        <div class="acciones">
          <BotonCargando class="btn btn-sm" cargando={pidiendoAtencion} onclick={atencion}><BellRing size={14} />Que consulte ahora</BotonCargando>
          <button
            class="btn btn-sm"
            onclick={async () => {
              await navigator.clipboard.writeText(equipo!.id);
              avisar("Identificador copiado.");
            }}><Copy size={14} />Copiar identificador</button
          >
          {#if puede.ordenar(rol)}
            <!-- El agente aún no admite estas dos: «próximamente». -->
            <button class="btn btn-sm" disabled use:tip={"Próximamente"}>Actualizar el agente · próximamente</button>
            <button class="btn btn-sm" onclick={() => abrir({ tipo: "desbloquear", cuerpo: { repo: "" }, descripcion: "Quita los bloqueos antiguos de los repositorios (de copias que se cortaron). Es seguro: no borra datos.", accion: "Quitar bloqueos", campos: "repo" })}>Quitar bloqueos antiguos</button>
          {/if}
        </div>
      </section>

      {#if puede.ordenar(rol)}
        <section class="card p peligro">
          <h2 class="section-title">Pausar, desvincular o dar de baja</h2>
          <p class="faint">Piden la clave de administración. Lo que puede dejar de proteger espera {actual.cliente?.espera_min_horas} h y se puede cancelar.</p>
          <div class="acciones">
            {#if !pausado}
              <button class="btn btn-sm" onclick={() => abrir({ tipo: "pausar", cuerpo: { repo: "", horas: 24 }, descripcion: "Las copias automáticas se detienen el tiempo que elijas. «Copiar ahora» sigue funcionando.", campos: "pausar" })}><CirclePause size={14} />Pausar las copias</button>
            {/if}
            {#if puede.administrar(rol)}
              {#if otrasConsolas.length}
                <!-- v1.36: con otras consolas, desvincular solo quita esta (las demás siguen). -->
                <button
                  class="btn btn-sm"
                  onclick={() =>
                    abrir({
                      tipo: "desvincular",
                      cuerpo: { modo: "seguir_local" },
                      titulo: "Dejar de gestionar este equipo",
                      descripcion: `Esta consola dejará de gestionar ${equipo!.nombre}. Lo seguirá${otrasConsolas.length > 1 ? "n" : ""} gestionando ${otrasConsolas.map(nombreConsola).join(", ")}, y el equipo sigue copiando igual.`,
                      accion: "Dejar de gestionar",
                    })}><Unlink size={14} />Dejar de gestionar desde aquí</button
                >
              {:else}
                <button class="btn btn-sm" onclick={() => abrir({ tipo: "desvincular", cuerpo: { modo: "seguir_local" }, descripcion: "El equipo dejará de obedecer a este servidor.", campos: "desvincular" })}><Unlink size={14} />Desvincular</button>
              {/if}
              <button class="btn btn-sm btn-danger" onclick={() => abrir({ tipo: "baja_equipo", cuerpo: {}, descripcion: `${equipo!.nombre} dejará de copiar y se quitará de este cliente. Lo ya copiado se conserva en sus destinos.` })}>Dar de baja</button>
            {/if}
          </div>
        </section>
      {/if}
    {/if}
  {/if}
</div>

{#if dialogo && equipo && actual.cliente}
  {#key dialogo}
  <OrdenDialog
    cliente={actual.cliente}
    {equipo}
    tipo={dialogo.tipo}
    cuerpo={dialogo.campos === "externa" ? cuerpoExterna : dialogo.campos === "espejo" ? cuerpoEspejo : dialogo.campos === "retencion" ? { repo: dialogo.cuerpo.repo, ...reglaParaOrden(reg) } : dialogo.cuerpo}
    titulo={dialogo.titulo}
    descripcion={dialogo.descripcion}
    repo={dialogo.repo}
    accion={dialogo.accion}
    campos={dialogo.campos ? camposOrden : undefined}
    valido={dialogo.campos === "quitar" ? !!String(dialogo.cuerpo.quitar ?? "").trim() : dialogo.campos === "guardar" ? !!String(dialogo.cuerpo.carpeta ?? "").trim() && !errorGuardar : dialogo.campos === "externa" ? externaValida : dialogo.campos === "espejo" ? espejoValido : dialogo.campos === "retencion" ? !errorRegla(reg, admitePlazos(equipo)) : true}
    probar={dialogo.campos === "externa" && extNueva ? { cuerpo: { solo_probar: true }, texto: "Probar" } : null}
    onclose={() => {
      dialogo = null;
      ext.secreto = ext.contrasenaDestino = "";
      extX.existente.secreto = extX.existente.contrasena = "";
      // Lo que haya cambiado el equipo (p. ej. el espejo) se ve al cerrar.
      void cargar();
    }}
    alEnviar={() => void cargar()}
    alTerminar={() => void cargar()}
  />
  {/key}
{/if}

{#if editarEtiquetas && equipo}
  <EditorEtiquetas {equipo} onclose={() => (editarEtiquetas = false)} alGuardar={(e) => equipo && (equipo.etiquetas = e.etiquetas)} />
{/if}

{#if conectarNube && equipo && actual.cliente}
  <ConectarNube cliente={actual.cliente} {equipo} onclose={() => ((conectarNube = false), void cargar())} />
{/if}

{#if elegirCarpeta && equipo && actual.cliente}
  <ElegirCarpetas
    cliente={actual.cliente}
    {equipo}
    unica
    validar={validarEleccion ?? undefined}
    iniciales={inicialEleccion ? [inicialEleccion] : []}
    onclose={() => ((elegirCarpeta = null), (validarEleccion = null))}
    alElegir={(rutas) => {
      elegirCarpeta?.(rutas[0] ?? "");
      elegirCarpeta = null;
    }}
  />
{/if}

{#if copiarEn && equipo && actual.cliente}
  <CopiarEnAlmacen cliente={actual.cliente} {equipo} almacen={copiarEn} onclose={() => ((copiarEn = null), void cargar())} />
{/if}

{#if mover && equipo && actual.cliente}
  <MoverRepositorio cliente={actual.cliente} {equipo} repo={mover} equipos={actual.equipos} onclose={() => (mover = null)} />
{/if}
{#if notasDestino}<NotasDialogo tipo="destino" objeto={notasDestino.id} nombre={notasDestino.nombre} onclose={() => (notasDestino = null)} />{/if}

{#if comprobar && equipo}
  <Modal labelledby="t-comprobar" onclose={() => ((comprobar = false), (claveComprobar = ""))} width={460} dismissible={false}>
    <form class="form" onsubmit={comprobarLlavesConClave}>
      <div class="dlg-title">
        <span class="ticon"><ShieldCheck size={18} /></span>
        <div>
          <h2 id="t-comprobar">Comprobar las llaves de {equipo.nombre}</h2>
          <p>Con la clave de administración se comprueba su etiqueta. Si cuadra, este navegador recuerda sus llaves y avisará si cambian.</p>
        </div>
      </div>
      {#if llaves === "cambiada"}<div class="notice notice-warn"><p>Hazlo solo si sabes que el equipo se reinstaló o se volvió a emparejar.</p></div>{/if}
      <CampoClave requerido id="clave-comprobar" etiqueta="Clave de administración" bind:value={claveComprobar} autofocus error={errorComprobar} />
      <footer>
        <button type="button" class="btn btn-ghost" onclick={() => ((comprobar = false), (claveComprobar = ""))}>Cancelar</button>
        <BotonCargando class="btn btn-primary" disabled={!claveComprobar} cargando={comprobando} textoCargando="Comprobando…">Comprobar</BotonCargando>
      </footer>
    </form>
  </Modal>
{/if}

{#snippet camposOrden()}
  {#if (dialogo?.campos === "pausar" || dialogo?.campos === "repo") && repos.length > 1}
    <div class="field">
      <label class="field-label" for="d-repo">Repositorio</label>
      <select id="d-repo" class="input" bind:value={dialogo.cuerpo.repo}>
        <option value="">Todos</option>
        {#each repos as r (r.id)}<option value={r.id}>{r.nombre}</option>{/each}
      </select>
    </div>
  {/if}
  {#if dialogo?.campos === "pausar"}
    <div class="field">
      <label class="field-label" for="horas">Durante</label>
      <select id="horas" class="input" bind:value={dialogo.cuerpo.horas}>
        <option value={4}>4 horas</option>
        <option value={24}>1 día</option>
        <option value={72}>3 días</option>
        <option value={168}>1 semana</option>
        <option value={0}>Hasta que la reanude</option>
      </select>
    </div>
  {:else if dialogo?.campos === "retencion"}
    <EditorRetencion id="ret" bind:regla={reg} admite={admitePlazos(equipo)} {...horarioDeCopias(copias, String(dialogo.cuerpo.repo ?? ""))} />
  {:else if dialogo?.campos === "externa"}
    {#if extNueva}
      <div class="segmented" role="group" aria-label="Repositorio de la copia externa">
        <button type="button" class:on={extX.modo === "nuevo"} aria-pressed={extX.modo === "nuevo"} onclick={() => (extX.modo = "nuevo")}><Plus size={14} />Crear uno nuevo</button>
        <button type="button" class:on={extX.modo === "existente"} aria-pressed={extX.modo === "existente"} onclick={() => (extX.modo = "existente")}><Database size={14} />Usar uno que ya existe</button>
      </div>
    {/if}
    {#if extNueva && extX.modo === "existente"}
      <p class="faint nota-esp">Por ejemplo, el de la nube al que subía la app de escritorio. Solo se sube lo que le falte: lo que ya tiene no se repite (si trocea igual que este repositorio; «Probar» lo comprueba). Nunca se crea otro allí.</p>
      <div class="field">
        <label class="field-label" for="xe-nombre">Nombre del destino <span class="faint">(opcional)</span></label>
        <input id="xe-nombre" class="input" bind:value={extX.nombreExistente} placeholder={extX.existente.tipo === "b2" ? "Backblaze B2" : extX.existente.tipo === "s3" ? "S3" : ""} />
      </div>
      <FormRepoExistente
        bind:repo={extX.existente}
        id="xe"
        nombreEquipo={equipo?.nombre}
        etiquetaContrasena="Contraseña de ese repositorio"
        ayudaContrasena="La que abre ese repositorio (p. ej. la que usaba la app de escritorio para la nube). Puede ser distinta de la del repositorio de origen."
      />
    {:else}
    <div class="field">
      <label class="field-label" for="x-destino">Copiar a</label>
      <select id="x-destino" class="input" bind:value={ext.destino}>
        {#each destinosExt as d (d.id)}<option value={d.id}>{d.nombre}{d.donde ? ` · ${d.donde}` : ""}</option>{/each}
        <option value="nuevo">Un destino nuevo…</option>
      </select>
    </div>
    {#if ext.destino === "nuevo"}
      <div class="nuevo-destino">
        <div class="fila-campos">
          <div class="field">
            <label class="field-label" for="x-tipo">Tipo</label>
            <select id="x-tipo" class="input" bind:value={ext.tipo}>
              <option value="local">Disco o carpeta</option>
              <option value="rest">Servidor de copias (rest-server)</option>
              <option value="sftp">SFTP</option>
              <option value="s3">S3 compatible</option>
              <option value="b2">Backblaze B2</option>
            </select>
          </div>
          <div class="field">
            <label class="field-label" for="x-nombre">Nombre</label>
            <input id="x-nombre" class="input" bind:value={ext.nombre} placeholder="Disco 2" />
          </div>
        </div>
        <div class="field">
          <label class="field-label" for="x-donde">{ext.tipo === "local" ? "Carpeta" : ext.tipo === "rest" ? "Dirección (https://servidor:puerto)" : ext.tipo === "sftp" ? "Servidor y ruta (usuario@servidor:/ruta)" : "Bucket"}</label>
          <div class="con-boton">
            <input id="x-donde" class="input mono" bind:value={ext.donde} placeholder={ext.tipo === "local" ? (/windows/i.test(equipo?.so ?? "") ? "E:\\Resguardo" : "/mnt/disco2/resguardo") : ""} spellcheck="false" />
            {#if ext.tipo === "local"}<button type="button" class="btn" onclick={() => ((validarEleccion = (r: string) => errorCarpetaDestino(r, win, true)), (inicialEleccion = ext.donde.trim()), (elegirCarpeta = (ruta) => (ext.donde = ruta)))}><FolderOpen size={15} />Explorar…</button>{/if}
          </div>
          {#if ext.tipo === "local" && ext.donde.trim()}
            <span class="field-hint">El repositorio irá en <code>{rutaExterna}</code>.</span>
          {/if}
        </div>
        {#if ext.tipo !== "local"}
          <div class="fila-campos">
            <div class="field">
              <label class="field-label" for="x-usuario">{ext.tipo === "rest" || ext.tipo === "sftp" ? "Usuario" : "Id de la clave"}</label>
              <input id="x-usuario" class="input mono" bind:value={ext.usuario} autocomplete="off" spellcheck="false" />
            </div>
            <CampoClave requerido id="x-secreto" etiqueta={ext.tipo === "rest" || ext.tipo === "sftp" ? "Contraseña" : "Clave secreta"} bind:value={ext.secreto} />
          </div>
        {/if}
      </div>
    {/if}
    {/if}
    <div class="field">
      <label class="field-label" for="x-hora">Cada día a las</label>
      <input id="x-hora" class="input num corto" type="time" bind:value={ext.hora} />
    </div>
    <label class="switch-row"><input type="checkbox" bind:checked={ext.conRetencion} /><span>Retención propia en el destino externo<span class="faint">Si no, guarda las mismas versiones que el repositorio de origen.</span></span></label>
    {#if ext.conRetencion}
      <EditorRetencion id="xr" bind:regla={ext.retencion} admite={admitePlazos(equipo)} {...horarioDeCopias(copias, ext.repo)} />
    {/if}
    {#if extNueva}
      <label class="switch-row"><input type="checkbox" bind:checked={extX.conBloqueo} /><span>El destino tiene bloqueo de objetos (Object Lock)<span class="faint">Lo subido no se puede borrar durante unos días (p. ej. un bucket de B2 o S3 con bloqueo). Así no se intenta borrar lo que aún está bloqueado.</span></span></label>
      {#if extX.conBloqueo}
        <div class="field">
          <label class="field-label" for="x-bloqueo">Días de bloqueo</label>
          <input id="x-bloqueo" class="input num corto" type="number" min="1" max={MAX_BLOQUEO} step="1" bind:value={extX.bloqueoDias} />
          {#if errorBloqueo(extX)}<p class="error-campo">{errorBloqueo(extX)}</p>{:else}<span class="field-hint">Los del bucket (en B2: «Object Lock», periodo de retención por defecto).</span>{/if}
        </div>
      {/if}
      {#if efectoExterna}<p class="faint nota-esp">{efectoExterna}</p>{/if}
    {/if}
    {#if !(extNueva && extX.modo === "existente")}
    <label class="switch-row"><input type="checkbox" bind:checked={ext.otraContrasena} /><span>Otra contraseña para la copia externa<span class="faint">Si no, usa la del repositorio de origen (la de su kit).</span></span></label>
    {#if ext.otraContrasena}
      <CampoClave requerido id="x-contrasena" etiqueta="Contraseña de la copia externa" bind:value={ext.contrasenaDestino} ayuda="Al menos 8 caracteres. Apúntala en el kit: sin ella no se puede leer la copia externa." />
    {/if}
    {/if}
  {:else if dialogo?.campos === "espejo"}
    {#if esp.editando}
      <p class="faint nota-esp">Destino: <strong>{esp.tipo === "nube" ? `«${esp.nube}», carpeta ${esp.carpetaNube}` : esp.carpeta}</strong></p>
    {:else}
    {#if destinosActuales.length}
      <p class="faint nota-esp">Ya copia a {destinosActuales.map((d) => (d.tipo === "nube" ? `«${d.nube}»` : d.carpeta)).join(", ")}: se mantiene{destinosActuales.length === 1 ? "" : "n"} y se añade el nuevo.</p>
    {/if}
    <div class="segmented" role="group" aria-label="Tipo de destino">
      <button type="button" class:on={esp.tipo === "carpeta"} aria-pressed={esp.tipo === "carpeta"} onclick={() => (esp.tipo = "carpeta")}><HardDrive size={14} />Otra carpeta</button>
      <button type="button" class:on={esp.tipo === "nube"} aria-pressed={esp.tipo === "nube"} onclick={() => (esp.tipo = "nube")}><Cloud size={14} />Nube</button>
    </div>
    {#if esp.tipo === "carpeta"}
      <div class="field">
        <label class="field-label" for="e-carpeta">Carpeta (mejor en otro disco)</label>
        <div class="con-boton">
          <input id="e-carpeta" class="input mono" bind:value={esp.carpeta} placeholder={/windows/i.test(equipo?.so ?? "") ? "E:\\Resguardo-espejo" : "/mnt/disco2/espejo"} spellcheck="false" />
          <button type="button" class="btn" onclick={() => ((validarEleccion = (r: string) => errorCarpetaEspejo(r, win)), (inicialEleccion = esp.carpeta.trim()), (elegirCarpeta = (ruta) => (esp.carpeta = ruta)))}><FolderOpen size={15} />Explorar…</button>
        </div>
        {#if errorEspejo}<p class="error-campo">{errorEspejo}</p>{:else}<span class="field-hint">Si falla el disco principal, el espejo sigue ahí.</span>{/if}
      </div>
    {:else if !nubes.length}
      <div class="notice notice-info nube-vacia">
        <Cloud size={16} />
        <div>
          <p>Primero conecta Dropbox en {equipo?.nombre}: se hace desde aquí, en dos minutos, y el permiso se guarda solo en el equipo.</p>
          <button type="button" class="btn btn-sm btn-primary conectar-nube" onclick={() => ((dialogo = null), (conectarNube = true))}><Cloud size={14} />Conectar Dropbox</button>
        </div>
      </div>
    {:else}
      <div class="fila-campos">
        <div class="field">
          <label class="field-label" for="e-nube">Nube</label>
          <select id="e-nube" class="input" bind:value={esp.nube}>
            {#each nubes as n (n.nombre)}<option value={n.nombre}>{n.nombre} ({n.tipo === "drive" ? "Google Drive" : "Dropbox"})</option>{/each}
          </select>
        </div>
        <div class="field">
          <label class="field-label" for="e-cnube">Carpeta dentro de la nube</label>
          <input id="e-cnube" class="input mono" bind:value={esp.carpetaNube} spellcheck="false" />
          {#if errorCarpetaNube}<p class="error-campo">{errorCarpetaNube}</p>{:else}<span class="field-hint">Relativa a Aplicaciones/Resguardo, que ya es la raíz: por ejemplo, «Sur».</span>{/if}
        </div>
      </div>
      <div class="field">
        <label class="field-label" for="e-limite">Límite de subida (opcional)</label>
        <div class="con-boton">
          <input id="e-limite" class="input num corto" type="number" min="1" placeholder={espejoActual?.limite_kib ? String(espejoActual.limite_kib) : "sin límite"} bind:value={esp.limite} />
          <span class="faint unidad">KiB/s</span>
        </div>
        <span class="field-hint">Para no saturar la conexión de la oficina por la noche.</span>
      </div>
      <p class="faint nota-esp">Dropbox y Google Drive no son inmutables: quien tenga la cuenta puede borrar lo subido (el historial de versiones de la nube ayuda a recuperarlo).</p>
    {/if}
    {#if repetido}<p class="error-campo">Ese destino ya está en el espejo.</p>{/if}
    {/if}
    {#if flexible}
      <EspejoOpciones id="e-op" bind:horario={esp.horario} bind:trasCopia={esp.trasCopia} bind:todos={esp.todos} bind:elegidos={esp.elegidos} repositorios={reposAlmacen} nombre={nombreRepoAlmacen} bind:verificarPct={esp.verificarPct} nube={esp.tipo === "nube"} bind:borrar={esp.borrar} bind:dias={esp.dias} />
    {:else}
    <div class="field">
      <label class="field-label" for="e-hora">Cada noche a las{destinosActuales.length ? " (para todos los destinos)" : ""}</label>
      <input id="e-hora" class="input num corto" type="time" bind:value={esp.hora} />
    </div>
    {/if}

  {:else if dialogo?.campos === "guardar"}
    <div class="field">
      <label class="field-label" for="g-carpeta">Carpeta o disco donde guardar</label>
      <div class="con-boton">
        <input id="g-carpeta" class="input mono" bind:value={dialogo.cuerpo.carpeta} placeholder={/windows/i.test(equipo?.so ?? "") ? "D:\\Resguardo" : "/srv/resguardo"} spellcheck="false" />
        <button type="button" class="btn" onclick={() => ((validarEleccion = (r: string) => errorCarpetaDestino(r, win)), (inicialEleccion = String(dialogo?.cuerpo.carpeta ?? "").trim()), (elegirCarpeta = (ruta) => dialogo && (dialogo.cuerpo.carpeta = ruta)))}><FolderOpen size={15} />Explorar…</button>
      </div>
      {#if errorGuardar}<p class="error-campo">{errorGuardar}</p>{/if}
    </div>
    <div class="field">
      <label class="field-label" for="g-puerto">Puerto</label>
      <input id="g-puerto" class="input num corto" type="number" min="1024" max="65535" bind:value={dialogo.cuerpo.puerto} />
      {#if equipo?.resumen?.puerto_libre}
        <p class="faint">{equipo.resumen.puerto_libre === 8000 ? "8000, el habitual de los almacenes, está libre en" : `8000 está ocupado en ${equipo.nombre}: te proponemos ${equipo.resumen.puerto_libre}, libre en`} {equipo.nombre}. Si eliges otro, el equipo no lo activa si está ocupado.</p>
      {:else}
        <p class="faint">8000 es el habitual de los almacenes. Si ese equipo ya lo usa (otro servidor de copias u otro programa), elige otro, p. ej. 8002: el equipo no lo activa si el puerto está ocupado.</p>
      {/if}
    </div>
    <label class="switch-row"><input type="checkbox" bind:checked={dialogo.cuerpo.solo_red_local as boolean} /><span>Solo redes internas<span class="faint">Equipos de la empresa, también de otras subredes o VLAN; nunca desde internet. Para otra sede hará falta abrir el puerto en su router.</span></span></label>
  {:else if dialogo?.campos === "quitar"}
    <div class="field">
      <label class="field-label" for="g-usuario">Usuario del equipo</label>
      <input id="g-usuario" class="input mono" bind:value={dialogo.cuerpo.quitar} spellcheck="false" />
      <span class="field-hint">El que se le dio al añadirlo (aparece en su kit de recuperación).</span>
    </div>
  {:else if dialogo?.campos === "destino"}
    <div class="field">
      <label class="field-label" for="d-donde">Nueva dirección</label>
      <input id="d-donde" class="input mono" bind:value={dialogo.cuerpo.donde} spellcheck="false" placeholder="Sin cambios" />
    </div>
    <div class="field">
      <label class="field-label" for="d-usuario">Usuario o id de la clave</label>
      <input id="d-usuario" class="input mono" bind:value={dialogo.cuerpo.usuario} spellcheck="false" placeholder="Sin cambios" autocomplete="off" />
    </div>
    <CampoClave id="d-secreto" etiqueta="Contraseña o clave secreta" bind:value={dialogo.cuerpo.secreto as string} />
    <div class="field">
      <label class="field-label" for="d-ca">Certificado de la autoridad (PEM, opcional)</label>
      <textarea id="d-ca" class="input mono" rows="3" bind:value={dialogo.cuerpo.ca_pem} spellcheck="false" placeholder="Sin cambios"></textarea>
    </div>
  {:else if dialogo?.campos === "desvincular"}
    <div class="radios">
      <label class="radio"><input type="radio" bind:group={dialogo.cuerpo.modo} value="seguir_local" />Seguir copiando en local, sin servidor (no espera)</label>
      <label class="radio"><input type="radio" bind:group={dialogo.cuerpo.modo} value="dejar_de_copiar" />Dejar de copiar (espera {actual.cliente?.espera_min_horas} h)</label>
    </div>
  {/if}
{/snippet}

<!-- El cajón de detalle de lo que se abre en «Historial y versiones» (la versión o la copia de la URL, de su repositorio). -->
{#if equipo && repoDetalle && actual.cliente && tab === "resumen"}
  <PanelDetalle cliente={actual.cliente} {equipo} repo={repoDetalle} inf={informeDe(equipo.ultimo_informe, repoDetalle.id)} {copias} historial={historia.entradas} puedeRestaurar={puede.ordenar(rol)} ahora={reloj.ahora} />
{/if}

<style>
  .consolas-eq {
    display: flex;
    gap: var(--sp-3);
    align-items: flex-start;
    margin-bottom: var(--sp-3);
    padding: 10px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius, 8px);
    background: var(--surface-2);
    font-size: var(--fs-sm);
  }
  .consolas-eq .ico {
    display: inline-flex;
    padding-top: 2px;
    color: var(--text-2);
  }
  .consolas-eq .txt {
    flex: 1;
    min-width: 0;
  }
  .consolas-eq p {
    margin: 0;
  }
  .consolas-eq .titulo {
    font-weight: 600;
  }
  .consolas-eq ul {
    margin: 4px 0 0;
    padding: 0;
    list-style: none;
  }
  .consolas-eq li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 6px 10px;
    padding: 2px 0;
  }
  .consolas-eq li code {
    font-size: 11px;
  }
  .consolas-eq .cambio {
    margin-top: 6px;
  }
  .en-camino {
    margin-bottom: var(--sp-2);
  }
  .etiquetas-eq {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
  }
  .et-enlace {
    display: inline-flex;
    text-decoration: none;
  }
  .mini-et {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--fs-sm);
  }
  .en-camino.dentro {
    margin: var(--sp-3) 0 0;
  }
  .sub {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .conn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .renombrar {
    display: flex;
    align-items: center;
    gap: 4px;
    max-width: 420px;
  }
  .punto {
    display: inline-block;
    width: 6px;
    height: 6px;
    margin-left: 6px;
    vertical-align: 2px;
    border-radius: 999px;
    background: var(--warn);
  }
  .repo {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-5);
  }
  .repo-cab {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }
  .repo-cab h3 {
    display: flex;
    align-items: center;
    font-size: var(--fs-h2);
  }
  .repo-cab p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .fact-boxes strong {
    font-weight: 600;
  }
  .retencion {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .detalles dl {
    display: grid;
    gap: var(--sp-3);
    margin: 0 0 var(--sp-5);
  }
  .detalles dl > div {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: var(--sp-3);
  }
  dt {
    display: flex;
    align-items: center;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  dd {
    margin: 0;
    min-width: 0;
  }
  .llave {
    font-size: 12px;
    word-break: break-all;
  }
  .peligro h2 {
    margin-bottom: 4px;
  }
  .peligro p {
    margin: 0 0 var(--sp-4);
    font-size: var(--fs-sm);
  }
  .nota {
    margin: var(--sp-2) 0 0;
    font-size: var(--fs-xs);
  }
  .msg {
    margin: 4px 0 0;
    font-size: var(--fs-xs);
  }
  .nota-almacen {
    margin: 4px 0 var(--sp-3);
    font-size: var(--fs-sm);
  }
  .corto {
    width: 140px;
  }
  .rejilla.repos {
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 400px), 1fr));
  }
  .repo-cab.enlace {
    color: inherit;
    text-decoration: none;
    border-radius: var(--radius);
  }
  .repo-cab.enlace > :global(svg:last-child) {
    margin-left: auto;
    color: var(--text-3);
  }
  .repo-cab.enlace:hover h3 {
    text-decoration: underline;
  }
  .repo-nombre {
    flex: 1;
    min-width: 0;
  }
  .repo-nombre .donde {
    margin: 2px 0 0;
  }
  .prot {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: inherit;
    text-decoration: none;
  }
  .espejo {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .freno {
    display: block;
    margin-top: 0.35rem;
  }
  .nuevos-espejo {
    display: grid;
    gap: 0.5rem;
    margin-top: 0.6rem;
  }
  .acciones-nuevos {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
  .destinos-espejo {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
    border-top: 1px solid var(--border);
  }
  .destinos-espejo li {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
  }
  .ic-d {
    display: grid;
    flex: none;
    color: var(--text-3);
  }
  .d-texto {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
    font-size: var(--fs-sm);
  }
  .d-texto strong {
    font-weight: 500;
    overflow-wrap: anywhere;
  }
  .d-texto .faint,
  .d-texto .msg-fallo {
    font-size: var(--fs-xs);
  }
  .pequeno-e {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .nota-esp {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .nube-vacia p {
    margin: 0;
  }
  .conectar-nube {
    margin-top: 8px;
  }
  .nube-c {
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
  }
  .quitar-nube {
    font-size: var(--fs-xs);
  }
  .unidad {
    align-self: center;
    font-size: var(--fs-sm);
  }
  .gancho-res {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 2px;
    white-space: normal;
  }
  .msg-fallo {
    color: var(--bad);
  }
  .mini-copia {
    display: block;
    margin-top: 4px;
  }
  .mini {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-xs);
  }
  .sin-versiones {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    padding: 10px 12px;
    font-size: var(--fs-sm);
    color: var(--text-2);
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .externa {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .con-boton {
    display: flex;
    gap: 8px;
  }
  .con-boton .input {
    flex: 1;
    min-width: 0;
  }
  .nuevo-destino {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-3);
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .fila-campos {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--sp-3);
  }
  @media (max-width: 640px) {
    .solo-ancho {
      display: none;
    }
    .detalles dl > div {
      grid-template-columns: 1fr;
      gap: 2px;
    }
    .fila-campos {
      grid-template-columns: 1fr;
    }
  }
  .sub .junto {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    white-space: nowrap;
  }
</style>
