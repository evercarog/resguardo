// Formato de fechas, tamaños y horarios en español (docs/diseno.md §6):
// espacio fino antes de las unidades, coma decimal y tiempos relativos.
import { copiasAlDia, horaCorta, normalizar, reconocer, reglasDe } from "./horario";
import type { Horario, ReglaHorario } from "./tipos";

const NBSP = " ";

interface PlanScheduleLike {
  days: number[];
  mode: string;
  times: string[];
  every_hours: number;
  from: string;
  to: string;
  /** `mode: "rules"` (agente ≥ 0.7.9). */
  rules?: PlanRuleLike[];
}
interface PlanRuleLike {
  kind: "at" | "every" | "every_days" | "monthly";
  days?: number[];
  times?: string[];
  every_min?: number;
  from?: string;
  to?: string;
  every?: number;
  start?: string;
  time?: string;
  day?: number;
}
const fmtFecha = new Intl.DateTimeFormat("es", { dateStyle: "long", timeStyle: "short" });
const fmtCorta = new Intl.DateTimeFormat("es", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
const fmtDia = new Intl.DateTimeFormat("es", { weekday: "long", day: "numeric", month: "long" });
const fmtHora = new Intl.DateTimeFormat("es", { hour: "2-digit", minute: "2-digit" });

export const fechaLarga = (iso: string | null | undefined) => (iso ? fmtFecha.format(new Date(iso)) : "—");
export const fechaCorta = (iso: string | null | undefined) => (iso ? fmtCorta.format(new Date(iso)) : "—");
export const dia = (iso: string) => fmtDia.format(new Date(iso));
export const hora = (iso: string) => fmtHora.format(new Date(iso));

const fmtSemana = new Intl.DateTimeFormat("es", { weekday: "long" });

/** Una hora próxima en frase: «hoy a las 13:00», «mañana a las 09:30», «el lunes a las 13:00» o «el 4 de octubre, 13:00». */
export function cuandoFrase(iso: string, ahora = Date.now()): string {
  const d = new Date(iso);
  const inicio = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const dias = Math.round((inicio(d) - inicio(new Date(ahora))) / 86_400_000);
  const h = hora(iso);
  if (dias <= 0) return `hoy a las ${h}`;
  if (dias === 1) return `mañana a las ${h}`;
  if (dias < 7) return `el ${fmtSemana.format(d)} a las ${h}`;
  return `el ${fmtFecha.format(d)}`;
}

/** «hace 12 min», «hace 3 h», «ayer», «hace 4 días», «dentro de 2 h». */
export function relativo(iso: string | null | undefined, ahora = Date.now()): string {
  if (!iso) return "nunca";
  const d = Date.parse(iso);
  const s = Math.round((d - ahora) / 1000);
  const abs = Math.abs(s);
  const futuro = s > 0;
  const con = (n: number, u: string) => (futuro ? `dentro de ${n}${NBSP}${u}` : `hace ${n}${NBSP}${u}`);
  if (abs < 45) return "ahora mismo";
  if (abs < 3600) return con(Math.round(abs / 60), "min");
  if (abs < 86_400) return con(Math.round(abs / 3600), "h");
  const dias = Math.round(abs / 86_400);
  if (dias === 1) return futuro ? "mañana" : "ayer";
  if (dias < 45) return futuro ? `dentro de ${dias} días` : `hace ${dias} días`;
  return fechaCorta(iso);
}

/** Lo que falta para una fecha: «23 h 40 min», «12 min», «menos de un minuto». */
export function cuentaAtras(iso: string, ahora = Date.now()): string {
  const s = Math.max(0, Math.round((Date.parse(iso) - ahora) / 1000));
  if (s < 60) return "menos de un minuto";
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (h >= 48) return `${Math.round(h / 24)} días`;
  return h ? `${h}${NBSP}h ${m}${NBSP}min` : `${m}${NBSP}min`;
}

/** Bytes en unidades decimales («2,1 GB»), como restic y los sistemas. */
export function bytes(n: number | null | undefined): string {
  if (n === null || n === undefined || !Number.isFinite(n)) return "—";
  const u = ["B", "KB", "MB", "GB", "TB", "PB"];
  let i = 0;
  let v = n;
  while (v >= 1000 && i < u.length - 1) {
    v /= 1000;
    i++;
  }
  const txt = v.toLocaleString("es", { maximumFractionDigits: v < 10 && i > 0 ? 1 : 0 });
  return `${txt}${NBSP}${u[i]}`;
}

export const numero = (n: number) => n.toLocaleString("es");

const DIAS = ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"];
const DIAS_CORTOS = ["L", "M", "X", "J", "V", "S", "D"];
export { DIAS, DIAS_CORTOS };

/** Una lista en frase: «a, b y c». */
export function lista(xs: string[]): string {
  if (xs.length <= 1) return xs.join("");
  return `${xs.slice(0, -1).join(", ")} y ${xs.at(-1)}`;
}

/** Un horario en frase: «Cada día laborable a las 13:00 y 19:00». */
export function horarioEnFrase(h: Horario | PlanScheduleLike | string | null | undefined): string {
  if (!h) return "Solo a mano";
  if (typeof h === "string") return h;
  if ("days" in h) {
    // Formato del agente (PlanSchedule): días 0 = lunes.
    if (h.mode === "rules") return horarioEnFrase({ dias: [], horas: [], reglas: (h.rules ?? []).map(reglaDePlan) });
    const dias = h.days.map((d) => d + 1);
    if (h.mode === "every") {
      const base = horarioEnFrase({ dias, horas: ["00:00"] }).replace(/ a las? 00:00$/, "");
      return `${base}, cada ${h.every_hours === 1 ? "hora" : `${h.every_hours} horas`} de ${h.from} a ${h.to}`;
    }
    return horarioEnFrase({ dias, horas: h.times });
  }
  // v1.24: reglas que se suman («Cada 10 minutos de 8:00 a 18:00, de lunes a viernes y el día 1 de cada mes a las 23:00»).
  if (h.reglas?.length) return lista(h.reglas.map((r, i) => (i ? minuscula(fraseRegla(r)) : fraseRegla(r))));
  if (!h.dias?.length || !h.horas?.length) return "Sin horario";
  const dias = [...h.dias].sort();
  const quien =
    dias.length === 7
      ? "Todos los días"
      : dias.join() === "1,2,3,4,5"
        ? "Cada día laborable"
        : dias.join() === "6,7"
          ? "Los fines de semana"
          : `Los ${lista(dias.map((d) => DIAS[d - 1]))}`;
  // «Cada N horas» guardado desplegado (horario.ts): se dice como intervalo, no como 13 horas sueltas.
  const iv = reconocer(h.horas);
  if (iv) return `${quien}, cada ${iv.cada === 1 ? "hora" : `${iv.cada} horas`} de ${horaCorta(iv.desde)} a ${horaCorta(iv.hasta)}`;
  return `${quien} a ${h.horas.length === 1 && h.horas[0].startsWith("01:") ? "la" : "las"} ${lista([...h.horas].sort())}`;
}

/** Los días en frase corta: «de lunes a viernes», «todos los días», «los sábados y domingos». */
export function diasEnFrase(dias: number[]): string {
  const d = [...new Set(dias)].sort();
  if (d.length === 7) return "todos los días";
  if (!d.length) return "ningún día";
  // Un tramo seguido de 3 o más días: «de lunes a viernes».
  if (d.length >= 3 && d.every((x, i) => i === 0 || x === d[i - 1] + 1)) return `de ${DIAS[d[0] - 1].toLowerCase()} a ${DIAS[d.at(-1)! - 1].toLowerCase()}`;
  return `los ${lista(d.map((x) => DIAS[x - 1]).map((n) => (n.endsWith("s") ? n : `${n}s`)))}`;
}

/** Resumen de un horario para el editor: «Cada hora de 7:00 a 19:00, de lunes a viernes · 13 copias al día». */
export function resumenHorario(h: Horario): string {
  return resumenReglas(reglasDe(h));
}

const MESES = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];
const minuscula = (s: string) => s.replace(/^./, (c) => c.toLowerCase());
/** «las 13:00», «la 1:30». */
const laHora = (h: string) => `${h.startsWith("01:") ? "la" : "las"} ${horaCorta(h)}`;
/** «2026-10-05» → «5 de octubre de 2026». */
function fechaEnFrase(iso: string): string {
  const [y, m, d] = iso.split("-").map(Number);
  return m >= 1 && m <= 12 ? `${d} de ${MESES[m - 1]} de ${y}` : iso;
}

