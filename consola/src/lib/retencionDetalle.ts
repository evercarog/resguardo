// «Retención en detalle» (la página `…/repositorios/[r]/retencion`): lo que no
// es pantalla.
//
// - Lo que se eliminó: las vueltas de la retención que anota cada equipo en su
//   historial (v1.4x, `tipo: "retencion"`; agente: retencion_registro.rs): la
//   del propio equipo, la del almacén (en el historial del almacén, por usuario
//   y carpeta) y la de la copia externa (en su destino). Con las versiones que
//   quitó cada una (las 50 vueltas más recientes; de las anteriores, las cifras).
// - Lo que se eliminará: una simulación con la regla de ahora (las reglas de
//   `restic forget`, como `seQuedan`/`motivosQuedan`, por copia: restic agrupa
//   por equipo y carpetas) sobre las versiones que conoce la consola, con las
//   copias que vendrán según su horario. Es una simulación: decide restic, y el
//   almacén no toca las versiones con una hora que no cuadra con su subida.
import { HUECOS, leerPlazo, PERIODOS, plazoEnPalabras, restarPlazo, SIEMPRE, type Periodo } from "./retencion";
import type { Regla } from "./tipos";

// ── Lo que anota el equipo ─────────────────────────────────────────────────

export type Origen = "equipo" | "almacen" | "externa";
export type Por = "orden" | "ventana" | "automatica";

/** Una entrada `retencion` del historial del equipo (tal como la manda el agente). */
export interface EntradaRetencion {
  id: string;
  hora: string;
  tipo: "retencion";
  inicio?: string;
  origen?: Origen;
  por?: Por;
  repo?: string;
  /** En el almacén: el usuario (equipo dueño); `repo` es entonces su carpeta. */
  usuario?: string;
  regla?: Regla;
  resultado?: "ok" | "fallo";
  mensaje?: string;
  antes?: number;
  quedan?: number;
  quitadas?: number;
  /** Bytes que liberó `prune`. */
  liberado?: number;
  sospechosas?: number;
  /** Cada grupo de restic (equipo y carpetas): su copia o, si no se sabe (almacén), unas versiones que quedan en él (la más reciente primero). */
  grupos?: { copia?: string | null; refs?: string[]; quedan?: number }[];
  motivos?: string[];
  /** [id corto, hora (s), grupo, bytes de sus archivos, motivo]. */
  versiones?: [string, number | null, number | null, number | null, number | null][];
  /** Quitadas que no caben en la lista. */
  mas?: number;
  /** Vuelta antigua: solo las cifras. */
  compactada?: boolean;
}

/** Por qué se quitó (o se quitaría) una versión. */
export interface Motivo {
  /** `cupo`: ya había tantas de ese tipo; `plazo`: fuera del plazo; `repe`: otra más reciente en el mismo hueco; `restic`: no cuadra con la simulación. */
  tipo: "cupo" | "plazo" | "repe" | "restic";
  periodo: Periodo | null;
}

export function leerMotivo(s: string | null | undefined): Motivo | null {
  if (!s) return null;
  const [t, p] = s.split(":");
  if (t === "restic") return { tipo: "restic", periodo: null };
  if ((t === "cupo" || t === "plazo" || t === "repe") && (PERIODOS as readonly string[]).includes(p)) return { tipo: t, periodo: p as Periodo };
  return null;
}

export interface VersionQuitada {
  id: string;
  hora: string | null;
  copia: string | null;
  bytes: number | null;
  motivo: Motivo | null;
}

export interface VueltaRetencion {
  id: string;
  hora: string;
  inicio: string | null;
  origen: Origen;
  por: Por;
  repo: string;
  usuario: string | null;
  regla: Regla | null;
  ok: boolean;
  mensaje: string | null;
  antes: number | null;
  quedan: number | null;
  quitadas: number | null;
  liberado: number | null;
  sospechosas: number;
  versiones: VersionQuitada[];
  /** Quitadas que no están en la lista (más de 2000, o una vuelta antigua). */
  mas: number;
  compactada: boolean;
}

const num = (x: unknown): number | null => (typeof x === "number" && Number.isFinite(x) ? x : null);

/**
 * Una entrada del historial como vuelta. `copiaDe`: la copia de una versión por
 * su id corto (las del informe), para los grupos que el almacén no sabe nombrar.
 */
