// Registro de restauraciones (plan 0.8.0, E2: 13d, 14l). Cada restauración la
// anota el equipo (quién, cuándo, qué versión, desde dónde, cuánto y cuánto
// tardó, sin rutas); aquí se juntan con el motivo que añadió una persona en esta
// consola y se sacan las cifras (número, tiempo medio —el RTO real— y éxito).
// Sin dependencias de Svelte (lo prueban los vectores).
import type { MotivoRestauracion, NotaRestauracion, Restauracion, Restauraciones } from "./tipos";
import { bytes as tamano, plural } from "./formato";

/** 15w: lo que de verdad se restaura (la operación `medir` del equipo, o lo que se sabe en la consola). */
export interface MedidaRestaurar {
  carpetas: number;
  archivos: number;
  bytes: number;
  fuera_archivos?: number;
  fuera_bytes?: number;
  /** Sin medir: carpetas quitadas (no se sabe cuántos archivos llevan). */
  fuera_carpetas?: number;
  /** `false`: lo que se sabe en la consola (sin lo de dentro de las carpetas). */
  exacta?: boolean;
}

/** 15w: «1 carpeta · 20 archivos · 5,4 GB» y, si se quitó algo, «sin 2 archivos (3 MB)». */
export function textoMedida(m: MedidaRestaurar): { linea: string; sin: string | null } {
  const partes: string[] = [];
  if (m.carpetas) partes.push(plural(m.carpetas, "carpeta", "carpetas"));
  // Sin medir, una carpeta no dice cuántos archivos lleva dentro: no se pone «0 archivos».
  if (m.archivos || m.exacta !== false || !m.carpetas) partes.push(plural(m.archivos, "archivo", "archivos"));
  if (m.bytes) partes.push(tamano(m.bytes));
  const f = m.fuera_archivos ?? 0;
  const fc = m.exacta === false ? (m.fuera_carpetas ?? 0) : 0;
  const quitado = [fc ? plural(fc, "carpeta", "carpetas") : "", f ? plural(f, "archivo", "archivos") : ""].filter(Boolean).join(" y ");
  const sin = quitado ? `sin ${quitado}${m.fuera_bytes ? ` (${tamano(m.fuera_bytes)})` : ""}` : null;
  return { linea: partes.join(" · "), sin };
}

/**
 * 15w: lo que se sabe en la consola sin preguntar al equipo (agente anterior o mientras
 * contesta): las carpetas y archivos elegidos, con el tamaño de los archivos, y cuántas
 * cosas se quitaron de dentro. Las carpetas no dicen su tamaño hasta medirlas.
 */
export function medidaEstimada(elegidas: { tipo: "dir" | "archivo"; bytes?: number }[], quitadas: { tipo: "dir" | "archivo"; bytes?: number }[]): MedidaRestaurar {
  const archivos = elegidas.filter((e) => e.tipo !== "dir");
  return {
    carpetas: elegidas.length - archivos.length,
    archivos: archivos.length,
    bytes: archivos.reduce((n, e) => n + (e.bytes ?? 0), 0),
    fuera_archivos: quitadas.filter((e) => e.tipo !== "dir").length,
    fuera_carpetas: quitadas.filter((e) => e.tipo === "dir").length,
    fuera_bytes: quitadas.reduce((n, e) => n + (e.tipo !== "dir" ? (e.bytes ?? 0) : 0), 0),
    exacta: false,
  };
}

export const MOTIVOS: { id: MotivoRestauracion; texto: string }[] = [
  { id: "borrado", texto: "Archivo borrado por error" },
  { id: "dano", texto: "Daño" },
  { id: "prueba", texto: "Prueba" },
  { id: "otro", texto: "Otro" },
];

export const textoMotivo = (m: string | null | undefined) => MOTIVOS.find((x) => x.id === m)?.texto ?? null;

/** 15aa: una restauración real, una prueba automática (su horario) o una prueba a mano («Probar ahora»). */
export type ClaseRestauracion = "real" | "prueba_auto" | "prueba_mano";

