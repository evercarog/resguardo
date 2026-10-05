// «Buscar archivos» en todas las versiones: la lógica (sin interfaz) de la
// página /c/{c}/buscar. El equipo busca con `buscar_todas` en la sesión
// cifrada `explorar` (api-servidor.md §7) y manda los archivos agrupados, con
// las versiones en las que está cada uno. Aquí se unen las páginas (y los
// repositorios), se comprueba lo que llega, se resume cada archivo (cuándo
// cambió, tamaños) y se calcula su línea de tiempo.
//
// En la URL solo van el equipo, el repositorio y las fechas: nunca el texto
// que se busca ni nombres de archivo (son datos del cliente).
import { marcarCambiosArchivo, partesRuta, plano, type ArchivoEnVersion } from "./detalle";

/** La operación que tiene que anunciar el agente (en `lista.ops`). */
export const OP_BUSCAR = "buscar_todas";
export const TEXTO_MIN = 2;
export const TEXTO_MAX = 100;
/** Lo que pide la consola (el máximo del agente). */
export const MAX_COINCIDENCIAS = 2000;

/** Una página de `buscar_todas` tal como llega del equipo. */
export interface PaginaBusqueda {
  texto?: string;
  total_archivos?: number;
  coincidencias?: number;
  versiones_buscadas?: number;
  versiones_en_rango?: number;
  recortado?: boolean;
  motivo?: "limite" | "tiempo" | null;
  indice?: number;
  siguiente?: number | null;
  archivos?: { ruta: string; versiones: ArchivoEnVersion[]; recortado?: boolean }[];
}

export type VersionDeArchivo = ArchivoEnVersion & { cambio: boolean };

export interface ArchivoEncontrado {
  /** Repositorio (en «todos los repositorios»: de cuál es). */
  repo: string;
  ruta: string;
  nombre: string;
  carpeta: string;
  /** La más reciente primero, marcando en cuáles cambió respecto a la anterior. */
  versiones: VersionDeArchivo[];
  /** Está en más versiones de las que llegaron. */
  recortado: boolean;
  /** La versión más reciente en la que está. */
  ultima: VersionDeArchivo;
  /** La primera (más antigua) en la que está. */
  primera: VersionDeArchivo;
  /** La última vez que cambió (la versión más reciente distinta de la anterior; `null`: igual desde que apareció). */
  ultimoCambio: VersionDeArchivo | null;
  /** En cuántas versiones cambió (sin contar la primera). */
  cambios: number;
  /** Tamaño en la más reciente y el menor y mayor que tuvo. */
  bytes: number | null;
  bytesMin: number | null;
  bytesMax: number | null;
}

export interface ResultadoRepo {
  repo: string;
  archivos: ArchivoEncontrado[];
  totalArchivos: number;
  coincidencias: number;
  versionesBuscadas: number;
  versionesEnRango: number;
  recortado: boolean;
  motivo: "limite" | "tiempo" | null;
}

const ID = /^[0-9a-f]{8,64}$/;
const RUTA_MAX = 4096;
const num = (x: unknown) => (typeof x === "number" && Number.isFinite(x) && x >= 0 ? x : null);
const fecha = (x: unknown) => typeof x === "string" && x.length <= 40 && !Number.isNaN(Date.parse(x));

/** ¿Vale el texto para buscar? `null` si sí; si no, por qué (como el agente). */
export function errorTexto(texto: string): string | null {
  const t = texto.trim();
  const n = [...t].length;
  if (n < TEXTO_MIN || n > TEXTO_MAX) return `Escribe qué buscar (de ${TEXTO_MIN} a ${TEXTO_MAX} caracteres).`;
  if (/[\u0000-\u001f\u007f/\\]/.test(t)) return "Busca por el nombre del archivo (sin / ni \\).";
  return null;
}

/** Una versión de archivo con la forma esperada (lo manda el equipo, pero se pinta). */
function versionValida(v: ArchivoEnVersion): ArchivoEnVersion | null {
  if (!v || typeof v.version !== "string" || !ID.test(v.version) || !fecha(v.cuando)) return null;
  return { version: v.version, cuando: v.cuando, bytes: num(v.bytes), modificado: fecha(v.modificado) ? v.modificado : null };
}

/** Resume un archivo: sus versiones marcadas, cuándo cambió y sus tamaños. */
export function resumirArchivo(repo: string, ruta: string, versiones: ArchivoEnVersion[], recortado = false): ArchivoEncontrado | null {
  const vs = marcarCambiosArchivo(versiones);
  if (!vs.length) return null;
  const { carpeta, nombre } = partesRuta(ruta);
  const cambiadas = vs.slice(0, -1).filter((x) => x.cambio);
  const tams = vs.map((x) => x.bytes).filter((x): x is number => typeof x === "number");
  return {
    repo,
    ruta,
    nombre,
    carpeta,
    versiones: vs,
    recortado,
    ultima: vs[0],
    primera: vs[vs.length - 1],
    ultimoCambio: cambiadas[0] ?? null,
    cambios: cambiadas.length,
    bytes: vs[0].bytes ?? null,
    bytesMin: tams.length ? Math.min(...tams) : null,
    bytesMax: tams.length ? Math.max(...tams) : null,
  };
}