export function leerVuelta(e: EntradaRetencion, copiaDe: (id: string) => string | null = () => null): VueltaRetencion | null {
  if (e?.tipo !== "retencion" || typeof e.hora !== "string") return null;
  const origen: Origen = e.origen === "almacen" || e.origen === "externa" ? e.origen : "equipo";
  const por: Por = e.por === "ventana" || e.por === "automatica" ? e.por : "orden";
  const grupos = (e.grupos ?? []).map((g) => g?.copia ?? (Array.isArray(g?.refs) ? g.refs.map((r) => copiaDe(r)).find((x) => !!x) : null) ?? null);
  const versiones: VersionQuitada[] = (Array.isArray(e.versiones) ? e.versiones : [])
    .filter((v) => Array.isArray(v) && typeof v[0] === "string")
    .map(([id, t, g, b, m]) => ({
      id,
      hora: num(t) !== null ? new Date(t! * 1000).toISOString() : null,
      copia: num(g) !== null ? (grupos[g!] ?? null) : null,
      bytes: num(b),
      motivo: num(m) !== null ? leerMotivo(e.motivos?.[m!]) : null,
    }));
  const quitadas = num(e.quitadas);
  return {
    id: e.id,
    hora: e.hora,
    inicio: e.inicio ?? null,
    origen,
    por,
    repo: e.repo ?? "",
    usuario: e.usuario ?? null,
    regla: e.regla ?? null,
    ok: e.resultado !== "fallo",
    mensaje: e.mensaje ?? null,
    antes: num(e.antes),
    quedan: num(e.quedan),
    quitadas,
    liberado: num(e.liberado),
    sospechosas: num(e.sospechosas) ?? 0,
    versiones,
    mas: Math.max(num(e.mas) ?? 0, (quitadas ?? 0) - versiones.length),
    compactada: !!e.compactada,
  };
}

/**
 * Las vueltas de un repositorio, de la más reciente a la más antigua: las del
 * equipo y su copia externa (historial del equipo, por `repo`) y las del
 * almacén (historial del almacén, por usuario y carpeta).
 */
export function vueltasDelRepo(o: {
  propias: EntradaRetencion[];
  delAlmacen?: EntradaRetencion[];
  repo: string;
  enAlmacen?: { usuario: string; carpeta: string } | null;
  copiaDe?: (id: string) => string | null;
}): VueltaRetencion[] {
  const propias = o.propias.filter((e) => e.repo === o.repo && e.origen !== "almacen");
  const alm = o.enAlmacen ? (o.delAlmacen ?? []).filter((e) => e.origen === "almacen" && e.usuario === o.enAlmacen!.usuario && e.repo === o.enAlmacen!.carpeta) : [];
  const vistas = new Set<string>();
  return [...propias, ...alm]
    .filter((e) => !vistas.has(e.id) && vistas.add(e.id))
    .map((e) => leerVuelta(e, o.copiaDe))
    .filter((v): v is VueltaRetencion => !!v)
    .sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora));
}

export interface Totales {
  vueltas: number;
  quitadas: number;
  liberado: number;
  /** ¿Alguna vuelta sin la cifra de lo liberado? */
  liberadoIncompleto: boolean;
  fallidas: number;
}
export function totales(vs: VueltaRetencion[]): Totales {
  return {
    vueltas: vs.length,
    quitadas: vs.reduce((s, v) => s + (v.quitadas ?? v.versiones.length), 0),
    liberado: vs.reduce((s, v) => s + (v.liberado ?? 0), 0),
    liberadoIncompleto: vs.some((v) => v.liberado === null && (v.quitadas ?? 0) > 0),
    fallidas: vs.filter((v) => !v.ok).length,
  };
}

/** Quién la aplicó, en corto: «Equipo», «Almacén X», «Copia externa». */
export function quienCorto(v: VueltaRetencion, almacen?: string | null): string {
  if (v.origen === "almacen") return `Almacén ${almacen ?? "(otro equipo)"}`;
  if (v.origen === "externa") return "Copia externa";
  return "Equipo";
}

/** Y en una frase. */
export function quienAplico(v: VueltaRetencion, nombres: { equipo: string; almacen?: string | null }): string {
  if (v.origen === "almacen") return `El almacén ${nombres.almacen ?? "(otro equipo)"}, ${v.por === "automatica" ? "a su hora (automática)" : "al pedirlo desde la consola («Aplicar ahora»)"}.`;
  if (v.origen === "externa") return `${nombres.equipo}, en el destino de su copia externa, al subirla (automática).`;
  return `${nombres.equipo}, ${v.por === "ventana" ? "desde su ventana" : "con una orden de la consola"}.`;
}