/** 15aa: de qué clase es. Una prueba de un agente anterior (sin `origen`) cuenta como automática. */
export function claseRestauracion(r: Pick<Restauracion, "tipo" | "origen">): ClaseRestauracion {
  if (r.tipo !== "prueba_restauracion") return "real";
  return r.origen === "a_mano" ? "prueba_mano" : "prueba_auto";
}

export const NOMBRES_CLASE: Record<ClaseRestauracion, string> = { real: "Reales", prueba_auto: "Pruebas automáticas", prueba_mano: "Pruebas a mano" };

/** 15aa: cómo se comprobó lo restaurado en una prueba, en palabras. */
export function textoComprobacion(r: Pick<Restauracion, "tipo" | "comprobacion">): string | null {
  if (r.tipo !== "prueba_restauracion") return null;
  if (r.comprobacion === "contenido" || !r.comprobacion) return "Cada archivo leído entero y comparado con sus huellas de la versión, y con su tamaño.";
  return r.comprobacion;
}

/** 15aa: qué se restauró en una prueba: «20 archivos al azar (de 13.200 de la versión)». */
export function textoMuestra(r: Pick<Restauracion, "archivos" | "candidatos">): string {
  const n = r.archivos ?? 0;
  if (!n) return "Ningún archivo";
  const base = `${plural(n, "archivo", "archivos")} al azar`;
  return r.candidatos ? `${base} (de ${plural(r.candidatos, "archivo", "archivos")} de la versión)` : base;
}

/** A dónde se restauró, en palabras (nunca la ruta: no sale del equipo). */
export function textoDestino(d: string | null | undefined): string {
  switch (d) {
    case "privada":
      return "En una carpeta privada del equipo, borrada al terminar";
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
export function textoDesde(r: Pick<Restauracion, "desde" | "desde_nombre"> & Partial<Pick<Restauracion, "espejo">>): string {
  if (r.espejo) return r.espejo.nombre ? `Espejo «${r.espejo.nombre}»` : "Un espejo";
  if (r.desde === "espejo") return r.desde_nombre ? `Espejo «${r.desde_nombre}» del equipo` : "Un espejo del equipo";
  if (r.desde === "derivada") return r.desde_nombre ? `Copia derivada en «${r.desde_nombre}»` : "Una copia derivada";
  if (r.desde === "externa") return r.desde_nombre ? `Copia derivada en «${r.desde_nombre}»` : "La copia derivada de cada día";
  return "Su repositorio";
}

/** Quién y desde dónde la pidió (una prueba automática: su horario). */
export function textoQuien(r: Pick<Restauracion, "por" | "consola" | "en_el_equipo"> & Partial<Pick<Restauracion, "tipo" | "origen" | "cuenta">>): string {
  if (r.tipo === "prueba_restauracion") {
    if (r.origen !== "a_mano") return "Su horario (automática)";
    const quien = r.por?.trim();
    return quien ? (r.consola ? `${quien}, con «Probar ahora» desde «${r.consola}»` : `${quien}, con «Probar ahora»`) : "Con «Probar ahora»";
  }
  // 15mm: la persona del equipo, desde su propia cuenta de Windows o de Linux.
  if (r.cuenta) return `${r.cuenta}, en su equipo`;
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

/** 15aa: solo las restauraciones reales (las pruebas van aparte en las cifras). */
export const soloReales = <R extends Pick<Restauracion, "tipo" | "origen">>(rs: R[]) => rs.filter((r) => claseRestauracion(r) === "real");

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
 * El recomendado: el más cercano que responde (en orden: el principal, las copias del
 * propio equipo y, al final, los espejos del almacén); unos avisos (una copia que falló)
 * no lo descartan, que no responda sí. Si ninguno responde, el primero disponible.
 */
export function recomendar(os: OrigenRestauracion[]): OrigenRestauracion[] {
  const posibles = os.filter((o) => !o.noSe);
  const elegido = posibles.find((o) => o.estado !== "fallo") ?? posibles[0];
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