/** «Cada 10 minutos», «Cada hora», «Cada 2 horas». */
export function cadaEnFrase(min: number): string {
  if (min % 60) return `Cada ${min} minutos`;
  return min === 60 ? "Cada hora" : `Cada ${min / 60} horas`;
}

/** Una regla en frase: «Cada 10 minutos de 8:00 a 18:00, de lunes a viernes», «El día 1 de cada mes a las 23:00». */
export function fraseRegla(r: ReglaHorario): string {
  switch (r.tipo) {
    case "horas": {
      const h = normalizar(r.horas);
      if (!h.length) return `Sin horas, ${diasEnFrase(r.dias)}`;
      return `A ${h.length === 1 ? laHora(h[0]) : `las ${lista(h.map(horaCorta))}`}, ${diasEnFrase(r.dias)}`;
    }
    case "intervalo":
      return `${cadaEnFrase(r.cada_min)} de ${horaCorta(r.desde)} a ${horaCorta(r.hasta)}, ${diasEnFrase(r.dias)}`;
    case "cada_dias":
      return `${r.cada === 1 ? "Cada día" : `Cada ${r.cada} días`} a ${laHora(r.hora)}, desde el ${fechaEnFrase(r.inicio)}`;
    case "mensual":
      return `${r.dia === -1 ? "El último día" : `El día ${r.dia}`} de cada mes a ${laHora(r.hora)}`;
  }
}

