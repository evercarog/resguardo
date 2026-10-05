// «Pulsar para ver más»: la lógica (sin interfaz) de los detalles de una
// versión, de «Qué cambió» entre dos versiones y de qué abre cada cifra.
//
// Lo que sale del informe (versiones, vueltas, recuentos) no lleva nombres de
// archivo. Los nombres llegan solo por la sesión cifrada `explorar`
// (api-servidor.md §7, `diferencias`, `ocupa`, `historial_archivo`): nunca
// pasan por el servidor en claro ni se ponen en la URL.
import type { EjecucionInforme, EntradaHistorial, ResultadoGancho, VersionInforme } from "./tipos";
import { versionDeVuelta } from "./repo";

// ---------------------------------------------------------------------------
// Qué cambió (respuesta de `diferencias`)
// ---------------------------------------------------------------------------

export type TipoCambio = "nuevo" | "cambiado" | "borrado" | "metadatos" | "otro";

export interface Cambio {
  /** Ruta dentro de la versión, como la guarda restic (`/C/Users/Ana/x.txt`). */
  ruta: string;
  tipo: TipoCambio;
  /** Tamaño en la versión nueva y en la anterior (si se midió). */
  bytes?: number;
  bytes_antes?: number;
}

export interface ResumenCambios {
  nuevos: number;
  cambiados: number;
  borrados: number;
  metadatos: number;
  otros: number;
  carpetas_nuevas: number;
  carpetas_borradas: number;
  bytes_anadidos: number | null;
  bytes_quitados: number | null;
}

/** Una página de `diferencias` tal como llega del equipo. */
export interface PaginaCambios {
  desde: string | null;
  hasta: string;
  desde_cuando?: string | null;
  /** Primera versión de su copia: no hay con qué compararla. */
  primera?: boolean;
  resumen?: ResumenCambios;
  total?: number;
  recortado?: boolean;
  con_tamanos?: boolean;
  indice?: number;
  siguiente?: number | null;
  cambios?: Cambio[];
}

/** Todas las páginas juntas. */
export interface Diferencias {
  desde: string | null;
  hasta: string;
  desdeCuando: string | null;
  primera: boolean;
  resumen: ResumenCambios;
  cambios: Cambio[];
  recortado: boolean;
  conTamanos: boolean;
}

const TIPOS: TipoCambio[] = ["nuevo", "cambiado", "borrado", "metadatos", "otro"];
const RUTA_MAX = 4096;

/**
 * Une las páginas en orden y descarta lo que no tenga la forma esperada (lo
 * manda el equipo, pero se pinta: nada de rutas raras ni tamaños negativos).
 */
export function unirPaginas(paginas: PaginaCambios[]): Diferencias {
  const p0 = paginas[0];
  const cambios: Cambio[] = [];
  const vistos = new Set<string>();
  for (const p of [...paginas].sort((a, b) => (a.indice ?? 0) - (b.indice ?? 0)))
    for (const c of p.cambios ?? []) {
      if (!c || typeof c.ruta !== "string" || !c.ruta.startsWith("/") || c.ruta.length > RUTA_MAX || !TIPOS.includes(c.tipo)) continue;
      if (vistos.has(c.ruta)) continue;
      vistos.add(c.ruta);
      const n = (x: unknown) => (typeof x === "number" && Number.isFinite(x) && x >= 0 ? x : undefined);
      cambios.push({ ruta: c.ruta, tipo: c.tipo, bytes: n(c.bytes), bytes_antes: n(c.bytes_antes) });
    }
  const r = p0?.resumen;
  const num = (x: unknown) => (typeof x === "number" && x >= 0 ? x : 0);
  return {
    desde: p0?.desde ?? null,
    hasta: p0?.hasta ?? "",
    desdeCuando: p0?.desde_cuando ?? null,
    primera: !!p0?.primera,
    resumen: {
      nuevos: num(r?.nuevos),
      cambiados: num(r?.cambiados),
      borrados: num(r?.borrados),
      metadatos: num(r?.metadatos),
      otros: num(r?.otros),
      carpetas_nuevas: num(r?.carpetas_nuevas),
      carpetas_borradas: num(r?.carpetas_borradas),
      bytes_anadidos: typeof r?.bytes_anadidos === "number" ? r.bytes_anadidos : null,
      bytes_quitados: typeof r?.bytes_quitados === "number" ? r.bytes_quitados : null,
    },
    cambios,
    recortado: !!p0?.recortado,
    conTamanos: !!p0?.con_tamanos,
  };
}