// ── En palabras ───────────────────────────────────────────────────────────

const SINGULAR: Record<Periodo, string> = { horarias: "horaria", diarias: "diaria", semanales: "semanal", mensuales: "mensual", anuales: "anual" };
const EN_ESE: Record<Periodo, string> = { horarias: "en esa hora", diarias: "ese día", semanales: "esa semana", mensuales: "ese mes", anuales: "ese año" };
const fmtDiaMes = new Intl.DateTimeFormat("es", { day: "numeric", month: "short" });
const fmtMes = new Intl.DateTimeFormat("es", { month: "long", year: "numeric" });
const fmtHora = new Intl.DateTimeFormat("es", { hour: "2-digit", minute: "2-digit" });
const sinPunto = (s: string) => s.replace(/\.$/, "").replace(/\. /g, " ");

/** «de las 14:00 del 3 oct», «del 3 oct», «de la semana del 29 sep», «de octubre de 2026», «de 2026». */
export function etiquetaHueco(p: Periodo, d: Date): string {
  switch (p) {
    case "horarias": {
      const h = new Date(d.getFullYear(), d.getMonth(), d.getDate(), d.getHours());
      return `de las ${fmtHora.format(h)} del ${sinPunto(fmtDiaMes.format(d))}`;
    }
    case "diarias":
      return `del ${sinPunto(fmtDiaMes.format(d))}`;
    case "semanales": {
      const lunes = new Date(d.getFullYear(), d.getMonth(), d.getDate() - ((d.getDay() + 6) % 7));
      return `de la semana del ${sinPunto(fmtDiaMes.format(lunes))}`;
    }
    case "mensuales":
      return `de ${fmtMes.format(d)}`;
    case "anuales":
      return `de ${d.getFullYear()}`;
  }
}

const cantidad = (r: Regla, k: Periodo) => (k === "horarias" ? (r.horarias ?? 0) : r[k]) || 0;
const plazoDe = (r: Regla, k: Periodo) => r.plazos?.[k] || null;

/** Por qué se va (o se fue): «Era la diaria del 3 oct, pero ya había 7 diarias más recientes». */
export function textoSeVa(m: Motivo | null, hora: Date | null, regla: Regla | null): string {
  if (!m) return "Sin motivo anotado (de un agente anterior o con una regla que la consola no conoce).";
  if (m.tipo === "restic" || !m.periodo) return "La quitó restic, aunque la simulación no lo esperaba (otra agrupación, o la regla cambió).";
  const p = m.periodo;
  if (m.tipo === "repe") return `Ya había otra versión más reciente ${EN_ESE[p]}.`;
  const era = hora ? `Era la ${SINGULAR[p]} ${etiquetaHueco(p, hora)}` : `Era la última ${SINGULAR[p]} de su ${p === "horarias" ? "hora" : "periodo"}`;
  if (m.tipo === "cupo") {
    const n = regla ? cantidad(regla, p) : 0;
    return `${era}, pero ya había ${n > 0 ? `${n} ${n === 1 ? SINGULAR[p] : p}` : `las ${p} que pide la regla`} más ${n === 1 ? "reciente" : "recientes"}.`;
  }
  const plazo = regla ? plazoDe(regla, p) : null;
  return `${era}, pero quedaba fuera del plazo ${plazo ? `de ${plazoEnPalabras(plazo)} ` : ""}de las ${p}.`;
}

/** Por qué se queda: «Se queda: la diaria del 3 oct». */
export const textoSeQueda = (p: Periodo, hora: Date) => `Se queda: la ${SINGULAR[p]} ${etiquetaHueco(p, hora)}.`;

// ── La simulación ─────────────────────────────────────────────────────────

/** Por qué se queda (el periodo más largo que la guarda) o por qué se iría. */
export interface Explicacion {
  queda: Periodo | null;
  motivo: Motivo | null;
}

/**
 * Las reglas de `restic forget` sobre un grupo (de la más reciente a la más
 * antigua), diciendo por qué: lo mismo que `seQuedan` y, en el agente,
 * `retencion_registro::motivos`.
 */
