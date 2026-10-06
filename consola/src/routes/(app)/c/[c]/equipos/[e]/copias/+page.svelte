<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Editor de copias de un equipo (orden «config», con la clave de administración).
  //
  // Las carpetas y exclusiones van cifradas con K_cfg: para verlas y cambiarlas
  // hace falta la clave de administración. Al abrir se calculan K_cfg (para
  // descifrar) y la prueba del equipo (para la orden), se comprueba la etiqueta
  // y la clave se olvida. Todo se borra al salir.
  import Migas from "$lib/componentes/Migas.svelte";
  import { onDestroy, untrack } from "svelte";
  import { seguirCambios, tocaEquipo } from "$lib/vivo.svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { ArrowDown, ArrowUp, CalendarClock, FlaskConical, FolderOpen, KeyRound, LayoutTemplate, Link2, LoaderCircle, LockKeyhole, MonitorCheck, Plus, Save, ShieldCheck, Trash2, TriangleAlert, Undo2, X } from "@lucide/svelte";
  import { ADMITE, admite, errorCadenas, lineaCadena, lineaEnTexto, mover, posiblesAnteriores, recomendarFueraRetencion, TEXTO_FUERA_RETENCION } from "$lib/cadenas";
  import * as api from "$lib/api";
  import { ApiError } from "$lib/api";
  import { argon2Navegador } from "$lib/cripto/argon2";
  import { borrar, deB64 } from "$lib/cripto/bytes";
  import { pruebaAdmin } from "$lib/cripto/claves";
  import { descifrarConfig } from "$lib/cripto/simetrico";
  import { ErrorEtiqueta, ErrorLlavesCambiadas, kcfgComprobada, mandarOrden } from "$lib/ordenar";
  import AlertaLlaves from "$lib/componentes/AlertaLlaves.svelte";
  import { actual, cargarCliente, puede, reloj } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { horarioEnFrase, lista, plural, relativo, resumenHorario } from "$lib/formato";
  import { errorReglas, reglasDe, VERSION_REGLAS, VERSION_SOLO_CAMBIOS } from "$lib/horario";
  import EditorHorario from "$lib/componentes/EditorHorario.svelte";
  import Observaciones from "$lib/componentes/notas/Observaciones.svelte";
  import { objetoDe } from "$lib/notas.svelte";
  import type { Configuracion, CopiaConfig, DestinoResumen, EquipoDetalle, Escritorio, Gancho, VerificacionAuto } from "$lib/tipos";
  // v1.41: dónde guarda cada repositorio (con la carpeta, que aquí se ve: la configuración está descifrada en este navegador).
  import { lugarDe, riesgoMismoEquipo } from "$lib/dondeGuarda";
  import SeGuardaEn from "$lib/componentes/SeGuardaEn.svelte";
  import { admiteVerificacion, admiteVerificacionHorario, errorVerificacion, VERIFICACION_POR_DEFECTO } from "$lib/verificacion";
  import EditorVerificacion from "$lib/componentes/EditorVerificacion.svelte";
  import { errorGancho, fraseGancho, ganchosDe, VERSION_GANCHOS, versionAlMenos } from "$lib/ganchos";
  import EditorGanchos from "$lib/componentes/EditorGanchos.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import CampoClave from "$lib/componentes/CampoClave.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import ElegirCarpetas from "$lib/componentes/ElegirCarpetas.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import MenuAcciones from "$lib/componentes/MenuAcciones.svelte";
  import BotonCargando from "$lib/componentes/BotonCargando.svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { cargarPlantillas, guardarPlantilla, plantillaDe, retencionEnFrase, type Plantilla } from "$lib/plantillas";
  import { admitePruebaAuto, configParaEnviar, PRUEBA_POR_DEFECTO } from "$lib/configEnvio";
  // Tarea 8: cómo queda cada copia en la regla 3-2-1-1-0 y la plantilla «3-2-1 recomendada».
  import TiraRegla from "$lib/componentes/regla/TiraRegla.svelte";
  import Plantilla321 from "$lib/componentes/regla/Plantilla321.svelte";
  import { fraseConfig, queHacer, reglaEnEdicion } from "$lib/regla321";
  import { zonaDeDestino } from "$lib/destinos";
  import { destinoDe } from "$lib/repo";
  import { catalogoDe, cargarCatalogo } from "$lib/catalogoDestinos.svelte";

  const c = $derived(page.params.c ?? "");
  const id = $derived(page.params.e ?? "");

  let equipo = $state<EquipoDetalle | null>(null);
  /** Versión del agente (la del último informe si la hay): los ganchos piden ≥ 0.7.2. */
  const versionAgente = $derived((equipo?.ultimo_informe?.datos.version as string | undefined) ?? equipo?.version_agente ?? null);
  const admiteGanchos = $derived(versionAlMenos(versionAgente, VERSION_GANCHOS));
  let clave = $state("");
  let abriendo = $state(false);
  let error = $state("");
  let paso = $state("");
  /** Material en memoria mientras se edita (se borra al salir). */
  let kcfg: Uint8Array | null = null;
  let prueba = $state<Uint8Array | null>(null);
  let original = $state("");
  let cfg = $state<Configuracion | null>(null);
  let guardando = $state(false);
  let llavesCambiadas = $state(false);
  let elegirPara = $state<number | null>(null);

  $effect(() => {
    void id;
    api.equipo(c, id).then((e) => (equipo = e), (e) => (error = e.message));
  });
  // Su estado al día (próxima vez, última copia…) sin tocar lo que se está editando (`cfg`).
  $effect(() => {
    const [cc, ee] = [c, id];
    return untrack(() =>
      seguirCambios(() => api.equipo(cc, ee).then((e) => ee === id && (equipo = e), () => {}), {
        ms: 0,
        toca: (x) => (x.t === "informe" || x.t === "config" || x.t === "equipo" || (x.t === "progreso" && x.estado === "termina")) && tocaEquipo(x, ee),
      }),
    );
  });
  onDestroy(() => {
    borrar(kcfg, prueba);
    kcfg = prueba = null;
    clave = "";
  });

  async function abrir(e: SubmitEvent) {
    e.preventDefault();
    if (!equipo || !actual.cliente) return;
    error = "";
    abriendo = true;
    try {
      paso = "Comprobando la clave y las llaves del equipo…";
      kcfg = await kcfgComprobada(actual.cliente, equipo, clave);
      paso = "Preparando la autorización…";
      prueba = await pruebaAdmin(argon2Navegador, clave, equipo.sal_equipo);
      clave = "";
      paso = "Descifrando la configuración…";
      let c0: Configuracion;
      try {
        const cifrada = await api.configEquipo(c, equipo.id);
        c0 = descifrarConfig<Configuracion>(kcfg, equipo.id, cifrada.seq, deB64(cifrada.cifrado));
      } catch (err) {
        if (err instanceof ApiError && err.codigo === "no_existe") c0 = { v: 1, copias: [], repositorios: equipo.resumen?.repositorios?.map((r) => ({ id: r.id, nombre: r.nombre, destino: r.destino })) ?? [], destinos: equipo.resumen?.destinos ?? [] };
        else if (err instanceof ApiError) throw err;
        else throw new Error("No se pudo descifrar la configuración del equipo con esta clave.");
      }
      // Los ganchos, siempre como lista mientras se edita (al enviar: null, uno o la lista).
      // «Solo guardar si hay cambios»: sin el campo, encendido (lo que hace el agente).
      c0.copias = c0.copias.map((k) => ({ ...k, gancho: ganchosDe(k.gancho), solo_si_cambios: k.solo_si_cambios ?? true }));
      original = JSON.stringify(c0);
      cfg = c0;
      void recargarPlantillas();
      // Tarea 7f: desde «Añadir una copia» (?nueva=1, y ?tras=<copia> si va después de otra).
      if (page.url.searchParams.get("nueva") === "1") {
        nueva();
        const tras = page.url.searchParams.get("tras");
        const k = cfg.copias.at(-1);
        if (k && tras && cfg.copias.some((x) => x.id === tras)) {
          k.tras = tras;
          k.horario = { dias: [], horas: [] };
        }
      }
    } catch (err) {
      borrar(kcfg, prueba);
      kcfg = prueba = null;
      if (err instanceof ErrorLlavesCambiadas) llavesCambiadas = true;
      error = err instanceof ErrorEtiqueta || err instanceof ErrorLlavesCambiadas ? err.message : (err as Error).message;
    } finally {
      abriendo = false;
      paso = "";
    }
  }

  const cambiado = $derived(cfg ? JSON.stringify(cfg) !== original : false);

  // --- Horario: reglas que se suman (lib/horario.ts, EditorHorario) -----------
  // Con un agente anterior a la 0.7.9 se guarda como siempre (la lista de horas
  // desplegada); con uno nuevo, también las reglas si hacen falta.
  const admiteReglas = $derived(versionAlMenos(versionAgente, VERSION_REGLAS));
  /** Lo que falla en el horario de una copia, en frase para «Antes de enviar», o null. Sin horario solo importa si está activa. */
  function problemaHorario(k: CopiaConfig): string | null {
    const reglas = reglasDe(k.horario, admiteReglas);
    // Tarea 7c: «después de la anterior» no necesita horario propio.
    if (!reglas.length) return k.activa && !k.tras ? `«${k.nombre}» no tiene horario.` : null;
    const e = errorReglas(reglas, admiteReglas);
    return e ? `«${k.nombre}»: ${e.replace(/^./, (x) => x.toLowerCase()).replace(/\.$/, "")}` : null;
  }

  // --- Plantillas (v1.20) --------------------------------------------------
  // Cifradas con la clave de administración (lib/plantillas.ts). Usar una solo
  // rellena la copia aquí: se revisa y se envía como siempre.
  let plantillas = $state<Plantilla[]>([]);
  let ilegibles = $state(0);
  /** La plantilla con la que se rellenó cada copia (para avisar de revisarla). */
  let usadas = $state<Record<string, Plantilla>>({});
  let guardarComo = $state<{ i: number; nombre: string; enviando: boolean; error: string } | null>(null);
  let gestionar = $state(false);
  let borrandoPlantilla = $state<string | null>(null);

  async function recargarPlantillas() {
    if (!kcfg) return;
    try {
      const r = await cargarPlantillas(c, kcfg);
      plantillas = r.lista;
      ilegibles = r.ilegibles;
      ponerPlantillaPedida();
    } catch {
      /* sin plantillas (servidor anterior): no se ofrecen */
    }
  }
  // v1.52: «?plantilla=<id>» (la de una etiqueta del equipo, desde su ficha): al abrir
  // con la clave se añade una copia rellena con ella, para revisarla y enviarla.
  // Nunca se envía sola.
  let plantillaPedida = $state<{ nombre: string } | { falta: true } | null>(null);
  function ponerPlantillaPedida() {
    const id = page.url.searchParams.get("plantilla");
    if (!id || plantillaPedida || !cfg) return;
    const p = plantillas.find((x) => x.id === id);
    if (!p) return void (plantillaPedida = { falta: true });
    nuevaDesde(p);
    plantillaPedida = { nombre: p.nombre };
  }
  function aplicarPlantilla(k: CopiaConfig, p: Plantilla) {
    k.carpetas = [...p.copia.carpetas];
    k.exclusiones = [...p.copia.exclusiones];
    k.horario = JSON.parse(JSON.stringify(p.copia.horario)) as CopiaConfig["horario"];
    k.solo_si_cambios = p.copia.solo_si_cambios !== false;
    k.gancho = JSON.parse(JSON.stringify(p.copia.gancho ?? [])) as Gancho[];
    usadas[k.id] = p;
  }
  function nuevaDesde(p: Plantilla) {
    nueva();
    const k = cfg?.copias.at(-1);
    if (!k) return;
    k.nombre = p.nombre;
    aplicarPlantilla(k, p);
  }
  async function guardarPlantillaDe(e: SubmitEvent) {
    e.preventDefault();
    if (!guardarComo || !cfg || !kcfg || !guardarComo.nombre.trim()) return;
    const k = cfg.copias[guardarComo.i];
    if (!k) return;
    guardarComo.enviando = true;
    guardarComo.error = "";
    try {
      const repo = cfg.repositorios.find((r) => r.id === k.repo);
      const p = plantillaDe($state.snapshot(k) as CopiaConfig, guardarComo.nombre, ganchosDe($state.snapshot(k.gancho) as Gancho[]), repo?.retencion ?? null);
      await guardarPlantilla(c, kcfg, p);
      avisar(`Plantilla «${p.nombre}» guardada (cifrada: el servidor no ve su contenido).`);
      guardarComo = null;
      await recargarPlantillas();
    } catch (err) {
      if (guardarComo) {
        guardarComo.enviando = false;
        guardarComo.error = (err as Error).message;
      }
    }
  }
  async function borrarPlantillaGuardada(p: Plantilla) {
    try {
      await api.borrarPlantilla(c, p.id);
      plantillas = plantillas.filter((x) => x.id !== p.id);
      avisar(`Plantilla «${p.nombre}» borrada. Las copias hechas con ella no cambian.`);
    } catch (err) {
      error = (err as Error).message;
    } finally {
      borrandoPlantilla = null;
    }
  }
  /** ¿Sugiere la plantilla otra retención que la del repositorio de la copia? */
  function retencionDistinta(k: CopiaConfig): string | null {
    const r = usadas[k.id]?.retencion;
    if (!r) return null;
    const actual = cfg?.repositorios.find((x) => x.id === k.repo)?.retencion;
    return actual && JSON.stringify(actual) === JSON.stringify(r) ? null : retencionEnFrase(r);
  }

  /** Quitar una copia: si aún no se envió, desaparece; si ya estaba, deja de hacerse al enviar. */
  function quitar(i: number) {
    if (!cfg) return;
    cfg.copias.splice(i, 1);
  }
  /** Vuelve a lo que tiene el equipo (descarta todo lo no enviado). */
  function cancelarCambios() {
    cfg = JSON.parse(original) as Configuracion;
  }
  const guardadas = $derived(new Set((JSON.parse(original || "{}") as Partial<Configuracion>).copias?.map((k) => k.id) ?? []));
  const quitadas = $derived(((JSON.parse(original || "{}") as Partial<Configuracion>).copias ?? []).filter((k) => !cfg?.copias.some((x) => x.id === k.id)));
  const admiteSoloCambios = $derived(versionAlMenos(versionAgente, VERSION_SOLO_CAMBIOS));
  // Los importados de otro equipo (§10) son solo de lectura: ninguna copia escribe en ellos.
  const repos = $derived((cfg?.repositorios ?? []).filter((r) => !r.solo_lectura));
  /** Dónde guarda un repositorio: lo del resumen (unidad, extraíble…) y, de la configuración, la carpeta. */
  function lugarDeRepo(id: string) {
    const r = cfg?.repositorios.find((x) => x.id === id);
    if (!r || !equipo) return null;
    const dr = equipo.resumen?.destinos?.find((d) => d.id === r.destino || d.nombre === r.destino);
    const dc = cfg?.destinos.find((d) => d.id === r.destino);
    const l = lugarDe(dr || dc ? ({ ...dr, ...dc } as DestinoResumen) : undefined, equipo, actual.equipos);
    const rr = equipo.resumen?.repositorios?.find((x) => x.id === id);
    return { lugar: dc?.tipo === "local" && dc.donde ? { ...l, detalle: dc.donde } : l, riesgo: !!rr && !!riesgoMismoEquipo(rr, equipo, actual.equipos) };
  }

  // --- Tarea 8: la regla 3-2-1-1-0 de cada copia mientras se edita -----------------
  $effect(() => {
    const cc = c;
    if (cc) untrack(() => void cargarCatalogo(cc));
  });
  const catalogo = $derived(catalogoDe(c));
  /** Cómo queda una copia con lo que se va a enviar (su repositorio, la verificación y la prueba). */
  function reglaDe(k: CopiaConfig) {
    if (!equipo || !k.repo) return null;
    return reglaEnEdicion(equipo, k, actual.equipos.map((x) => (x.id === equipo!.id ? equipo! : x)), equipo.ultimo_informe, catalogo, reloj.ahora, {
      verificacion: !!cfg?.verificaciones?.[k.repo],
      prueba: !!cfg?.pruebas_restauracion?.[k.repo],
    });
  }
  /** Las copias añadidas con la plantilla «3-2-1 recomendada» (para enseñar sus pasos). */
  let con321 = $state<string[]>([]);
  /** Plantilla «3-2-1 recomendada» (8d): una copia al almacén con verificación semanal y prueba mensual. */
  function nueva321() {
    if (!cfg || !equipo) return;
    const enAlmacen = repos.find((r) => zonaDeDestino(destinoDe(equipo!.resumen?.destinos, equipo!.resumen?.repositorios?.find((x) => x.id === r.id) ?? r), actual.equipos)?.principal);
    const repo = (enAlmacen ?? repos[0])?.id ?? "";
    const id = `copia-${crypto.randomUUID().slice(0, 8)}`;
    cfg.copias.push({ id, nombre: "Copia 3-2-1", repo, carpetas: [], exclusiones: ["*.tmp", "~$*", "Thumbs.db"], horario: { dias: [1, 2, 3, 4, 5, 6, 7], horas: ["21:00"] }, activa: true, gancho: [], solo_si_cambios: true });
    if (repo && admiteVerif && !cfg.verificaciones?.[repo]) ponerVerif(repo, { ...VERIFICACION_POR_DEFECTO, cada_dias: 7, porcentaje: 10 });
    if (repo && admitePrueba && !cfg.pruebas_restauracion?.[repo]) ponerPrueba(repo, { ...PRUEBA_POR_DEFECTO });
    con321 = [...con321, id];
  }

  function nueva() {
    if (!cfg) return;
    cfg.copias.push({
      id: `copia-${crypto.randomUUID().slice(0, 8)}`,
      nombre: "Nueva copia",
      repo: repos[0]?.id ?? "",
      carpetas: [],
      exclusiones: ["*.tmp", "~$*", "Thumbs.db"],
      horario: { dias: [1, 2, 3, 4, 5], horas: ["13:00"] },
      activa: true,
      gancho: [],
      solo_si_cambios: true,
    });
  }

  function lineas(t: string) {
    return t
      .split(/\r?\n/)
      .map((x) => x.trim())
      .filter(Boolean);
  }

  function frase(k: CopiaConfig) {
    const r = repos.find((x) => x.id === k.repo)?.nombre ?? "sin repositorio";
    if (!k.activa) return "Desactivada: no se hará hasta que la actives.";
    const antes = ganchosDe(k.gancho).map(fraseGancho);
    // Tarea 7c: «después de la anterior» (y, si tiene, también con su horario).
    const anterior = k.tras ? cfg?.copias.find((x) => x.id === k.tras) : undefined;
    const cuando = anterior
      ? `Cuando «${anterior.nombre}» termine bien${reglasDe(k.horario, admiteReglas).length ? ` (y además ${horarioEnFrase(k.horario).toLowerCase()})` : ""}`
      : horarioEnFrase(k.horario);
    return `${cuando} se ${k.carpetas.length === 1 ? "copiará 1 carpeta" : `copiarán ${k.carpetas.length} carpetas`} de ${equipo?.nombre} en «${r}»${antes.length ? `; antes, ${lista(antes)}` : ""}.`;
  }

  // --- Copias en cadena (tarea 7c, docs/copias-en-cadena.md) -------------------
  /** El agente entiende «después de la anterior» (`config.copias[].tras`). */
  const admiteCadenas = $derived(admite(equipo, ADMITE.cadenas));
  /** «Después de» otra copia: sin horario propio (se puede añadir) o sin cadena. */
  function ponerTras(k: CopiaConfig, tras: string) {
    k.tras = tras || null;
    if (!k.tras && !reglasDe(k.horario, admiteReglas).length) k.horario = { dias: [1, 2, 3, 4, 5], horas: ["13:00"] };
  }
  /** «Además, con su horario» en una que va «después de»: sin él, su horario se vacía. */
  function conHorarioPropio(k: CopiaConfig, si: boolean) {
    k.horario = si ? { dias: [1, 2, 3, 4, 5], horas: ["13:00"] } : { dias: [], horas: [] };
  }
  function moverCopia(i: number, paso: -1 | 1) {
    if (cfg) cfg.copias = mover(cfg.copias, i, paso);
  }
  /** La línea de una copia ya enviada (lo que tiene el equipo ahora, de su resumen). */
  const lineaDe = (id: string) => (equipo ? lineaCadena(equipo, id, actual.equipos) : []);

  // --- Verificación automática (v1.28, `config.verificaciones`) ------------------
  // Por repositorio: cada N días, un porcentaje rotativo. Solo con un agente que
  // la entiende (`admite`); con uno anterior no se manda el campo.
  const admiteVerif = $derived(admiteVerificacion(equipo));
  /** v1.40: con un horario de reglas (si no, solo «cada N días»). */
  const admiteVerifHorario = $derived(admiteVerificacionHorario(equipo));
  /** v1.36: la ventana y los avisos del equipo (docs/agente-ventana.md). */
  const admiteEscritorio = $derived(!!equipo?.resumen?.admite?.includes("escritorio"));
  const escritorio = $derived<Escritorio>(cfg?.escritorio ?? { ventana: cfg?.bandeja?.visible === false ? "off" : "siempre_disponible", avisos: cfg?.bandeja?.avisos ? "errores" : "off" });
  function ponerEscritorio(cambio: Partial<Escritorio>) {
    if (!cfg) return;
    cfg.escritorio = { ...escritorio, ...cambio };
  }
  const VENTANAS: [Escritorio["ventana"], string][] = [
    ["off", "Sin ventana"],
    ["siempre_disponible", "Desde el icono"],
    ["al_trabajar", "Al trabajar"],
  ];
  const AVISOS_ESCRITORIO: [Escritorio["avisos"], string][] = [
    ["off", "Ninguno"],
    ["errores", "Errores"],
    ["todo", "Todo"],
  ];
  /** «?verificacion=<repo>»: desde la página del repositorio. */
  const verifPedida = $derived(page.url.searchParams.get("verificacion"));
  let verifVista = false;
  $effect(() => {
    if (!cfg || !verifPedida || verifVista) return;
    verifVista = true;
    queueMicrotask(() => document.getElementById("verificacion")?.scrollIntoView({ block: "start" }));
  });
  function ponerVerif(repo: string, v: VerificacionAuto | null) {
    if (!cfg) return;
    const mapa = { ...(cfg.verificaciones ?? {}) };
    if (v) mapa[repo] = v;
    else delete mapa[repo];
    // Sin ninguna y sin haberla tenido: como estaba (sin el campo).
    if (!Object.keys(mapa).length && !(JSON.parse(original || "{}") as Partial<Configuracion>).verificaciones) delete cfg.verificaciones;
    else cfg.verificaciones = mapa;
  }
  // --- Tarea 8: prueba de restauración automática (`config.pruebas_restauracion`) ---
  const admitePrueba = $derived(admitePruebaAuto(equipo));
  /** «?prueba=<repo>»: desde la regla 3-2-1 de una copia. */
  const pruebaPedida = $derived(page.url.searchParams.get("prueba"));
  let pruebaVista = false;
  $effect(() => {
    if (!cfg || !pruebaPedida || pruebaVista) return;
    pruebaVista = true;
    queueMicrotask(() => document.getElementById("prueba-restauracion")?.scrollIntoView({ block: "start" }));
  });
  function ponerPrueba(repo: string, p: { cada_dias: number } | null) {
    if (!cfg) return;
    const mapa = { ...(cfg.pruebas_restauracion ?? {}) };
    if (p) mapa[repo] = p;
    else delete mapa[repo];
    if (!Object.keys(mapa).length && !(JSON.parse(original || "{}") as Partial<Configuracion>).pruebas_restauracion) delete cfg.pruebas_restauracion;
    else cfg.pruebas_restauracion = mapa;
  }
  /** ¿Tiene alguna copia activa? (La verificación automática va con las copias del agente.) */
  const conCopias = (repo: string) => !!cfg?.copias.some((k) => k.repo === repo && k.activa);

  const problemas = $derived(
    [
      ...Object.entries(cfg?.verificaciones ?? {}).flatMap(([r, v]) => (errorVerificacion(v, admiteVerifHorario) ? [`la verificación de «${repos.find((x) => x.id === r)?.nombre ?? r}»: ${errorVerificacion(v, admiteVerifHorario)!.toLowerCase()}`] : [])),
      ...Object.entries(cfg?.pruebas_restauracion ?? {}).flatMap(([r, p]) => (!Number.isInteger(p.cada_dias) || p.cada_dias < 1 || p.cada_dias > 31 ? [`la prueba de restauración de «${repos.find((x) => x.id === r)?.nombre ?? r}»: de cada día a cada 31 días`] : [])),
    ].concat(
    (cfg?.copias ?? []).flatMap((k) => [
      ...(k.activa && !k.carpetas.length ? [`«${k.nombre}» no tiene carpetas.`] : []),
      ...(k.activa && !k.repo ? [`«${k.nombre}» no tiene repositorio.`] : []),
      ...(k.tras && !admiteCadenas ? [`«${k.nombre}» va «después de» otra copia y el agente de este equipo aún no lo admite`] : []),
      ...(problemaHorario(k) ? [problemaHorario(k)!] : []),
      ...(ganchosDe(k.gancho).length && !admiteGanchos ? [`«${k.nombre}» tiene pasos «Antes de copiar» que este agente aún no admite`] : []),
      ...(ganchosDe(k.gancho).some((g) => errorGancho(g)) ? [`«${k.nombre}» tiene un paso «Antes de copiar» por completar`] : []),
    ])).concat(cfg && errorCadenas(cfg.copias) ? [errorCadenas(cfg.copias)!.replace(/\.$/, "")] : []),
  );

  async function guardar() {
    if (!cfg || !equipo || !actual.cliente || !prueba) return;
    guardando = true;
    error = "";
    try {
      // Solo lo que decide la consola (lib/configEnvio.ts, lo mismo que «Aplicar una plantilla» a varios).
      const config = configParaEnviar($state.snapshot(cfg) as Configuracion, {
        reglas: admiteReglas,
        ganchos: admiteGanchos,
        soloCambios: admiteSoloCambios,
        verif: admiteVerif,
        verifHorario: admiteVerifHorario,
        escritorio: admiteEscritorio,
        pruebas: admitePrueba,
      });
      const o = await mandarOrden({ cliente: actual.cliente, equipo, tipo: "config", cuerpo: { config }, secretos: { prueba }, alPaso: (t) => (paso = t) });
      original = JSON.stringify(cfg);
      avisar(
        o.not_before && Date.parse(o.not_before) > Date.now()
          ? `Enviada (orden n.º ${o.seq}). Deja el equipo sin copias activas: se aplicará ${relativo(o.not_before)}, y hasta entonces se puede cancelar en «Órdenes».`
          : `Enviada al equipo (orden n.º ${o.seq}). Verás su respuesta en la ficha.`,
      );
      void cargarCliente(c, { silencioso: true });
      // A la ficha: allí se ve «Cambios en las copias» en camino hasta que el equipo los aplica.
      await goto(`/c/${c}/equipos/${equipo.id}`);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      guardando = false;
      paso = "";
    }
  }