/** Cuánto cambió de tamaño un archivo (nuevo: lo que ocupa; borrado: en negativo). */
export function deltaDe(c: Cambio): number | null {
  if (c.tipo === "nuevo") return c.bytes ?? null;
  if (c.tipo === "borrado") return c.bytes_antes != null ? -c.bytes_antes : null;
  if (c.bytes != null && c.bytes_antes != null) return c.bytes - c.bytes_antes;
  return null;
}

/** El tamaño que importa de un cambio: el de ahora o, si se borró, el de antes. */
export const tamanoDe = (c: Cambio): number => c.bytes ?? c.bytes_antes ?? 0;

export type FiltroCambios = "todos" | "nuevos" | "cambiados" | "borrados";
export const FILTROS: FiltroCambios[] = ["todos", "nuevos", "cambiados", "borrados"];
export const esFiltro = (x: string | null | undefined): x is FiltroCambios => !!x && (FILTROS as string[]).includes(x);

/** «cambiados» incluye solo metadatos y cambios de tipo (también se tocaron). */
export function pasaFiltro(c: Cambio, f: FiltroCambios): boolean {
  if (f === "todos") return true;
  if (f === "nuevos") return c.tipo === "nuevo";
  if (f === "borrados") return c.tipo === "borrado";
  return c.tipo === "cambiado" || c.tipo === "metadatos" || c.tipo === "otro";
}

export function cuentaFiltro(r: ResumenCambios, f: FiltroCambios): number {
  if (f === "nuevos") return r.nuevos;
  if (f === "borrados") return r.borrados;
  if (f === "cambiados") return r.cambiados + r.metadatos + r.otros;
  return r.nuevos + r.borrados + r.cambiados + r.metadatos + r.otros;
}

/** Sin acentos ni mayúsculas, para buscar. */
export const plano = (s: string) =>
  s
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();

export type OrdenCambios = "ruta" | "tamano" | "delta";

export function filtrarCambios(cambios: Cambio[], filtro: FiltroCambios, texto: string, orden: OrdenCambios = "ruta"): Cambio[] {
  const q = plano(texto.trim());
  const out = cambios.filter((c) => pasaFiltro(c, filtro) && (!q || plano(c.ruta).includes(q)));
  if (orden === "tamano") out.sort((a, b) => tamanoDe(b) - tamanoDe(a) || a.ruta.localeCompare(b.ruta));
  else if (orden === "delta") out.sort((a, b) => Math.abs(deltaDe(b) ?? 0) - Math.abs(deltaDe(a) ?? 0) || a.ruta.localeCompare(b.ruta));
  else out.sort((a, b) => a.ruta.localeCompare(b.ruta));
  return out;
}

/** Carpeta y nombre de una ruta de versión. */
export function partesRuta(ruta: string): { carpeta: string; nombre: string } {
  const i = ruta.lastIndexOf("/");
  return { carpeta: i <= 0 ? "/" : ruta.slice(0, i), nombre: ruta.slice(i + 1) };
}

/**
 * Una ruta de versión como la ve quien usa el equipo: `/C/Users/Ana` →
 * `C:\Users\Ana` (Windows); en Linux, igual.
 */
export function rutaLegible(ruta: string): string {
  const m = /^\/([A-Za-z])(\/.*)?$/.exec(ruta);
  if (m) return `${m[1].toUpperCase()}:${(m[2] ?? "\\").replaceAll("/", "\\")}`;
  return ruta;
}

export interface GrupoCarpeta {
  carpeta: string;
  cambios: Cambio[];
  /** Suma de los tamaños y de las diferencias de tamaño. */
  bytes: number;
  delta: number;
  nuevos: number;
  cambiados: number;
  borrados: number;
}

/**
 * Agrupa por carpeta (la del archivo), en el orden en que llegan los cambios
 * (ya ordenados). Con `orden` por tamaño, las carpetas también van por tamaño.
 */
export function agruparPorCarpeta(cambios: Cambio[], orden: OrdenCambios = "ruta"): GrupoCarpeta[] {
  const m = new Map<string, GrupoCarpeta>();
  for (const c of cambios) {
    const { carpeta } = partesRuta(c.ruta);
    let g = m.get(carpeta);
    if (!g) m.set(carpeta, (g = { carpeta, cambios: [], bytes: 0, delta: 0, nuevos: 0, cambiados: 0, borrados: 0 }));
    g.cambios.push(c);
    g.bytes += tamanoDe(c);
    g.delta += deltaDe(c) ?? 0;
    if (c.tipo === "nuevo") g.nuevos++;
    else if (c.tipo === "borrado") g.borrados++;
    else g.cambiados++;
  }
  const out = [...m.values()];
  if (orden === "tamano") out.sort((a, b) => b.bytes - a.bytes || a.carpeta.localeCompare(b.carpeta));
  else if (orden === "delta") out.sort((a, b) => Math.abs(b.delta) - Math.abs(a.delta) || a.carpeta.localeCompare(b.carpeta));
  else out.sort((a, b) => a.carpeta.localeCompare(b.carpeta));
  return out;
}

