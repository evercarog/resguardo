// «Modo discreto»: el mismo horario que mira el agente (ver discreto.rs).
import type { Discreet } from "$lib/api";

export const DISCREET_DEFAULT: Discreet = { days: [0, 1, 2, 3, 4, 5], from: "07:00", to: "19:00", upload_kib: null };

const minutes = (t: string) => {
  const m = /^(\d{1,2}):(\d{2})$/.exec(t.trim());
  return m ? Number(m[1]) * 60 + Number(m[2]) : null;
};
/** 0 = lunes … 6 = domingo. */
const weekday = (d: Date) => (d.getDay() + 6) % 7;

/** ¿Estamos dentro del horario? */
export function discreetActive(d: Discreet | null | undefined, now = new Date()): boolean {
  if (!d) return false;
  const from = minutes(d.from);
  const to = minutes(d.to);
  if (from === null || to === null || from === to) return false;
  const t = now.getHours() * 60 + now.getMinutes();
  const today = d.days.includes(weekday(now));
  if (from < to) return today && t >= from && t < to;
  const yesterday = d.days.includes((weekday(now) + 6) % 7);
  return (today && t >= from) || (yesterday && t < to);
}

/** «1 MB/s», «512 KB/s». */
export const uploadLabel = (kib: number) => (kib >= 1024 ? `${(kib / 1024).toLocaleString("es-ES", { maximumFractionDigits: 1 })} MB/s` : `${kib} KB/s`);

const DAY_SHORT = ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"];

/** «de lunes a sábado, de 07:00 a 19:00». */
export function discreetSummary(d: Discreet) {
  const days = [...d.days].sort();
  const consecutive = days.every((x, i) => i === 0 || x === days[i - 1] + 1);
  const dayText =
    days.length === 7
      ? "todos los días"
      : consecutive && days.length > 2
        ? `de ${DAY_SHORT[days[0]]} a ${DAY_SHORT[days[days.length - 1]]}`
        : days.map((x) => DAY_SHORT[x]).join(", ");
  return `${dayText}, de ${d.from} a ${d.to}`;
}