/**
 * Une las páginas de un repositorio, en orden, sin repetir rutas y
 * descartando lo que no tenga la forma esperada.
 */
export function unirBusqueda(repo: string, paginas: PaginaBusqueda[]): ResultadoRepo {
  const p0 = [...paginas].sort((a, b) => (a.indice ?? 0) - (b.indice ?? 0));
  const vistos = new Set<string>();
  const archivos: ArchivoEncontrado[] = [];
  for (const p of p0)
    for (const a of p.archivos ?? []) {
      if (!a || typeof a.ruta !== "string" || !a.ruta.startsWith("/") || a.ruta.endsWith("/") || a.ruta.length > RUTA_MAX || vistos.has(a.ruta)) continue;
      const vs = (Array.isArray(a.versiones) ? a.versiones : []).map(versionValida).filter((x): x is ArchivoEnVersion => !!x);
      const r = resumirArchivo(repo, a.ruta, vs, !!a.recortado);
      if (!r) continue;
      vistos.add(a.ruta);
      archivos.push(r);
    }
  const c = p0[0];
  return {
    repo,
    archivos,
    totalArchivos: num(c?.total_archivos) ?? archivos.length,
    coincidencias: num(c?.coincidencias) ?? 0,
    versionesBuscadas: num(c?.versiones_buscadas) ?? 0,
    versionesEnRango: num(c?.versiones_en_rango) ?? 0,
    recortado: !!c?.recortado,
    motivo: c?.motivo === "limite" || c?.motivo === "tiempo" ? c.motivo : null,
  };
}

export type OrdenBusqueda = "reciente" | "nombre" | "tamano" | "cambios";

/** Todos los archivos de varios repositorios, ordenados. */
export function ordenarArchivos(xs: ArchivoEncontrado[], orden: OrdenBusqueda = "reciente"): ArchivoEncontrado[] {
  const out = [...xs];
  const t = (a: ArchivoEncontrado) => Date.parse(a.ultima.cuando);
  const porNombre = (a: ArchivoEncontrado, b: ArchivoEncontrado) => a.nombre.localeCompare(b.nombre, "es", { sensitivity: "base" }) || a.ruta.localeCompare(b.ruta) || a.repo.localeCompare(b.repo);
  if (orden === "nombre") out.sort(porNombre);
  else if (orden === "tamano") out.sort((a, b) => (b.bytes ?? -1) - (a.bytes ?? -1) || porNombre(a, b));
  else if (orden === "cambios") out.sort((a, b) => b.cambios - a.cambios || t(b) - t(a) || porNombre(a, b));
  else out.sort((a, b) => t(b) - t(a) || porNombre(a, b));
  return out;
}

/** Filtra lo ya encontrado (sin acentos ni mayúsculas) por nombre o carpeta. */
export function filtrarEncontrados(xs: ArchivoEncontrado[], texto: string): ArchivoEncontrado[] {
  const q = plano(texto.trim());
  return q ? xs.filter((a) => plano(a.ruta).includes(q)) : xs;
}

/** Dónde está el texto en el nombre (sin distinguir mayúsculas), para resaltarlo. */
export function trozosNombre(nombre: string, texto: string): { texto: string; marca: boolean }[] {
  const q = texto.trim().toLowerCase();
  const i = q ? nombre.toLowerCase().indexOf(q) : -1;
  // Si pasar a minúsculas cambia el largo (raro), sin resaltar.
  if (i < 0 || nombre.toLowerCase().length !== nombre.length) return [{ texto: nombre, marca: false }];
  return [
    { texto: nombre.slice(0, i), marca: false },
    { texto: nombre.slice(i, i + q.length), marca: true },
    { texto: nombre.slice(i + q.length), marca: false },
  ].filter((x) => x.texto);
}

/** ¿Ya no está en la versión más reciente del repositorio (dentro del rango)? */
export function borradoDespues(a: ArchivoEncontrado, versionesRepo: { id: string; cuando: string }[]): boolean {
  const ultima = [...versionesRepo].sort((x, y) => Date.parse(y.cuando) - Date.parse(x.cuando))[0];
  return !!ultima && Date.parse(ultima.cuando) > Date.parse(a.ultima.cuando) && !mismoId(ultima.id, a.ultima.version);
}

export const mismoId = (a: string, b: string) => a.length >= 8 && b.length >= 8 && (a.startsWith(b) || b.startsWith(a));

// ---------------------------------------------------------------------------
// Línea de tiempo de un archivo
// ---------------------------------------------------------------------------

export type MarcaLinea = { x: number; version: string; cuando: string; estado: "falta" | "igual" | "cambio" | "aparece" };

/**
 * Las marcas de la línea de tiempo de un archivo: una por versión del
 * repositorio (en el rango), con `x` de 0 a 1 según su fecha, y si el archivo
 * está en ella (igual, distinto de la anterior o la primera vez) o no.
 * Sin las versiones del repositorio, solo las del archivo.
 */
