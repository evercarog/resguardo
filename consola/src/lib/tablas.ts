// 16t: lo común de las listas largas (historial, restauraciones, órdenes, actividad,
// versiones): buscar por palabras, un rango de fechas claro y «Ver más» por tandas.
// Solo cálculo (sin runas), para probarlo en scripts/vectores.ts. Los controles, en
// `componentes/BarraTabla.svelte` y `componentes/VerMas.svelte`.
import { sinTildes } from "./global";
import { inicioDia, nombreIntervalo } from "./lineaTiempo";

/** Cuántas filas se añaden con cada «Ver más». */
export const TANDA = 50;

/** Un rango de días: el inicio del primero y el del último (los dos incluidos), o nada (todas las fechas). */
export interface Rango {
  desde: number | null;
  hasta: number | null;
}

export const SIN_RANGO: Rango = Object.freeze({ desde: null, hasta: null }) as Rango;

export const hayRango = (r: Rango) => r.desde != null && r.hasta != null;

/** ¿Cae la fecha dentro del rango? (Sin rango, siempre; una fecha que no se entiende, nunca con rango.) */
export function enRango(iso: string | number | null | undefined, r: Rango): boolean {
  if (!hayRango(r)) return true;
  const t = typeof iso === "number" ? iso : iso ? Date.parse(iso) : NaN;
  if (!Number.isFinite(t)) return false;
  const a = Math.min(r.desde!, r.hasta!);
  const b = Math.max(r.desde!, r.hasta!);
  return t >= inicioDia(a) && t < inicioDia(b, 1);
}

/** Los últimos N días (hoy incluido). */
export const ultimosDias = (n: number, ahora = Date.now()): Rango => ({ desde: inicioDia(ahora, -(n - 1)), hasta: inicioDia(ahora) });

/** El rango en palabras («Del 3 al 9 de oct.», «Hoy»), o «Todas las fechas». */
export const textoRango = (r: Rango, ahora = Date.now()) => (hayRango(r) ? nombreIntervalo(Math.min(r.desde!, r.hasta!), Math.max(r.desde!, r.hasta!), ahora) : "Todas las fechas");

/** ¿Están todas las palabras de la búsqueda (sin tildes ni mayúsculas) en alguno de los textos? */
export function casaBusqueda(consulta: string, ...textos: (string | null | undefined)[]): boolean {
  const palabras = sinTildes(consulta.trim()).split(/\s+/).filter(Boolean);
  if (!palabras.length) return true;
  const t = sinTildes(textos.filter(Boolean).join(" "));
  return palabras.every((p) => t.includes(p));
}

/** Lo que se ve con un límite y lo que queda. */
export function tanda<T>(lista: readonly T[], limite: number): { visibles: T[]; quedan: number } {
  const n = Math.max(0, limite);
  return { visibles: lista.slice(0, n), quedan: Math.max(0, lista.length - n) };
}

/** «Se ven 50 de 230; quedan 180.» */
export function textoSeVen(vistas: number, total: number): string {
  const f = (n: number) => n.toLocaleString("es-ES");
  return `Se ven ${f(vistas)} de ${f(total)}; quedan ${f(Math.max(0, total - vistas))}.`;
}

/** «12 de 80 restauraciones» (o «80 restauraciones» si no hay filtros que quiten alguna). */
export function textoFilas(vistas: number, total: number, uno: string, varios: string): string {
  const f = (n: number) => n.toLocaleString("es-ES");
  const nombre = (n: number) => (n === 1 ? uno : varios);
  return vistas === total ? `${f(total)} ${nombre(total)}` : `${f(vistas)} de ${f(total)} ${nombre(total)}`;
}

/** Rango para la dirección («2026-10-01..2026-10-31») y de vuelta. */
export function rangoATexto(r: Rango): string {
  if (!hayRango(r)) return "";
  const d = (t: number) => {
    const x = new Date(t);
    return `${x.getFullYear()}-${String(x.getMonth() + 1).padStart(2, "0")}-${String(x.getDate()).padStart(2, "0")}`;
  };
  return `${d(Math.min(r.desde!, r.hasta!))}..${d(Math.max(r.desde!, r.hasta!))}`;
}

export function rangoDeTexto(t: string | null | undefined): Rango {
  const m = /^(\d{4})-(\d{2})-(\d{2})\.\.(\d{4})-(\d{2})-(\d{2})$/.exec(t ?? "");
  if (!m) return { ...SIN_RANGO };
  const a = new Date(+m[1], +m[2] - 1, +m[3]).getTime();
  const b = new Date(+m[4], +m[5] - 1, +m[6]).getTime();
  if (!Number.isFinite(a) || !Number.isFinite(b)) return { ...SIN_RANGO };
  return { desde: Math.min(a, b), hasta: Math.max(a, b) };
}
