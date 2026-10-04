// Horarios de las copias gestionadas (gestion_v2.rs, `Horario { dias, horas,
// reglas }`).
//
// - Hasta el agente 0.7.8 solo hay una lista de horas por días: «cada N horas
//   entre las 07:00 y las 19:00» se guarda desplegado (07:00, 08:00 … 19:00),
//   y al abrir la configuración se reconoce el patrón para enseñarlo igual.
// - Desde la 0.7.9 (v1.24), `reglas`: «cada 10 minutos», «cada 3 días», «el
//   día 1 de cada mes»… que se suman. Si el horario se puede decir como una
//   lista de horas, se manda como siempre (y sin `reglas` si al volver a abrirlo
//   sale igual); si no, van las reglas.
//
// Este archivo no depende de formato.ts (que lo usa para las frases).
import type { Horario, ReglaHorario } from "./tipos";

/** Desde qué agente se puede apagar «Solo guardar si hay cambios» (v1.16; antes, siempre encendido). */
export const VERSION_SOLO_CAMBIOS = "0.7.7";
/** Desde qué agente se entienden las `reglas` del horario (v1.24). */
export const VERSION_REGLAS = "0.7.9";

/** Cada cuántas horas se puede elegir. */
export const INTERVALOS = [1, 2, 3, 4, 6, 8, 12] as const;
/** Cada cuántos minutos se puede elegir (agente ≥ 0.7.9). */
export const MINUTOS = [5, 10, 15, 20, 30] as const;
/** Como mucho, horas sueltas al día (y horas en la lista desplegada). */
export const MAX_HORAS = 48;
/** Como mucho, reglas en un horario. */
export const MAX_REGLAS = 20;
/** «Cada N días»: de 1 a 365. */
export const MAX_CADA_DIAS = 365;

/** Atajos de días (1 = lunes … 7 = domingo). */
export const DIAS_ATAJOS: { texto: string; dias: number[] }[] = [
  { texto: "Lun–Vie", dias: [1, 2, 3, 4, 5] },
  { texto: "Lun–Sáb", dias: [1, 2, 3, 4, 5, 6] },
  { texto: "Todos los días", dias: [1, 2, 3, 4, 5, 6, 7] },
];

export interface Intervalo {
  /** Horas entre una copia y la siguiente. */
  cada: number;
  desde: string;
  hasta: string;
}

const HORA = /^([01]\d|2[0-3]):([0-5]\d)$/;
export const horaValida = (h: string) => HORA.test(h);
const aMin = (h: string) => Number(h.slice(0, 2)) * 60 + Number(h.slice(3, 5));
const deMin = (m: number) => `${String(Math.floor(m / 60)).padStart(2, "0")}:${String(m % 60).padStart(2, "0")}`;

/** Ordenadas y sin repetir (solo las válidas). */
export const normalizar = (horas: string[]) => [...new Set(horas.filter(horaValida))].sort();
const diasNormales = (dias: number[]) => [...new Set(dias)].filter((d) => d >= 1 && d <= 7).sort((a, b) => a - b);

/** «Cada N horas de desde a hasta», desplegado en horas sueltas (vacío si no cuadra). */
export function desplegar(iv: Intervalo): string[] {
  if (!INTERVALOS.includes(iv.cada as (typeof INTERVALOS)[number])) return [];
  return desplegarMin(iv.cada * 60, iv.desde, iv.hasta, MAX_HORAS);
}

/** De `desde` a `hasta` (incluidas) cada `paso` minutos; vacío si no cuadra. */
function desplegarMin(paso: number, desde: string, hasta: string, max = 300): string[] {
  if (!horaValida(desde) || !horaValida(hasta) || !(paso > 0)) return [];
  const [a, b] = [aMin(desde), aMin(hasta)];
  if (b < a) return [];
  const out: string[] = [];
  for (let m = a; m <= b && out.length < max; m += paso) out.push(deMin(m));
  return out;
}

/** Lo que falla en un intervalo, o null. */
export function errorIntervalo(iv: Pick<Intervalo, "desde" | "hasta">): string | null {
  if (!horaValida(iv.desde) || !horaValida(iv.hasta)) return "Escribe las dos horas.";
  if (aMin(iv.hasta) < aMin(iv.desde)) return "La hora final tiene que ser igual o posterior a la inicial (dentro del mismo día).";
  return null;
}

/**
 * ¿Es la lista un «cada N horas» regular? (al menos 3 horas, todas a la misma
 * distancia, que sea de las que se pueden elegir). Si no, null.
 */
