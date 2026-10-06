<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Editor de copias de un equipo (orden «config», con la clave de administración).
  //
  // Las carpetas y exclusiones van cifradas con K_cfg: para verlas y cambiarlas
  // hace falta la clave de administración. Al abrir se calculan K_cfg (para
  // descifrar) y la prueba del equipo (para la orden), se comprueba la etiqueta
  // y la clave se olvida. Todo se borra al salir.
  import Migas from "$lib/componentes/Migas.svelte";
  import { onDestroy, tick, untrack } from "svelte";
  import { seguirCambios, tocaEquipo } from "$lib/vivo.svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { ArrowDown, ArrowUp, CalendarClock, ChevronDown, CloudUpload, CornerDownRight, Database, FlaskConical, FolderOpen, GitBranch, GripVertical, HardDrive, KeyRound, LayoutTemplate, Link2, LoaderCircle, LockKeyhole, MonitorCheck, Plus, Save, ShieldCheck, Trash2, TriangleAlert, Undo2 } from "@lucide/svelte";
  import { ADMITE, admite, despuesDeLaAnterior, destinosParaPasos, errorCadenas, moverA, pasosDelRepo, recomendarFueraRetencion, reenlazar, TEXTO_FUERA_RETENCION } from "$lib/cadenas";
  import PasosRepo from "$lib/componentes/PasosRepo.svelte";
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
  import { claveDeDestino } from "$lib/fichaDestino";
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
      // «Copia nueva» desde otra página (?nueva=1, y ?tras=<copia>: justo debajo de esa, después de ella).
      if (page.url.searchParams.get("nueva") === "1") {
        const tras = page.url.searchParams.get("tras");
        const i = tras ? cfg.copias.findIndex((x) => x.id === tras) : -1;
        if (i >= 0) nuevaDespues(i);
        else nueva();
        // ?destino=<clave> (desde «Usar en una copia» de la página de un destino): un repositorio de este equipo allí.
        const dest = page.url.searchParams.get("destino");
        const enDestino = dest ? equipo.resumen?.repositorios?.find((r) => {
          const d = destinoDe(equipo!.resumen?.destinos, r);
          return !!d && claveDeDestino(d, actual.equipos) === dest;
        }) : undefined;
        const k = i >= 0 ? cfg.copias[i + 1] : cfg.copias.at(-1);
        if (k && enDestino) k.repo = enDestino.id;
      }
      // ?copia=<id>: desde la página de la copia, con su tarjeta abierta.
      const pedida = page.url.searchParams.get("copia");
      if (pedida && cfg.copias.some((x) => x.id === pedida)) {
        abiertas[pedida] = true;
        void tick().then(() => document.getElementById(`copia-${pedida}`)?.scrollIntoView({ block: "start" }));
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

  /** Una copia nueva justo debajo de la `i`, «después de la anterior» si el agente lo admite. */
  function nuevaDespues(i: number) {
    if (!cfg) return;
    const anterior = cfg.copias[i];
    nueva();
    const k = cfg.copias.pop()!;
    if (admiteCadenas && anterior) {
      k.tras = anterior.id;
      k.horario = { dias: [], horas: [] };
    }
    cfg.copias.splice(i + 1, 0, k);
    abiertas[k.id] = true;
    void tick().then(() => document.getElementById(`copia-${k.id}`)?.scrollIntoView({ block: "center", behavior: "smooth" }));
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
  // --- Ordenar (docs/editor-de-copias.md): asa para arrastrar, flechas y menú ---
  /** Lo que se dice al lector de pantalla al mover. */
  let anuncio = $state("");
  /** Aviso tranquilo cuando una copia con «después» llega al primer puesto. */
  let avisoOrden = $state("");
  /** Lleva la copia `de` a la posición `a`; «después de la anterior» sigue a la de encima. */
  async function moverCopiaA(de: number, a: number, enfocar = false) {
    if (!cfg || de === a || a < 0 || a >= cfg.copias.length) return;
    const antes = $state.snapshot(cfg.copias) as CopiaConfig[];
    const r = reenlazar(antes, moverA(antes, de, a));
    for (const k of r.copias) if (r.aHorario.includes(k.id) && !reglasDe(k.horario, admiteReglas).length) k.horario = { dias: [1, 2, 3, 4, 5], horas: ["13:00"] };
    cfg.copias = r.copias;
    const k = r.copias[a];
    avisoOrden = r.aHorario.length ? `«${r.copias.find((x) => x.id === r.aHorario[0])?.nombre}» es ahora la primera: empieza con su horario.` : "";
    anuncio = `«${k.nombre}», posición ${a + 1} de ${r.copias.length}.${avisoOrden ? ` ${avisoOrden}` : ""}`;
    if (enfocar) {
      await tick();
      document.querySelector<HTMLElement>(`[data-asa="${k.id}"]`)?.focus();
    }
  }
  function teclaAsa(e: KeyboardEvent, i: number) {
    if (!cfg) return;
    const a = e.key === "ArrowUp" ? i - 1 : e.key === "ArrowDown" ? i + 1 : e.key === "Home" ? 0 : e.key === "End" ? cfg.copias.length - 1 : null;
    if (a === null) return;
    e.preventDefault();
    void moverCopiaA(i, a, true);
  }
  // Arrastrar con el ratón o el dedo (eventos de puntero: el arrastre de HTML no va en el móvil).
  let flujo = $state<HTMLDivElement>();
  let arrastre = $state<{ de: number; a: number; y0: number; dy: number; id: number } | null>(null);
  function empezarArrastre(e: PointerEvent, i: number) {
    if (e.button !== 0) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    arrastre = { de: i, a: i, y0: e.clientY, dy: 0, id: e.pointerId };
  }
  function moverArrastre(e: PointerEvent) {
    if (!arrastre || e.pointerId !== arrastre.id || !flujo) return;
    const ar = arrastre;
    ar.dy = e.clientY - ar.y0;
    const tarjetas = [...flujo.querySelectorAll<HTMLElement>("[data-copia]")];
    let a = ar.de;
    tarjetas.forEach((t, j) => {
      const r = t.getBoundingClientRect();
      const medio = r.top + Math.min(r.height, 160) / 2;
      if (j < ar.de && e.clientY < medio) a = Math.min(a, j);
      if (j > ar.de && e.clientY > medio) a = Math.max(a, j);
    });
    ar.a = a;
    // Cerca del borde de la ventana, la página se desplaza sola.
    if (e.clientY < 56) window.scrollBy(0, -14);
    else if (e.clientY > window.innerHeight - 56) window.scrollBy(0, 14);
  }
  function soltar(e: PointerEvent) {
    if (!arrastre || e.pointerId !== arrastre.id) return;
    const { de, a } = arrastre;
    arrastre = null;
    if (a !== de) void moverCopiaA(de, a, true);
  }

  // --- Tarjetas plegables ---------------------------------------------------------
  /** Abiertas o cerradas a mano; sin tocar: abierta si es nueva o es la única. */
  let abiertas = $state<Record<string, boolean>>({});
  const abierta = (k: CopiaConfig) => abiertas[k.id] ?? (!guardadas.has(k.id) || (cfg?.copias.length ?? 0) === 1);
  /** Abre la tarjeta y lleva a sus ajustes del repositorio (verificación, prueba). */
  async function irAAjustes(k: CopiaConfig) {
    abiertas[k.id] = true;
    await tick();
    document.getElementById(`ajustes-${k.id}`)?.scrollIntoView({ block: "center", behavior: "smooth" });
  }
  /** Lo que cuelga del repositorio de una copia (su destino, espejos, copia externa y derivadas). */
  const pasosDe = (k: CopiaConfig) => (equipo && k.repo ? pasosDelRepo(equipo, k.repo, actual.equipos) : []);
  /** El repositorio de una copia tal como lo cuenta el equipo (para sus diálogos). */
  const repoResumen = (id: string) => equipo?.resumen?.repositorios?.find((r) => r.id === id) ?? null;
  /** Otras copias que guardan en el mismo repositorio (comparten retención, verificación y prueba). */
  const tambien = (k: CopiaConfig) => (cfg?.copias ?? []).filter((x) => x.id !== k.id && x.repo === k.repo);
  /** Las nubes del cliente (para decir en el repositorio de una copia que directo todavía no). */
  const nubesCliente = $derived(destinosParaPasos(actual.equipos, catalogo).filter((v) => v.clase === "nube"));
  /** Repositorios que no usa ninguna copia (su verificación y su prueba van al final). */
  const reposSinCopias = $derived(repos.filter((r) => !cfg?.copias.some((k) => k.repo === r.id)));

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
    const k = cfg.copias.find((x) => x.repo === verifPedida);
    untrack(() => (k ? void irAAjustes(k) : queueMicrotask(() => document.getElementById("verificacion")?.scrollIntoView({ block: "start" }))));
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
    const k = cfg.copias.find((x) => x.repo === pruebaPedida);
    untrack(() => (k ? void irAAjustes(k) : queueMicrotask(() => document.getElementById("verificacion")?.scrollIntoView({ block: "start" }))));
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
  <Migas items={[{ texto: "Equipos", href: `/c/${c}/equipos` }, { texto: equipo?.nombre ?? "Equipo", href: `/c/${c}/equipos/${id}` }, { texto: "Copias" }]} />
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

    <p class="sr-only" aria-live="polite">{anuncio}</p>
    {#if avisoOrden}
      <div class="notice notice-info aviso-orden" role="status">
        <CalendarClock size={16} />
        <p>{avisoOrden}</p>
        <button type="button" class="btn btn-sm btn-ghost" onclick={() => (avisoOrden = "")}>Entendido</button>
      </div>
    {/if}

    <!-- Las copias como un flujo: numeradas, con asa para ordenar y una flecha «después» entre las encadenadas. -->
    <div class="flujo" bind:this={flujo} class:arrastrando={!!arrastre}>
      {#each cfg.copias as k, i (k.id)}
        {@const enCadena = despuesDeLaAnterior(cfg.copias, i)}
        {@const otraAnterior = k.tras && !enCadena ? cfg.copias.find((x) => x.id === k.tras) : undefined}
        {@const abiertaK = abierta(k)}
        {@const pasos = pasosDe(k)}
        {@const rr = repoResumen(k.repo)}
        {#if i > 0}
          <div class="union" class:enlazada={enCadena} aria-hidden="true">
            {#if enCadena}<span class="union-txt"><ArrowDown size={13} />después</span>{/if}
          </div>
        {/if}
        <section
          class="card copia"
          class:inactiva={!k.activa}
          class:levantada={arrastre?.de === i}
          class:cae-arriba={arrastre && arrastre.a === i && arrastre.a < arrastre.de}
          class:cae-abajo={arrastre && arrastre.a === i && arrastre.a > arrastre.de}
          style:transform={arrastre?.de === i ? `translateY(${arrastre.dy}px)` : undefined}
          id="copia-{k.id}"
          data-copia={k.id}
          aria-labelledby="t-{k.id}"
        >
          <div class="lado">
            <span class="num" aria-hidden="true">{i + 1}</span>
            {#if cfg.copias.length > 1}
              <button
                type="button"
                class="asa"
                data-asa={k.id}
                aria-label="Mover «{k.nombre}», posición {i + 1} de {cfg.copias.length}. Usa las flechas arriba y abajo."
                use:tip={"Arrastra para ordenar (o usa las flechas del teclado)"}
                onpointerdown={(e) => empezarArrastre(e, i)}
                onpointermove={moverArrastre}
                onpointerup={soltar}
                onpointercancel={() => (arrastre = null)}
                onkeydown={(e) => teclaAsa(e, i)}><GripVertical size={18} /></button
              >
            {/if}
          </div>

          <div class="cuerpo">
            <div class="cab">
              <input id="t-{k.id}" class="input titulo" bind:value={k.nombre} aria-label="Nombre de la copia" />
              {#if !guardadas.has(k.id)}<span class="badge badge-sm tone-accent">Nueva</span>{/if}
              <label class="switch-row activa"><input type="checkbox" class="switch" bind:checked={k.activa} /><span>Activa</span></label>
              <button type="button" class="icon-btn plegar" class:girado={abiertaK} aria-expanded={abiertaK} aria-controls="form-{k.id}" aria-label={abiertaK ? `Cerrar «${k.nombre}»` : `Abrir «${k.nombre}»`} use:tip={abiertaK ? "Cerrar" : "Carpetas, horario y más"} onclick={() => (abiertas[k.id] = !abiertaK)}><ChevronDown size={16} /></button>
              <MenuAcciones
                etiqueta="Más de «{k.nombre}»"
                texto=""
                grupos={[
                  [
                    { texto: "Subir", icono: ArrowUp, disabled: i === 0, onclick: () => void moverCopiaA(i, i - 1) },
                    { texto: "Bajar", icono: ArrowDown, disabled: i === cfg!.copias.length - 1, onclick: () => void moverCopiaA(i, i + 1) },
                  ],
                  [{ texto: "Guardar como plantilla…", icono: LayoutTemplate, onclick: () => (guardarComo = { i, nombre: k.nombre, enviando: false, error: "" }) }, ...plantillas.map((p) => ({ texto: `Rellenar con «${p.nombre}»`, onclick: () => aplicarPlantilla(k, p) }))],
                  [{ texto: guardadas.has(k.id) ? "Quitar (deja de hacerse al enviar)" : "Quitar", icono: Trash2, peligro: true, onclick: () => quitar(i) }],
                ]}
              />
            </div>

            <!-- Cuándo empieza: la primera, siempre con su horario. -->
            <div class="cuando-k">
              <div class="segmented inline" role="radiogroup" aria-label="Cuándo empieza «{k.nombre}»">
                <button type="button" role="radio" aria-checked={!k.tras} class:on={!k.tras} onclick={() => ponerTras(k, "")}><CalendarClock size={14} />Con horario</button>
                <button
                  type="button"
                  role="radio"
                  aria-checked={enCadena}
                  class:on={enCadena}
                  disabled={i === 0 || !admiteCadenas}
                  use:tip={i === 0 ? "La primera empieza con su horario" : !admiteCadenas ? `Actualiza el agente de ${equipo.nombre} para encadenar copias` : "Empieza cuando la de arriba termina bien"}
                  onclick={() => ponerTras(k, cfg!.copias[i - 1].id)}><Link2 size={14} />Después de la anterior</button
                >
                {#if otraAnterior}<button type="button" role="radio" aria-checked="true" class="on">Después de «{otraAnterior.nombre}»</button>{/if}
              </div>
              <span class="faint cuando-txt">
                {#if !k.activa}Desactivada{:else if k.tras}{reglasDe(k.horario, admiteReglas).length ? `y además ${horarioEnFrase(k.horario).toLowerCase()}` : "Si la anterior falla, esta no se hace y se avisa"}{:else}{horarioEnFrase(k.horario)}{/if}
              </span>
            </div>

            <!-- Su camino: carpetas → repositorio en su destino, y lo que cuelga de él. -->
            <ol class="camino" aria-label="Camino de «{k.nombre}»">
              <li><FolderOpen size={14} />{plural(k.carpetas.length, "carpeta", "carpetas")}</li>
              <li class="flecha" aria-hidden="true">→</li>
              <li><Database size={14} /><strong>{repos.find((x) => x.id === k.repo)?.nombre ?? "Sin repositorio"}</strong>{#if pasos[0]}<span class="faint">· {pasos[0].texto}</span>{/if}</li>
            </ol>
            {#if pasos.length > 1}
              <ul class="ramas" aria-label="Lo que se copia además desde «{repos.find((x) => x.id === k.repo)?.nombre ?? k.repo}»">
                {#each pasos.slice(1) as p, pi (pi)}
                  <li use:tip={p.detalle + (p.noInmutable ? ` · ${p.noInmutable}` : "")}>
                    <CornerDownRight size={13} />
                    <span class="rama-t">{#if p.clase === "espejo"}<HardDrive size={13} />Espejo{:else if p.detalle.startsWith("copia externa")}<CloudUpload size={13} />Copia externa{:else}<GitBranch size={13} />Repositorio derivado{/if}</span>
                    <span class="faint rama-d">{p.texto}{p.despues ? " · después de cada copia" : ""}</span>
                  </li>
                {/each}
              </ul>
              {#if recomendarFueraRetencion([{ clase: "origen", texto: "", detalle: "", despues: false, nivel: 0, fueraRetencion: false }, ...pasos])}<p class="faint nota">{TEXTO_FUERA_RETENCION}</p>{/if}
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
            {#if usadas[k.id]}
              <div class="notice notice-info usada">
                <LayoutTemplate size={16} />
                <p>
                  Rellenada con la plantilla «{usadas[k.id].nombre}»: revisa que las carpetas existan en {equipo.nombre} antes de enviar.
                  {#if retencionDistinta(k)}La plantilla sugiere guardar {retencionDistinta(k)}; la retención es del repositorio: cámbiala con «Retención».{/if}
                </p>
              </div>
            {/if}

            {#if abiertaK}
              <div class="form-k" id="form-{k.id}">
                <!-- v1.40: van aparte (en el servidor, sin la clave): se guardan al momento, no con «Enviar». -->
                {#if guardadas.has(k.id)}<Observaciones tipo="copia" objeto={objetoDe(equipo.id, k.id)} compacto />{/if}
                <div class="rejilla-2">
                  <div class="field">
                    <label class="field-label" for="carp-{k.id}">Carpetas</label>
                    <textarea id="carp-{k.id}" class="input mono" rows="4" spellcheck="false" value={k.carpetas.join("\n")} oninput={(e) => (k.carpetas = lineas(e.currentTarget.value))} placeholder={/windows/i.test(equipo.so) ? "C:\\Users\\nombre\\Documents" : "/home/nombre"}></textarea>
                    <button type="button" class="btn btn-sm elegir" onclick={() => (elegirPara = i)}><FolderOpen size={14} />Elegir en el equipo</button>
                  </div>
                  <div class="field">
                    <label class="field-label" for="exc-{k.id}">No copiar</label>
                    <textarea id="exc-{k.id}" class="input mono" rows="4" spellcheck="false" value={k.exclusiones.join("\n")} oninput={(e) => (k.exclusiones = lineas(e.currentTarget.value))}></textarea>
                    <span class="field-hint">Una regla por línea: <code>*.tmp</code> deja fuera todos los .tmp y <code>node_modules</code>, las carpetas con ese nombre.</span>
                  </div>
                </div>

                {#if true}
                  <div class="field">
                    <span class="field-label">Horario</span>
                    {#if k.tras}
                      <label class="switch-row"><input type="checkbox" class="switch" checked={reglasDe(k.horario, admiteReglas).length > 0} onchange={(e) => conHorarioPropio(k, e.currentTarget.checked)} /><span>Además, con su horario</span></label>
                    {/if}
                    {#if !k.tras || reglasDe(k.horario, admiteReglas).length > 0}
                      <EditorHorario id={k.id} bind:horario={k.horario} {admiteReglas} version={versionAgente} />
                    {/if}
                  </div>
                {/if}
                {#if admiteSoloCambios}
                  <label class="switch-row"
                    ><input type="checkbox" class="switch" checked={k.solo_si_cambios !== false} onchange={(e) => (k.solo_si_cambios = e.currentTarget.checked)} /><span
                      >Solo guardar si hay cambios<span class="faint">Si lo apagas, cada copia guarda una versión aunque nada haya cambiado.</span></span
                    ></label
                  >
                {:else}
                  <label class="switch-row apagado"
                    ><input type="checkbox" class="switch" checked disabled /><span
                      >Solo guardar si hay cambios<span class="faint">Actualiza el agente para poder apagarlo{versionAgente ? ` (tiene la ${versionAgente})` : ""}.</span></span
                    ></label
                  >
                {/if}

                <div class="repo-k" id="ajustes-{k.id}">
                  <div class="field">
                    <label class="field-label" for="repo-{k.id}">Repositorio</label>
                    <select id="repo-{k.id}" class="input" bind:value={k.repo}>
                      {#each repos as r (r.id)}<option value={r.id}>{r.nombre}</option>{/each}
                      {#if nubesCliente.length}
                        <!-- 4a pendiente: carpetas directas a una nube, todavía no. Se ven, desactivadas, con el camino que sí. -->
                        <optgroup label="Nubes: copia aquí y después «Repositorio nuevo a partir de esta»">
                          {#each nubesCliente as n (n.clave)}<option disabled value="">{n.nombre} · directo, todavía no</option>{/each}
                        </optgroup>
                      {/if}
                    </select>
                    {#if lugarDeRepo(k.repo)}{@const x = lugarDeRepo(k.repo)!}<span class="field-hint"><SeGuardaEn pequeno lugar={x.lugar} riesgo={x.riesgo} /></span>{/if}
                    {#if tambien(k).length}<span class="field-hint">También guarda{tambien(k).length === 1 ? "" : "n"} aquí {lista(tambien(k).map((x) => `«${x.nombre}»`))}: la verificación y la prueba son del repositorio.</span>{/if}
                  </div>
                  {#if k.repo}
                    {@const rk = repos.find((x) => x.id === k.repo)}
                    {#if rk}{@render ajustesRepo(rk, k.id)}{/if}
                  {/if}
                </div>
                <EditorGanchos id={k.id} bind:ganchos={() => ganchosDe(k.gancho), (v) => (k.gancho = v)} admite={admiteGanchos} version={versionAgente} cliente={actual.cliente ?? undefined} {equipo} prueba={prueba ?? undefined} />
                {#if !k.activa}
                  <p class="faint nota"><Trash2 size={13} />Para quitarla del todo y dejar de proteger esas carpetas, usa «Dejar de copiar» en el repositorio (espera {actual.cliente?.espera_min_horas} h).</p>
                {/if}
              </div>
            {/if}

            <!-- Lo del repositorio, a la vista; y «Añadir paso» debajo de esta copia. -->
            <div class="pie-k">
              {#if rr && actual.cliente}
                <PasosRepo
                  cliente={actual.cliente}
                  {equipo}
                  repo={rr}
                  equipos={actual.equipos}
                  administra
                  ordena
                  copia={k.id}
                  alNuevaCopia={() => nuevaDespues(i)}
                  alAutomatica={() => void irAAjustes(k)}
                  alCambiar={() => void api.equipo(c, id).then((e) => (equipo = e), () => {})}
                />
              {:else}
                <button type="button" class="btn btn-sm" onclick={() => nuevaDespues(i)}><Plus size={14} />Añadir paso: copia nueva</button>
              {/if}
            </div>
          </div>
        </section>
      {:else}
        <div class="card"><Vacio icono={FolderOpen} titulo="Sin copias todavía" texto="Crea la primera: elige carpetas, el repositorio y el horario." /></div>
      {/each}
    </div>

    <div class="nueva-fila">
      <button class="btn btn-primary nueva" onclick={() => { nueva(); const k = cfg?.copias.at(-1); if (k) abiertas[k.id] = true; }} disabled={!repos.length}><Plus size={16} />Añadir una copia</button>
      <button class="btn btn-ghost" onclick={nueva321} disabled={!repos.length} use:tip={"Una copia cada día al almacén, con verificación semanal y prueba de restauración mensual; y dice dónde añadir el espejo a otro disco y la copia en la nube"}><ShieldCheck size={16} />Con la plantilla 3-2-1</button>
      {#if plantillas.length && repos.length}
        <MenuAcciones texto="Desde una plantilla" icono={LayoutTemplate} etiqueta="Añadir una copia desde una plantilla" grupos={[plantillas.map((p) => ({ texto: p.nombre, onclick: () => nuevaDesde(p) }))]} izquierda />
      {/if}
      <button class="btn btn-ghost btn-sm" onclick={() => (gestionar = true)}><LayoutTemplate size={14} />Plantillas{plantillas.length ? ` · ${plantillas.length}` : ""}</button>
    </div>

    {#if reposSinCopias.length}
      <section class="card p verif" id="verificacion" aria-labelledby="t-verif">
        <h2 class="section-title" id="t-verif"><Database size={16} />Repositorios sin copias</h2>
        <p class="faint">Su verificación y su prueba de restauración se ponen cuando alguna copia guarde en ellos.</p>
        {#each reposSinCopias as r (r.id)}
          <div class="sin-copias">
            <strong>{r.nombre}</strong>
            {@render ajustesRepo(r, `sin-${r.id}`)}
          </div>
        {/each}
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

{#snippet ajustesRepo(r: { id: string; nombre: string }, clave: string)}
  {@const va = cfg?.verificaciones?.[r.id]}
  {@const pr = cfg?.pruebas_restauracion?.[r.id]}
  <div class="verif-fila" class:resaltada={r.id === verifPedida}>
    {#if !admiteVerif}
      <span class="faint frase-verif"><ShieldCheck size={14} />Verificación automática: actualiza el agente de {equipo?.nombre}{versionAgente ? ` (tiene la ${versionAgente})` : ""}. Mientras, «Verificar ahora».</span>
    {:else}
      <label class="switch-row"><input type="checkbox" class="switch" checked={!!va} onchange={(e) => ponerVerif(r.id, e.currentTarget.checked ? { ...VERIFICACION_POR_DEFECTO } : null)} /><span>Verificación automática</span></label><Ayuda id="prot-verificacion" />
      {#if va}<EditorVerificacion id={clave} nombre={r.nombre} valor={va} admiteHorario={admiteVerifHorario} onchange={(v) => ponerVerif(r.id, v)} />{/if}
    {/if}
  </div>
  <div class="verif-fila" class:resaltada={r.id === pruebaPedida}>
    {#if !admitePrueba}
      <span class="faint frase-verif"><FlaskConical size={14} />Prueba de restauración automática: actualiza el agente de {equipo?.nombre}. Mientras, «Probar ahora».</span>
    {:else}
      <label class="switch-row"><input type="checkbox" class="switch" checked={!!pr} onchange={(e) => ponerPrueba(r.id, e.currentTarget.checked ? { ...PRUEBA_POR_DEFECTO } : null)} /><span>Prueba de restauración automática</span></label><Ayuda id="regla-321" />
      {#if pr}
        <label class="cada-dias">Cada <input class="input num" type="number" min="1" max="31" value={pr.cada_dias} oninput={(e) => ponerPrueba(r.id, { cada_dias: Number(e.currentTarget.value) })} aria-label="Cada cuántos días, la prueba de «{r.nombre}»" /> días</label>
      {/if}
    {/if}
  </div>
{/snippet}

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
  /* El flujo de copias (docs/editor-de-copias.md): numeradas, con asa y la flecha «después». */
  .flujo {
    display: flex;
    flex-direction: column;
  }
  .copia {
    position: relative;
    display: grid;
    grid-template-columns: 36px minmax(0, 1fr);
    gap: 0 var(--sp-3);
    padding: var(--sp-4) var(--sp-5) var(--sp-4) var(--sp-3);
    transition:
      box-shadow var(--dur-fast) var(--ease),
      border-color var(--dur-fast) var(--ease);
  }
  .copia.levantada {
    z-index: 3;
    border-color: var(--accent);
    box-shadow: var(--shadow-lg);
    transition: none;
  }
  /* Dónde cae al soltar: una raya del acento. */
  .cae-arriba::before,
  .cae-abajo::after {
    content: "";
    position: absolute;
    left: 8px;
    right: 8px;
    height: 3px;
    border-radius: 3px;
    background: var(--accent);
  }
  .cae-arriba::before {
    top: -12px;
  }
  .cae-abajo::after {
    bottom: -12px;
  }
  .lado {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-2);
  }
  .num {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent-text);
    font-size: var(--fs-sm);
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }
  .inactiva .num {
    background: var(--surface-3);
    color: var(--text-2);
  }
  .asa {
    display: grid;
    place-items: center;
    width: 32px;
    height: 44px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    color: var(--text-2);
    cursor: grab;
    touch-action: none;
  }
  .asa:hover {
    color: var(--text-1);
    border-color: var(--border-strong, var(--text-3));
  }
  .levantada .asa {
    cursor: grabbing;
    color: var(--accent-text);
  }
  .arrastrando {
    user-select: none;
  }
  .cuerpo {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
  }
  .cab {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--sp-2) var(--sp-3);
  }
  .titulo {
    flex: 1 1 200px;
    min-width: 0;
    max-width: 360px;
    font-weight: 600;
    font-size: var(--fs-h2);
  }
  .activa {
    margin-left: auto;
  }
  .plegar :global(svg) {
    transition: transform var(--dur) var(--ease);
  }
  .plegar.girado :global(svg) {
    transform: rotate(180deg);
  }
  /* Entre dos tarjetas: hueco; si la de abajo va después, la línea y «después». */
  .union {
    position: relative;
    height: var(--sp-5);
  }
  .union.enlazada {
    height: 36px;
    margin-left: 30px;
    border-left: 2px solid var(--accent);
  }
  .union-txt {
    position: absolute;
    top: 50%;
    left: 10px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    transform: translateY(-50%);
    font-size: var(--fs-xs);
    font-weight: 550;
    color: var(--accent-text);
  }
  .cuando-k {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px var(--sp-3);
  }
  .cuando-k .segmented {
    flex-wrap: wrap;
  }
  .cuando-txt {
    font-size: var(--fs-sm);
  }
  .camino {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .camino li,
  .ramas li {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .camino :global(svg),
  .ramas :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .camino strong {
    color: var(--text-1);
    font-weight: 550;
  }
  .flecha {
    color: var(--text-3);
  }
  .ramas {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: -6px 0 0;
    padding: 0 0 0 6px;
    list-style: none;
    font-size: var(--fs-sm);
    color: var(--text-1);
  }
  .ramas li {
    align-items: flex-start;
  }
  .ramas li > :global(svg) {
    margin-top: 3px;
  }
  .rama-t {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }
  .rama-d {
    min-width: 0;
  }
  .cuando-k [role="radio"]:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .form-k {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding-top: var(--sp-4);
    border-top: 1px solid var(--border);
  }
  .repo-k {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    scroll-margin-top: 96px;
  }
  .pie-k {
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .aviso-orden {
    align-items: center;
  }
  .aviso-orden p {
    flex: 1;
  }
  .sin-copias {
    display: flex;
    flex-direction: column;
    gap: 6px;
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
  .usada {
    margin-top: -6px;
  }
  @media (prefers-reduced-motion: reduce) {
    .copia,
    .plegar :global(svg) {
      transition: none;
    }
  }
  @media (max-width: 560px) {
    .copia {
      grid-template-columns: 32px minmax(0, 1fr);
      gap: 0 var(--sp-2);
      padding: var(--sp-3) var(--sp-3) var(--sp-3) var(--sp-2);
    }
    .union.enlazada {
      margin-left: 24px;
    }
    .titulo {
      flex-basis: 100%;
      max-width: none;
    }
    .activa {
      margin-left: 0;
    }
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
    display: inline-flex;
    align-items: center;
    gap: 6px;
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
  }
</style>
