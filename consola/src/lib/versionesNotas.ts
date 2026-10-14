// Plan 0.8.0, F2: comentarios en las versiones (14ww, parte 2) y versiones que
// se conservan para siempre (14cc). Sin estado ni red: lo prueban los vectores
// (scripts/vectores-versiones.ts).
//
// Los guarda el **equipo** (como su nombre o su observación) y los dice en su
// resumen: los ven todas sus consolas. Nunca van al repositorio. «Conservar»
// pide la clave de administración y un motivo (también al quitarlo, que espera:
// reduce la protección). Si el repositorio está en un almacén de solo añadir, su
// retención la aplica el almacén: la orden va también al almacén, que guarda la
// lista de ids que nunca quita (crates/agente/src/versiones_v2.rs).
import { destinoDe } from "./repo";
import { almacenDe } from "./retencion";
import type { ComentarioVersion, Equipo, RepositorioResumen, VersionConservada } from "./tipos";

export const ADMITE_COMENTARIOS = "comentarios_version";
export const ADMITE_CONSERVAR = "conservar_version";
export const admiteComentarios = (e: Equipo | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_COMENTARIOS);
export const admiteConservar = (e: Equipo | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_CONSERVAR);

/** Como el agente. */
export const MAX_TEXTO = 280;
export const MIN_MOTIVO = 3;
export const MAX_MOTIVO = 200;
/** Lo que se dice junto al campo (14ww: «sin contraseñas ni claves»). */
export const AVISO_SECRETOS = "No pongas contraseñas ni claves aquí: lo ven todas las consolas del equipo.";

/** ¿Es la misma versión? (Por el principio del id: corto o completo.) */
export function mismaVersion(a: string, b: string): boolean {
  const n = Math.min(a.length, b.length);
  return n >= 8 && a.slice(0, n).toLowerCase() === b.slice(0, n).toLowerCase();
}

/**
 * ¿Parece una contraseña o una clave? Lo mismo que el agente
 * (`versiones_v2::parece_secreto`): un bloque PEM, «contraseña: …» o una palabra
 * de 20 o más caracteres que mezcla letras y números (sin ser una dirección).
 */
export function pareceSecreto(t: string): boolean {
  const bajo = t.toLowerCase();
  if (bajo.includes("-----begin")) return true;
  for (const p of ["contraseña", "contrasena", "password", "passwd", "clave", "token", "secret", "api key", "apikey"]) {
    let i = bajo.indexOf(p);
    while (i >= 0) {
      const resto = bajo.slice(i + p.length).trimStart();
      if (resto.startsWith(":") || resto.startsWith("=") || resto.startsWith("es ")) return true;
      i = bajo.indexOf(p, i + 1);
    }
  }
  return t.split(/\s+/).some((w0) => {
    const w = w0.replace(/^[.,;:()[\]{}«»"']+|[.,;:()[\]{}«»"']+$/g, "");
    return [...w].length >= 20 && /\p{L}/u.test(w) && /\d/.test(w) && !w.includes("://");
  });
}

/** Lo que falla de un comentario (vacío: borrar), o null. */
export function errorComentario(texto: string): string | null {
  const t = texto.replace(/\r\n?/g, "\n").trim();
  if ([...t].length > MAX_TEXTO) return `Como mucho ${MAX_TEXTO} caracteres.`;
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u0009\u000b-\u001f\u007f]/.test(t)) return "El texto lleva caracteres no válidos.";
  if (pareceSecreto(t)) return "Parece una contraseña o una clave: no la pongas en un comentario.";
  return null;
}

/** Lo que falla del motivo de «Conservar» (y de quitarlo), o null. */
export function errorMotivo(motivo: string): string | null {
  const t = motivo.replace(/\s+/g, " ").trim();
  if ([...t].length < MIN_MOTIVO || [...t].length > MAX_MOTIVO) return `Escribe el motivo (de ${MIN_MOTIVO} a ${MAX_MOTIVO} caracteres).`;
  if (pareceSecreto(t)) return "Parece una contraseña o una clave: no la pongas en el motivo.";
  return null;
}

/** Los comentarios de una versión, del más antiguo al más reciente. */
export function comentariosDe(e: Equipo | null | undefined, repo: string, version: string): ComentarioVersion[] {
  return (e?.resumen?.comentarios ?? []).filter((c) => c.repo === repo && mismaVersion(c.version, version)).sort((a, b) => a.cuando.localeCompare(b.cuando));
}

/** ¿La puede cambiar o borrar esta persona? (La misma cuenta desde esta consola.) */
export const esMio = (c: ComentarioVersion, cuenta: string | null | undefined) => !!c.esta && !!cuenta && c.autor === cuenta;

/** La marca «Conservar para siempre» de una versión en el equipo dueño, o null. */
export function conservadaDe(e: Equipo | null | undefined, repo: string, version: string): VersionConservada | null {
  return (e?.resumen?.conservadas ?? []).find((c) => c.repo === repo && mismaVersion(c.version, version)) ?? null;
}

/** Ids conservados (para marcarlos en las listas y en la simulación de la retención). */
export function idsConservados(e: Equipo | null | undefined, repo: string): string[] {
  return (e?.resumen?.conservadas ?? []).filter((c) => c.repo === repo).map((c) => c.version);
}

/** ¿Está entre estos ids (cortos o completos)? */
export const estaEn = (ids: string[], version: string) => ids.some((x) => mismaVersion(x, version));

/** A quién se manda «Conservar»: al equipo dueño y, si su repositorio está en un almacén que lo entiende, al almacén. */
export interface DestinoConservar {
  equipo: Equipo;
  cuerpo: Record<string, unknown>;
  /** «almacén» o «equipo» (para decirlo). */
  quien: "equipo" | "almacen";
}

export function destinosConservar(e: Equipo, repo: RepositorioResumen, equipos: Equipo[], version: string, motivo: string, conservar: boolean): DestinoConservar[] {
  const m = motivo.replace(/\s+/g, " ").trim();
  const out: DestinoConservar[] = [{ equipo: e, quien: "equipo", cuerpo: { repo: repo.id, version, motivo: m, conservar } }];
  const a = almacenDe(repo, destinoDe(e.resumen?.destinos, repo), equipos);
  if (a && a.almacen.id !== e.id && admiteConservar(a.almacen)) out.push({ equipo: a.almacen, quien: "almacen", cuerpo: { usuario: a.usuario, repo: a.carpeta, version, motivo: m, conservar } });
  return out;
}

/** ¿La conserva también el almacén? (Si el repositorio está en uno que lo entiende.) */
export function conservadaEnAlmacen(e: Equipo, repo: RepositorioResumen, equipos: Equipo[], version: string): boolean | null {
  const a = almacenDe(repo, destinoDe(e.resumen?.destinos, repo), equipos);
  if (!a || !admiteConservar(a.almacen)) return null;
  const l = a.almacen.resumen?.guarda_copias?.conservadas?.find((x) => x.usuario === a.usuario && x.repo === a.carpeta);
  return !!l?.versiones.some((v) => mismaVersion(v.version, version));
}
