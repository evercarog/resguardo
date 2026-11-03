// 0.8.3 (16b): los archivos que no se pudieron leer en la última copia, pedidos al equipo
// por la sesión cifrada (`elegir_carpetas`, con la clave de administración: las rutas no
// pasan en claro por el servidor), con el motivo en palabras, agrupados por motivo y por
// carpeta, y «No copiar esto». Sin runas: se prueba en scripts/vectores-083-servidor.ts.
import { misma } from "./arbolCarpetas";

/** `resumen.admite` de un agente que sabe decirlos (operación `archivos_con_error`). */
export const ADMITE_ARCHIVOS_CON_ERROR = "archivos_con_error";

export interface ArchivoConError {
  ruta: string;
  motivo: string;
  /** En cuántas de las copias miradas falló (contando la última). */
  veces: number;
}

export type ClaseError = "en_uso" | "permiso" | "nube" | "larga" | "no_existe" | "otro";

export const TEXTO_CLASE: Record<ClaseError, { titulo: string; explica: string }> = {
  en_uso: { titulo: "En uso por otro programa", explica: "Estaban abiertos (correo, bases de datos, programas). Entran en la próxima copia si están libres; si siempre fallan, cierra ese programa a la hora de la copia o usa «Antes de copiar»." },
  permiso: { titulo: "Sin permiso", explica: "El agente no tiene permiso para leerlos (carpetas protegidas o de otro usuario con permisos especiales)." },
  nube: {
    titulo: "Archivos de OneDrive (u otra nube) sin descargar",
    explica: "Están solo en la nube: en el equipo hay un marcador, no el archivo. Para copiarlos, márcalos como «Mantener siempre en este dispositivo»; si ya están a salvo en la nube, puedes no copiarlos.",
  },
  larga: { titulo: "Ruta demasiado larga", explica: "La ruta pasa del límite de Windows. Acorta los nombres de las carpetas o mueve el archivo más arriba." },
  no_existe: { titulo: "Ya no estaban", explica: "Se borraron o movieron mientras se copiaba. Normalmente no hay que hacer nada." },
  otro: { titulo: "Otros motivos", explica: "El motivo exacto va junto a cada archivo." },
};

/** El motivo en una clase (por el texto del error del equipo, en inglés o en español). */
export function claseError(motivo: string): ClaseError {
  const m = motivo.toLowerCase();
  if (/cloud file|cloud operation|cloud provider|nube|0x8007016a|onedrive/.test(m)) return "nube";
  if (/being used by another process|used by another|en uso|sharing violation|locked a portion|lock violation/.test(m)) return "en_uso";
  if (/access is denied|acceso denegado|permission denied|permiso|operation not permitted/.test(m)) return "permiso";
  if (/too long|demasiado larg|name too long|path too long/.test(m)) return "larga";
  if (/cannot find|no such file|not found|no se encuentra|no existe|el agente no encuentra/.test(m)) return "no_existe";
  return "otro";
}

/** La carpeta de una ruta (Windows o no). */
export function carpetaDe(ruta: string): string {
  const i = Math.max(ruta.lastIndexOf("\\"), ruta.lastIndexOf("/"));
  if (i <= 0) return ruta.startsWith("/") ? "/" : ruta;
  // «C:\x» → «C:\».
  return /^[A-Za-z]:$/.test(ruta.slice(0, i)) ? ruta.slice(0, i + 1) : ruta.slice(0, i);
}

/** El nombre del archivo (lo último de la ruta). */
export const nombreDe = (ruta: string) => ruta.slice(Math.max(ruta.lastIndexOf("\\"), ruta.lastIndexOf("/")) + 1) || ruta;

export interface GrupoErrores {
  clase: ClaseError;
  titulo: string;
  explica: string;
  carpetas: { carpeta: string; archivos: ArchivoConError[] }[];
  total: number;
}

/** Por motivo (los más numerosos primero) y, dentro, por carpeta. */
export function agruparArchivos(archivos: readonly ArchivoConError[]): GrupoErrores[] {
  const porClase = new Map<ClaseError, Map<string, ArchivoConError[]>>();
  for (const a of archivos) {
    const c = claseError(a.motivo);
    const m = porClase.get(c) ?? new Map<string, ArchivoConError[]>();
    const k = carpetaDe(a.ruta);
    m.set(k, [...(m.get(k) ?? []), a]);
    porClase.set(c, m);
  }
  return [...porClase.entries()]
    .map(([clase, m]) => ({
      clase,
      ...TEXTO_CLASE[clase],
      carpetas: [...m.entries()].map(([carpeta, xs]) => ({ carpeta, archivos: xs })).sort((a, b) => a.carpeta.localeCompare(b.carpeta)),
      total: [...m.values()].reduce((n, xs) => n + xs.length, 0),
    }))
    .sort((a, b) => b.total - a.total || (a.clase === "otro" ? 1 : b.clase === "otro" ? -1 : 0));
}

/** «Son siempre los mismos: fallaron en las últimas 5 copias», o cuántos se repiten; null si no hay con qué comparar. */
export function resumenRepeticion(archivos: readonly ArchivoConError[], copias: number): string | null {
  if (copias < 2 || !archivos.length) return null;
  const siempre = archivos.filter((a) => a.veces >= copias).length;
  if (siempre === archivos.length) return archivos.length === 1 ? `Es siempre el mismo archivo: falló en las últimas ${copias} copias.` : `Son siempre los mismos ${archivos.length} archivos: fallaron en las últimas ${copias} copias.`;
  if (siempre) return `${siempre} de ${archivos.length} fallaron en todas las últimas ${copias} copias; los demás, solo en algunas.`;
  return null;
}

/** Lo que llega del equipo, comprobado (lo que no cuadra, fuera). */
export function leerArchivos(x: unknown): { archivos: ArchivoConError[]; copias: number } {
  const o = (x && typeof x === "object" ? x : {}) as Record<string, unknown>;
  const archivos = (Array.isArray(o.archivos) ? o.archivos : [])
    .slice(0, 500)
    .filter((a): a is Record<string, unknown> => !!a && typeof a === "object")
    .map((a) => ({ ruta: typeof a.ruta === "string" ? a.ruta.slice(0, 4096) : "", motivo: typeof a.motivo === "string" ? a.motivo.slice(0, 1000) : "", veces: typeof a.veces === "number" && a.veces > 0 ? Math.floor(a.veces) : 1 }))
    .filter((a) => a.ruta);
  const copias = typeof o.copias === "number" && o.copias > 0 ? Math.min(10, Math.floor(o.copias)) : 1;
  return { archivos, copias };
}

/** Las exclusiones de la copia con estas rutas añadidas (sin repetir ninguna). */
export function conExclusiones(actuales: readonly string[], rutas: readonly string[], win: boolean): string[] {
  const out = [...actuales];
  for (const r of rutas) if (r && !out.some((x) => misma(x, r, win))) out.push(r);
  return out;
}
