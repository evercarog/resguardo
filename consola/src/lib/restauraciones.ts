// Registro de restauraciones (plan 0.8.0, E2: 13d, 14l). Cada restauración la
// anota el equipo (quién, cuándo, qué versión, desde dónde, cuánto y cuánto
// tardó, sin rutas); aquí se juntan con el motivo que añadió una persona en esta
// consola y se sacan las cifras (número, tiempo medio —el RTO real— y éxito).
// Sin dependencias de Svelte (lo prueban los vectores).
import type { MotivoRestauracion, NotaRestauracion, Restauracion, Restauraciones } from "./tipos";

export const MOTIVOS: { id: MotivoRestauracion; texto: string }[] = [
  { id: "borrado", texto: "Archivo borrado por error" },
  { id: "dano", texto: "Daño" },
  { id: "prueba", texto: "Prueba" },
  { id: "otro", texto: "Otro" },
];

export const textoMotivo = (m: string | null | undefined) => MOTIVOS.find((x) => x.id === m)?.texto ?? null;

/** A dónde se restauró, en palabras (nunca la ruta: no sale del equipo). */
export function textoDestino(d: string | null | undefined): string {
  switch (d) {
    case "junto":
      return "Junto al original";
    case "original":
      return "En su sitio";
    case "carpeta":
    case "nueva":
      return "En otra carpeta";
    case "zip":
      return "En un .zip en el equipo";
    case "descarga":
      return "Descargado en el navegador";
    default:
      return "En el equipo";
  }
}

/** Desde dónde se restauró. */
export function textoDesde(r: Pick<Restauracion, "desde" | "desde_nombre">): string {
  if (r.desde === "derivada") return r.desde_nombre ? `Copia derivada en «${r.desde_nombre}»` : "Una copia derivada";
  if (r.desde === "externa") return r.desde_nombre ? `Copia externa en «${r.desde_nombre}»` : "La copia externa";
  return "Su repositorio";
}

/** Quién y desde dónde la pidió. */
export function textoQuien(r: Pick<Restauracion, "por" | "consola" | "en_el_equipo">): string {
  if (r.en_el_equipo) return "Desde la ventana del equipo";
  const quien = r.por?.trim() || "Alguien";
  return r.consola ? `${quien}, desde «${r.consola}»` : quien;
}

/** Duración en palabras cortas: «45 s», «3 min», «1 h 20 min». */
export function textoTardo(s: number | null | undefined): string {
  if (s == null || !Number.isFinite(s)) return "—";
  if (s < 60) return `${Math.max(0, Math.round(s))} s`;
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  return m % 60 ? `${h} h ${m % 60} min` : `${h} h`;
}

/** Cada restauración con su motivo: el de esta consola manda sobre el que mandó la orden. */
export type ConMotivo = Restauracion & { motivoFinal: { tipo: MotivoRestauracion; notas?: string | null; por?: string | null } | null };

export function juntar(r: Restauraciones): ConMotivo[] {
  const notas = new Map<string, NotaRestauracion>(r.notas.map((n) => [`${n.equipo}|${n.id}`, n]));
  return [...r.entradas]
    .map((e) => {
      const n = notas.get(`${e.equipo}|${e.id}`);
      const motivoFinal = n ? { tipo: n.motivo, notas: n.notas ?? null, por: n.por ?? null } : e.motivo ? { tipo: e.motivo.tipo, notas: e.motivo.notas ?? null, por: e.por ?? null } : null;
      return { ...e, motivoFinal };
    })
    .sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora));
}

export interface Estadisticas {
  total: number;
  correctas: number;
  fallidas: number;
  /** De 0 a 100 (null sin restauraciones). */
  exito: number | null;
  /** Segundos: media y mediana de las correctas (el RTO real). */
  medio_s: number | null;
  mediana_s: number | null;
  /** La más larga de las correctas. */
  maximo_s: number | null;
  bytes: number;
  archivos: number;
  /** Las que cuentan como prueba de restauración (correctas y con algo restaurado). */
  reales: number;
  sinMotivo: number;
}