/** Cuántas copias, en frase: «61 copias al día», «unas 61 copias al día», «1 copia cada 3 días»… */
export function copiasEnFrase(reglas: ReglaHorario[]): string {
  const c = copiasAlDia(reglas);
  if (c) {
    const t = c.n === 1 ? "1 copia" : `${numero(c.n)} copias`;
    if (c.modo === "exacto") return `${t} al día`;
    if (c.modo === "hasta") return `hasta ${t} al día`;
    return c.n === 1 ? "alrededor de 1 copia al día" : `unas ${t} al día`;
  }
  if (reglas.length === 1) {
    const r = reglas[0];
    if (r.tipo === "cada_dias") return r.cada === 1 ? "1 copia al día" : `1 copia cada ${r.cada} días`;
    return "1 copia al mes";
  }
  const alMes = Math.round(reglas.reduce((s, r) => s + (r.tipo === "cada_dias" ? 30 / Math.max(1, r.cada) : 1), 0));
  return alMes === 1 ? "1 copia al mes" : `unas ${numero(alMes)} copias al mes`;
}

/**
 * Resumen de unas reglas para el editor: «Cada 10 minutos de 8:00 a 18:00, de
 * lunes a viernes · y el día 1 de cada mes a las 23:00 · unas 61 copias al día».
 */
export function resumenReglas(reglas: ReglaHorario[]): string {
  if (!reglas.length) return "Sin horario: solo se hará con «Copiar ahora».";
  const partes = reglas.map((r, i) => (i ? `y ${minuscula(fraseRegla(r))}` : fraseRegla(r)));
  return `${partes.join(" · ")} · ${copiasEnFrase(reglas)}`;
}

/** Del formato del agente (`PlanSchedule.rules`, días 0 = lunes) a las reglas de la consola. */
function reglaDePlan(r: PlanRuleLike): ReglaHorario {
  const dias = (r.days ?? []).map((d) => d + 1);
  switch (r.kind) {
    case "every":
      return { tipo: "intervalo", dias, cada_min: r.every_min ?? 60, desde: r.from ?? "", hasta: r.to ?? "" };
    case "every_days":
      return { tipo: "cada_dias", cada: r.every ?? 1, inicio: r.start ?? "", hora: r.time ?? "" };
    case "monthly":
      return { tipo: "mensual", dia: r.day ?? 1, hora: r.time ?? "" };
    default:
      return { tipo: "horas", dias, horas: r.times ?? [] };
  }
}

/** «1 equipo», «3 equipos». */
export const plural = (n: number, uno: string, varios: string) => `${numero(n)} ${n === 1 ? uno : varios}`;
