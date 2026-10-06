// Etiquetas de los equipos: lo que no depende del estado de la pantalla
// (colores, grupos, plantillas propuestas), para usarlo en cualquier sitio y
// probarlo en `npm run test:vectores` (scripts/vectores-etiquetas.ts).
//
// Color: de una paleta de 7 pensada para distinguirse también con daltonismo
// (Okabe-Ito, con su versión para el tema oscuro en app.css: --et-0 … --et-6).
// Sin elegir, cada etiqueta tiene siempre el mismo (por su nombre); v1.52: el
// propietario o un administrador puede elegir otro de la misma paleta. El
// color va solo en el punto: el texto, en tinta neutra, es lo que se lee.
import type { AjusteEtiqueta, AvisosEtiqueta, Equipo } from "./tipos";

export const N_COLORES = 7;

/** El nombre de cada color de la paleta (para leerlo, no solo verlo). */
export const NOMBRES_COLOR = ["Azul", "Ámbar", "Verde", "Rosa", "Bermellón", "Celeste", "Gris"] as const;

const clave = (t: string) => t.trim().toLowerCase();
export const mismaEtiqueta = (a: string, b: string) => clave(a) === clave(b);

/** El color que sale del nombre (el de siempre, sin elegir). */
export function colorPorNombre(nombre: string): number {
  let h = 0;
  for (const c of clave(nombre)) h = (h * 31 + c.codePointAt(0)!) >>> 0;
  return h % N_COLORES;
}

/** Los ajustes de una etiqueta (sin distinguir mayúsculas), si los tiene. */
export function ajusteDe(nombre: string, ajustes: readonly AjusteEtiqueta[] | undefined): AjusteEtiqueta | undefined {
  return ajustes?.find((a) => mismaEtiqueta(a.nombre, nombre));
}

/** El color de una etiqueta: el elegido si lo hay (y es de la paleta); si no, el de su nombre. */
export function colorDe(nombre: string, ajustes?: readonly AjusteEtiqueta[]): number {
  const c = ajusteDe(nombre, ajustes)?.color;
  return typeof c === "number" && Number.isInteger(c) && c >= 0 && c < N_COLORES ? c : colorPorNombre(nombre);
}

/** Todas las etiquetas del cliente, ordenadas, con cuántos equipos la llevan. */
export function etiquetasDe(equipos: readonly Equipo[]): { nombre: string; n: number }[] {
  const m = new Map<string, { nombre: string; n: number }>();
  for (const e of equipos)
    for (const t of e.etiquetas ?? []) {
      const k = clave(t);
      const x = m.get(k) ?? { nombre: t, n: 0 };
      x.n++;
      m.set(k, x);
    }
  return [...m.values()].sort((a, b) => a.nombre.localeCompare(b.nombre, "es"));
}

/** ¿Lleva el equipo esa etiqueta? (sin etiqueta, todos). */
export function pasaFiltro(e: Equipo | undefined, etiqueta: string): boolean {
  if (!etiqueta) return true;
  return !!e?.etiquetas?.some((t) => mismaEtiqueta(t, etiqueta));
}

export interface Grupo<E extends Pick<Equipo, "etiquetas"> = Equipo> {
  /** null: los que no tienen ninguna. */
  etiqueta: string | null;
  equipos: E[];
}

/**
 * Los equipos agrupados por etiqueta, en orden alfabético, y al final los que
 * no tienen ninguna. Un equipo con dos etiquetas sale en los dos grupos (cada
 * grupo es «todos los de X», como el filtro). Dentro de cada grupo, en el orden
 * en que llegan.
 */
export function gruposPorEtiqueta<E extends Pick<Equipo, "etiquetas">>(equipos: readonly E[]): Grupo<E>[] {
  const grupos: Grupo<E>[] = etiquetasDe(equipos as unknown as Equipo[]).map((t) => ({ etiqueta: t.nombre, equipos: equipos.filter((e) => e.etiquetas?.some((x) => mismaEtiqueta(x, t.nombre))) }));
  const sin = equipos.filter((e) => !e.etiquetas?.length);
  if (sin.length) grupos.push({ etiqueta: null, equipos: sin });
  return grupos;
}

/**
 * Las plantillas que proponen las etiquetas de un equipo (una por plantilla,
 * con las etiquetas que la piden). Solo si aún no tiene copias: es para los
 * equipos nuevos. Nunca se aplican solas: se ofrecen, y se revisan y envían con
 * la clave de administración como cualquier cambio de copias.
 */
export function plantillasPropuestas(equipo: Pick<Equipo, "etiquetas" | "resumen" | "confirmado" | "modo">, ajustes: readonly AjusteEtiqueta[] | undefined): { plantilla: string; etiquetas: string[] }[] {
  if (!equipo.confirmado || equipo.modo === "trasladado" || (equipo.resumen?.copias?.length ?? 0) > 0) return [];
  const out: { plantilla: string; etiquetas: string[] }[] = [];
  for (const t of equipo.etiquetas ?? []) {
    const p = ajusteDe(t, ajustes)?.plantilla;
    if (!p) continue;
    const x = out.find((o) => o.plantilla === p);
    if (x) x.etiquetas.push(t);
    else out.push({ plantilla: p, etiquetas: [t] });
  }
  return out;
}

/** ¿Hay algo de los avisos que contar? */
export const conAvisos = (a: AvisosEtiqueta | null | undefined) => !!a && (!!a.importancia || !!a.canales?.length);

/** Los ajustes que se envían al servidor (null en lo que no se usa; sin nada, vuelve a lo de siempre). */
export function cuerpoAjuste(nombre: string, color: number | null, plantilla: string | null, avisos: AvisosEtiqueta | null | undefined) {
  const av = conAvisos(avisos) ? { ...(avisos!.importancia ? { importancia: avisos!.importancia } : {}), ...(avisos!.canales?.length ? { canales: avisos!.canales } : {}) } : null;
  return { nombre: nombre.trim(), color: color ?? null, plantilla: plantilla || null, avisos: av };
}
