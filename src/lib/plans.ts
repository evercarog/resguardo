// Planes de copia: textos, validación y cálculo de horarios. Imita a
// src-tauri/src/plans.rs para que la interfaz muestre lo mismo que hará el agente.
import type { AgentPlan, Plan, PlanSchedule } from "$lib/api";

/** Iniciales de los días (0 = lunes). */
export const DAY_LETTERS = ["L", "M", "X", "J", "V", "S", "D"];
export const DAY_NAMES = ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"];
/** Plural para «los lunes», «los sábados»… */
const DAY_PLURAL = ["lunes", "martes", "miércoles", "jueves", "viernes", "sábados", "domingos"];

export const MAX_PLANS = 20;
export const MAX_TAGS = 10;
export const MAX_TIMES = 48;

/** Horario con el que empieza una copia al activar su horario. */
export const defaultSchedule = (): PlanSchedule => ({
  days: [0, 1, 2, 3, 4],
  mode: "at",
  times: ["13:00"],
  every_hours: 1,
  from: "08:00",
  to: "18:00",
});

// ---------- Horas ----------

/** "HH:MM" → minutos desde medianoche, o null si no es válida. */
export function parseTime(t: string): number | null {
  const m = /^(\d{1,2}):(\d{2})$/.exec(t.trim());
  if (!m) return null;
  const h = Number(m[1]);
  const min = Number(m[2]);
  return h < 24 && min < 60 ? h * 60 + min : null;
}

const pad = (n: number) => String(n).padStart(2, "0");
const fmtMinutes = (m: number) => `${pad(Math.floor(m / 60))}:${pad(m % 60)}`;

/** Horas del día en que toca, en minutos, ordenadas y sin repetir (como `times_of_day`). */
export function timesOfDay(s: PlanSchedule): number[] {
  let out: number[];
  if (s.mode === "at") {
    out = s.times.map(parseTime).filter((t): t is number => t != null);
  } else {
    const from = parseTime(s.from);
    const to = parseTime(s.to);
    if (from == null || to == null) return [];
    const step = Math.min(24, Math.max(1, Math.floor(s.every_hours) || 1)) * 60;
    out = [];
    // Al pasar de medianoche se corta, igual que en Rust.
    for (let t = from; t <= to && t < 24 * 60 && out.length < MAX_TIMES; t += step) out.push(t);
  }
  return [...new Set(out)].sort((a, b) => a - b);
}

/** Día de la semana con lunes = 0. */
const weekday = (d: Date) => (d.getDay() + 6) % 7;

function slotsOn(s: PlanSchedule, day: Date): Date[] {
  if (!s.days.includes(weekday(day))) return [];
  const out: Date[] = [];
  for (const m of timesOfDay(s)) {
    const [h, min] = [Math.floor(m / 60), m % 60];
    const d = new Date(day.getFullYear(), day.getMonth(), day.getDate(), h, min);
    // Hora que no existe (salto de horario de verano): se omite, igual que en Rust.
    if (d.getHours() !== h || d.getMinutes() !== min) continue;
    out.push(d);
  }
  return out;
}

const addDays = (d: Date, n: number) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);

/** Último momento programado que ya pasó (hasta 8 días atrás). */
export function latestPlanSlot(s: PlanSchedule, now: Date): Date | null {
  for (let back = 0; back <= 8; back++) {
    const slot = slotsOn(s, addDays(now, -back))
      .reverse()
      .find((x) => x <= now);
    if (slot) return slot;
  }
  return null;
}

/** Próximo momento programado después de `after` (hasta 8 días adelante). */
export function nextPlanSlot(s: PlanSchedule, after: Date): Date | null {
  for (let fwd = 0; fwd <= 8; fwd++) {
    const slot = slotsOn(s, addDays(after, fwd)).find((x) => x > after);
    if (slot) return slot;
  }
  return null;
}