export function lineaArchivo(a: ArchivoEncontrado, versionesRepo: { id: string; cuando: string }[]): MarcaLinea[] {
  const delArchivo = new Map(a.versiones.map((v) => [v.version, v]));
  const buscar = (id: string) => delArchivo.get(id) ?? a.versiones.find((v) => mismoId(v.version, id));
  const base = versionesRepo.length ? versionesRepo.map((v) => ({ version: v.id, cuando: v.cuando })) : a.versiones.map((v) => ({ version: v.version, cuando: v.cuando }));
  const todas = base.filter((v) => fecha(v.cuando)).sort((x, y) => Date.parse(x.cuando) - Date.parse(y.cuando));
  if (!todas.length) return [];
  const t0 = Date.parse(todas[0].cuando);
  const t1 = Date.parse(todas[todas.length - 1].cuando);
  const ancho = t1 - t0;
  return todas.map((v) => {
    const f = buscar(v.version);
    const estado: MarcaLinea["estado"] = !f ? "falta" : f === a.primera ? "aparece" : f.cambio ? "cambio" : "igual";
    return { x: ancho > 0 ? (Date.parse(v.cuando) - t0) / ancho : 0.5, version: v.version, cuando: v.cuando, estado };
  });
}

// ---------------------------------------------------------------------------
// Qué va en la URL: equipo, repositorio («todos») y fechas (AAAA-MM-DD)
// ---------------------------------------------------------------------------

export interface FiltrosBusqueda {
  equipo: string | null;
  /** Un repositorio o `"todos"` (los del equipo). */
  repo: string | null;
  desde: string | null;
  hasta: string | null;
}

const DIA = /^\d{4}-\d{2}-\d{2}$/;
const ID_OBJETO = /^[A-Za-z0-9._-]{1,100}$/;

export function leerFiltros(q: URLSearchParams): FiltrosBusqueda {
  const id = (k: string) => {
    const x = q.get(k);
    return x && ID_OBJETO.test(x) ? x : null;
  };
  const dia = (k: string) => {
    const x = q.get(k);
    return x && DIA.test(x) && !Number.isNaN(Date.parse(`${x}T00:00:00`)) ? x : null;
  };
  let [desde, hasta] = [dia("desde"), dia("hasta")];
  if (desde && hasta && desde > hasta) [desde, hasta] = [hasta, desde];
  return { equipo: id("equipo"), repo: id("repo"), desde, hasta };
}

/** La URL (solo `?…`) con estos filtros, sin tocar los demás parámetros. */
export function conFiltros(q: URLSearchParams, f: Partial<FiltrosBusqueda>): string {
  const n = new URLSearchParams(q);
  for (const k of ["equipo", "repo", "desde", "hasta"] as const) {
    if (!(k in f)) continue;
    const x = f[k];
    if (x == null || x === "") n.delete(k);
    else n.set(k, x);
  }
  const t = n.toString();
  return t ? `?${t}` : "?";
}

/**
 * El rango que se manda al equipo (RFC 3339 con el huso de este navegador):
 * de las 00:00 de `desde` a las 23:59:59.999 de `hasta`, en hora local.
 */
export function rangoIso(desde: string | null, hasta: string | null): { desde?: string; hasta?: string } {
  const local = (d: string, fin: boolean) => {
    const [a, m, dd] = d.split("-").map(Number);
    const x = fin ? new Date(a, m - 1, dd, 23, 59, 59, 999) : new Date(a, m - 1, dd, 0, 0, 0, 0);
    return x.toISOString();
  };
  return { ...(desde ? { desde: local(desde, false) } : {}), ...(hasta ? { hasta: local(hasta, true) } : {}) };
}

/** AAAA-MM-DD de hoy menos `dias` (hora local). */
export function diaDeHace(dias: number, ahora = Date.now()): string {
  const d = new Date(ahora - dias * 86_400_000);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

/** Por qué el resultado no está completo, en una frase (o `null`). */
export function fraseRecorte(r: Pick<ResultadoRepo, "recortado" | "motivo" | "versionesBuscadas" | "versionesEnRango">): string | null {
  const partes: string[] = [];
  if (r.motivo === "limite") partes.push(`Hay muchísimas coincidencias: aquí están las ${MAX_COINCIDENCIAS.toLocaleString("es")} más recientes. Escribe un nombre más concreto o acota las fechas.`);
  else if (r.motivo === "tiempo") partes.push("La búsqueda tardaba demasiado: aquí está lo encontrado en 5 minutos (lo más reciente). Acota las fechas para buscar en menos versiones.");
  if (r.versionesEnRango > r.versionesBuscadas) partes.push(`Se buscó en las ${r.versionesBuscadas.toLocaleString("es")} versiones más recientes de ${r.versionesEnRango.toLocaleString("es")}.`);
  if (!partes.length && r.recortado) partes.push("Algún archivo está en más versiones de las que se enseñan: «Ver todas sus versiones» las trae todas.");
  return partes.length ? partes.join(" ") : null;
}
