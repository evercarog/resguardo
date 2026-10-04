// El «agente» simulado para los detalles (api-servidor.md §7, v1.33):
// `diferencias` (qué cambió entre dos versiones, por páginas), `ocupa` (lo que
// más ocupa de una versión) e `historial_archivo` (en qué versiones está un
// archivo). Todo sale de una semilla (los ids de las versiones), así que es lo
// mismo cada vez, y los recuentos cuadran con los del informe del equipo.
import type * as T from "../lib/tipos";
import { estado } from "./estado";

/** Desde qué versión del agente se anuncian (el agente real: 0.7.14). */
export const VERSION_DETALLE = "0.7.14";
export const OPS_DETALLE = ["diferencias", "ocupa", "historial_archivo"];
/** Página pequeña en el simulador, para que se vea la carga por páginas. */
const PAGINA = 400;

/** Generador pseudoaleatorio con semilla (mulberry32 sobre un hash del texto). */
function azar(semilla: string) {
  let h = 2166136261;
  for (let i = 0; i < semilla.length; i++) h = Math.imul(h ^ semilla.charCodeAt(i), 16777619);
  let a = h >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const CARPETAS = [
  "/C/Users/recepcion/Documents/Facturas 2026/Octubre",
  "/C/Users/recepcion/Documents/Facturas 2026/Septiembre",
  "/C/Users/recepcion/Documents/Clientes",
  "/C/Users/recepcion/Documents/Clientes/Contratos",
  "/C/Users/recepcion/Desktop",
  "/C/Users/recepcion/Pictures/Escaneos",
  "/C/SIIWI01/Datos",
  "/C/SIIWI01/BckRebuild",
  "/C/SIIWI01/Informes",
];
const NOMBRES: Record<string, string[]> = {
  Octubre: ["FV-10234.pdf", "FV-10235.pdf", "FV-10236.pdf", "FV-10237.pdf", "FV-10238.pdf", "Resumen octubre.xlsx", "NC-0045.pdf"],
  Septiembre: ["FV-10101.pdf", "FV-10102.pdf", "FV-10150.pdf", "Resumen septiembre.xlsx"],
  Clientes: ["Lista de clientes.xlsx", "Cartera vencida.xlsx", "Directorio proveedores.docx"],
  Contratos: ["Contrato arriendo oficina.pdf", "Contrato soporte 2026.pdf", "Otrosí 3.pdf"],
  Desktop: ["Notas reunión.txt", "Pendientes.docx", "Precios nuevos [borrador].xlsx"],
  Escaneos: ["Escaneo 0001.jpg", "Escaneo 0002.jpg", "Escaneo 0003.jpg", "Cédula Ana.png"],
  Datos: ["Z01_2026.DAT", "Z02_2026.DAT", "Z49.DAT", "CONTAB.IDX", "TERCEROS.DAT", "INVENT.DAT"],
  BckRebuild: ["Rebuild_20261001.bak", "Rebuild_20261002.bak", "Rebuild_20261003.bak"],
  Informes: ["Balance general.pdf", "Estado de resultados.pdf", "Auxiliar 1105.xlsx"],
};
const TAM_BASE: Record<string, number> = { pdf: 240_000, xlsx: 90_000, docx: 60_000, txt: 3_000, jpg: 1_800_000, png: 900_000, DAT: 48_000_000, IDX: 6_000_000, bak: 1_400_000_000 };

/** Todos los archivos «del equipo» con un tamaño base. */
const ARCHIVOS = CARPETAS.flatMap((c) => (NOMBRES[c.split("/").pop()!] ?? []).map((n) => `${c}/${n}`));
function tamano(ruta: string, semilla: string) {
  const ext = ruta.split(".").pop() ?? "";
  const r = azar(`${ruta}|${semilla}`);
  return Math.round((TAM_BASE[ext] ?? 50_000) * (0.4 + r() * 1.2));
}

interface VersionMock {
  id: string;
  hora: string;
  copia: string | null;
  nuevos: number;
  cambiados: number;
}

function versionesDe(equipo: string, repo: string | null): VersionMock[] {
  const inf = estado.equipos.find((x) => x.id === equipo)?.informes[0]?.datos.repos?.find((x: T.RepoInforme) => x.id === repo);
  return (inf?.versiones ?? []).map((v: T.VersionInforme) => ({ id: v.id, hora: v.hora, copia: v.copia, nuevos: v.archivos_nuevos ?? 0, cambiados: v.archivos_cambiados ?? 0 }));
}
const mismo = (a: string, b: string) => a.startsWith(b) || b.startsWith(a);

/** Archivos nuevos que no son de la lista base: nombres creíbles numerados. */
function nombreNuevo(r: () => number, i: number) {
  const c = CARPETAS[Math.floor(r() * 4)];
  const n = 10_400 + Math.floor(r() * 500) + i;
  return `${c}/${r() < 0.7 ? `FV-${n}.pdf` : `Escaneo ${String(n).slice(-4)}.jpg`}`;
}

interface Cambio {
  ruta: string;
  tipo: "nuevo" | "cambiado" | "borrado" | "metadatos";
  bytes?: number;
  bytes_antes?: number;
}

/** Lo que cambió de `desde` a `hasta`: tantos nuevos y cambiados como dice el informe. */
function calcular(hasta: VersionMock, desde: VersionMock, versiones: VersionMock[]) {
  const r = azar(`${desde.id}>${hasta.id}`);
  // Si se compara con una más antigua, se suman las de en medio (de la misma copia).
  const entre = versiones.filter((v) => v.copia === hasta.copia && Date.parse(v.hora) > Date.parse(desde.hora) && Date.parse(v.hora) <= Date.parse(hasta.hora));
  const nNuevos = Math.min(3000, entre.reduce((n, v) => n + v.nuevos, 0) || hasta.nuevos);
  const nCambiados = Math.min(3000, entre.reduce((n, v) => n + v.cambiados, 0) || hasta.cambiados);
  const nBorrados = Math.floor(r() * 4);
  const cambios = new Map<string, Cambio>();
  for (let i = 0; cambios.size < nNuevos && i < nNuevos * 3; i++) {
    const ruta = nombreNuevo(r, i);
    if (!cambios.has(ruta)) cambios.set(ruta, { ruta, tipo: "nuevo", bytes: tamano(ruta, hasta.id) });
  }
  const base = [...ARCHIVOS].sort(() => r() - 0.5);
  for (let i = 0; i < nCambiados; i++) {
    const ruta = i < base.length ? base[i] : `/C/SIIWI01/Datos/Z${String(i).padStart(3, "0")}.DAT`;
    if (cambios.has(ruta)) continue;
    const antes = tamano(ruta, desde.id);
    cambios.set(ruta, { ruta, tipo: r() < 0.08 ? "metadatos" : "cambiado", bytes_antes: antes, bytes: Math.max(0, Math.round(antes * (0.9 + r() * 0.3))) });
  }
  for (let i = 0; i < nBorrados; i++) {
    const ruta = `/C/Users/recepcion/Desktop/${["Copia de Pendientes.docx", "Nuevo documento de texto.txt", "temp_export.csv", "Borrador viejo.xlsx"][i]}`;
    cambios.set(ruta, { ruta, tipo: "borrado", bytes_antes: tamano(ruta, desde.id) });
  }
  const lista = [...cambios.values()].sort((a, b) => a.ruta.localeCompare(b.ruta));
  const cuenta = (t: Cambio["tipo"]) => lista.filter((c) => c.tipo === t).length;
  const carpetasNuevas = new Set(lista.filter((c) => c.tipo === "nuevo").map((c) => c.ruta.slice(0, c.ruta.lastIndexOf("/")))).size > 3 ? 1 : 0;
  return {
    lista,
    resumen: {
      nuevos: cuenta("nuevo"),
      cambiados: cuenta("cambiado"),
      borrados: cuenta("borrado"),
      metadatos: cuenta("metadatos"),
      otros: 0,
      carpetas_nuevas: carpetasNuevas,
      carpetas_borradas: 0,
      bytes_anadidos: lista.filter((c) => c.tipo !== "borrado").reduce((n, c) => n + Math.round((c.bytes ?? 0) * 0.35), 0),
      bytes_quitados: lista.filter((c) => c.tipo === "borrado").reduce((n, c) => n + (c.bytes_antes ?? 0), 0),
    },
  };
}

/** Responde una operación de detalle; `null` si no es de estas. */
export async function operarDetalle(
  op: string,
  m: Record<string, unknown>,
  s: { equipo: string; repo: string | null },
  trabajando: () => void,
): Promise<Record<string, unknown> | null> {
  const versiones = versionesDe(s.equipo, s.repo);
  const valida = (x: unknown) => typeof x === "string" && /^[0-9a-f]{8,64}$/.test(x);
  if (op === "diferencias") {
    if (!valida(m.hasta) || (m.desde != null && !valida(m.desde))) return { error: "Versión no válida." };
    const hasta = versiones.find((v) => mismo(v.id, String(m.hasta)));
    if (!hasta) return { error: "Esa versión ya no está en el repositorio." };
    let desde: VersionMock | undefined;
    if (m.desde) desde = versiones.find((v) => mismo(v.id, String(m.desde)));
    else
      desde = versiones
        .filter((v) => v.copia === hasta.copia && Date.parse(v.hora) < Date.parse(hasta.hora))
        .sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora))[0];
    if (!desde) return m.desde ? { error: "Esa versión ya no está en el repositorio." } : { hasta: hasta.id, desde: null, primera: true };
    const indice = Math.max(0, Number(m.indice ?? 0) | 0);
    // La primera página «tarda»: el equipo dice que sigue con ello.
    if (indice === 0) {
      trabajando();
      await new Promise((r) => setTimeout(r, 700));
    }
    const { lista, resumen } = calcular(hasta, desde, versiones);
    const pagina = lista.slice(indice, indice + PAGINA);
    const siguiente = indice + PAGINA < lista.length ? indice + PAGINA : null;
    return { desde: desde.id, hasta: hasta.id, desde_cuando: desde.hora, resumen, total: lista.length, recortado: false, con_tamanos: true, indice, siguiente, cambios: pagina };
  }
  if (op === "ocupa") {
    if (!valida(m.version)) return { error: "Versión no válida." };
    trabajando();
    await new Promise((r) => setTimeout(r, 600));
    const v = String(m.version);
    const archivos = ARCHIVOS.map((ruta) => ({ ruta, bytes: tamano(ruta, v), archivos: 1 })).sort((a, b) => b.bytes - a.bytes);
    const carpetas = CARPETAS.map((c) => {
      const dentro = archivos.filter((a) => a.ruta.startsWith(`${c}/`));
      return { ruta: c, bytes: dentro.reduce((n, a) => n + a.bytes, 0), archivos: dentro.length * 37 };
    }).sort((a, b) => b.bytes - a.bytes);
    return { version: v, total_bytes: archivos.reduce((n, a) => n + a.bytes, 0), total_archivos: 14_200, carpetas, archivos: archivos.slice(0, 50) };
  }
  if (op === "historial_archivo") {
    const ruta = String(m.ruta ?? "");
    if (!ruta.startsWith("/") || ruta.endsWith("/") || ruta.split("/").slice(1).some((p) => !p || p === "." || p === "..")) return { error: "Ruta no válida dentro de la versión." };
    trabajando();
    await new Promise((r) => setTimeout(r, 500));
    const r = azar(ruta);
    const nace = Math.floor(r() * Math.max(1, versiones.length - 2));
    let bytes = tamano(ruta, "inicio");
    let modificado = new Date(Date.parse(versiones.at(-1)?.hora ?? new Date().toISOString()) - 86_400_000).toISOString();
    const out = [];
    // De la más antigua a la más reciente: a veces cambia.
    for (const v of [...versiones].reverse().slice(nace)) {
      if (r() < 0.25) {
        bytes = Math.round(bytes * (0.95 + r() * 0.2));
        modificado = new Date(Date.parse(v.hora) - 3600_000).toISOString();
      }
      out.push({ version: v.id, cuando: v.hora, bytes, modificado });
    }
    return { ruta, versiones: out.reverse() };
  }
  return null;
}