export function reconocer(horas: string[]): Intervalo | null {
  const r = reconocerPaso(horas, false);
  return r && { cada: r.cada_min / 60, desde: r.desde, hasta: r.hasta };
}

/** Como `reconocer`, en minutos; con `minutos`, también los pasos de 5 a 30 minutos. */
function reconocerPaso(horas: string[], minutos: boolean): { cada_min: number; desde: string; hasta: string } | null {
  const h = normalizar(horas);
  if (h.length < 3 || h.length !== horas.length) return null;
  const paso = aMin(h[1]) - aMin(h[0]);
  const vale = paso % 60 === 0 ? INTERVALOS.includes((paso / 60) as (typeof INTERVALOS)[number]) : minutos && MINUTOS.includes(paso as (typeof MINUTOS)[number]);
  if (!vale) return null;
  for (let i = 2; i < h.length; i++) if (aMin(h[i]) - aMin(h[i - 1]) !== paso) return null;
  return { cada_min: paso, desde: h[0], hasta: h[h.length - 1] };
}

/** «7:00» en vez de «07:00» (para las frases). */
export const horaCorta = (h: string) => (h.startsWith("0") ? h.slice(1) : h);

// --- Reglas (agente ≥ 0.7.9) -------------------------------------------------

/** ¿Se puede elegir este intervalo? 5, 10, 15, 20 o 30 minutos, o de 1 a 24 horas enteras. */
export const intervaloValido = (min: number) => MINUTOS.includes(min as (typeof MINUTOS)[number]) || (Number.isInteger(min) && min % 60 === 0 && min >= 60 && min <= 1440);

/** ¿Hace falta un agente ≥ 0.7.9 para esta regla? (minutos, cada N días, cada mes). */
export const reglaNueva = (r: ReglaHorario) => r.tipo === "cada_dias" || r.tipo === "mensual" || (r.tipo === "intervalo" && r.cada_min % 60 !== 0);

/** Horas en que toca una regla «horas» o «intervalo» (las demás no tienen lista). */
export function horasDeRegla(r: ReglaHorario): string[] {
  if (r.tipo === "horas") return normalizar(r.horas);
  if (r.tipo === "intervalo") return desplegarMin(r.cada_min, r.desde, r.hasta);
  return [];
}

/** Fecha local «AAAA-MM-DD». */
export function fechaISO(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}
const FECHA = /^(\d{4})-(\d{2})-(\d{2})$/;
/** La fecha «AAAA-MM-DD» a medianoche local, o null si no existe (2026-02-30). */
export function deFechaISO(s: string): Date | null {
  const m = FECHA.exec(s);
  if (!m) return null;
  const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  return fechaISO(d) === s ? d : null;
}

/**
 * Las reglas de un horario: las suyas, o las de la lista de horas de siempre
 * dichas de la forma más corta («cada 2 horas de 8:00 a 18:00» en vez de seis
 * horas sueltas). Con `minutos` (agente ≥ 0.7.9), también «cada 30 minutos»;
 * sin él, solo lo que entiende cualquier agente.
 */
export function reglasDe(h: Horario | null | undefined, minutos = true): ReglaHorario[] {
  if (!h) return [];
  if (h.reglas?.length) return JSON.parse(JSON.stringify(h.reglas)) as ReglaHorario[];
  if (!h.dias?.length || !h.horas?.length) return [];
  const dias = diasNormales(h.dias);
  const iv = reconocerPaso(h.horas, minutos);
  if (iv) return [{ tipo: "intervalo", dias, ...iv }];
  return [{ tipo: "horas", dias, horas: [...h.horas] }];
}

/**
 * El horario como lista de horas por días (lo que entiende cualquier agente),
 * o null si no se puede: hay minutos, «cada N días» o «cada mes», reglas con
 * días distintos o más de 48 horas.
 */
export function expresable(reglas: ReglaHorario[]): { dias: number[]; horas: string[] } | null {
  if (!reglas.length || reglas.some(reglaNueva)) return null;
  const dias = diasNormales((reglas[0] as { dias: number[] }).dias);
  if (reglas.some((r) => diasNormales((r as { dias: number[] }).dias).join() !== dias.join())) return null;
  const horas = normalizar(reglas.flatMap(horasDeRegla));
  if (!dias.length || !horas.length || horas.length > MAX_HORAS) return null;
  return { dias, horas };
}

