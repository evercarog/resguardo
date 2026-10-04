// Políticas de retención (ver src-tauri/src/retention.rs): valores por
// defecto, comparación, sus argumentos de restic, un resumen en palabras y los
// comandos para aplicarla a mano (p. ej. en un servidor REST append-only).
import type { Policy } from "$lib/api";

/** Cantidad «sin límite» (`unlimited` en restic). */
export const UNLIMITED = -1;

export const EMPTY_POLICY: Policy = {
  keep_last: 0,
  keep_hourly: 0,
  keep_daily: 0,
  keep_weekly: 0,
  keep_monthly: 0,
  keep_yearly: 0,
  keep_within: null,
  keep_within_hourly: null,
  keep_within_daily: null,
  keep_within_weekly: null,
  keep_within_monthly: null,
  keep_within_yearly: null,
  group_by: null,
  filter_tags: [],
  filter_host: null,
  filter_paths: [],
  keep_tags: [],
};

export const COUNT_KEYS = ["keep_last", "keep_hourly", "keep_daily", "keep_weekly", "keep_monthly", "keep_yearly"] as const;
export const WITHIN_KEYS = [
  "keep_within",
  "keep_within_hourly",
  "keep_within_daily",
  "keep_within_weekly",
  "keep_within_monthly",
  "keep_within_yearly",
] as const;
export type CountKey = (typeof COUNT_KEYS)[number];
export type WithinKey = (typeof WITHIN_KEYS)[number];

const FLAG: Record<CountKey | WithinKey, string> = {
  keep_last: "--keep-last",
  keep_hourly: "--keep-hourly",
  keep_daily: "--keep-daily",
  keep_weekly: "--keep-weekly",
  keep_monthly: "--keep-monthly",
  keep_yearly: "--keep-yearly",
  keep_within: "--keep-within",
  keep_within_hourly: "--keep-within-hourly",
  keep_within_daily: "--keep-within-daily",
  keep_within_weekly: "--keep-within-weekly",
  keep_within_monthly: "--keep-within-monthly",
  keep_within_yearly: "--keep-within-yearly",
};

const cleanList = (l: string[] | undefined) => [...new Set((l ?? []).map((x) => x.trim()).filter(Boolean))];

/** Política completa y limpia: enteros (≥ 0 o -1), plazos vacíos → null, listas sin repetidos. */
export function normalizePolicy(p: Partial<Policy> | null | undefined): Policy {
  const out: Policy = { ...EMPTY_POLICY, ...(p ?? {}) };
  for (const k of COUNT_KEYS) {
    const n = Math.floor(Number(out[k]) || 0);
    out[k] = n === UNLIMITED ? UNLIMITED : Math.max(0, n);
  }
  for (const k of WITHIN_KEYS) out[k] = (out[k] ?? "").trim() || null;
  out.group_by = out.group_by ? (["host", "paths", "tags"] as const).filter((g) => out.group_by!.includes(g)) : null;
  out.filter_tags = cleanList(out.filter_tags);
  out.filter_paths = cleanList(out.filter_paths);
  out.keep_tags = cleanList(out.keep_tags);
  out.filter_host = (out.filter_host ?? "").trim() || null;
  return out;
}

export const samePolicy = (a: Partial<Policy> | null | undefined, b: Partial<Policy> | null | undefined) =>
  JSON.stringify(normalizePolicy(a)) === JSON.stringify(normalizePolicy(b));

/** Sin reglas de plazos ni cantidades (agrupar, filtrar o proteger solos no son una política). */
export function isEmptyPolicy(p: Partial<Policy> | null | undefined) {
  const n = normalizePolicy(p);
  return COUNT_KEYS.every((k) => !n[k]) && WITHIN_KEYS.every((k) => !n[k]);
}