</script>

<svelte:head><title>Copias de {equipo?.nombre ?? "equipo"} · Resguardo Server</title></svelte:head>

<div class="page">
  <Migas items={[{ texto: "Equipos", href: `/c/${c}/equipos` }, { texto: equipo?.nombre ?? "Equipo", href: `/c/${c}/equipos/${id}` }, { texto: "Cambiar las copias" }]} />
  <div class="page-top">
    <div>
      <h1>Copias de {equipo?.nombre ?? "…"}</h1>
      <p>Qué carpetas se copian, dónde y cuándo. Los cambios se envían al equipo firmados con la clave de administración.</p>
    </div>
    {#if cfg}
      <div class="page-actions">
        {#if cambiado && !guardando}<button class="btn btn-ghost" onclick={cancelarCambios} use:tip={"Vuelve a lo que tiene el equipo ahora"}><Undo2 size={16} />Cancelar cambios</button>{/if}
        <button class="btn btn-primary" disabled={!cambiado || guardando || problemas.length > 0} onclick={guardar}>
          {#if guardando}<LoaderCircle size={16} class="spin" />{paso || "Enviando…"}{:else}<Save size={16} />Enviar al equipo{/if}
        </button>
      </div>
    {/if}
  </div>

  {#if llavesCambiadas && equipo}
    <AlertaLlaves {equipo} cliente={c} />
  {:else if !puede.administrar(actual.cliente?.rol) && actual.cliente}
    <div class="notice notice-info"><p>Tu papel en este cliente no permite cambiar las copias.</p></div>
  {:else if !equipo}
    {#if error}<div class="notice notice-danger"><p>{error}</p></div>{:else}<Cargando />{/if}
  {:else if !cfg}
    <section class="card p desbloquear">
      <form class="form" onsubmit={abrir}>
        <div class="dlg-title">
          <span class="ticon"><LockKeyhole size={18} /></span>
          <div>
            <h2>Escribe la clave de administración</h2>
            <p>Las carpetas de cada copia viajan y se guardan cifradas: solo se ven con la clave de {actual.cliente?.nombre}.</p>
          </div>
        </div>
        <CampoClave requerido id="clave-admin" etiqueta="Clave de administración" bind:value={clave} autofocus>
          {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
        </CampoClave>
        {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
        <div class="fin">
          {#if abriendo}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{paso}</span>{/if}
          <button class="btn btn-primary" disabled={!clave || abriendo}><KeyRound size={16} />Abrir las copias</button>
        </div>
      </form>
    </section>
  {:else}
    {#if plantillaPedida && "nombre" in plantillaPedida}
      <div class="notice notice-info">
        <LayoutTemplate size={16} />
        <p>Abajo tienes una copia nueva rellena con la plantilla «{plantillaPedida.nombre}» de su etiqueta. Revisa las carpetas y el repositorio y pulsa «Enviar al equipo»: no se envía nada hasta entonces.</p>
      </div>
    {:else if plantillaPedida}
      <div class="notice notice-warn">
        <TriangleAlert size={16} />
        <p>La plantilla de su etiqueta ya no existe o no se abre con esta clave. Elige otra abajo («Desde una plantilla») o cámbiala en los ajustes de la etiqueta.</p>
      </div>
    {/if}
    {#if !repos.length}
      <div class="notice notice-warn">
        <TriangleAlert size={16} />
        <p>Este equipo aún no tiene repositorios. <a href="/c/{c}/repositorios">Crea uno</a> y vuelve aquí.</p>
      </div>
    {/if}

    {#each cfg.copias as k, i (k.id)}
      <section class="card p copia" aria-labelledby="t-{k.id}">
        <div class="cab">
          <input id="t-{k.id}" class="input titulo" bind:value={k.nombre} aria-label="Nombre de la copia" />
          {#if cfg.copias.length > 1}
            <span class="orden">
              <button type="button" class="icon-btn" aria-label="Subir «{k.nombre}»" use:tip={"Subir"} disabled={i === 0} onclick={() => moverCopia(i, -1)}><ArrowUp size={14} /></button>
              <button type="button" class="icon-btn" aria-label="Bajar «{k.nombre}»" use:tip={"Bajar"} disabled={i === cfg.copias.length - 1} onclick={() => moverCopia(i, 1)}><ArrowDown size={14} /></button>
            </span>
          {/if}
          <label class="switch-row"><input type="checkbox" class="switch" bind:checked={k.activa} /><span>Activa</span></label>
          <button
            type="button"
            class="btn btn-sm btn-ghost quitar"
            use:tip={guardadas.has(k.id) ? "Deja de hacerse al enviar los cambios" : "Aún no se ha enviado: desaparece sin más"}
            onclick={() => quitar(i)}><Trash2 size={14} />Quitar</button
          >
          <MenuAcciones
            etiqueta="Plantillas para «{k.nombre}»"
            grupos={[
              [{ texto: "Guardar como plantilla…", onclick: () => (guardarComo = { i, nombre: k.nombre, enviando: false, error: "" }) }],
              plantillas.map((p) => ({ texto: `Rellenar con «${p.nombre}»`, onclick: () => aplicarPlantilla(k, p) })),
            ]}
          />
        </div>
        <p class="frase">{frase(k)}</p>
        {#if guardadas.has(k.id) && lineaDe(k.id).length > 2}
          {@const pasos = lineaDe(k.id)}
          <p class="linea-cadena"><Link2 size={13} /><span>{lineaEnTexto(pasos)}</span></p>
          {#if recomendarFueraRetencion(pasos)}<p class="faint nota">{TEXTO_FUERA_RETENCION}</p>{/if}
        {/if}
        {#if k.activa}
          {@const rc = reglaDe(k)}
          {#if rc}
            {@const falta = rc.regla.partes.find((p) => !p.cumple_config)}
            {@const que = falta ? queHacer(falta, rc, c, reloj.ahora) : null}
            <p class="regla-k" class:cumple={rc.regla.cumple_config}>
              <TiraRegla {rc} cliente={c} ahora={reloj.ahora} compacta />
              <span>{fraseConfig(rc.regla)}{#if que}{" "}<span class="faint">{que.texto}</span>{#if que.enlace}{" "}<a class="link" href={que.enlace.href}>{que.enlace.texto} →</a>{/if}{/if}</span>
            </p>
          {/if}
        {/if}
        {#if con321.includes(k.id)}
          <Plantilla321 {equipo} repo={k.repo} cliente={c} equipos={actual.equipos} verificacion={!!cfg.verificaciones?.[k.repo]} prueba={!!cfg.pruebas_restauracion?.[k.repo]} {admitePrueba} onquitar={() => (con321 = con321.filter((x) => x !== k.id))} />
        {/if}
        <!-- v1.40: van aparte (en el servidor, sin la clave): se guardan al momento, no con «Enviar». -->
        {#if guardadas.has(k.id)}<Observaciones tipo="copia" objeto={objetoDe(equipo.id, k.id)} compacto />{/if}
        {#if usadas[k.id]}
          <div class="notice notice-info usada">
            <LayoutTemplate size={16} />
            <p>
              Rellenada con la plantilla «{usadas[k.id].nombre}»: revisa que las carpetas existan en {equipo.nombre} antes de enviar.
              {#if retencionDistinta(k)}La plantilla sugiere guardar {retencionDistinta(k)}; la retención es del repositorio: cámbiala en su ficha («Cambiar la retención», con su contraseña).{/if}
            </p>
          </div>
        {/if}

        <div class="rejilla-2">
          <div class="field">
            <div class="label-row">
              <label class="field-label" for="carp-{k.id}">Carpetas</label>
            </div>
            <textarea id="carp-{k.id}" class="input mono" rows="4" spellcheck="false" value={k.carpetas.join("\n")} oninput={(e) => (k.carpetas = lineas(e.currentTarget.value))} placeholder={/windows/i.test(equipo.so) ? "C:\\Users\\nombre\\Documents" : "/home/nombre"}></textarea>
            <button type="button" class="btn btn-sm elegir" onclick={() => (elegirPara = i)}><FolderOpen size={14} />Elegir en el equipo</button>
          </div>
          <div class="field">
            <label class="field-label" for="exc-{k.id}">No copiar</label>
            <textarea id="exc-{k.id}" class="input mono" rows="4" spellcheck="false" value={k.exclusiones.join("\n")} oninput={(e) => (k.exclusiones = lineas(e.currentTarget.value))}></textarea>
            <span class="field-hint">Una regla por línea: <code>*.tmp</code> deja fuera todos los .tmp y <code>node_modules</code>, las carpetas con ese nombre.</span>
          </div>
        </div>

        <div class="field">
          <span class="field-label">Cuándo</span>
          {#if admiteCadenas && cfg.copias.length > 1}
            <div class="cuando-cadena">
              <label class="field-label sub" for="tras-{k.id}">Empieza</label>
              <select id="tras-{k.id}" class="input" value={k.tras ?? ""} onchange={(e) => ponerTras(k, e.currentTarget.value)}>
                <option value="">Con su horario</option>
                {#each posiblesAnteriores(cfg.copias, k) as a (a.id)}<option value={a.id}>Después de «{a.nombre}»</option>{/each}
              </select>
            </div>
            {#if k.tras}
              <p class="faint nota">Empieza cuando «{cfg.copias.find((x) => x.id === k.tras)?.nombre}» termina bien. Si falla, esta no se hace y se avisa («Cadena parada»).</p>
              <label class="switch-row"><input type="checkbox" class="switch" checked={reglasDe(k.horario, admiteReglas).length > 0} onchange={(e) => conHorarioPropio(k, e.currentTarget.checked)} /><span>Además, con su horario</span></label>
            {/if}
          {/if}
          {#if !k.tras || reglasDe(k.horario, admiteReglas).length > 0}
            <EditorHorario id={k.id} bind:horario={k.horario} {admiteReglas} version={versionAgente} />
          {/if}
        </div>
        {#if admiteSoloCambios}
          <label class="switch-row"
            ><input type="checkbox" class="switch" checked={k.solo_si_cambios !== false} onchange={(e) => (k.solo_si_cambios = e.currentTarget.checked)} /><span
              >Solo guardar si hay cambios<span class="faint">Si lo apagas, cada copia guarda una versión aunque nada haya cambiado.</span></span
            ></label
          >
        {:else}
          <label class="switch-row apagado"
            ><input type="checkbox" class="switch" checked disabled /><span
              >Solo guardar si hay cambios<span class="faint">Solo se guarda una versión nueva si algo cambió. Actualiza el agente para poder apagarlo{versionAgente ? ` (tiene la ${versionAgente})` : ""}.</span></span
            ></label
          >
        {/if}

        <div class="rejilla-2">
          <div class="field">
            <label class="field-label" for="repo-{k.id}">Repositorio</label>
            <select id="repo-{k.id}" class="input" bind:value={k.repo}>
              {#each repos as r (r.id)}<option value={r.id}>{r.nombre}</option>{/each}
            </select>
            {#if lugarDeRepo(k.repo)}{@const x = lugarDeRepo(k.repo)!}<span class="field-hint"><SeGuardaEn pequeno lugar={x.lugar} riesgo={x.riesgo} /></span>{/if}
          </div>
        </div>
        <EditorGanchos id={k.id} bind:ganchos={() => ganchosDe(k.gancho), (v) => (k.gancho = v)} admite={admiteGanchos} version={versionAgente} cliente={actual.cliente ?? undefined} {equipo} prueba={prueba ?? undefined} />
        {#if !k.activa}
          <p class="faint nota"><Trash2 size={13} />Para quitarla del todo y dejar de proteger esas carpetas, usa «Dejar de copiar» en el repositorio (espera {actual.cliente?.espera_min_horas} h).</p>
        {/if}
      </section>
    {:else}
      <div class="card"><Vacio icono={FolderOpen} titulo="Sin copias todavía" texto="Crea la primera: elige carpetas, el repositorio y el horario." /></div>
    {/each}

    <div class="nueva-fila">
      <button class="btn nueva" onclick={nueva} disabled={!repos.length}><Plus size={16} />Añadir una copia</button>
      <button class="btn btn-ghost" onclick={nueva321} disabled={!repos.length} use:tip={"Una copia cada día al almacén, con verificación semanal y prueba de restauración mensual; y dice dónde añadir el espejo a otro disco y la copia en la nube"}><ShieldCheck size={16} />Con la plantilla 3-2-1</button>
      {#if plantillas.length && repos.length}
        <MenuAcciones texto="Desde una plantilla" etiqueta="Añadir una copia desde una plantilla" grupos={[plantillas.map((p) => ({ texto: p.nombre, onclick: () => nuevaDesde(p) }))]} />
      {/if}
      <button class="btn btn-ghost btn-sm" onclick={() => (gestionar = true)}><LayoutTemplate size={14} />Plantillas{plantillas.length ? ` · ${plantillas.length}` : ""}</button>
    </div>

    {#if repos.length}
      <section class="card p verif" id="verificacion" aria-labelledby="t-verif">
        <h2 class="section-title" id="t-verif"><ShieldCheck size={16} />Verificación automática <Ayuda id="prot-verificacion" /></h2>
        {#if !admiteVerif}
          <p class="faint">Actualiza el agente de {equipo.nombre}{versionAgente ? ` (tiene la ${versionAgente})` : ""} para programarla desde aquí: cada N días, un porcentaje de los datos que va rotando. Mientras, «Verificar» en la página de cada repositorio la hace a mano.</p>
        {:else}
          <p class="faint">{equipo.nombre} comprueba el repositorio y lee una parte de sus datos cada vez, rotando: con un 10 %, en 10 verificaciones ha leído todo. Cada N días, la primera a las 03:00 siguientes{admiteVerifHorario ? "; o con un horario, como el de las copias" : ""}. Mientras verifica, las copias de ese repositorio esperan.</p>
          {#each repos as r (r.id)}
            {@const va = cfg.verificaciones?.[r.id]}
            <div class="verif-fila" class:resaltada={r.id === verifPedida}>
              <label class="switch-row"><input type="checkbox" class="switch" checked={!!va} onchange={(e) => ponerVerif(r.id, e.currentTarget.checked ? { ...VERIFICACION_POR_DEFECTO } : null)} /><span>{r.nombre}</span></label>
              {#if va}
                <EditorVerificacion id={r.id} nombre={r.nombre} valor={va} admiteHorario={admiteVerifHorario} onchange={(v) => ponerVerif(r.id, v)} />
                {#if !conCopias(r.id)}<span class="faint frase-verif">Se pondrá cuando el repositorio tenga alguna copia activa.</span>{/if}
              {:else}
                <span class="faint frase-verif">Sin verificación automática.</span>
              {/if}
            </div>
          {/each}
        {/if}
      </section>

      <!-- Tarea 8: prueba de restauración automática (el «0» de la regla 3-2-1-1-0). -->
      <section class="card p verif" id="prueba-restauracion" aria-labelledby="t-prueba">
        <h2 class="section-title" id="t-prueba"><FlaskConical size={16} />Prueba de restauración automática <Ayuda id="regla-321" /></h2>
        {#if !admitePrueba}
          <p class="faint">Actualiza el agente de {equipo.nombre}{versionAgente ? ` (tiene la ${versionAgente})` : ""} para programarla desde aquí. Mientras, «Probar la restauración» en su ficha la hace a mano (conviene cada mes).</p>
        {:else}
          <p class="faint">{equipo.nombre} restaura unos archivos al azar de la última versión a una carpeta temporal y los compara con lo guardado. No toca tus archivos. La primera, a las 04:00 siguientes.</p>
          {#each repos as r (r.id)}
            {@const pr = cfg.pruebas_restauracion?.[r.id]}
            <div class="verif-fila" class:resaltada={r.id === pruebaPedida}>
              <label class="switch-row"><input type="checkbox" class="switch" checked={!!pr} onchange={(e) => ponerPrueba(r.id, e.currentTarget.checked ? { ...PRUEBA_POR_DEFECTO } : null)} /><span>{r.nombre}</span></label>
              {#if pr}
                <label class="cada-dias">Cada <input class="input num" type="number" min="1" max="31" value={pr.cada_dias} oninput={(e) => ponerPrueba(r.id, { cada_dias: Number(e.currentTarget.value) })} aria-label="Cada cuántos días, la prueba de «{r.nombre}»" /> días</label>
                {#if !conCopias(r.id)}<span class="faint frase-verif">Se pondrá cuando el repositorio tenga alguna copia activa.</span>{/if}
              {:else}
                <span class="faint frase-verif">Sin prueba automática.</span>
              {/if}
            </div>
          {/each}
        {/if}
      </section>
    {/if}

    <section class="card p verif" id="escritorio" aria-labelledby="t-escritorio">
      <h2 class="section-title" id="t-escritorio"><MonitorCheck size={16} />En el equipo</h2>
      {#if !admiteEscritorio}
        <p class="faint">Actualiza el agente de {equipo.nombre}{versionAgente ? ` (tiene la ${versionAgente})` : ""} para elegir desde aquí su ventana y sus avisos.</p>
      {:else}
        <p class="faint">
          Lo que ve quien usa {equipo.nombre}: una ventana pequeña con el progreso en vivo y avisos de Windows. También se puede cambiar en el propio equipo, con la clave de administración.
          {#if cfg.cambiado_en_equipo}<span class="chip-equipo">Cambiado en el equipo {relativo(cfg.cambiado_en_equipo)}</span>{/if}
        </p>
        <div class="field">
          <span class="field-label" id="l-esc-ventana">Ventana</span>
          <div class="segmented inline" role="radiogroup" aria-labelledby="l-esc-ventana">
            {#each VENTANAS as [v, t] (v)}<button type="button" role="radio" aria-checked={escritorio.ventana === v} class:on={escritorio.ventana === v} onclick={() => ponerEscritorio({ ventana: v })}>{t}</button>{/each}
          </div>
          <span class="faint">«Al trabajar»: se abre sola al empezar una copia, una restauración, una verificación o una subida.</span>
        </div>
        <div class="field">
          <span class="field-label" id="l-esc-avisos">Avisos</span>
          <div class="segmented inline" role="radiogroup" aria-labelledby="l-esc-avisos">
            {#each AVISOS_ESCRITORIO as [v, t] (v)}<button type="button" role="radio" aria-checked={escritorio.avisos === v} class:on={escritorio.avisos === v} onclick={() => ponerEscritorio({ avisos: v })}>{t}</button>{/each}
          </div>
          <span class="faint">«Errores»: al fallar y al recuperarse. «Todo»: también al empezar y al terminar.</span>
        </div>
      {/if}
    </section>

    {#if quitadas.length}
      <div class="notice notice-info"><Trash2 size={16} /><p>Al enviar, dejará{quitadas.length === 1 ? "" : "n"} de hacerse {lista(quitadas.map((k) => `«${k.nombre}»`))}. Lo ya guardado sigue en su repositorio. ¿Fue sin querer? «Cancelar cambios» lo deja como estaba.</p></div>
    {/if}
    {#if problemas.length}
      <div class="notice notice-warn"><TriangleAlert size={16} /><p>Antes de enviar: {lista(problemas)}</p></div>
    {:else if cambiado}
      <div class="notice notice-info"><p>Hay cambios sin enviar: {plural(cfg.copias.length, "copia", "copias")} en total. Pulsa «Enviar al equipo».</p></div>
    {/if}
    {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
  {/if}
</div>

{#if guardarComo}
  <Modal labelledby="t-guardar-pla" onclose={() => (guardarComo = null)} width={460}>
    <form class="form" onsubmit={guardarPlantillaDe}>
      <div class="dlg-title">
        <span class="ticon"><LayoutTemplate size={18} /></span>
        <div>
          <h2 id="t-guardar-pla">Guardar como plantilla</h2>
          <p>Carpetas, exclusiones, horario, «solo si hay cambios» y «Antes de copiar», para usarlos en otros equipos de {actual.cliente?.nombre}.</p>
        </div>
      </div>
      <div class="field">
        <label class="field-label" for="pla-nombre">Nombre de la plantilla</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input id="pla-nombre" class="input" bind:value={guardarComo.nombre} maxlength="80" autofocus placeholder="Por ejemplo: Copia de Siigo" aria-required="true" aria-invalid={!!guardarComo.error} aria-describedby={guardarComo.error ? "pla-nombre-error" : undefined} />
      </div>
      <p class="faint nota">Se guarda cifrada con la clave de administración: el servidor no ve ni el nombre ni las carpetas. Usarla en un equipo solo rellena sus copias; se revisan y se envían como siempre.</p>
      {#if guardarComo.error}<p class="error-campo" id="pla-nombre-error" role="alert">{guardarComo.error}</p>{/if}
      <footer>
        <button type="button" class="btn btn-ghost" onclick={() => (guardarComo = null)}>Cancelar</button>
        <BotonCargando class="btn btn-primary" disabled={!guardarComo.nombre.trim()} cargando={guardarComo.enviando} textoCargando="Guardando…">Guardar plantilla</BotonCargando>
      </footer>
    </form>
  </Modal>
{/if}

{#if gestionar}
  <Modal labelledby="t-plantillas" onclose={() => ((gestionar = false), (borrandoPlantilla = null))} width={560}>
    <div class="dlg-title">
      <span class="ticon"><LayoutTemplate size={18} /></span>
      <div>
        <h2 id="t-plantillas">Plantillas de {actual.cliente?.nombre}</h2>
        <p>Configuraciones de copia para reutilizar en cualquier equipo del cliente. Se guardan cifradas.</p>
      </div>
    </div>
    {#if plantillas.length}
      <ul class="lista-pla">
        {#each plantillas as p (p.id)}
          <li>
            <span class="pla-texto">
              <strong>{p.nombre}</strong>
              <span class="faint">{plural(p.copia.carpetas.length, "carpeta", "carpetas")} · {resumenHorario(p.copia.horario)}{p.copia.gancho?.length ? ` · ${plural(p.copia.gancho.length, "paso antes de copiar", "pasos antes de copiar")}` : ""}</span>
              {#if p.por}<span class="faint">Guardada por {p.por} · {relativo(p.creada)}</span>{/if}
            </span>
            {#if borrandoPlantilla === p.id}
              <span class="confirmar">¿Borrarla? <button class="btn btn-sm btn-danger" onclick={() => borrarPlantillaGuardada(p)}>Borrar</button><button class="btn btn-sm btn-ghost" onclick={() => (borrandoPlantilla = null)}>No</button></span>
            {:else}
              <button class="icon-btn" aria-label="Borrar la plantilla «{p.nombre}»" use:tip={"Borrar"} onclick={() => (borrandoPlantilla = p.id)}><Trash2 size={14} /></button>
            {/if}
          </li>
        {/each}
      </ul>
    {:else}
      <p class="faint">Todavía no hay plantillas. En una copia, «Guardar como plantilla…» (en su menú) guarda su configuración para usarla en otros equipos.</p>
    {/if}
    {#if ilegibles}<p class="faint nota">{plural(ilegibles, "plantilla no se abre", "plantillas no se abren")} con esta clave (se guardaron con otra clave de administración).</p>{/if}
    <footer><button class="btn btn-primary" onclick={() => ((gestionar = false), (borrandoPlantilla = null))}>Cerrar</button></footer>
  </Modal>
{/if}

{#if elegirPara !== null && equipo && actual.cliente && prueba && cfg}
  <ElegirCarpetas
    cliente={actual.cliente}
    {equipo}
    {prueba}
    iniciales={cfg.copias[elegirPara].carpetas}
    onclose={() => (elegirPara = null)}
    alElegir={(rutas, gancho) => {
      const k = cfg!.copias[elegirPara!];
      k.carpetas = rutas;
      void gancho;
      elegirPara = null;
    }}
  />
{/if}

<style>
  .desbloquear {
    max-width: 560px;
  }
  .fin {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 8px;
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .copia {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .cab {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--sp-2) var(--sp-4);
  }
  .titulo {
    flex: 1 1 220px;
    min-width: 0;
    max-width: 360px;
    font-weight: 600;
    font-size: var(--fs-h2);
  }
  .frase {
    margin: -6px 0 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .rejilla-2 {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-4);
  }
  textarea.mono {
    font-family: var(--mono);
    font-size: 12.5px;
  }
  .elegir {
    align-self: flex-start;
  }
  .quitar {
    margin-left: auto;
  }
  .orden {
    display: inline-flex;
    gap: 2px;
  }
  .cuando-cadena {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }
  .cuando-cadena .sub {
    margin: 0;
  }
  .cuando-cadena select {
    width: auto;
    min-width: 0;
    max-width: 100%;
  }
  .linea-cadena {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: -8px 0 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
    overflow-wrap: anywhere;
  }
  .linea-cadena :global(svg) {
    flex: none;
    margin-top: 3px;
  }
  .usada {
    margin-top: -6px;
  }
  .nueva-fila {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .verif {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    scroll-margin-top: var(--sp-6, 24px);
  }
  .verif h2 {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
  }
  .verif p {
    margin: 0;
  }
  .chip-equipo {
    display: inline-block;
    margin-left: 6px;
    padding: 0 8px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent-text);
    font-size: var(--fs-xs);
  }
  .verif-fila {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px var(--sp-3);
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .verif-fila.resaltada {
    border-color: var(--accent, var(--text-1));
  }
  .frase-verif {
    flex-basis: 100%;
    font-size: var(--fs-sm);
  }
  /* Tarea 8: la prueba de restauración (cada N días) y la regla 3-2-1 de cada copia. */
  .cada-dias {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .cada-dias .input {
    width: 4.5em;
  }
  .regla-k {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-2);
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .regla-k.cumple {
    color: var(--text-1);
  }
  .lista-pla {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .lista-pla li {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: 10px var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .lista-pla li:first-child {
    border-top: none;
  }
  .pla-texto {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
    font-size: var(--fs-sm);
  }
  .confirmar {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .apagado {
    opacity: 0.85;
  }
  .nueva {
    align-self: flex-start;
  }
  .nota {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-xs);
  }
  @media (max-width: 760px) {
    .rejilla-2 {
      grid-template-columns: 1fr;
    }
    .cab {
      flex-wrap: wrap;
    }
  }
</style>
