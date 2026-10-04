// Lo que está en marcha ahora en los equipos del cliente abierto (v1.25): el
// progreso de las copias y de las tareas largas, casi en vivo.
//
// El servidor lo tiene solo en memoria (`GET …/progreso`): aquí se pregunta
// cada 3 s mientras algo está en marcha (o justo después de mandar «Copiar
// ahora») y cada 12 s si no. Con un servidor anterior (404), se toma de los
// informes (`progreso`, si el agente lo manda), cada 15 s.
//
// Cuando algo termina, se vuelve a cargar el cliente (el estado de la copia,
// la última vuelta…) y avisa a quien lo pida con `alTerminar`.

import * as api from "./api";
import { cargarCliente } from "./estado.svelte";
import type { TareaEnMarcha, TipoTarea } from "./tipos";

export interface EnMarchaEquipo {
  recibido: string;
  tareas: TareaEnMarcha[];
}

export const enMarcha = $state({
  cliente: "",
  porEquipo: {} as Record<string, EnMarchaEquipo>,
  /** ¿Lo da el servidor (v1.25)? Si no, sale de los informes. */
  enVivo: true,
});

const RAPIDO = 3_000;
const LENTO = 12_000;
const INFORMES = 15_000;
/** Tras mandar una orden que pone algo en marcha, se pregunta deprisa este rato. */
const PRONTO = 90_000;
/** Un informe más viejo que esto ya no dice qué está en marcha. */
const INFORME_VIGENTE = 3 * 60_000;

let temporizador: ReturnType<typeof setTimeout> | null = null;
/**
 * Cuándo llegó (con el reloj de aquí) la última noticia de cada tarea. Así la
 * barra que avanza sola entre noticias es la misma en todas partes: la barra
 * lateral, la fila de la copia y el chip de la tarjeta dicen el mismo número.
 */
const llegadas = new Map<string, number>();
/** Reloj compartido (cada segundo, solo mientras hay algo en marcha). */
export const pulso = $state({ ahora: Date.now() });
let latido: ReturnType<typeof setInterval> | null = null;
function latir() {
  if (typeof window === "undefined") return;
  if (hayAlgo()) latido ??= setInterval(() => (pulso.ahora = Date.now()), 1000);
  else if (latido) {
    clearInterval(latido);
    latido = null;
  }
}
let deprisaHasta = 0;
let vuelta = 0;
const oyentes = new Set<(equipo: string, t: TareaEnMarcha) => void>();

/** Clave estable de una tarea (para saber cuándo termina). */
export const claveTarea = (equipo: string, t: TareaEnMarcha) => `${equipo}|${t.tipo}|${t.repo}|${t.copia ?? ""}`;

function hayAlgo() {
  return Object.values(enMarcha.porEquipo).some((x) => x.tareas.length);
}

async function leer(cliente: string): Promise<Record<string, EnMarchaEquipo>> {
  if (enMarcha.enVivo) {
    try {
      const xs = await api.progreso(cliente);
      return Object.fromEntries(xs.filter((x) => x.tareas.length).map((x) => [x.equipo, { recibido: x.recibido, tareas: x.tareas }]));
    } catch (e) {
      if (!(e instanceof api.ApiError && e.estado === 404)) throw e;
      enMarcha.enVivo = false;
    }
  }
  const xs = await api.ultimosInformes(cliente);
  const ahora = Date.now();
  return Object.fromEntries(
    xs
      .filter((x) => Array.isArray(x.datos.progreso) && x.datos.progreso.length && ahora - Date.parse(x.recibido) < INFORME_VIGENTE)
      .map((x) => [x.equipo, { recibido: x.recibido, tareas: x.datos.progreso as TareaEnMarcha[] }]),
  );
}

async function preguntar(cliente: string, n: number, siempre = false) {
  let siguiente = LENTO;
  try {
    // Con la pestaña oculta no se pregunta (salvo la primera vez, para tener algo que enseñar).
    if (siempre || typeof document === "undefined" || document.visibilityState === "visible") {
      const nuevo = await leer(cliente);
      if (n !== vuelta) return;
      const antes = enMarcha.porEquipo;
      const quedan = new Set(Object.entries(nuevo).flatMap(([e, x]) => x.tareas.map((t) => claveTarea(e, t))));
      const terminadas = Object.entries(antes).flatMap(([e, x]) => x.tareas.filter((t) => !quedan.has(claveTarea(e, t))).map((t) => [e, t] as const));
      const ahora = Date.now();
      for (const k of [...llegadas.keys()]) if (!quedan.has(k)) llegadas.delete(k);
      for (const [e, x] of Object.entries(nuevo)) for (const t of x.tareas) llegadas.set(claveTarea(e, t), ahora);
      enMarcha.porEquipo = nuevo;
      pulso.ahora = ahora;
      latir();
      if (terminadas.length) {
        void cargarCliente(cliente, { silencioso: true });
        for (const [e, t] of terminadas) for (const f of oyentes) f(e, t);
      }
    }
    siguiente = !enMarcha.enVivo ? INFORMES : hayAlgo() || Date.now() < deprisaHasta ? RAPIDO : LENTO;
  } catch {
    /* sin conexión o sin sesión: se reintenta despacio */
  }
  if (n === vuelta) temporizador = setTimeout(() => void preguntar(cliente, n), siguiente);
}