/** Los próximos `n` momentos programados después de `after` (para la vista previa). */
export function nextPlanSlots(s: PlanSchedule, after: Date, n: number): Date[] {
  const out: Date[] = [];
  for (let fwd = 0; fwd <= 14 && out.length < n; fwd++) {
    for (const slot of slotsOn(s, addDays(after, fwd))) {
      if (slot > after) out.push(slot);
      if (out.length >= n) break;
    }
  }
  return out;
}

/** ¿Toca copia? `since` es la última copia del plan (o cuándo se activó). */
export function isPlanDue(s: PlanSchedule, since: Date, now: Date) {
  const slot = latestPlanSlot(s, now);
  return !!slot && since < slot;
}

// ---------- Textos ----------

/** "todos los días", "de lunes a viernes", "los domingos", "los lunes, miércoles y viernes"… */
export function daysLabel(days: number[]) {
  const d = [...new Set(days)].filter((x) => x >= 0 && x <= 6).sort((a, b) => a - b);
  if (d.length === 7) return "todos los días";
  if (d.length === 0) return "ningún día";
  if (d.length === 2 && d[0] === 5 && d[1] === 6) return "los fines de semana";
  if (d.length === 1) return `los ${DAY_PLURAL[d[0]]}`;
  const contiguous = d.every((x, i) => i === 0 || x === d[i - 1] + 1);
  if (contiguous && d.length >= 3) return `de ${DAY_NAMES[d[0]]} a ${DAY_NAMES[d[d.length - 1]]}`;
  return `los ${listJoin(d.map((x) => DAY_NAMES[x]))}`;
}

function listJoin(items: string[]) {
  return items.length <= 1 ? (items[0] ?? "") : `${items.slice(0, -1).join(", ")} y ${items[items.length - 1]}`;
}

function timesLabel(times: number[]) {
  if (times.length === 0) return "sin horas";
  const shown = times.slice(0, 4).map(fmtMinutes);
  const rest = times.length - shown.length;
  const list = rest > 0 ? `${shown.join(", ")} y ${rest} ${rest === 1 ? "hora" : "horas"} más` : listJoin(shown);
  // «a la 01:00», pero «a las 13:00».
  return `${times[0] >= 60 && times[0] < 120 && times.length === 1 ? "a la" : "a las"} ${list}`;
}

/**
 * El horario en palabras: "de lunes a sábado, cada hora de 07:00 a 19:00",
 * "los domingos a las 23:00", "todos los días a las 13:00 y 18:30".
 */
export function planScheduleLabel(s: PlanSchedule) {
  const days = daysLabel(s.days);
  if (s.mode === "at") return `${days} ${timesLabel(timesOfDay(s))}`;
  const n = Math.max(1, Math.floor(s.every_hours) || 1);
  const every = n === 1 ? "cada hora" : `cada ${n} horas`;
  const range = s.from === "00:00" && s.to === "23:59" ? "durante todo el día" : `de ${s.from} a ${s.to}`;
  return `${days}, ${every} ${range}`;
}

/** Igual que `planScheduleLabel`, con mayúscula inicial. */
export function planScheduleSentence(s: PlanSchedule) {
  const l = planScheduleLabel(s);
  return l.charAt(0).toUpperCase() + l.slice(1);
}

const wdFmt = new Intl.DateTimeFormat("es", { weekday: "short" });
const monFmt = new Intl.DateTimeFormat("es", { month: "short" });

/** "hoy", "mañana" o "lun 29 sept". */
export function slotDayLabel(d: Date, now = new Date()) {
  const diff = Math.round((new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime() - new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime()) / 86_400_000);
  if (diff === 0) return "hoy";
  if (diff === 1) return "mañana";
  return `${wdFmt.format(d).replace(".", "")} ${d.getDate()} ${monFmt.format(d).replace(".", "")}`;
}

export const slotTime = (d: Date) => `${pad(d.getHours())}:${pad(d.getMinutes())}`;

/** "hoy 13:00 · 14:00 · mañana 07:00": el día solo se repite cuando cambia. */
export function slotsPreview(slots: Date[], now = new Date()) {
  let prevDay = "";
  return slots
    .map((d) => {
      const day = slotDayLabel(d, now);
      const text = day === prevDay ? slotTime(d) : `${day} ${slotTime(d)}`;
      prevDay = day;
      return text;
    })
    .join(" · ");
}