export function explicar(horas: Date[], r: Regla): Explicacion[] {
  const out: Explicacion[] = horas.map(() => ({ queda: null, motivo: null }));
  if (!horas.length) return out;
  const limites = PERIODOS.map((k) => {
    const p = leerPlazo(plazoDe(r, k));
    return p ? restarPlazo(horas[0], p).getTime() : null;
  });
  const iniciales = PERIODOS.map((k) => cantidad(r, k));
  const cuantas = [...iniciales];
  const activo = PERIODOS.map((k, i) => iniciales[i] !== 0 || limites[i] !== null);
  const masCorto = Math.max(0, activo.indexOf(true));
  const anterior: (number | null)[] = PERIODOS.map(() => null);
  const enPlazo: (number | null)[] = PERIODOS.map(() => null);
  const visto: (number | null)[] = PERIODOS.map(() => null);
  const ultima = horas.length - 1;
  horas.forEach((t, n) => {
    let queda: Periodo | null = null;
    let motivo: Motivo | null = null;
    for (let i = 0; i < PERIODOS.length; i++) {
      const h = HUECOS[i](t);
      const nuevo = visto[i] !== h;
      visto[i] = h;
      if (cuantas[i] > 0 || cuantas[i] === SIEMPRE) {
        if (anterior[i] !== h || n === ultima) {
          queda = PERIODOS[i];
          anterior[i] = h;
          if (cuantas[i] > 0) cuantas[i]--;
        }
      } else if (iniciales[i] !== 0 && nuevo) motivo = { tipo: "cupo", periodo: PERIODOS[i] };
      const lim = limites[i];
      if (lim !== null) {
        if (t.getTime() > lim) {
          if (enPlazo[i] !== h || n === ultima) {
            queda = PERIODOS[i];
            enPlazo[i] = h;
          }
        } else if (nuevo) motivo = { tipo: "plazo", periodo: PERIODOS[i] };
      }
    }
    out[n] = queda ? { queda, motivo: null } : { queda: null, motivo: motivo ?? { tipo: "repe", periodo: PERIODOS[masCorto] } };
  });
  return out;
}

export interface VersionSim {
  id: string;
  hora: string;
  copia: string | null;
}

export interface Prevision {
  /** Lo que quitaría la próxima vuelta: en `cuando` (su hora, si tiene horario) o al aplicarla (`null`). */
  proxima: { cuando: number | null; ids: string[] };
  /** Cada versión: cómo queda en la próxima vuelta y, si deja de entrar dentro del horizonte, cuándo y por qué. */
  porVersion: Map<string, { ahora: Explicacion; seVa: number | null; motivoSeVa: Motivo | null }>;
  /** Los días (desde hoy, `dias` días) en que dejan de entrar en la regla, con las que lo hacen. */
  porDia: { dia: string; t: number; ids: string[] }[];
  /** La consola no conoce todas las versiones (solo las de 60 días del informe). */
  incompleta: boolean;
  /** Copias que se suponen (según su horario) en el horizonte. */
  supuestas: number;
  /** Copias (de las versiones) sin horario conocido: no se suponen versiones nuevas suyas. */
  sinHorario: (string | null)[];
}

const DIA = 86_400_000;
const inicioDia = (t: number, n = 0) => {
  const d = new Date(t);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + n).getTime();
};
export const claveDia = (t: number) => {
  const d = new Date(t);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};

/**
 * Qué quitará la retención: la próxima vuelta y, día a día durante `dias`,
 * las que irán dejando de entrar en la regla según llegan las copias nuevas
 * (`horasDe`: las horas de copia de cada copia; sin horario, no se suponen).
 * Por copia (restic agrupa por equipo y carpetas). Si hay más versiones de las
 * que se conocen (`total`), la más antigua conocida no cuenta como «la más
 * antigua de todas» (restic la guarda solo si es la última de verdad).
 */