/** Empieza a seguir lo que está en marcha en un cliente. Devuelve cómo parar. */
export function vigilarProgreso(cliente: string): () => void {
  const n = ++vuelta;
  if (temporizador) clearTimeout(temporizador);
  if (enMarcha.cliente !== cliente) {
    enMarcha.cliente = cliente;
    enMarcha.porEquipo = {};
    enMarcha.enVivo = true;
    llegadas.clear();
    latir();
  }
  void preguntar(cliente, n, true);
  const alVolver = () => {
    if (document.visibilityState === "visible" && n === vuelta) {
      if (temporizador) clearTimeout(temporizador);
      void preguntar(cliente, n);
    }
  };
  document.addEventListener("visibilitychange", alVolver);
  return () => {
    document.removeEventListener("visibilitychange", alVolver);
    if (n === vuelta) {
      vuelta++;
      if (temporizador) clearTimeout(temporizador);
      temporizador = null;
    }
  };
}

/** Se acaba de mandar algo que pone una tarea en marcha: preguntar deprisa un rato (y ya). */
export function progresoPronto() {
  deprisaHasta = Date.now() + PRONTO;
  const cliente = enMarcha.cliente;
  if (!cliente || typeof window === "undefined") return;
  const n = vuelta;
  if (temporizador) clearTimeout(temporizador);
  temporizador = setTimeout(() => void preguntar(cliente, n), 1_500);
}

/** Avisa cuando termina algo que estaba en marcha. Devuelve cómo dejar de oír. */
export function alTerminarTarea(f: (equipo: string, t: TareaEnMarcha) => void): () => void {
  oyentes.add(f);
  return () => oyentes.delete(f);
}

/** Las tareas en marcha de un equipo, filtradas (por copia, repositorio o tipo). */
export function tareasDe(equipo: string, filtro: { copia?: string; repo?: string; tipos?: TipoTarea[] } = {}): TareaEnMarcha[] {
  return (enMarcha.porEquipo[equipo]?.tareas ?? []).filter(
    (t) => (filtro.copia === undefined || (t.tipo === "copia" && t.copia === filtro.copia)) && (filtro.repo === undefined || t.repo === filtro.repo) && (!filtro.tipos || filtro.tipos.includes(t.tipo)),
  );
}

/** Todo lo que está en marcha en el cliente: `[{ equipo, tarea }]`. */
export function todasEnMarcha(): { equipo: string; tarea: TareaEnMarcha }[] {
  return Object.entries(enMarcha.porEquipo).flatMap(([equipo, x]) => x.tareas.map((tarea) => ({ equipo, tarea })));
}

/**
 * El porcentaje que se pinta (0–100, con decimales): el real más lo que habrá
 * avanzado desde la última noticia, al ritmo que lleva (como mucho 6 s, y
 * nunca el 100 %). Null si aún no se sabe. Todas las vistas usan este.
 */
export function pctPintado(equipo: string, t: TareaEnMarcha, ahora = pulso.ahora): number | null {
  if (t.porcentaje == null) return null;
  let p = t.porcentaje;
  if (t.fase === "subiendo" && t.quedan_s && t.quedan_s > 0) {
    const llegada = llegadas.get(claveTarea(equipo, t)) ?? ahora;
    const dt = Math.min(6, Math.max(0, (ahora - llegada) / 1000));
    p += ((1 - p) * dt) / (t.quedan_s + dt);
  }
  return Math.min(99.5, Math.max(0, p * 100));
}

/** El porcentaje entero que se escribe (el mismo en todas partes). */
export function pctVisible(equipo: string, t: TareaEnMarcha, ahora = pulso.ahora): number | null {
  const p = pctPintado(equipo, t, ahora);
  if (p == null) return null;
  const real = t.porcentaje == null ? 0 : Math.floor(Math.max(0, Math.min(1, t.porcentaje)) * 100);
  return Math.max(real, Math.floor(p));
}

/**
 * El progreso de varias tareas juntas: por bytes si todas los dicen; si no, la
 * media de las que tienen porcentaje. Null si ninguna lo sabe aún.
 */
export function pctConjunto(xs: { equipo: string; tarea: TareaEnMarcha }[], ahora = pulso.ahora): number | null {
  const con = xs.map((x) => ({ ...x, p: pctPintado(x.equipo, x.tarea, ahora) })).filter((x) => x.p != null);
  if (!con.length) return null;
  if (con.length === xs.length && con.every((x) => x.tarea.bytes_total)) {
    const total = con.reduce((n, x) => n + (x.tarea.bytes_total ?? 0), 0);
    return Math.floor(con.reduce((n, x) => n + (x.p! / 100) * (x.tarea.bytes_total ?? 0), 0) / total * 100);
  }
  return Math.floor(con.reduce((n, x) => n + x.p!, 0) / con.length);
}
