const LOCALE = "es";

export function formatBytes(n?: number | null) {
  if (n == null) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  while (n >= 1024 && i < units.length - 1) {
    n /= 1024;
    i++;
  }
  return `${n.toLocaleString(LOCALE, { maximumFractionDigits: i === 0 ? 0 : 1 })} ${units[i]}`;
}

export const formatNumber = (n: number) => n.toLocaleString(LOCALE);

export function formatDuration(seconds?: number | null) {
  if (seconds == null) return "—";
  const s = Math.round(seconds);
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} min ${s % 60} s`;
  return `${Math.floor(m / 60)} h ${m % 60} min`;
}

const dateFmt = new Intl.DateTimeFormat(LOCALE, { dateStyle: "medium", timeStyle: "short" });
export const formatDate = (iso: string) => dateFmt.format(new Date(iso));

const timeFmt = new Intl.DateTimeFormat(LOCALE, { hour: "2-digit", minute: "2-digit" });
export const formatTime = (iso: string) => timeFmt.format(new Date(iso));

const shortDayFmt = new Intl.DateTimeFormat(LOCALE, { weekday: "short", day: "numeric", month: "short" });
/** "lun, 3 feb": día corto para tiras y calendarios. */
export const formatShortDay = (d: Date) => shortDayFmt.format(d);

const dayFmt = new Intl.DateTimeFormat(LOCALE, { weekday: "long", day: "numeric", month: "long" });
const dayYearFmt = new Intl.DateTimeFormat(LOCALE, { weekday: "long", day: "numeric", month: "long", year: "numeric" });

const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();

/** "Hoy", "Ayer" o la fecha larga, para agrupar snapshots por día. */
export function formatDay(iso: string) {
  const d = new Date(iso);
  const days = Math.round((startOfDay(new Date()) - startOfDay(d)) / 86_400_000);
  if (days === 0) return "Hoy";
  if (days === 1) return "Ayer";
  const text = (d.getFullYear() === new Date().getFullYear() ? dayFmt : dayYearFmt).format(d);
  return text.charAt(0).toUpperCase() + text.slice(1);
}

const rtf = new Intl.RelativeTimeFormat(LOCALE, { numeric: "auto" });

/** "hace 5 minutos", "ayer", "hace 3 meses"… */
export function formatRelative(iso: string) {
  const diff = (new Date(iso).getTime() - Date.now()) / 1000;
  const abs = Math.abs(diff);
  if (abs < 60) return "hace un momento";
  if (abs < 3600) return rtf.format(Math.round(diff / 60), "minute");
  if (abs < 86_400) return rtf.format(Math.round(diff / 3600), "hour");
  if (abs < 86_400 * 30) return rtf.format(Math.round(diff / 86_400), "day");
  if (abs < 86_400 * 365) return rtf.format(Math.round(diff / (86_400 * 30)), "month");
  return rtf.format(Math.round(diff / (86_400 * 365)), "year");
}

const shortDateFmt = new Intl.DateTimeFormat(LOCALE, { day: "numeric", month: "short" });
/** «hoy 18:05», «ayer 18:05» o «3 sept 18:05» (para fechas pasadas cercanas). */
export function formatShortWhen(iso: string) {
  const d = new Date(iso);
  const days = Math.round((startOfDay(new Date()) - startOfDay(d)) / 86_400_000);
  const day = days === 0 ? "hoy" : days === 1 ? "ayer" : shortDateFmt.format(d).replace(".", "");
  return `${day} ${formatTime(iso)}`;
}
