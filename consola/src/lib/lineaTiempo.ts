// La línea de tiempo de las versiones («máquina del tiempo», docs/diseno.md §4):
// lo que no es pantalla. El «calendario de calor» (días × horas, o un año día
// a día como el historial de contribuciones), la «bitácora» (la lista por días),
// el color de cada copia (tres de una paleta validada para daltonismo, en orden
// fijo; las demás, en tinta neutra) y la retención simulada (qué se queda y
// por qué, qué quitaría la próxima vez).
import type { DestinoResumen, Equipo, Regla, RepositorioResumen } from "./tipos";
import { almacenDe, motivosQuedan, reglaDe, type Periodo } from "./retencion";

const HORA = 3_600_000;
const DIA = 24 * HORA;

/** Una versión en la línea de tiempo (del informe o de la sesión de Restaurar). */
export interface VersionLinea {
  id: string;
  hora: string;
  copia?: string | null;
  /** Su repositorio (un equipo enseña los de todos sus repositorios a la vez). */
  repo?: string | null;
  /** Lo que ocupan sus archivos. */
  bytes?: number | null;
  /** Lo nuevo que añadió. */
  anadido?: number | null;
  archivos?: number | null;
  etiquetas?: string[];
}

/** Lo que se ve del calendario: los últimos 7, 30, 60 o 365 días. */
export type Rango = 7 | 30 | 60 | 365;
export const RANGOS: { dias: Rango; texto: string }[] = [
  { dias: 7, texto: "7 días" },
  { dias: 30, texto: "30 días" },
  { dias: 60, texto: "60 días" },
  { dias: 365, texto: "Un año" },
];

/** El inicio del día (hora local) de `t`, `n` días después. */
export function inicioDia(t: number, n = 0): number {
  const d = new Date(t);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + n).getTime();
}
/** «2026-09-29» (hora local): la clave de un día, la misma que la de la URL. */
export const claveDia = (t: number) => {
  const d = new Date(t);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};

/**
 * El alto (px) del marco del calendario: el mayor de sus vistas (por horas
 * con `filas` filas, el año de 13 meses o la tira del móvil), para que cambiar
 * de periodo no mueva lo de debajo. Las medidas son las de CalendarioCalor:
 * rótulos de 14, cabecera de días de 18, casillas de 12 como poco (26 la tira;
 * en el año, 10, o 9 en el móvil, con «Desliza…» debajo) y 3 de separación
 * vertical. Dentro, las filas crecen hasta llenarlo (`altoFila`).
 */
export function altoCalendario(filas: number, movil: boolean): number {
  const ano = 14 + 3 + 13 * (movil ? 9 : 10) + 12 * 3 + 4 + (movil ? 30 : 0);
  if (movil) return Math.max(14 + 3 + 26 + 3 + 14 + 4, ano);
  return Math.max(14 + 3 + 18 + 3 + filas * 12 + (filas - 1) * 3 + 3 + 14, ano);
}

/**
 * El alto (px) de cada fila de casillas para llenar el marco de alto `marco`:
 * así la sección tiene las mismas proporciones en todos los periodos (el año
 * no se queda pequeño en medio de un marco grande). Por horas, de 12 a 26; en
 * el año, de 9 a 24 (`pista`: se reserva sitio para «Desliza…» en el móvil);
 * la tira, siempre 26.
 */
export function altoFila(modo: Calendario["modo"], filas: number, marco: number, pista = false): number {
  if (modo === "tira" || filas <= 0) return 26;
  const fijo = modo === "horas" ? 14 + 3 + 18 + 3 + 3 + 14 : 14 + 3 + 4 + (pista ? 30 : 0);
  const [min, max] = modo === "horas" ? [12, 26] : [9, 24];
  const cabe = Math.floor((marco - fijo - (filas - 1) * 3) / filas);
  return Math.max(min, Math.min(max, cabe));
}

/** Un día en la URL («2026-09-29», hora local) como su inicio; null si no vale. */
export function diaDeClave(k: string | null | undefined): number | null {
  if (!k || !/^\d{4}-\d{2}-\d{2}$/.test(k)) return null;
  const t = Date.parse(`${k}T00:00:00`);
  return Number.isFinite(t) ? t : null;
}