/** La carpeta común a todas las rutas (para no repetirla en cada grupo). */
export function prefijoComun(rutas: string[]): string {
  if (!rutas.length) return "/";
  let partes = partesRuta(rutas[0]).carpeta.split("/");
  for (const r of rutas.slice(1)) {
    const otras = partesRuta(r).carpeta.split("/");
    let i = 0;
    while (i < partes.length && i < otras.length && partes[i] === otras[i]) i++;
    partes = partes.slice(0, i);
    if (partes.length <= 1) return "/";
  }
  return partes.join("/") || "/";
}

// ---------------------------------------------------------------------------
// Versiones y vueltas (del informe)
// ---------------------------------------------------------------------------

/** La versión anterior de la misma copia en el informe (o `null`: la primera de estos 60 días). */
export function anteriorDeLaCopia(versiones: VersionInforme[], v: VersionInforme): VersionInforme | null {
  const t = Date.parse(v.hora);
  let mejor: VersionInforme | null = null;
  for (const x of versiones) {
    if (x.id === v.id || x.copia !== v.copia) continue;
    const tx = Date.parse(x.hora);
    if (tx < t && (!mejor || tx > Date.parse(mejor.hora))) mejor = x;
  }
  return mejor;
}

/** La versión con ese id (o que empieza por él). */
export const versionPorId = (versiones: VersionInforme[], id: string | null | undefined): VersionInforme | null =>
  id ? (versiones.find((v) => v.id === id) ?? versiones.find((v) => v.id.startsWith(id) || id.startsWith(v.id)) ?? null) : null;

/**
 * Las vueltas que acabaron en esta versión: la que la guardó y los intentos
 * fallidos de la misma copia justo antes (reintentos), más antiguos al final.
 */
export function vueltasDeVersion(ejecuciones: EjecucionInforme[], versiones: VersionInforme[], v: VersionInforme): EjecucionInforme[] {
  const propia = ejecuciones.find((e) => versionDeVuelta(versiones, e)?.id === v.id);
  if (!propia) return [];
  const out = [propia];
  // Hacia atrás: los fallos de la misma copia desde la versión anterior (como mucho 12 h).
  const ant = anteriorDeLaCopia(versiones, v);
  const desde = Math.max(ant ? Date.parse(ant.hora) : 0, Date.parse(propia.hora) - 12 * 3600_000);
  for (const e of ejecuciones) {
    if (e === propia || e.copia !== propia.copia) continue;
    const t = Date.parse(e.hora);
    if (t < Date.parse(propia.hora) && t > desde && (e.resultado === "fallo" || e.resultado === "aviso")) out.push(e);
  }
  return out.sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora));
}

/** La vuelta con esa hora exacta (ISO) o la más cercana en un minuto. */
export function vueltaPorHora(ejecuciones: EjecucionInforme[], hora: string | null | undefined): EjecucionInforme | null {
  if (!hora) return null;
  const t = Date.parse(hora);
  if (Number.isNaN(t)) return null;
  let mejor: EjecucionInforme | null = null;
  for (const e of ejecuciones) {
    const d = Math.abs(Date.parse(e.hora) - t);
    if (d <= 60_000 && (!mejor || d < Math.abs(Date.parse(mejor.hora) - t))) mejor = e;
  }
  return mejor;
}

/**
 * Los pasos «Antes de copiar» de esa vuelta, del historial del equipo (la
 * entrada de la copia a esa hora), si los guardó.
 */
export function ganchosDeVuelta(historial: EntradaHistorial[], e: EjecucionInforme | null, repo: string): ResultadoGancho[] {
  if (!e) return [];
  const t = Date.parse(e.hora);
  const h = historial.find((x) => x.tipo === "copia" && x.repo === repo && (x.copia ?? null) === (e.copia ?? null) && Math.abs(Date.parse(x.hora) - t) <= 60_000);
  return (h?.ganchos ?? []) as ResultadoGancho[];
}