/** Una regla limpia para mandar (solo sus campos, horas ordenadas). */
function limpia(r: ReglaHorario): ReglaHorario {
  switch (r.tipo) {
    case "horas":
      return { tipo: "horas", dias: diasNormales(r.dias), horas: normalizar(r.horas) };
    case "intervalo":
      return { tipo: "intervalo", dias: diasNormales(r.dias), cada_min: Number(r.cada_min), desde: r.desde, hasta: r.hasta };
    case "cada_dias":
      return { tipo: "cada_dias", cada: Number(r.cada), inicio: r.inicio, hora: r.hora };
    case "mensual":
      return { tipo: "mensual", dia: Number(r.dia), hora: r.hora };
  }
}

/**
 * El horario que se manda al equipo. Con un agente anterior a la 0.7.9, solo
 * la lista de horas. Con uno nuevo, también: si al volver a abrirla sale lo
 * mismo, sin `reglas` (la configuración queda como siempre); si no, con las
 * reglas (y la lista, si se puede, para las consolas anteriores).
 */
export function horarioParaEnviar(reglas: ReglaHorario[], admiteReglas: boolean): Horario {
  const lim = reglas.map(limpia);
  if (!lim.length) return { dias: [], horas: [] };
  const exp = expresable(lim);
  if (exp && (!admiteReglas || JSON.stringify(reglasDe(exp, false).map(limpia)) === JSON.stringify(lim))) return exp;
  // Con un agente anterior, unas reglas que no se pueden desplegar se quedan en
  // el editor (para decir qué falla); «Antes de enviar» no deja mandarlas.
  return { dias: exp?.dias ?? [], horas: exp?.horas ?? [], reglas: lim };
}

/** Texto para lo que pide un agente más nuevo. */
export const ACTUALIZA = "Actualiza el agente para usar esto";

/** Lo que falla en una regla, o null. */
export function errorRegla(r: ReglaHorario, admiteReglas = true): string | null {
  if (!admiteReglas && reglaNueva(r)) return `${ACTUALIZA} (necesita la ${VERSION_REGLAS} o posterior).`;
  switch (r.tipo) {
    case "horas":
      if (!r.dias.length) return "Elige al menos un día.";
      if (!r.horas.length) return "Añade al menos una hora.";
      if (r.horas.some((h) => !horaValida(h))) return "Hay una hora sin completar.";
      if (new Set(r.horas).size > MAX_HORAS) return `Como mucho, ${MAX_HORAS} horas al día.`;
      return null;
    case "intervalo":
      if (!r.dias.length) return "Elige al menos un día.";
      if (!intervaloValido(Number(r.cada_min))) return "Elige cada cuánto (de 5 minutos a 12 horas).";
      return errorIntervalo(r);
    case "cada_dias":
      if (!Number.isInteger(Number(r.cada)) || r.cada < 1 || r.cada > MAX_CADA_DIAS) return `«Cada N días»: de 1 a ${MAX_CADA_DIAS}.`;
      if (!deFechaISO(r.inicio)) return "Elige desde qué día.";
      return horaValida(r.hora) ? null : "Escribe la hora.";
    case "mensual":
      if (!(r.dia === -1 || (Number.isInteger(r.dia) && r.dia >= 1 && r.dia <= 28))) return "Elige un día del 1 al 28, o el último.";
      return horaValida(r.hora) ? null : "Escribe la hora.";
  }
}

/** Lo que falla en un horario entero, o null (sin reglas: «no tiene horario»). */
export function errorReglas(reglas: ReglaHorario[], admiteReglas = true): string | null {
  if (!reglas.length) return "No tiene horario.";
  if (reglas.length > MAX_REGLAS) return `Como mucho, ${MAX_REGLAS} reglas.`;
  for (const r of reglas) {
    const e = errorRegla(r, admiteReglas);
    if (e) return e;
  }
  if (!admiteReglas && !expresable(reglas)) {
    return reglas.length > 1
      ? `${ACTUALIZA}: este agente solo admite reglas con los mismos días y hasta ${MAX_HORAS} horas al día.`
      : `Como mucho, ${MAX_HORAS} copias al día.`;
  }
  return null;
}

