// La línea de tiempo de las versiones («máquina del tiempo», docs/diseno.md §4):
// lo que no es pantalla. Las escalas (día, semana, mes, año), las marcas del
// eje, el color de cada copia (tres de una paleta validada para daltonismo,
// en orden fijo; las demás, en tinta neutra), la retención simulada (qué se
// queda y por qué, qué quitaría la próxima vez) y lo que se dibuja con ella:
// el «río» de versiones por periodo, las burbujas de las que no caben y las
// franjas de cada regla.
import type { DestinoResumen, Equipo, Regla, RepositorioResumen } from "./tipos";
import { almacenDe, motivosQuedan, reglaDe, type Periodo } from "./retencion";

const HORA = 3_600_000;
const DIA = 24 * HORA;

/** Una versión en la línea de tiempo (del informe o de la sesión de Restaurar). */
export interface VersionLinea {
  id: string;
  hora: string;
  copia?: string | null;
  /** Lo que ocupan sus archivos. */
  bytes?: number | null;
  /** Lo nuevo que añadió. */
  anadido?: number | null;
  archivos?: number | null;
}

export type Zoom = "dia" | "semana" | "mes" | "ano";
/** Cada escala: lo que se ve (`ms`) y el tramo del «río» (`rio`: versiones por hora, por día…). */
export const ZOOM: Record<Zoom, { texto: string; ms: number; rio: number; por: string }> = {
  dia: { texto: "Día", ms: DIA, rio: HORA, por: "hora" },
  semana: { texto: "Semana", ms: 7 * DIA, rio: 6 * HORA, por: "6 horas" },
  mes: { texto: "Mes", ms: 31 * DIA, rio: DIA, por: "día" },
  ano: { texto: "Año", ms: 366 * DIA, rio: 7 * DIA, por: "semana" },
};
export const ZOOMS = Object.keys(ZOOM) as Zoom[];

/** La escala en la que caben bien las versiones (al menos 6 a la vista, o todas). */
export function zoomInicial(horas: number[], ahora: number): Zoom {
  for (const z of ZOOMS) if (horas.filter((t) => t > ahora - ZOOM[z].ms).length >= Math.min(6, horas.length)) return z;
  return "ano";
}

const fmtHora = new Intl.DateTimeFormat("es", { hour: "2-digit", minute: "2-digit" });
const fmtDiaSemana = new Intl.DateTimeFormat("es", { weekday: "short", day: "numeric" });
const fmtDia = new Intl.DateTimeFormat("es", { day: "numeric", month: "short" });
const fmtMes = new Intl.DateTimeFormat("es", { month: "short" });
const fmtMesAno = new Intl.DateTimeFormat("es", { month: "short", year: "numeric" });

/** Marcas del eje entre `desde` y `hasta` según la escala, alineadas al calendario. */
export function marcasEje(desde: number, hasta: number, z: Zoom): { t: number; texto: string; fuerte: boolean }[] {
  const out: { t: number; texto: string; fuerte: boolean }[] = [];
  const d = new Date(desde);
  if (z === "dia") {
    const c = new Date(d.getFullYear(), d.getMonth(), d.getDate(), Math.floor(d.getHours() / 3) * 3);
    for (let t = c.getTime(); t <= hasta; t += 3 * HORA) {
      const x = new Date(t);
      if (t >= desde) out.push({ t, texto: x.getHours() === 0 ? fmtDia.format(x) : fmtHora.format(x), fuerte: x.getHours() === 0 });
    }
  } else if (z === "semana" || z === "mes") {
    const paso = z === "semana" ? 1 : 5;
    for (let x = new Date(d.getFullYear(), d.getMonth(), d.getDate()); x.getTime() <= hasta; x = new Date(x.getFullYear(), x.getMonth(), x.getDate() + 1)) {
      if (x.getTime() < desde) continue;
      if (z === "mes" && x.getDate() !== 1 && (x.getDate() % paso !== 0 || x.getDate() > 29)) continue;
      out.push({ t: x.getTime(), texto: z === "semana" ? fmtDiaSemana.format(x) : fmtDia.format(x), fuerte: x.getDate() === 1 });
    }
  } else {
    for (let x = new Date(d.getFullYear(), d.getMonth(), 1); x.getTime() <= hasta; x = new Date(x.getFullYear(), x.getMonth() + 1, 1)) {
      if (x.getTime() >= desde) out.push({ t: x.getTime(), texto: x.getMonth() === 0 ? fmtMesAno.format(x) : fmtMes.format(x), fuerte: x.getMonth() === 0 });
    }
  }
  return out;
}

/**
 * El hueco de color de cada copia: las tres primeras (en el orden de la
 * configuración) toman los tres colores; las demás y las versiones sin
 * copia, «otras» en tinta neutra. Validados para daltonismo (todos los
 * pares, claro y oscuro: scripts de dataviz); además cada hueco tiene su
 * forma, así que el color nunca va solo.
 */
export function huecosDeCopia(copias: { id: string }[], usadas: (string | null | undefined)[]): Map<string, 0 | 1 | 2 | 3> {
  const m = new Map<string, 0 | 1 | 2 | 3>();
  const orden = [...copias.map((k) => k.id), ...usadas.filter((x): x is string => !!x)];
  for (const id of orden) if (!m.has(id) && new Set(usadas).has(id)) m.set(id, (m.size < 3 ? m.size : 3) as 0 | 1 | 2 | 3);
  return m;
}