export function prever(o: {
  versiones: VersionSim[];
  total?: number | null;
  regla: Regla;
  ahora: number;
  proxima?: number | null;
  dias: number;
  horasDe?: (copia: string | null) => ((fecha: Date) => string[]) | null;
}): Prevision {
  const { regla, ahora, dias } = o;
  const incompleta = (o.total ?? 0) > o.versiones.length;
  const grupos = new Map<string, VersionSim[]>();
  for (const v of o.versiones) (grupos.get(v.copia ?? "") ?? grupos.set(v.copia ?? "", []).get(v.copia ?? "")!).push(v);
  const fin = inicioDia(ahora, dias);
  const t0 = o.proxima && o.proxima > ahora ? o.proxima : ahora;
  // Las copias que vendrán, por grupo (hasta el final del horizonte o la próxima vuelta).
  const hasta = Math.max(fin, t0);
  const futuras = new Map<string, number[]>();
  const sinHorario: (string | null)[] = [];
  let supuestas = 0;
  for (const [k, vs] of grupos) {
    const f = o.horasDe?.(vs[0].copia ?? null) ?? null;
    if (!f) {
      sinHorario.push(vs[0].copia ?? null);
      continue;
    }
    const ts: number[] = [];
    for (let d = 0; inicioDia(ahora, d) < hasta; d++) {
      const fecha = new Date(inicioDia(ahora, d));
      for (const hm of f(fecha)) {
        const [h, m] = hm.split(":").map(Number);
        const t = new Date(fecha.getFullYear(), fecha.getMonth(), fecha.getDate(), h, m).getTime();
        if (t > ahora && t <= hasta) ts.push(t);
      }
    }
    futuras.set(k, ts);
    supuestas += ts.filter((t) => t <= fin).length;
  }
  const centinela = -8.64e15;
  /** Cómo queda cada versión de ahora si se aplicara la retención en `T`. */
  const en = (T: number) => {
    const m = new Map<string, Explicacion>();
    for (const [k, vs] of grupos) {
      const lista: { id: string | null; t: number }[] = [...vs.map((v) => ({ id: v.id, t: Date.parse(v.hora) })), ...(futuras.get(k) ?? []).filter((t) => t <= T).map((t) => ({ id: null, t }))];
      lista.sort((a, b) => b.t - a.t || (b.id ?? "").localeCompare(a.id ?? ""));
      if (incompleta) lista.push({ id: null, t: centinela });
      const ex = explicar(
        lista.map((x) => new Date(x.t)),
        regla,
      );
      lista.forEach((x, i) => x.id && m.set(x.id, ex[i]));
    }
    return m;
  };
  const primera = en(t0);
  const proxIds = [...primera].filter(([, x]) => !x.queda).map(([id]) => id);
  const porVersion = new Map<string, { ahora: Explicacion; seVa: number | null; motivoSeVa: Motivo | null }>();
  for (const [id, x] of primera) porVersion.set(id, { ahora: x, seVa: x.queda ? null : t0, motivoSeVa: x.queda ? null : x.motivo });
  const porDia: Prevision["porDia"] = [];
  for (let d = 0; d < dias; d++) {
    const T = inicioDia(ahora, d + 1) - 1;
    if (T <= t0) continue;
    const ids: string[] = [];
    for (const [id, x] of en(T)) {
      const p = porVersion.get(id)!;
      if (!x.queda && p.seVa === null) {
        p.seVa = T;
        p.motivoSeVa = x.motivo;
        ids.push(id);
      }
    }
    if (ids.length) porDia.push({ dia: claveDia(T), t: inicioDia(T), ids });
  }
  const orden = (ids: string[]) => {
    const t = new Map(o.versiones.map((v) => [v.id, Date.parse(v.hora)]));
    return ids.sort((a, b) => t.get(b)! - t.get(a)!);
  };
  porDia.forEach((x) => orden(x.ids));
  return { proxima: { cuando: o.proxima && o.proxima > ahora ? o.proxima : null, ids: orden(proxIds) }, porVersion, porDia, incompleta, supuestas, sinHorario };
}

/** Lo que pasa cada día en el calendario: versiones quitadas (vueltas de ese día) y las que dejarán de entrar. */
export function marcasPorDia(vueltas: VueltaRetencion[], p: Prevision | null, ahora: number): Map<string, { quitadas: number; vueltas: number; fallos: number; previstas: number }> {
  const m = new Map<string, { quitadas: number; vueltas: number; fallos: number; previstas: number }>();
  const de = (k: string) => m.get(k) ?? m.set(k, { quitadas: 0, vueltas: 0, fallos: 0, previstas: 0 }).get(k)!;
  for (const v of vueltas) {
    const x = de(claveDia(Date.parse(v.hora)));
    x.vueltas++;
    x.quitadas += v.quitadas ?? v.versiones.length;
    if (!v.ok) x.fallos++;
  }
  if (p) {
    if (p.proxima.ids.length) de(claveDia(p.proxima.cuando ?? ahora)).previstas += p.proxima.ids.length;
    for (const d of p.porDia) de(d.dia).previstas += d.ids.length;
  }
  return m;
}

export { DIA };

/** Una fila de las listas de versiones de la página. */
export interface FilaVersion {
  id: string;
  hora: string | null;
  /** El nombre de su copia. */
  copia: string | null;
  bytes: number | null;
  porque: string;
  /** «va»: se fue o se irá; «queda»: se queda. */
  tono?: "va" | "queda";
}
