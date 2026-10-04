// Progreso en vivo simulado (v1.25, `GET /api/clientes/{c}/progreso`).
//
// - RECEPCION tiene siempre una copia larga en marcha («Documentos», ~38 GB,
//   unos 15 min por vuelta, que vuelve a empezar): para ver la barra sin hacer nada.
// - «Copiar ahora» de cualquier copia la pone en marcha unos 45 s, con sus
//   fases (antes de copiar si tiene ganchos, preparando, buscando cambios,
//   copiando y guardando); al terminar, la copia cuenta como hecha.
// - «Verificar» deja una verificación en marcha unos 20 s en su repositorio.
import type * as T from "../lib/tipos";
import { estado, ID } from "./estado";
import { ID_OTROS } from "./otrosClientes";

interface Sim {
  cliente: string;
  equipo: string;
  tipo: T.TipoTarea;
  repo: string;
  copia?: string;
  nombre?: string;
  empezo: number;
  /** Duraciones de cada fase (ms). */
  ganchos: number;
  preparar: number;
  escanear: number;
  subir: number;
  terminar: number;
  archivos: number;
  bytes: number;
  /** Vuelve a empezar al terminar (la copia larga de RECEPCION). */
  bucle?: boolean;
  alTerminar?: () => void;
  hecha?: boolean;
}

let sims: Sim[] = [];
let sembradoPara: unknown = null;

const total = (s: Sim) => s.ganchos + s.preparar + s.escanear + s.subir + s.terminar;
const iso = (ms: number) => new Date(ms).toISOString();

function sembrar() {
  if (sembradoPara === estado) return;
  sembradoPara = estado;
  sims = [];
  const recepcion = estado.equipos.find((e) => e.id === ID.recepcion);
  const k = recepcion?.resumen?.copias?.[0];
  if (!recepcion || !k) return;
  const s: Sim = {
    cliente: recepcion.cliente,
    equipo: recepcion.id,
    tipo: "copia",
    repo: k.repo,
    copia: k.id,
    nombre: k.nombre,
    empezo: 0,
    ganchos: 0,
    preparar: 4_000,
    escanear: 40_000,
    subir: 15 * 60_000,
    terminar: 6_000,
    archivos: 184_213,
    bytes: 38_400_000_000,
    bucle: true,
  };
  // Ya va por un tercio, para que se vea moverse con cifras desde el principio.
  s.empezo = Date.now() - (s.preparar + s.escanear + s.subir * 0.35);
  sims.push(s);
  // v1.3x: otra copia larga en otro cliente (HISTORIAS, de la clínica), para «Todos los clientes».
  const historias = estado.equipos.find((e) => e.id === ID_OTROS.historias);
  const kh = historias?.resumen?.copias?.[0];
  if (historias && kh) {
    const h: Sim = { cliente: historias.cliente, equipo: historias.id, tipo: "copia", repo: kh.repo, copia: kh.id, nombre: kh.nombre, empezo: 0, ganchos: 0, preparar: 3_000, escanear: 20_000, subir: 9 * 60_000, terminar: 4_000, archivos: 52_310, bytes: 6_200_000_000, bucle: true };
    h.empezo = Date.now() - (h.preparar + h.escanear + h.subir * 0.6);
    sims.push(h);
  }
}

