// La página de cada espejo (plan 0.8.0, C2; plan-mejoras 14i) y el aviso de su
// primera subida (14s): lo que no es pantalla. Su registro de vueltas (`subidas`,
// que manda el agente, como mucho 30), lo que ocupa en el destino (`ocupa`), las
// cifras para las gráficas y cuánto subirá la primera vez. Sin runas (lo prueban
// scripts/vectores-espejo-pagina.ts).
import { reposDeTrabajo } from "./fichaDestino";
import { bytesRepo } from "./repo";
import { claveDestinoTrabajo } from "./espejoReduce";
import type { Equipo, SubidaEspejo, TrabajoEspejo, TrabajoEspejoResumen } from "./tipos";

/** Por qué empezó una vuelta. */
export const TEXTO_MOTIVO: Record<string, string> = {
  horario: "Por su horario",
  tras_copia: "Después de una copia nueva",
  tras_otro: "Después de otro espejo",
  ahora: "Pedida desde una consola",
};

/** Una vuelta para las listas y las gráficas (con `hora`, la de cuando terminó, y su duración). */
export interface FilaSubida extends SubidaEspejo {
  hora: string;
  duracion_s: number | null;
}

/** Las vueltas de un espejo, de la más reciente a la más antigua (como las versiones de una copia). */
export function filasSubidas(t: Pick<TrabajoEspejoResumen, "subidas">): FilaSubida[] {
  return [...(t.subidas ?? [])]
    .map((s) => {
      const d = (Date.parse(s.fin) - Date.parse(s.inicio)) / 1000;
      return { ...s, hora: s.fin, duracion_s: Number.isFinite(d) && d >= 0 ? Math.round(d) : null };
    })
    .sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora));
}

export interface ResumenSubidas {
  vueltas: number;
  fallidas: number;
  /** Lo subido en todas las vueltas del registro. */
  bytes: number;
  archivos: number;
  /** Lo que quitó su retención en esas vueltas. */
  borrados: number;
  borradosBytes: number;
  /** Duración media de las que salieron bien (segundos). */
  mediaDuracion: number | null;
  /** La primera vuelta del registro (desde cuándo se cuenta). */
  desde: string | null;
}

export function resumenSubidas(filas: FilaSubida[]): ResumenSubidas {
  const bien = filas.filter((f) => f.resultado === "ok" && f.duracion_s != null);
  return {
    vueltas: filas.length,
    fallidas: filas.filter((f) => f.resultado !== "ok").length,
    bytes: filas.reduce((n, f) => n + (f.bytes ?? 0), 0),
    archivos: filas.reduce((n, f) => n + (f.archivos ?? 0), 0),
    borrados: filas.reduce((n, f) => n + (f.borrados ?? 0), 0),
    borradosBytes: filas.reduce((n, f) => n + (f.borrados_bytes ?? 0), 0),
    mediaDuracion: bien.length ? Math.round(bien.reduce((n, f) => n + (f.duracion_s ?? 0), 0) / bien.length) : null,
    desde: filas.at(-1)?.inicio ?? null,
  };
}

/** Lo que ocupa cada repositorio del espejo en su destino (de mayor a menor), si el agente lo cuenta. */
export function ocupaPorRepo(t: Pick<TrabajoEspejoResumen, "ocupa">): { clave: string; bytes: number }[] {
  return Object.entries(t.ocupa?.repos ?? {})
    .filter(([k]) => k !== "")
    .map(([clave, bytes]) => ({ clave, bytes }))
    .sort((a, b) => b.bytes - a.bytes);
}

// --- 14s: la primera subida, avisada ---

/** El agente entiende `cuando.primera_desde` (la primera subida, no antes de esa hora). */
export const ADMITE_PRIMERA = "espejo_primera";
export const admitePrimeraDesde = (e: Pick<Equipo, "resumen"> | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_PRIMERA);

/**
 * ¿Guardarlo empieza una subida desde cero? Un espejo nuevo, o uno que cambia de
 * destino (o de qué repositorios copia a más), lo sube todo la primera vez.
 */
export function empiezaDeCero(t: TrabajoEspejo, original: TrabajoEspejo | null, nuevo: boolean): boolean {
  if (nuevo || !original) return true;
  return claveDestinoTrabajo(t) !== claveDestinoTrabajo(original);
}

export interface PrimeraSubida {
  /** Lo que se sabe que hay que subir (bytes). */
  bytes: number;
  /** Repositorios que entran. */
  repos: number;
  /** De cuántos no se sabe el tamaño (de equipos que no están en esta consola). */
  sinTamano: number;
}

/**
 * 14s: cuánto subirá la primera vez: lo que ocupan los repositorios que entran (el de
 * cada repositorio en su almacén o disco). `null` si no entra ninguno con versiones.
 * Lo que ya esté en el destino no se vuelve a subir: es lo más que subirá.
 */
export function primeraSubida(t: TrabajoEspejo, hace: Equipo, equipos: Equipo[]): PrimeraSubida | null {
  const repos = reposDeTrabajo(t as TrabajoEspejoResumen, hace, equipos);
  if (!repos.length) return null;
  let bytes = 0;
  let sinTamano = 0;
  for (const r of repos) {
    const b = r.repo && !r.derivada ? bytesRepo(r.repo) : null;
    if (b == null) sinTamano++;
    else bytes += b;
  }
  // Todos vacíos (sin versiones todavía): no hay nada que subir.
  if (!bytes && !sinTamano) return null;
  return { bytes, repos: repos.length, sinTamano };
}

/** La próxima franja de la madrugada (las `hora`:00 de esta consola), en ISO. */
export function madrugadaSiguiente(ahora: number, hora = 2): string {
  const d = new Date(ahora);
  d.setHours(hora, 0, 0, 0);
  if (d.getTime() <= ahora) d.setDate(d.getDate() + 1);
  return d.toISOString();
}