/** Argumentos de restic, en el mismo orden que `Policy::args` en el backend. */
export function policyArgs(p: Partial<Policy>): string[] {
  const n = normalizePolicy(p);
  const args: string[] = [];
  if (n.group_by) args.push("--group-by", n.group_by.join(","));
  for (const t of n.filter_tags ?? []) args.push("--tag", t);
  if (n.filter_host) args.push("--host", n.filter_host);
  for (const path of n.filter_paths ?? []) args.push("--path", path);
  for (const k of WITHIN_KEYS) if (n[k]) args.push(FLAG[k], n[k]!);
  for (const k of COUNT_KEYS) if (n[k]) args.push(FLAG[k], n[k] === UNLIMITED ? "unlimited" : String(n[k]));
  for (const t of n.keep_tags ?? []) args.push("--keep-tag", t);
  return args;
}

/** Un argumento para bash: tal cual si es seguro; si no (o si está vacío), entre comillas simples. */
export function shellQuote(v: string) {
  if (/^[A-Za-z0-9_./:,@%+=-]+$/.test(v)) return v;
  return `'${v.replaceAll("'", `'\\''`)}'`;
}

/** Argumentos listos para una línea de bash (p. ej. `--group-by ''`). */
export const policyShellArgs = (p: Partial<Policy>) => policyArgs(p).map(shellQuote).join(" ");

const UNITS: Record<string, [string, string]> = { h: ["hora", "horas"], d: ["día", "días"], w: ["semana", "semanas"], m: ["mes", "meses"], y: ["año", "años"] };

/** "15d" → "15 días", "1y" → "1 año"; si es compuesta ("1y6m"), tal cual. */
export function durationWords(d: string) {
  const m = /^(\d+)([hdwmy])$/.exec(d);
  if (!m) return d;
  const n = Number(m[1]);
  return `${n} ${UNITS[m[2]][n === 1 ? 0 : 1]}`;
}

/** Lista en castellano: «a», «a y b», «a, b y c». */
function list(items: string[]) {
  return items.length <= 1 ? (items[0] ?? "") : `${items.slice(0, -1).join(", ")} y ${items.at(-1)}`;
}

const PER: Record<string, string> = { hourly: "hora", daily: "día", weekly: "semana", monthly: "mes", yearly: "año" };
const COUNT_WORD: Record<string, [string, string]> = {
  hourly: ["horaria", "horarias"],
  daily: ["diaria", "diarias"],
  weekly: ["semanal", "semanales"],
  monthly: ["mensual", "mensuales"],
  yearly: ["anual", "anuales"],
};
const GROUP_WORD: Record<string, string> = { host: "equipo", paths: "carpetas", tags: "etiquetas" };

/** Resumen de una política en una frase («Conserva todas las de los últimos 15 días, una por día durante 1 año…»). */
export function policySummary(p: Partial<Policy> | null | undefined) {
  const n = normalizePolicy(p);
  if (isEmptyPolicy(n)) return "Sin política: se conservan todas las versiones.";
  const keep: string[] = [];
  if (n.keep_within) keep.push(`todas las de los últimos ${durationWords(n.keep_within)}`);
  if (n.keep_last) keep.push(n.keep_last === UNLIMITED ? "todas" : n.keep_last === 1 ? "la última" : `las últimas ${n.keep_last}`);
  for (const period of ["hourly", "daily", "weekly", "monthly", "yearly"] as const) {
    const within = n[`keep_within_${period}`];
    const count = n[`keep_${period}`];
    if (within) keep.push(`una por ${PER[period]} durante ${durationWords(within)}`);
    if (count === UNLIMITED) keep.push(`una por ${PER[period]} siempre`);
    else if (count) keep.push(`${count} ${COUNT_WORD[period][count === 1 ? 0 : 1]}`);
  }
  const extra: string[] = [];
  if (n.filter_tags?.length || n.filter_host || n.filter_paths?.length) {
    const f: string[] = [];
    if (n.filter_tags?.length) f.push(`con la etiqueta ${n.filter_tags.map((t) => `«${t}»`).join(" o ")}`);
    if (n.filter_host) f.push(`del equipo «${n.filter_host}»`);
    if (n.filter_paths?.length) f.push(`de ${n.filter_paths.map((x) => `«${x}»`).join(" o ")}`);
    extra.push(`Solo se aplica a las versiones ${f.join(", ")}; las demás no se tocan.`);
  }
  if (n.keep_tags?.length) extra.push(`Nunca borra las versiones con la etiqueta ${n.keep_tags.map((t) => `«${t}»`).join(" o ")}.`);
  if (n.group_by) extra.push(n.group_by.length ? `Agrupa por ${list(n.group_by.map((g) => GROUP_WORD[g]))}.` : "Todas las versiones como un solo grupo.");
  return [`Conserva ${list(keep)}.`, ...extra].join(" ");
}

/** Motivos de restic → etiqueta corta en castellano. */
const REASONS: [RegExp, string][] = [
  [/^outside filter/, "Fuera del filtro"],
  [/^has tags/, "Etiqueta protegida"],
  [/^last/, "Última"],
  [/^(oldest )?hourly within/, "Horaria (plazo)"],
  [/^(oldest )?daily within/, "Diaria (plazo)"],
  [/^(oldest )?weekly within/, "Semanal (plazo)"],
  [/^(oldest )?monthly within/, "Mensual (plazo)"],
  [/^(oldest )?yearly within/, "Anual (plazo)"],
  [/^(oldest )?hourly/, "Horaria"],
  [/^(oldest )?daily/, "Diaria"],
  [/^(oldest )?weekly/, "Semanal"],
  [/^(oldest )?monthly/, "Mensual"],
  [/^(oldest )?yearly/, "Anual"],
  [/^within/, "Reciente"],
  [/tag/, "Etiqueta"],
];
export const reasonLabel = (r: string) => REASONS.find(([re]) => re.test(r))?.[1] ?? r;

/** Ruta del repositorio dentro de un servidor REST (`rest:https://h:8000/siigo/` → "siigo"); null si no es REST. */
export function restRepoPath(location: string): string | null {
  if (!/^rest:/i.test(location.trim())) return null;
  try {
    return new URL(location.trim().slice(5)).pathname.replace(/^\/+|\/+$/g, "");
  } catch {
    return "";
  }
}

/** Nombre corto para archivos del servidor: «Siigo · Oficina» → "siigo-oficina". */
export const fileSlug = (s: string) =>
  s
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "") || "repo";