/** Las horas («HH:MM») en que toca algo ese día (fecha local), ordenadas y sin repetir. */
export function horasDelDia(reglas: ReglaHorario[], fecha: Date): string[] {
  const dow = fecha.getDay() === 0 ? 7 : fecha.getDay();
  const dia = new Date(fecha.getFullYear(), fecha.getMonth(), fecha.getDate());
  const out: string[] = [];
  for (const r of reglas) {
    if (r.tipo === "horas" || r.tipo === "intervalo") {
      if (r.dias.includes(dow)) out.push(...horasDeRegla(r));
    } else if (r.tipo === "cada_dias") {
      const inicio = deFechaISO(r.inicio);
      if (!inicio || dia < inicio || r.cada < 1) continue;
      // Días de calendario (no de 24 h: un cambio de hora no descuadra la cuenta).
      const n = Math.round((Date.UTC(dia.getFullYear(), dia.getMonth(), dia.getDate()) - Date.UTC(inicio.getFullYear(), inicio.getMonth(), inicio.getDate())) / 86_400_000);
      if (n % r.cada === 0) out.push(r.hora);
    } else if (r.tipo === "mensual") {
      const ultimo = new Date(dia.getFullYear(), dia.getMonth() + 1, 0).getDate();
      if (r.dia === -1 ? dia.getDate() === ultimo : dia.getDate() === r.dia) out.push(r.hora);
    }
  }
  return normalizar(out);
}

/** Cuántos días mirar (atrás o adelante) para encontrar la última o la próxima vez. */
function horizonte(reglas: ReglaHorario[], hoy: Date): number {
  let h = 8;
  for (const r of reglas) {
    if (r.tipo === "mensual") h = Math.max(h, 62);
    if (r.tipo === "cada_dias") {
      const inicio = deFechaISO(r.inicio);
      const espera = inicio ? Math.max(0, Math.round((inicio.getTime() - hoy.getTime()) / 86_400_000)) : 0;
      h = Math.max(h, espera + r.cada + 1);
    }
  }
  return Math.min(h, 800);
}

/** Los momentos (ms) en que toca ese día, con la hora del navegador. */
function momentos(reglas: ReglaHorario[], d: Date): number[] {
  return horasDelDia(reglas, d).map((h) => new Date(d.getFullYear(), d.getMonth(), d.getDate(), Number(h.slice(0, 2)), Number(h.slice(3, 5))).getTime());
}

/** La última vez que tocó (≤ `ahora`), o null. */
export function ultimaVez(reglas: ReglaHorario[], ahora: number): number | null {
  const hoy = new Date(ahora);
  const n = horizonte(reglas, hoy);
  for (let atras = 0; atras <= n; atras++) {
    const t = momentos(reglas, new Date(hoy.getFullYear(), hoy.getMonth(), hoy.getDate() - atras)).filter((x) => x <= ahora);
    if (t.length) return t[t.length - 1];
  }
  return null;
}

/** La próxima vez que toca (> `ahora`), o null. */
export function proximaVez(reglas: ReglaHorario[], ahora: number): number | null {
  const hoy = new Date(ahora);
  const n = horizonte(reglas, hoy);
  for (let adelante = 0; adelante <= n; adelante++) {
    const t = momentos(reglas, new Date(hoy.getFullYear(), hoy.getMonth(), hoy.getDate() + adelante)).find((x) => x > ahora);
    if (t !== undefined) return t;
  }
  return null;
}

/**
 * Cuántas copias al día, para el resumen: `n` en el día de la semana con más
 * (de las reglas por días), y si es aproximado (`hasta`: unos días más que
 * otros; `unas`: además hay reglas por fecha). Sin reglas por días, null.
 */
export function copiasAlDia(reglas: ReglaHorario[]): { n: number; modo: "exacto" | "hasta" | "unas" } | null {
  const semanales = reglas.filter((r) => r.tipo === "horas" || r.tipo === "intervalo") as Extract<ReglaHorario, { dias: number[] }>[];
  if (!semanales.length) return null;
  const porDia = [1, 2, 3, 4, 5, 6, 7].map((d) => normalizar(semanales.filter((r) => r.dias.includes(d)).flatMap(horasDeRegla)).length).filter((n) => n > 0);
  if (!porDia.length) return null;
  const n = Math.max(...porDia);
  const modo = semanales.length !== reglas.length ? "unas" : porDia.some((x) => x !== n) ? "hasta" : "exacto";
  return { n, modo };
}

/** Una regla nueva de cada tipo, con valores razonables. */
export function reglaNuevaDe(tipo: ReglaHorario["tipo"], dias: number[] = [1, 2, 3, 4, 5], hoy = new Date()): ReglaHorario {
  switch (tipo) {
    case "horas":
      return { tipo, dias: [...dias], horas: ["13:00"] };
    case "intervalo":
      return { tipo, dias: [...dias], cada_min: 60, desde: "08:00", hasta: "18:00" };
    case "cada_dias":
      return { tipo, cada: 2, inicio: fechaISO(hoy), hora: "23:00" };
    case "mensual":
      return { tipo, dia: 1, hora: "23:00" };
  }
}
