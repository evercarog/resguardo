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
 * con `filas` filas, el año de 7 filas o la tira del móvil), para que cambiar
 * de periodo no mueva lo de debajo. Las medidas son las de CalendarioCalor:
 * rótulos de 14, cabecera de días de 18, casillas de 12 (26 la tira, 15 como
 * mucho las del año) y 3 de separación vertical.
 */
export function altoCalendario(filas: number, movil: boolean): number {
  const ano = 14 + 3 + 7 * 15 + 6 * 3 + 4;
  if (movil) return Math.max(14 + 3 + 26 + 3 + 14 + 4, ano);
  return Math.max(14 + 3 + 18 + 3 + filas * 12 + (filas - 1) * 3 + 3 + 14, ano);
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
  /** «horas»: columnas = días, filas = horas; «dias»: columnas = semanas, filas = días de la semana; «tira»: una fila de días. */
  modo: "horas" | "dias" | "tira";
  filas: { texto: string; corto: string | null }[];
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
const INICIALES = ["D", "L", "M", "X", "J", "V", "S"];
const DIAS_SEMANA = ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"];

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
 * fila de días (el móvil); con 365 días, siempre semanas × días de la semana.
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
    // Semanas (de lunes a domingo) en columnas y los días de la semana en filas.
    modo = "dias";
    const lunes = inicioDia(inicio, -((new Date(inicio).getDay() + 6) % 7));
    const n = Math.round((inicioDia(hoy, 7 - ((new Date(hoy).getDay() + 6) % 7)) - lunes) / DIA / 7);
    filas = DIAS_SEMANA.map((d, i) => ({ texto: d, corto: i % 2 === 0 && i < 6 ? INICIALES[(i + 1) % 7] : null }));
    celdas = DIAS_SEMANA.map(() => []);
    for (let c = 0; c < n; c++) {
      const desde = inicioDia(lunes, c * 7);
      columnas.push({ desde, hasta: inicioDia(desde, 7), mes: null, pie: null, inicial: null, hoy: hoy >= desde && hoy < inicioDia(desde, 7), finde: false, dia: null });
      for (let f = 0; f < 7; f++) {
        const d = inicioDia(desde, f);
        celdas[f].push(mk(`${f}-${c}`, d, inicioDia(d, 1), d < inicio));
      }
    }
    // El mes, en la semana que tiene su día 1 (y en la primera, si cabe).
    let ultima = -9;
    columnas.forEach((col, c) => {
      for (let t = Math.max(col.desde, inicio); t < col.hasta; t = inicioDia(t, 1)) {
        const d = new Date(t);
        if (d.getDate() === 1 || (c === 0 && d.getDate() < 22)) {
          col.mes = d.getMonth() === 0 ? fmtMesAno.format(d) : fmtMes.format(d);
          if (c - ultima < 3 && ultima >= 0) columnas[ultima].mes = null;
          ultima = c;
          break;
        }
      }
    });
    for (const v of enRango) {
      const c = Math.floor((inicioDia(v.t) - lunes) / DIA / 7 + 1e-6);
      const f = (new Date(v.t).getDay() + 6) % 7;
      celdas[f]?.[c]?.ids.push(v.id);
    }
    for (const t of fallosEnRango) {
      const x = celdas[(new Date(t).getDay() + 6) % 7]?.[Math.floor((inicioDia(t) - lunes) / DIA / 7 + 1e-6)];
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