export function estadisticas(rs: ConMotivo[]): Estadisticas {
  const ok = rs.filter((r) => r.resultado === "ok");
  const tiempos = ok.map((r) => r.duracion_s).filter((x): x is number => typeof x === "number" && Number.isFinite(x) && x >= 0).sort((a, b) => a - b);
  const medio = tiempos.length ? tiempos.reduce((a, b) => a + b, 0) / tiempos.length : null;
  const mediana = tiempos.length ? (tiempos.length % 2 ? tiempos[(tiempos.length - 1) / 2] : (tiempos[tiempos.length / 2 - 1] + tiempos[tiempos.length / 2]) / 2) : null;
  return {
    total: rs.length,
    correctas: ok.length,
    fallidas: rs.length - ok.length,
    exito: rs.length ? Math.round((ok.length / rs.length) * 100) : null,
    medio_s: medio == null ? null : Math.round(medio),
    mediana_s: mediana == null ? null : Math.round(mediana),
    maximo_s: tiempos.length ? tiempos[tiempos.length - 1] : null,
    bytes: ok.reduce((n, r) => n + (r.bytes ?? 0), 0),
    archivos: ok.reduce((n, r) => n + (r.archivos ?? 0), 0),
    reales: ok.filter((r) => (r.archivos ?? 0) > 0 || (r.bytes ?? 0) > 0).length,
    sinMotivo: rs.filter((r) => !r.motivoFinal).length,
  };
}

/** Restauraciones por mes (los `meses` últimos hasta `ahora`, el más antiguo primero), con su tiempo medio. */
export function porMes(rs: ConMotivo[], ahora: number, meses = 12): { mes: string; etiqueta: string; correctas: number; fallidas: number; medio_s: number | null }[] {
  const d = new Date(ahora);
  const out: { mes: string; etiqueta: string; correctas: number; fallidas: number; medio_s: number | null }[] = [];
  const NOMBRES = ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic"];
  for (let i = meses - 1; i >= 0; i--) {
    const x = new Date(d.getFullYear(), d.getMonth() - i, 1);
    const mes = `${x.getFullYear()}-${String(x.getMonth() + 1).padStart(2, "0")}`;
    const del = rs.filter((r) => {
      const t = new Date(r.hora);
      return t.getFullYear() === x.getFullYear() && t.getMonth() === x.getMonth();
    });
    const ok = del.filter((r) => r.resultado === "ok");
    const ts = ok.map((r) => r.duracion_s).filter((v): v is number => typeof v === "number");
    out.push({ mes, etiqueta: NOMBRES[x.getMonth()], correctas: ok.length, fallidas: del.length - ok.length, medio_s: ts.length ? Math.round(ts.reduce((a, b) => a + b, 0) / ts.length) : null });
  }
  return out;
}

/** Plan 0.8.0 (14m): de dónde se puede restaurar un repositorio, de lo más cercano a lo más lejano. */
export interface OrigenRestauracion {
  /** `principal`, `externa`, una copia derivada (su id) o un espejo (`espejo:<clave>`). */
  id: string;
  tipo: "principal" | "externa" | "derivada" | "espejo";
  nombre: string;
  /** Dónde está (destino), en palabras. */
  donde: string;
  /** `ok`, `aviso`, `fallo` o `desconocido`. */
  estado: "ok" | "aviso" | "fallo" | "desconocido";
  /** Última actualización (RFC 3339) si se sabe. */
  ultima: string | null;
  /** Por qué no se puede (agente anterior, el equipo no responde…), si no se puede. */
  noSe?: string | null;
  recomendado?: boolean;
}

/**
 * El recomendado: el primero disponible en orden de cercanía (el principal, las copias del
 * propio equipo y, al final, los espejos del almacén), sin fallos; si no hay ninguno sin
 * fallos, el primero disponible.
 */