/** Las vueltas con problemas, más recientes primero (para «¿Por qué?»). */
export const problemasRecientes = (ejecuciones: EjecucionInforme[], cuantos = 10): EjecucionInforme[] =>
  ejecuciones
    .filter((e) => e.resultado === "fallo" || e.resultado === "aviso")
    .sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora))
    .slice(0, cuantos);

// ---------------------------------------------------------------------------
// Qué se ve: parámetros de la URL (sin nombres de archivo)
// ---------------------------------------------------------------------------

export type Vista = "version" | "cambios" | "ocupa" | "espacio" | "vuelta" | "estado";

/**
 * Lo que abre el panel de detalle. En la URL solo van ids de versión, una hora,
 * el filtro y el día (nada que diga qué archivos hay): `v`, `vista`, `con`
 * (comparar con otra versión), `filtro`, `vuelta` y `dia`.
 */
export interface Seleccion {
  vista: Vista | null;
  version: string | null;
  con: string | null;
  filtro: FiltroCambios;
  vuelta: string | null;
  dia: string | null;
  /** Un intervalo de días («Historial y versiones»: `?desde=…&hasta=…`), si no hay un día. */
  desde: string | null;
  hasta: string | null;
}

const ID = /^[0-9a-f]{8,64}$/;
const DIA = /^\d{4}-\d{2}-\d{2}$/;
const VISTAS: Vista[] = ["version", "cambios", "ocupa", "espacio", "vuelta", "estado"];

/** Lee la selección de la URL; lo que no tenga la forma esperada se ignora. */
export function leerSeleccion(q: URLSearchParams): Seleccion {
  const id = (k: string) => {
    const x = q.get(k)?.toLowerCase() ?? null;
    return x && ID.test(x) ? x : null;
  };
  const version = id("v");
  const vuelta = q.get("vuelta");
  const vueltaOk = vuelta && vuelta.length <= 40 && !Number.isNaN(Date.parse(vuelta)) ? vuelta : null;
  const v = q.get("vista") as Vista | null;
  let vista: Vista | null = v && VISTAS.includes(v) ? v : null;
  if (!vista && version) vista = "version";
  if (!vista && vueltaOk) vista = "vuelta";
  // Las que necesitan una versión, sin ella, no se abren.
  if ((vista === "version" || vista === "cambios" || vista === "ocupa") && !version) vista = null;
  if (vista === "vuelta" && !vueltaOk) vista = null;
  const f = q.get("filtro");
  const dia = q.get("dia");
  const diaOk = dia && DIA.test(dia) ? dia : null;
  // El intervalo, solo entero y en orden (y sin día: el día manda).
  let desde = q.get("desde");
  let hasta = q.get("hasta");
  if (!desde || !DIA.test(desde) || !hasta || !DIA.test(hasta) || diaOk) desde = hasta = null;
  else if (desde > hasta) [desde, hasta] = [hasta, desde];
  return { vista, version, con: id("con"), filtro: esFiltro(f) ? f : "todos", vuelta: vueltaOk, dia: diaOk, desde, hasta };
}

const CLAVES = ["v", "vista", "con", "filtro", "vuelta"] as const;

/**
 * La URL (solo `?…`) con otra selección, sin tocar los demás parámetros de
 * la página. `null` quita. El día se cambia aparte (filtra las listas).
 */
export function conSeleccion(q: URLSearchParams, s: Partial<Seleccion> & { cerrar?: boolean }): string {
  const n = new URLSearchParams(q);
  if (s.cerrar) for (const k of CLAVES) n.delete(k);
  const poner = (k: string, x: string | null | undefined) => (x == null || x === "" ? n.delete(k) : n.set(k, x));
  if ("version" in s) poner("v", s.version);
  if ("vista" in s) poner("vista", s.vista === "version" ? null : s.vista);
  if ("con" in s) poner("con", s.con);
  if ("filtro" in s) poner("filtro", s.filtro === "todos" ? null : s.filtro);
  if ("vuelta" in s) poner("vuelta", s.vuelta);
  if ("dia" in s) poner("dia", s.dia);
  if ("desde" in s) poner("desde", s.desde);
  if ("hasta" in s) poner("hasta", s.hasta);
  const t = n.toString();
  return t ? `?${t}` : "?";
}

