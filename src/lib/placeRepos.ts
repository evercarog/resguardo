// Ayudas para crear o usar repositorios dentro de un destino: ruta sugerida
// y contraseña generada (ver docs/destinos.md, fase 2).
import type { Repo } from "$lib/api";

export const slug = (s: string) =>
  s
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "") || "repositorio";

/**
 * Contraseña fuerte y fácil de copiar a mano: 6 grupos de 4 letras y cifras
 * sin caracteres que se confundan (unos 124 bits).
 */
export function generatePassword() {
  const alphabet = "abcdefghjkmnpqrstuvwxyzABCDEFGHJKMNPQRSTUVWXYZ23456789";
  const bytes = new Uint32Array(24);
  crypto.getRandomValues(bytes);
  const chars = [...bytes].map((n) => alphabet[n % alphabet.length]);
  return Array.from({ length: 6 }, (_, i) => chars.slice(i * 4, i * 4 + 4).join("")).join("-");
}

/** Ubicación nueva dentro del mismo destino que `template` (junto a él, con otro nombre). */
export function siblingLocation(template: Repo, name: string) {
  const loc = template.location.trim().replace(/[\\/]+$/, "");
  const leaf = slug(name);
  if (/^[a-z0-9]+:/i.test(loc) && !/^[a-z]:/i.test(loc)) {
    if (loc.startsWith("s3:")) {
      const m = /^(s3:(?:https?:\/\/)?[^/]+\/[^/]+)(\/.*)?$/.exec(loc);
      const base = m?.[1] ?? loc;
      const rest = (m?.[2] ?? "").split("/").filter(Boolean);
      // Junto al repositorio de ejemplo: misma carpeta padre dentro del bucket.
      return [base, ...rest.slice(0, -1), leaf].join("/");
    }
    if (loc.startsWith("rest:")) {
      const m = /^(rest:https?:\/\/[^/]+)(\/.*)?$/.exec(loc);
      const rest = (m?.[2] ?? "").split("/").filter(Boolean);
      return `${[m?.[1] ?? loc, ...rest.slice(0, -1), leaf].join("/")}/`;
    }
    if (loc.startsWith("sftp:")) {
      const i = loc.lastIndexOf("/");
      return i > loc.indexOf(":") ? `${loc.slice(0, i)}/${leaf}` : `${loc}/${leaf}`;
    }
    const i = loc.lastIndexOf("/");
    return i > 0 ? `${loc.slice(0, i)}/${leaf}` : `${loc}/${leaf}`;
  }
  // Carpeta local o de red: junto a la del repositorio de ejemplo.
  const i = Math.max(loc.lastIndexOf("\\"), loc.lastIndexOf("/"));
  const sep = loc.includes("\\") ? "\\" : "/";
  return i > 2 ? `${loc.slice(0, i)}${sep}${leaf}` : `${loc}${sep}${leaf}`;
}