export function recomendar(os: OrigenRestauracion[]): OrigenRestauracion[] {
  const posibles = os.filter((o) => !o.noSe);
  const elegido = posibles.find((o) => o.estado === "ok") ?? posibles.find((o) => o.estado !== "fallo") ?? posibles[0];
  return os.map((o) => ({ ...o, recomendado: !!elegido && o.id === elegido.id }));
}

const SISTEMA_WINDOWS = ["windows", "program files", "program files (x86)", "programdata"];
const SISTEMA_LINUX = ["/proc", "/sys", "/dev", "/run", "/boot", "/bin", "/sbin", "/lib", "/lib64", "/usr", "/etc", "/var/lib/resguardo-agente", "/opt/resguardo-agente"];

/**
 * Por qué no vale una carpeta para restaurar (13a, 13b), antes de mandarla (el
 * equipo lo vuelve a comprobar todo: nueva o vacía, enlaces, su ruta real).
 * `null` si parece valer.
 */
export function errorCarpetaRestaurar(ruta: string, windows: boolean): string | null {
  const r = ruta.trim();
  if (!r) return "Escribe la carpeta o elígela con «Explorar…».";
  if ([...r].some((c) => c.charCodeAt(0) < 32)) return "La ruta tiene caracteres que no valen.";
  if (windows) {
    if (!/^[A-Za-z]:\\/.test(r)) return "Escribe la ruta entera de un disco de este equipo, por ejemplo D:\\Restaurado.";
    const partes = r.slice(3).replace(/\\+$/, "").split("\\");
    const malo = (x: string) => x === "." || x === ".." || /[. ]$/.test(x) || /[/:*?"<>|]/.test(x);
    if (r.slice(3).replace(/\\+$/, "") && partes.some((x) => x === "" || malo(x))) return "La ruta tiene nombres que no valen (sin «..», ni : * ? \" < > |, ni punto o espacio al final).";
    const primera = (partes[0] ?? "").toLowerCase();
    if (SISTEMA_WINDOWS.includes(primera)) return "No se restaura en carpetas del sistema ni de los programas: elige otra.";
  } else {
    if (!r.startsWith("/") || r.startsWith("//")) return "Escribe la ruta entera, por ejemplo /srv/restaurado.";
    if (r.split("/").some((x) => x === "." || x === "..")) return "La ruta no puede llevar «..».";
    const sin = r.replace(/\/+$/, "") || "/";
    if (sin === "/" || SISTEMA_LINUX.some((x) => sin === x || sin.startsWith(`${x}/`))) return "No se restaura en carpetas del sistema: elige otra.";
  }
  return null;
}

/** El nombre del .zip (sin «.zip»): vacío vale (el equipo pone «Restaurado …»). */
export function errorNombreZip(n: string): string | null {
  const x = n.trim().replace(/\.zip$/i, "");
  if (!x) return null;
  if (x.length > 120 || /[\\/:*?"<>|]/.test(x) || [...x].some((c) => c.charCodeAt(0) < 32) || x.startsWith(".") || /[. ]$/.test(x)) {
    return "Ese nombre no vale: hasta 120 caracteres, sin \\ / : * ? \" < > |.";
  }
  if (/^(con|prn|aux|nul|com\d|lpt\d)(\.|$)/i.test(x)) return "Ese nombre no vale en Windows.";
  return null;
}

/** La contraseña del .zip: opcional; si se escribe, de 8 a 128 caracteres y repetida igual. */
export function errorContrasenaZip(a: string, b: string): string | null {
  if (!a && !b) return null;
  if (a.length < 8) return "La contraseña del .zip, de 8 caracteres o más.";
  if (a.length > 128) return "La contraseña del .zip, como mucho 128 caracteres.";
  if (a !== b) return "Las dos contraseñas no coinciden.";
  return null;
}