/** Migas del panel: de lo general a lo concreto. */
export function migasDe(s: Seleccion, nombreVersion: (id: string) => string): { texto: string; sel: Partial<Seleccion> }[] {
  const out: { texto: string; sel: Partial<Seleccion> }[] = [];
  if (s.version && (s.vista === "version" || s.vista === "cambios" || s.vista === "ocupa")) {
    out.push({ texto: nombreVersion(s.version), sel: { vista: "version", version: s.version, con: null, filtro: "todos", vuelta: null } });
    if (s.vista === "cambios")
      out.push({ texto: s.con ? `Comparada con ${nombreVersion(s.con)}` : "Qué cambió", sel: { vista: "cambios", version: s.version, con: s.con, filtro: "todos" } });
    if (s.vista === "cambios" && s.filtro !== "todos") out.push({ texto: TEXTO_FILTRO[s.filtro], sel: { vista: "cambios", version: s.version, con: s.con, filtro: s.filtro } });
    if (s.vista === "ocupa") out.push({ texto: "Lo que más ocupa", sel: { vista: "ocupa", version: s.version } });
  } else if (s.vista === "espacio") out.push({ texto: "Espacio", sel: { vista: "espacio", version: null } });
  else if (s.vista === "vuelta") out.push({ texto: "Detalle de la copia", sel: { vista: "vuelta", vuelta: s.vuelta } });
  else if (s.vista === "estado") out.push({ texto: "¿Por qué este estado?", sel: { vista: "estado" } });
  return out;
}

export const TEXTO_FILTRO: Record<FiltroCambios, string> = { todos: "Todos", nuevos: "Nuevos", cambiados: "Cambiados", borrados: "Borrados" };
export const TEXTO_TIPO: Record<TipoCambio, string> = { nuevo: "Nuevo", cambiado: "Cambiado", borrado: "Borrado", metadatos: "Solo fechas o permisos", otro: "Cambió de tipo" };

// ---------------------------------------------------------------------------
// Avisos: a qué vuelta o repositorio llevan
// ---------------------------------------------------------------------------

/**
 * Lo que explica un aviso de un equipo, en su último informe: una copia
 * fallida lleva a esa vuelta (la que falló más cerca, antes del aviso o justo
 * después, en 24 h); una verificación o prueba fallida, al repositorio.
 * `null`: no se encuentra (se queda el enlace al equipo).
 */
export function destinoAviso(
  aviso: { tipo: string; equipo: string | null; creado: string },
  repos: { id: string; ejecuciones: EjecucionInforme[]; verificacion?: { resultado: string; ultima: string | null } | null; prueba_restauracion?: { resultado: string; ultima: string | null } | null; externa?: { resultado: string; ultima: string | null } | null }[],
): { repo: string; vuelta: string | null } | null {
  const t = Date.parse(aviso.creado);
  if (!aviso.equipo || Number.isNaN(t)) return null;
  if (aviso.tipo === "copia_fallida" || aviso.tipo === "copia_atrasada") {
    let mejor: { repo: string; e: EjecucionInforme; d: number } | null = null;
    for (const r of repos)
      for (const e of r.ejecuciones) {
        if (e.resultado !== "fallo" && e.resultado !== "aviso") continue;
        const d = t - Date.parse(e.hora);
        if (d < -10 * 60_000 || d > 24 * 3600_000) continue;
        if (!mejor || Math.abs(d) < Math.abs(mejor.d)) mejor = { repo: r.id, e, d };
      }
    return mejor ? { repo: mejor.repo, vuelta: mejor.e.hora } : null;
  }
  const campo = aviso.tipo === "verificacion_fallida" ? "verificacion" : aviso.tipo === "prueba_fallida" ? "prueba_restauracion" : aviso.tipo === "externa_fallida" ? "externa" : null;
  if (!campo) return null;
  const r = repos.find((x) => x[campo]?.resultado === "fallo") ?? null;
  return r ? { repo: r.id, vuelta: null } : null;
}

// ---------------------------------------------------------------------------
// Versiones de un archivo (`historial_archivo`)
// ---------------------------------------------------------------------------

export interface ArchivoEnVersion {
  version: string;
  cuando: string;
  bytes?: number | null;
  modificado?: string | null;
}

/**
 * Las versiones de un archivo, la más reciente primero, marcando en cuáles
 * cambió respecto a la anterior (otro tamaño u otra fecha de modificación).
 */
export function marcarCambiosArchivo(xs: ArchivoEnVersion[]): (ArchivoEnVersion & { cambio: boolean })[] {
  const orden = [...xs].sort((a, b) => Date.parse(b.cuando) - Date.parse(a.cuando));
  return orden.map((x, i) => {
    const antes = orden[i + 1];
    return { ...x, cambio: !antes || antes.bytes !== x.bytes || (antes.modificado ?? null) !== (x.modificado ?? null) };
  });
}