// ---------- Validación (mismos mensajes que el backend) ----------

export function validateSchedule(s: PlanSchedule): string | null {
  if (s.days.length === 0) return "Elige al menos un día.";
  if (s.days.some((d) => d < 0 || d > 6)) return "Día de la semana no válido.";
  if (s.mode === "at") {
    if (s.times.length === 0) return "Añade al menos una hora.";
    if (s.times.length > MAX_TIMES) return "Demasiadas horas (máximo 48 al día).";
    const bad = s.times.find((t) => parseTime(t) == null);
    if (bad != null) return `Hora no válida: «${bad}» (usa HH:MM, p. ej. 13:00).`;
  } else if (s.mode === "every") {
    if (!Number.isInteger(s.every_hours) || s.every_hours < 1 || s.every_hours > 24) return "El intervalo debe estar entre 1 y 24 horas.";
    const from = parseTime(s.from);
    const to = parseTime(s.to);
    if (from == null) return `Hora no válida: «${s.from}» (usa HH:MM, p. ej. 13:00).`;
    if (to == null) return `Hora no válida: «${s.to}» (usa HH:MM, p. ej. 13:00).`;
    if (to < from) return "La hora final debe ser posterior a la inicial.";
  } else {
    return "Tipo de horario no válido.";
  }
  return null;
}

/** Error de una etiqueta, o null si vale. */
export function tagError(t: string): string | null {
  if (!t || [...t].length > 40 || t.includes(",") || /\s/.test(t)) return `Etiqueta no válida: «${t}» (sin espacios ni comas, hasta 40 caracteres).`;
  return null;
}

/** Comprueba un plan. `others`: los demás planes del repositorio (para no repetir nombre). */
export function validatePlan(p: Plan, others: Plan[] = []): string | null {
  const name = p.name.trim();
  if (!name) return "La copia necesita un nombre.";
  if ([...name].length > 60) return "El nombre de la copia es demasiado largo (máximo 60 caracteres).";
  if (others.some((o) => o.name.trim().toLocaleLowerCase() === name.toLocaleLowerCase())) return `Ya hay una copia llamada «${name}» en este repositorio.`;
  if (p.paths.length === 0) return `La copia «${name}» no tiene carpetas: elige al menos una.`;
  if (p.tags.length > MAX_TAGS) return "Demasiadas etiquetas (máximo 10).";
  for (const t of p.tags) {
    const e = tagError(t);
    if (e) return e;
  }
  return p.schedule ? validateSchedule(p.schedule) : null;
}

// ---------- Comparación con lo que tiene el agente ----------

function scheduleKey(s: PlanSchedule) {
  return JSON.stringify([[...s.days].sort(), s.mode, s.times, s.every_hours, s.from, s.to]);
}

const planKey = (p: Plan | AgentPlan) =>
  JSON.stringify([p.id, p.name, p.paths, p.excludes, p.tags, p.schedule ? scheduleKey(p.schedule) : null, !!p.skip_unchanged]);

/** true si el agente tiene este plan tal cual está guardado (mismas carpetas, etiquetas y horario). */
export const samePlan = (p: Plan, a: AgentPlan) => planKey(p) === planKey(a);

/** true si los planes con horario ya no coinciden con los que programó el agente. */
export function plansDiffer(stored: Plan[], agentPlans: AgentPlan[]) {
  const a = stored.filter((p) => p.schedule).map(planKey).sort();
  const b = agentPlans.map(planKey).sort();
  return a.length !== b.length || a.some((k, i) => k !== b[i]);
}

/** Resumen corto de qué copia un plan: "2 carpetas · 3 exclusiones". */
export function planContentsLabel(p: Plan) {
  const parts = [`${p.paths.length} ${p.paths.length === 1 ? "carpeta" : "carpetas"}`];
  if (p.excludes.length) parts.push(`${p.excludes.length} ${p.excludes.length === 1 ? "exclusión" : "exclusiones"}`);
  return parts.join(" · ");
}
