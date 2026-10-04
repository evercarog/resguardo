// Rutas dentro de un snapshot. restic guarda las rutas de Windows como
// `/C/Users/Ana` (la unidad es la primera carpeta); en Linux y macOS son
// las rutas normales.

/** `C:\Users\Ana\` → `/C/Users/Ana`; `/home/ana/` → `/home/ana`. */
export function toSnapshotPath(osPath: string): string {
  const unified = osPath.replace(/\\/g, "/");
  const drive = /^([A-Za-z]):(\/|$)(.*)$/.exec(unified);
  const path = drive ? `/${drive[1].toUpperCase()}/${drive[3]}` : unified.startsWith("/") ? unified : `/${unified}`;
  return path.replace(/\/+$/, "") || "/";
}

export function parentDir(path: string): string {
  const i = path.lastIndexOf("/");
  return i <= 0 ? "/" : path.slice(0, i);
}

export function joinPath(dir: string, name: string): string {
  return dir === "/" ? `/${name}` : `${dir}/${name}`;
}

/** Segmentos para las migas de pan: `/C/Users` → [{C:, /C}, {Users, /C/Users}]. */
export function breadcrumbs(path: string, windowsStyle: boolean) {
  const parts = path.split("/").filter(Boolean);
  return parts.map((part, i) => ({
    label: i === 0 && windowsStyle && /^[A-Za-z]$/.test(part) ? `${part}:` : part,
    path: "/" + parts.slice(0, i + 1).join("/"),
  }));
}

/** ¿Las rutas del snapshot son de Windows? (la primera carpeta es una letra de unidad) */
export const isWindowsSnapshot = (paths: string[]) => paths.some((p) => /^[A-Za-z]:[\\/]/.test(p));

/** Ruta del snapshot para mostrar: `/C/Users/Ana` → `C:\Users\Ana` en Windows. */
export function displayPath(path: string, windowsStyle: boolean): string {
  if (!windowsStyle) return path;
  const m = /^\/([A-Za-z])(\/.*)?$/.exec(path);
  return m ? `${m[1]}:${(m[2] ?? "\\").replace(/\//g, "\\")}` : path.replace(/\//g, "\\");
}

/**
 * ¿`path` es `dir` o está dentro de ella? Ambas en formato del snapshot.
 * En Windows no distingue mayúsculas.
 */
export function isWithin(path: string, dir: string, windowsStyle: boolean): boolean {
  const norm = (p: string) => (windowsStyle ? p.toLowerCase() : p).replace(/\/+$/, "") || "/";
  const a = norm(path);
  const b = norm(dir);
  return b === "/" || a === b || a.startsWith(`${b}/`);
}

/** Carpeta común más profunda de varias rutas del snapshot. */
export function commonDir(paths: string[]): string {
  if (!paths.length) return "/";
  const split = paths.map((p) => toSnapshotPath(p).split("/").filter(Boolean));
  const out: string[] = [];
  for (let i = 0; split.every((s) => i < s.length && s[i] === split[0][i]); i++) out.push(split[0][i]);
  return "/" + out.join("/");
}

/** Separa una ruta del sistema en carpeta padre y nombre final para mostrarla mejor. */
export function splitPath(path: string) {
  const trimmed = path.replace(/[\/]+$/, "");
  const i = Math.max(trimmed.lastIndexOf("\\"), trimmed.lastIndexOf("/"));
  return i < 0 ? { parent: "", name: trimmed } : { parent: trimmed.slice(0, i + 1), name: trimmed.slice(i + 1) || trimmed };
}