/** La retención que se le aplica de verdad: la del almacén (si la aplica él) o la del repositorio. */
export function reglaEfectiva(repo: RepositorioResumen | null | undefined, destino: DestinoResumen | undefined, equipos: Equipo[]): { regla: Regla; quien: string | null } | null {
  if (!repo) return null;
  const alm = almacenDe(repo, destino, equipos);
  if (alm?.retencion) return { regla: alm.retencion.retencion, quien: alm.almacen.nombre };
  const r = reglaDe(repo);
  return r ? { regla: r, quien: null } : null;
}

export const NOMBRE_MOTIVO: Record<Periodo, string> = { horarias: "horaria", diarias: "diaria", semanales: "semanal", mensuales: "mensual", anuales: "anual" };
/** El rótulo de la franja de cada regla de la retención. */
export const NOMBRE_FRANJA: Record<Periodo, string> = { horarias: "por hora", diarias: "diarias", semanales: "semanales", mensuales: "mensuales", anuales: "anuales" };

/**
 * El «río»: cuántas versiones caen en cada tramo de `paso` ms, con los tramos
 * alineados a múltiplos de `paso` (así no tiembla al arrastrar) y suavizado
 * con un núcleo binomial (1 4 6 4 1) dos veces. `desde` es el inicio del
 * primer tramo; `n`, cuántos tramos. Los valores van de 0 a 1 (el máximo).
 */
export function rio(horas: number[], paso: number, desde: number, hasta: number): { desde: number; paso: number; v: number[] } {
  const ini = Math.floor(desde / paso) * paso - 3 * paso;
  const n = Math.max(1, Math.ceil((hasta - ini) / paso) + 4);
  let v = new Array<number>(n).fill(0);
  for (const t of horas) {
    const i = Math.floor((t - ini) / paso);
    if (i >= 0 && i < n) v[i]++;
  }
  const k = [1, 4, 6, 4, 1];
  for (let pasada = 0; pasada < 2; pasada++) {
    v = v.map((_, i) => k.reduce((s, w, j) => s + w * (v[i + j - 2] ?? 0), 0) / 16);
  }
  const max = Math.max(...v);
  return { desde: ini, paso, v: max > 0 ? v.map((x) => x / max) : v };
}

/**
 * Junta las marcas que quedan demasiado cerca (en píxeles) para verse: de
 * izquierda a derecha, una marca se une al grupo anterior si está a menos de
 * `minimo` px de la última del grupo. Devuelve los grupos con su posición
 * media; los de una sola marca son marcas sueltas.
 */
export function agrupar<T extends { x: number }>(marcas: T[], minimo: number): { x: number; xs: T[] }[] {
  const orden = [...marcas].sort((a, b) => a.x - b.x);
  const out: { x: number; xs: T[] }[] = [];
  for (const m of orden) {
    const g = out.at(-1);
    if (g && m.x - g.xs.at(-1)!.x < minimo) g.xs.push(m);
    else out.push({ x: m.x, xs: [m] });
  }
  for (const g of out) g.x = g.xs.reduce((s, m) => s + m.x, 0) / g.xs.length;
  return out;
}

/**
 * Las franjas de la retención, de la más reciente a la más antigua: el tramo
 * de tiempo que guarda cada regla (de su versión más antigua a la más
 * reciente, unidas por la mitad del hueco con la de al lado para que no se
 * pisen ni dejen huecos). `n`: cuántas versiones guarda. La más reciente
 * llega hasta `ahora`.
 */
export function franjasRetencion(versiones: { id: string; t: number }[], motivos: Map<string, Periodo | null> | null, ahora: number): { p: Periodo; desde: number; hasta: number; n: number }[] {
  if (!motivos) return [];
  const grupos = new Map<Periodo, { min: number; max: number; n: number }>();
  for (const v of versiones) {
    const p = motivos.get(v.id);
    if (!p) continue;
    const g = grupos.get(p) ?? { min: Infinity, max: -Infinity, n: 0 };
    g.min = Math.min(g.min, v.t);
    g.max = Math.max(g.max, v.t);
    g.n++;
    grupos.set(p, g);
  }
  const fr = [...grupos].map(([p, g]) => ({ p, ...g })).sort((a, b) => b.max - a.max);
  return fr.map((f, i) => {
    const nueva = fr[i - 1];
    const vieja = fr[i + 1];
    // Unidas por la mitad: si dos reglas se cruzan (raro), la frontera queda entre las dos.
    const hasta = nueva ? (f.max + nueva.min) / 2 : Math.max(ahora, f.max);
    const desde = vieja ? (f.min + vieja.max) / 2 : f.min - Math.max(HORA, (f.max - f.min) * 0.02);
    return { p: f.p, desde: Math.min(desde, hasta), hasta, n: f.n };
  });
}

/** Por qué se queda cada versión (de la lista en cualquier orden), con su id: null = la próxima retención la quitaría. */
export function retencionDe(versiones: { id: string; hora: string }[], regla: Regla | null): Map<string, Periodo | null> | null {
  if (!regla) return null;
  const orden = [...versiones].sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora));
  const motivos = motivosQuedan(
    orden.map((v) => new Date(v.hora)),
    regla,
  );
  return new Map(orden.map((v, i) => [v.id, motivos[i]]));
}