/**
 * Comandos para aplicar la política en el propio servidor de rest-server
 * (Linux), donde el repositorio es una carpeta local. En el cron, `%` se
 * escapa (cron lo convierte en salto de línea).
 */
export function serverCommands(p: Partial<Policy>, repoName: string, location: string) {
  const path = restRepoPath(location) ?? "";
  const repoDir = `/ruta/de/rest-server${path ? `/${path}` : ""}`;
  const name = fileSlug(path.split("/").pop() || repoName);
  const pwFile = `/root/.restic-${name}`;
  const args = policyShellArgs(p);
  return [
    "# Carpeta del repositorio en el servidor (cambia /ruta/de/rest-server por la carpeta de datos de rest-server)",
    `export RESTIC_REPOSITORY=${shellQuote(repoDir)}`,
    "# La contraseña del repositorio, en un archivo que solo puede leer root",
    `install -m 600 /dev/null ${pwFile} && nano ${pwFile}`,
    `chmod 600 ${pwFile}`,
    `export RESTIC_PASSWORD_FILE=${pwFile}`,
    "",
    "# 1. Prueba: muestra qué se borraría, sin tocar nada",
    `restic forget --dry-run ${args}`,
    "# 2. Olvida las versiones que sobran",
    `restic forget ${args}`,
    "# 3. Libera el espacio que ya no usa ninguna versión",
    "restic prune",
    "",
    "# Cada domingo a las 3:00 (añádelo con crontab -e)",
    `0 3 * * 0 RESTIC_REPOSITORY=${shellQuote(repoDir)} RESTIC_PASSWORD_FILE=${pwFile} restic forget ${args.replaceAll("%", "\\%")} --prune >> /var/log/restic-${name}.log 2>&1`,
  ].join("\n");
}