/** Una tarea en marcha según el tiempo que lleva. */
function tareaDe(s: Sim, ahora: number): T.TareaEnMarcha {
  const t = Math.max(0, ahora - s.empezo);
  const base = { tipo: s.tipo, repo: s.repo, copia: s.copia ?? null, nombre: s.nombre ?? null, empezo: iso(s.empezo), actualizado: iso(ahora - (ahora % 3000)) };
  if (s.tipo !== "copia") {
    const f = Math.min(1, t / total(s));
    const parte = f < 0.15 ? "Revisando copias, carpetas y bloques…" : `Leyendo el 5 % de los datos…`;
    return { ...base, fase: f < 0.05 ? "preparando" : "en_marcha", etapa: f < 0.05 ? "Preparando…" : parte, porcentaje: f < 0.05 ? null : Math.round(f * 1000) / 1000, quedan_s: f < 0.05 ? null : Math.round((total(s) - t) / 1000) };
  }
  if (t < s.ganchos) return { ...base, fase: "antes_de_copiar" };
  if (t < s.ganchos + s.preparar) return { ...base, fase: "preparando" };
  // Restic cuenta mientras copia: al principio sin «quedan» y con totales que crecen.
  const tEsc = t - s.ganchos - s.preparar;
  if (tEsc < s.escanear) {
    const f = tEsc / s.escanear;
    const archivosVistos = Math.round(s.archivos * f);
    const bytesVistos = Math.round(s.bytes * f);
    const hechos = Math.round(archivosVistos * 0.12 * f);
    const bytesHechos = Math.round(bytesVistos * 0.04 * f);
    return { ...base, fase: "escaneando", porcentaje: bytesVistos ? bytesHechos / bytesVistos : 0, archivos: hechos, archivos_total: archivosVistos, bytes: bytesHechos, bytes_total: bytesVistos, velocidad: Math.round(42_000_000 + 9_000_000 * Math.sin(ahora / 4000)) };
  }
  const tSub = tEsc - s.escanear;
  if (tSub < s.subir) {
    const inicio = 0.04;
    const f = inicio + (1 - inicio) * (tSub / s.subir);
    // Velocidad con algo de vaivén (archivos grandes y pequeños).
    const velocidad = Math.round((s.bytes * (1 - inicio)) / (s.subir / 1000) * (1 + 0.18 * Math.sin(ahora / 5000)));
    return {
      ...base,
      fase: "subiendo",
      porcentaje: Math.round(f * 10000) / 10000,
      archivos: Math.round(s.archivos * f),
      archivos_total: s.archivos,
      bytes: Math.round(s.bytes * f),
      bytes_total: s.bytes,
      velocidad,
      quedan_s: Math.max(1, Math.round((s.subir - tSub) / 1000)),
    };
  }
  return { ...base, fase: "terminando", porcentaje: 1, archivos: s.archivos, archivos_total: s.archivos, bytes: s.bytes, bytes_total: s.bytes };
}

/** Las que ya acabaron: cuentan como hechas y se quitan. */
function terminar(ahora: number) {
  for (const s of sims)
    if (!s.bucle && !s.hecha && ahora - s.empezo >= total(s)) {
      s.hecha = true;
      s.alTerminar?.();
    }
  sims = sims.filter((s) => !s.hecha);
}

/** Termina a su hora aunque nadie esté mirando. */
function programar(s: Sim) {
  setTimeout(() => terminar(Date.now()), s.empezo + total(s) - Date.now() + 50);
}

/** Lo que está en marcha en los equipos de un cliente, ahora. */
export function progresoDe(cliente: string): T.ProgresoEquipo[] {
  sembrar();
  const ahora = Date.now();
  for (const s of sims) if (s.bucle && ahora - s.empezo >= total(s)) s.empezo = ahora;
  terminar(ahora);
  const porEquipo = new Map<string, T.TareaEnMarcha[]>();
  for (const s of sims.filter((x) => x.cliente === cliente)) porEquipo.set(s.equipo, [...(porEquipo.get(s.equipo) ?? []), tareaDe(s, ahora)]);
  return [...porEquipo].map(([equipo, tareas]) => ({ equipo, recibido: iso(ahora), tareas }));
}

/** «Copiar ahora»: la copia se pone en marcha (sustituye a la que hubiera de esa copia). */
export function empezarCopia(cliente: string, equipo: string, repo: string, copia: string, nombre: string, conGanchos: boolean, alTerminar: () => void) {
  sembrar();
  sims = sims.filter((s) => !(s.equipo === equipo && s.copia === copia));
  const s: Sim = {
    cliente,
    equipo,
    tipo: "copia",
    repo,
    copia,
    nombre,
    empezo: Date.now() + 1_500,
    ganchos: conGanchos ? 5_000 : 0,
    preparar: 3_000,
    escanear: 9_000,
    subir: 28_000,
    terminar: 3_000,
    archivos: 12_480,
    bytes: 2_140_000_000,
    alTerminar,
  };
  sims.push(s);
  programar(s);
}

/** Una tarea larga de un repositorio (verificar, copia externa…). */
export function empezarTarea(cliente: string, equipo: string, repo: string, tipo: T.TipoTarea, ms: number, alTerminar?: () => void) {
  sembrar();
  sims = sims.filter((s) => !(s.equipo === equipo && s.repo === repo && s.tipo === tipo));
  const s: Sim = { cliente, equipo, tipo, repo, empezo: Date.now() + 1_000, ganchos: 0, preparar: 0, escanear: 0, subir: ms, terminar: 0, archivos: 0, bytes: 0, alTerminar };
  sims.push(s);
  programar(s);
}