const fmtCorto = new Intl.DateTimeFormat("es", { day: "numeric", month: "short" });
const fmtCortoAno = new Intl.DateTimeFormat("es", { day: "numeric", month: "short", year: "numeric" });
/** «Del 3 al 10 oct», «Del 28 sept al 4 oct», con el año si no es este; un solo día, con `nombreDia`. */
export function nombreIntervalo(desde: number, hasta: number, ahora: number): string {
  if (inicioDia(desde) === inicioDia(hasta)) return nombreDia(desde, ahora);
  const a = new Date(desde);
  const b = new Date(hasta);
  const esteAno = a.getFullYear() === new Date(ahora).getFullYear() && b.getFullYear() === a.getFullYear();
  const f = esteAno ? fmtCorto : fmtCortoAno;
  if (a.getMonth() === b.getMonth() && a.getFullYear() === b.getFullYear()) return `Del ${a.getDate()} al ${f.format(b)}`;
  return `Del ${(esteAno ? fmtCorto : fmtCortoAno).format(a)} al ${f.format(b)}`;
}

/** El rango inicial: el menor en el que cae al menos la mitad de las versiones. */
export function rangoInicial(horas: number[], ahora: number): Rango {
  if (!horas.length) return 30;
  for (const { dias } of RANGOS) {
    const desde = inicioDia(ahora, 1 - dias);
    if (horas.filter((t) => t >= desde).length * 2 >= horas.length) return dias;
  }
  return 365;
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

// ── El calendario de calor ────────────────────────────────────────────────

/** Una casilla: un tramo de tiempo con sus versiones. */
export interface Celda {
  /** «fila-columna»: para el teclado y el ratón. */
  k: string;
  desde: number;
  hasta: number;
  /** Las versiones del tramo, de la más reciente a la más antigua. */
  ids: string[];
  /** Cuántas quitaría la próxima retención (0 sin regla). */
  quitan: number;
  /** Cuántas cosas fallaron en el tramo (copias, comprobaciones…): una marca aparte del color. */
  fallos: number;
  /** 0 sin versiones; 1–4, de menos a más (relativo al máximo a la vista). */
  nivel: 0 | 1 | 2 | 3 | 4;
  /** Después de ahora (aún no puede haber nada). */
  futura: boolean;
  /** Antes del rango (la primera semana del año, incompleta). */
  fuera: boolean;
  /** El tramo de ahora mismo. */
  ahora: boolean;
}
export interface Columna {
  desde: number;
  hasta: number;
  /** El mes, en la primera columna de cada mes. */
  mes: string | null;
  /** Debajo: el día del mes (vista por horas) o nada. */
  pie: string | null;
  /** La inicial del día de la semana (vista por horas). */
  inicial: string | null;
  hoy: boolean;
  /** Fin de semana (vista por horas): una pista más tenue en la cabecera. */
  finde: boolean;
  /** El día entero (vista por horas: la cabecera se puede pulsar). */
  dia: Celda | null;
}
export interface Calendario {
  /** «horas»: columnas = días, filas = horas; «meses» (un año): filas = meses, columnas = días del mes; «tira»: una fila de días. */
  modo: "horas" | "meses" | "tira";
  /** `ano`: la fila empieza un año (enero, salvo la primera): lleva una raya encima. */
  filas: { texto: string; corto: string | null; ano?: boolean }[];
  columnas: Columna[];
  celdas: Celda[][];
  /** Cuántas horas junta cada fila (vista por horas). */
  paso: number;
  /** Lo que hay en el rango. */
  total: number;
  quitan: number;
  diasCon: number;
  max: number;
}

const fmtMes = new Intl.DateTimeFormat("es", { month: "short" });
const fmtMesAno = new Intl.DateTimeFormat("es", { month: "short", year: "numeric" });
const fmtMesLargo = new Intl.DateTimeFormat("es", { month: "long", year: "numeric" });
const INICIALES = ["D", "L", "M", "X", "J", "V", "S"];

/** Nivel de intensidad (cuatro escalones, como el historial de contribuciones). */
export function nivelDe(n: number, max: number): 0 | 1 | 2 | 3 | 4 {
  if (n <= 0) return 0;
  if (max <= 1) return 4;
  return Math.max(1, Math.min(4, Math.ceil((n / max) * 4))) as 1 | 2 | 3 | 4;
}

/**
 * Las filas de horas: si todas las versiones del rango caen en 14 horas
 * seguidas o menos (un horario de oficina), una fila por hora de esas;
 * si no, las 24 horas en filas de 2.
 */
export function filasHoras(horas: number[]): { desde: number; paso: number; n: number } {
  if (!horas.length) return { desde: 0, paso: 2, n: 12 };
  const hs = horas.map((t) => new Date(t).getHours());
  const min = Math.min(...hs);
  const max = Math.max(...hs);
  if (max - min + 1 <= 14) {
    // Un poco de aire alrededor, sin pasarse del día.
    const a = Math.max(0, min - 1);
    const b = Math.min(23, max + 1);
    return { desde: a, paso: 1, n: b - a + 1 };
  }
  return { desde: 0, paso: 2, n: 12 };
}

/**
 * El calendario de las versiones de los últimos `dias` días. `tira`: una sola
 * fila de días (el móvil); con 365 días, siempre un mes por fila y los días del
 * mes en columnas (también en el móvil).
 * `filas`: las filas de horas ya elegidas (las mismas en 7, 30 y 60 días, para
 * que el calendario no cambie de alto al cambiar de periodo); sin ellas, las
 * de lo que hay en el rango.
 */
export function calendario(
  versiones: { id: string; t: number }[],
  motivos: Map<string, Periodo | null> | null,
  ahora: number,
  dias: Rango,
  tira = false,
  fallos: number[] = [],
  filas_?: { desde: number; paso: number; n: number },
): Calendario {
  const hoy = inicioDia(ahora);
  const inicio = inicioDia(ahora, 1 - dias);
  const enRango = versiones.filter((v) => v.t >= inicio && v.t <= ahora).sort((a, b) => b.t - a.t);
  const fallosEnRango = fallos.filter((t) => t >= inicio && t <= ahora);
  const quita = (id: string) => !!motivos && motivos.get(id) === null;
  const mk = (k: string, desde: number, hasta: number, fuera = false): Celda => ({ k, desde, hasta, ids: [], quitan: 0, fallos: 0, nivel: 0, futura: desde > ahora, fuera, ahora: desde <= ahora && ahora < hasta });

  let modo: Calendario["modo"];
  let filas: Calendario["filas"];
  let columnas: Columna[] = [];
  let celdas: Celda[][];
  let paso = 24;
  // El mes, en la primera columna de cada mes (la del principio se quita si la siguiente queda pegada).
  const ponMes = (cols: Columna[]) => {
    let anterior = -1;
    let ultima = -1;
    cols.forEach((c, i) => {
      const d = new Date(Math.max(c.desde, inicio));
      if (d.getMonth() === anterior) return;
      c.mes = d.getMonth() === 0 || anterior === -1 ? fmtMesAno.format(d) : fmtMes.format(d);
      if (ultima === 0 && i < 4) cols[0].mes = null;
      anterior = d.getMonth();
      ultima = i;
    });
  };

  if (dias === 365) {
    // Un mes por fila (del más antiguo arriba a este abajo) y los días del mes
    // en columnas (del 1 al 31): cada mes empieza y acaba en su fila, así que
    // se ve de un vistazo dónde está cada uno, y el calendario tiene las mismas
    // proporciones que la vista por horas (un rótulo a la izquierda, filas de
    // casillas y la cabecera arriba). Los días que no tiene el mes (el 30 de
    // febrero) y los de antes del periodo quedan vacíos.
    modo = "meses";
    const primero = new Date(inicio);
    const ultimo = new Date(hoy);
    const nMeses = (ultimo.getFullYear() - primero.getFullYear()) * 12 + ultimo.getMonth() - primero.getMonth() + 1;
    const meses = Array.from({ length: nMeses }, (_, i) => new Date(primero.getFullYear(), primero.getMonth() + i, 1));
    filas = meses.map((m, i) => ({
      texto: fmtMesLargo.format(m),
      corto: i === 0 || m.getMonth() === 0 ? fmtMesAno.format(m) : fmtMes.format(m),
      ano: i > 0 && m.getMonth() === 0,
    }));
    const diaHoy = new Date(hoy).getDate();
    columnas = Array.from({ length: 31 }, (_, c) => ({ desde: c, hasta: c + 1, mes: null, pie: String(c + 1), inicial: null, hoy: c + 1 === diaHoy, finde: false, dia: null }));
    celdas = meses.map((m, f) => {
      const largo = new Date(m.getFullYear(), m.getMonth() + 1, 0).getDate();
      return columnas.map((_, c) => {
        const d = new Date(m.getFullYear(), m.getMonth(), c + 1).getTime();
        return mk(`${f}-${c}`, d, inicioDia(d, 1), c >= largo || d < inicio);
      });
    });
    const casilla = (t: number) => {
      const d = new Date(t);
      const f = (d.getFullYear() - primero.getFullYear()) * 12 + d.getMonth() - primero.getMonth();
      return celdas[f]?.[d.getDate() - 1];
    };
    for (const v of enRango) casilla(v.t)?.ids.push(v.id);
    for (const t of fallosEnRango) {
      const x = casilla(t);
      if (x) x.fallos++;
    }
  } else {
    // Días en columnas; en la tira, una fila; si no, las horas en filas.
    modo = tira ? "tira" : "horas";
    const fh = tira ? { desde: 0, paso: 24, n: 1 } : (filas_ ?? filasHoras([...enRango.map((v) => v.t), ...fallosEnRango]));
    paso = fh.paso;
    filas = Array.from({ length: fh.n }, (_, i) => {
      const h = fh.desde + i * fh.paso;
      const texto = tira ? "Día" : fh.paso === 1 ? `${h}:00` : `${h}:00–${h + fh.paso}:00`;
      // Un rótulo cada 3 filas por hora (o cada 6 horas en filas de 2).
      return { texto, corto: tira ? null : (fh.paso === 1 ? h % 3 === 0 : h % 6 === 0) ? `${h}:00` : null };
    });
    celdas = filas.map(() => []);
    for (let c = 0; c < dias; c++) {
      const desde = inicioDia(inicio, c);
      const d = new Date(desde);
      const dia = mk(`d-${c}`, desde, inicioDia(desde, 1));
      columnas.push({ desde, hasta: dia.hasta, mes: null, pie: String(d.getDate()), inicial: INICIALES[d.getDay()], hoy: desde === hoy, finde: d.getDay() === 0 || d.getDay() === 6, dia });
      for (let f = 0; f < fh.n; f++) {
        const a = tira ? desde : desde + (fh.desde + f * fh.paso) * HORA;
        celdas[f].push(mk(`${f}-${c}`, a, tira ? dia.hasta : a + fh.paso * HORA));
      }
    }
    ponMes(columnas);
    for (const v of enRango) {
      const c = Math.round((inicioDia(v.t) - inicio) / DIA);
      const col = columnas[c];
      if (!col) continue;
      col.dia!.ids.push(v.id);
      const h = new Date(v.t).getHours();
      const f = tira ? 0 : Math.floor((h - fh.desde) / fh.paso);
      celdas[f]?.[c]?.ids.push(v.id);
    }
    for (const t of fallosEnRango) {
      const c = Math.round((inicioDia(t) - inicio) / DIA);
      const col = columnas[c];
      if (!col) continue;
      col.dia!.fallos++;
      // Una hora fuera de las filas (las filas siguen a las versiones): la de más cerca.
      const f = tira ? 0 : Math.max(0, Math.min(fh.n - 1, Math.floor((new Date(t).getHours() - fh.desde) / fh.paso)));
      const x = celdas[f]?.[c];
      if (x) x.fallos++;
    }
  }

  const todas = celdas.flat();
  const max = Math.max(1, ...todas.map((x) => x.ids.length));
  for (const x of todas) {
    x.nivel = nivelDe(x.ids.length, max);
    x.quitan = x.ids.filter(quita).length;
  }
  const maxDia = Math.max(1, ...columnas.map((c) => c.dia?.ids.length ?? 0));
  for (const c of columnas)
    if (c.dia) {
      c.dia.nivel = nivelDe(c.dia.ids.length, maxDia);
      c.dia.quitan = c.dia.ids.filter(quita).length;
    }
  const diasCon = new Set(enRango.map((v) => claveDia(v.t))).size;
  return { modo, filas, columnas, celdas, paso, total: enRango.length, quitan: enRango.filter((v) => quita(v.id)).length, diasCon, max };
}

// ── La bitácora ───────────────────────────────────────────────────────────

const fmtDiaLargo = new Intl.DateTimeFormat("es", { weekday: "short", day: "numeric", month: "short" });
const fmtDiaAno = new Intl.DateTimeFormat("es", { weekday: "short", day: "numeric", month: "short", year: "numeric" });

/** «Hoy», «Ayer» o «Mar, 29 sept» (con el año si no es este). */
export function nombreDia(t: number, ahora: number): string {
  const d = inicioDia(t);
  if (d === inicioDia(ahora)) return "Hoy";
  if (d === inicioDia(ahora, -1)) return "Ayer";
  return (new Date(t).getFullYear() === new Date(ahora).getFullYear() ? fmtDiaLargo : fmtDiaAno).format(t).replace(/^./, (x) => x.toUpperCase());
}

/** Las versiones por días (hora local), del más reciente al más antiguo; dentro, de la más reciente a la más antigua. */
export function porDias<T extends { t: number }>(versiones: T[]): { dia: number; vs: T[] }[] {
  const m = new Map<number, T[]>();
  for (const v of [...versiones].sort((a, b) => b.t - a.t)) {
    const d = inicioDia(v.t);
    const g = m.get(d);
    if (g) g.push(v);
    else m.set(d, [v]);
  }
  return [...m].map(([dia, vs]) => ({ dia, vs }));
}
