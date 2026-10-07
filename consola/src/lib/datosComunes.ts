// 0.7.26 (bloque 8): los datos comunes del cliente, iguales en todas sus consolas
// (docs/consolas-multiples.md §6.5). Sin runas: lo prueban los vectores
// (scripts/vectores-datos-comunes.ts, con los de Rust en
// crates/protocolo/vectors/datos-cliente.json).
//
// Los colores y la plantilla de las etiquetas, el catálogo de destinos (nombre,
// tipo y marcas) y las plantillas de copia viajan por los equipos del cliente:
// cada consola manda sus cambios a todos con la orden `datos_cliente` (o
// `datos_cliente_admin`, con la clave, para el tipo y las marcas de un destino) y
// las demás los juntan dato a dato (gana el cambio más reciente). La primera vez,
// si dos consolas tienen valores distintos para lo mismo, no se pisa nada: se
// enseña la diferencia y se elige cuál vale para todas.
import type { AtributosDestino, Equipo } from "./tipos";
import { NOMBRES_COLOR } from "./etiquetasGrupos";

export const ADMITE_DATOS_CLIENTE = "datos_cliente";
/** Como mucho, datos en una orden (como el agente). */
export const MAX_ENTRADAS_ORDEN = 100;
/** Lo que cabe de datos en una orden (el sobre no pasa de 64 KiB, en base64). */
export const MAX_BYTES_ORDEN = 40_000;

/** ¿Guarda el equipo los datos comunes del cliente? */
export const admiteDatosCliente = (e: Pick<Equipo, "resumen"> | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_DATOS_CLIENTE);

/** Los equipos a los que se mandan: confirmados, gestionados desde aquí y con un agente que los guarda. */
export const equiposQueGuardan = (equipos: Equipo[]) => equipos.filter((e) => e.confirmado && e.modo === "gestionado" && admiteDatosCliente(e));

export type ClaseDato = "etiqueta.color" | "etiqueta.plantilla" | "destino.regla" | "destino" | "plantilla";
const CLASES: ClaseDato[] = ["etiqueta.color", "etiqueta.plantilla", "destino.regla", "destino", "plantilla"];

/** Una etiqueta limpia como las de los equipos (o null). */
export function normalizarEtiqueta(x: string): string | null {
  const t = x.split(/\s+/).filter(Boolean).join(" ");
  // eslint-disable-next-line no-control-regex
  return t && [...t].length <= 32 && !/[\u0000-\u001f\u007f-\u009f,]/.test(t) ? t : null;
}
const idDestinoValido = (id: string) => id.length > 0 && id.length <= 120 && /^[a-z0-9:_.-]+$/.test(id) && !id.split(":").some((p) => p === "" || p === "." || p === "..");
const idSimple = (id: string) => /^[A-Za-z0-9_-]{1,64}$/.test(id);

/** La clase de un dato y su sujeto (la etiqueta, el id del destino o de la plantilla), o null. */
export function claseDe(clave: string): { clase: ClaseDato; sujeto: string } | null {
  const clase = CLASES.find((c) => clave.startsWith(`${c}:`));
  if (!clase) return null;
  const sujeto = clave.slice(clase.length + 1);
  const ok =
    clase === "etiqueta.color" || clase === "etiqueta.plantilla"
      ? normalizarEtiqueta(sujeto)?.toLowerCase() === sujeto
      : clase === "plantilla"
        ? idSimple(sujeto)
        : idDestinoValido(sujeto);
  return ok ? { clase, sujeto } : null;
}

/** ¿Cambiarlo pide la clave de administración? (El tipo y las marcas de un destino: cuentan en la regla 3-2-1.) */
export const pideClave = (clave: string) => clave.startsWith("destino.regla:");

export const claveColor = (etiqueta: string) => {
  const n = normalizarEtiqueta(etiqueta);
  return n ? `etiqueta.color:${n.toLowerCase()}` : null;
};

/** Un dato tal como lo guarda un equipo. */
export interface EntradaComun {
  valor: unknown;
  cambiado: string;
  consola?: string;
  identidad?: string;
  por?: string | null;
  semilla?: boolean;
}

/** Lo que va en la orden `datos_cliente` por cada dato. */
export interface EntradaOrden {
  clave: string;
  valor: unknown;
  cambiado: string;
  semilla?: boolean;
  por?: string | null;
}

export type EstadoComun = "aplicado" | "conflicto" | "por_traer";

/** Un dato común en este servidor (`GET /api/clientes/{c}/datos-comunes`). */
export interface FilaComun {
  clave: string;
  valor: unknown;
  cambiado: string;
  /** El nombre que la otra consola tiene en el equipo (null: esta, o sin nombre). */
  consola: string | null;
  /** Lo cambió esta consola. */
  esta: boolean;
  por: string | null;
  semilla: boolean;
  estado: EstadoComun;
  /** En una diferencia: lo que había aquí. */
  local: unknown;
  /** Cambiado aquí y aún sin mandar a los equipos. */
  por_enviar: boolean;
}

export interface DatosComunesCliente {
  filas: FilaComun[];
  /** Lo de aquí de antes de compartir (para mandarlo como semilla). */
  sin_compartir: { clave: string; valor: unknown }[];
}

// ---------------------------------------------------------------------------
// Orden entre dos cambios (como crates/protocolo/src/datos_cliente.rs)
// ---------------------------------------------------------------------------

/** La hora de un cambio en milisegundos (NaN si no es una fecha). */
export const milis = (cambiado: string) => (/^\d{4}-\d{2}-\d{2}T/.test(cambiado) ? Date.parse(cambiado) : NaN);

/** -1, 0 o 1: un cambio de verdad gana a una semilla; después, el más reciente; a la misma hora, la identidad mayor. */
export function comparar(a: EntradaComun, b: EntradaComun): number {
  const ka = [a.semilla ? 0 : 1, Number.isFinite(milis(a.cambiado)) ? milis(a.cambiado) : -Infinity] as const;
  const kb = [b.semilla ? 0 : 1, Number.isFinite(milis(b.cambiado)) ? milis(b.cambiado) : -Infinity] as const;
  if (ka[0] !== kb[0]) return ka[0] < kb[0] ? -1 : 1;
  if (ka[1] !== kb[1]) return ka[1] < kb[1] ? -1 : 1;
  const ia = a.identidad ?? "";
  const ib = b.identidad ?? "";
  return ia === ib ? 0 : ia < ib ? -1 : 1;
}

export const gana = (a: EntradaComun, b: EntradaComun) => comparar(a, b) > 0;

/** Pone `e` en el documento si gana a lo que hubiera. Devuelve si cambió. */
export function fusionar(doc: Record<string, EntradaComun>, clave: string, e: EntradaComun): boolean {
  const x = doc[clave];
  if (x && !gana(e, x)) return false;
  doc[clave] = e;
  return true;
}

/** Junta los documentos de varios equipos (cada uno puede ir por una versión distinta). */
export function juntar(docs: Record<string, EntradaComun>[]): Record<string, EntradaComun> {
  const out: Record<string, EntradaComun> = {};
  for (const d of docs) for (const [k, e] of Object.entries(d)) if (claseDe(k)) fusionar(out, k, e);
  return out;
}

// ---------------------------------------------------------------------------
// Qué hay que hacer
// ---------------------------------------------------------------------------

export interface Pendiente {
  /** Diferencias entre consolas: hay que elegir. */
  diferencias: FilaComun[];
  /** Por mandar a los equipos sin clave (colores, nombres, plantillas). */
  porEnviar: EntradaOrden[];
  /** Por mandar con la clave de administración (tipo y marcas de destinos). */
  porEnviarConClave: EntradaOrden[];
  /** Lo de aquí de antes de compartir, sin clave y con clave. */
  sinCompartir: { clave: string; valor: unknown }[];
  sinCompartirConClave: { clave: string; valor: unknown }[];
  /** Plantillas de otra consola: hay que abrirlas con la clave y guardarlas aquí. */
  porTraer: FilaComun[];
}

export const entradaDeFila = (f: FilaComun): EntradaOrden => ({ clave: f.clave, valor: f.valor, cambiado: f.cambiado, ...(f.semilla ? { semilla: true } : {}), ...(f.por ? { por: f.por } : {}) });

export function pendiente(d: DatosComunesCliente | null | undefined): Pendiente {
  const filas = d?.filas ?? [];
  const enviar = filas.filter((f) => f.por_enviar && f.estado !== "conflicto").map(entradaDeFila);
  const sin = (d?.sin_compartir ?? []).filter((x) => claseDe(x.clave));
  return {
    diferencias: filas.filter((f) => f.estado === "conflicto"),
    porEnviar: enviar.filter((e) => !pideClave(e.clave)),
    porEnviarConClave: enviar.filter((e) => pideClave(e.clave)),
    sinCompartir: sin.filter((x) => !pideClave(x.clave)),
    sinCompartirConClave: sin.filter((x) => pideClave(x.clave)),
    porTraer: filas.filter((f) => f.estado === "por_traer"),
  };
}

/** La línea del aviso (o null si no hay nada que hacer a mano). */
export function textoAviso(p: Pendiente): string | null {
  const partes: string[] = [];
  const n = p.diferencias.length;
  if (n) partes.push(n === 1 ? "1 dato distinto entre consolas" : `${n} datos distintos entre consolas`);
  const k = p.porEnviarConClave.length + p.sinCompartirConClave.length;
  if (k) partes.push(k === 1 ? "1 tipo de destino por repartir (pide la clave)" : `${k} tipos de destino por repartir (piden la clave)`);
  const t = p.porTraer.length;
  if (t) partes.push(t === 1 ? "1 plantilla de otra consola por traer" : `${t} plantillas de otra consola por traer`);
  return partes.length ? partes.join(" · ") : null;
}

/** Reparte las entradas en órdenes: como mucho 100 y ~40 KB cada una. */
export function trozos(entradas: EntradaOrden[], maxBytes = MAX_BYTES_ORDEN): EntradaOrden[][] {
  const out: EntradaOrden[][] = [];
  let actual: EntradaOrden[] = [];
  let bytes = 0;
  for (const e of entradas) {
    const t = JSON.stringify(e).length + 1;
    if (actual.length && (actual.length >= MAX_ENTRADAS_ORDEN || bytes + t > maxBytes)) {
      out.push(actual);
      actual = [];
      bytes = 0;
    }
    actual.push(e);
    bytes += t;
  }
  if (actual.length) out.push(actual);
  return out;
}

// ---------------------------------------------------------------------------
// En palabras
// ---------------------------------------------------------------------------

const NOMBRES_TIPO: Record<string, string> = { local: "Local", fuera: "Fuera del sitio", nube: "Nube" };

/** El tipo y las marcas de un destino en pocas palabras: «Fuera del sitio · Inmutable 30 días». */
export function textoRegla(a: AtributosDestino | null | undefined): string {
  if (!a) return "lo deducido";
  const partes: string[] = [];
  if (a.tipo) partes.push(NOMBRES_TIPO[a.tipo] ?? a.tipo);
  if (a.inmutable && a.inmutable !== "no" && a.inmutable !== "desconectado") partes.push(a.bloqueo_dias ? `Inmutable ${a.bloqueo_dias} días` : "Inmutable");
  if (a.aislado || a.inmutable === "desconectado") partes.push("Aislado");
  if (a.soporte) partes.push(`soporte «${a.soporte}»`);
  return partes.length ? partes.join(" · ") : "sin marcas";
}

/** Lo que dice un valor, en palabras y en minúscula (para «En esta consola: azul»). */
export function textoValor(clave: string, valor: unknown, nombres: { destino?: (id: string) => string | null } = {}): string {
  const c = claseDe(clave);
  const v = (valor ?? null) as Record<string, unknown> | null;
  switch (c?.clase) {
    case "etiqueta.color": {
      const n = typeof v?.color === "number" ? NOMBRES_COLOR[v.color] : undefined;
      return n ? n.toLowerCase() : "sin color";
    }
    case "etiqueta.plantilla":
      return v?.plantilla ? "con plantilla" : "sin plantilla";
    case "destino":
      return typeof v?.nombre === "string" && v.nombre.trim() ? `«${v.nombre.trim()}»` : (nombres.destino?.(c.sujeto) ?? "el nombre de siempre");
    case "destino.regla":
      return v ? textoRegla(v.atributos as AtributosDestino) : "lo deducido";
    case "plantilla":
      return v ? "la tiene" : "borrada";
    default:
      return "—";
  }
}

/** Qué dato es, en palabras: «Color de «Servidor»». */
export function tituloDato(f: Pick<FilaComun, "clave" | "valor" | "local">, nombres: { destino?: (id: string) => string | null; plantilla?: (id: string) => string | null } = {}): string {
  const c = claseDe(f.clave);
  const nombre = (x: unknown) => ((x as { nombre?: unknown } | null)?.nombre as string | undefined) ?? undefined;
  switch (c?.clase) {
    case "etiqueta.color":
      return `Color de «${nombre(f.valor) ?? nombre(f.local) ?? c.sujeto}»`;
    case "etiqueta.plantilla":
      return `Plantilla de «${nombre(f.valor) ?? nombre(f.local) ?? c.sujeto}»`;
    case "destino":
      return `Nombre del destino ${nombres.destino?.(c.sujeto) ? `«${nombres.destino(c.sujeto)}»` : c.sujeto}`;
    case "destino.regla":
      return `Tipo y marcas de ${nombres.destino?.(c.sujeto) ? `«${nombres.destino(c.sujeto)}»` : c.sujeto}`;
    case "plantilla":
      return `Plantilla de copia ${nombres.plantilla?.(c.sujeto) ? `«${nombres.plantilla(c.sujeto)}»` : ""}`.trim();
    default:
      return f.clave;
  }
}

/** «la consola «Oficina»» u «otra consola». */
export const deQueConsola = (f: Pick<FilaComun, "consola">) => (f.consola?.trim() ? `la consola «${f.consola.trim()}»` : "otra consola");

export interface Diferencia {
  clave: string;
  titulo: string;
  aqui: string;
  otra: string;
  /** «En esta consola: azul · En la otra: sin color». */
  frase: string;
  /** El valor de cada lado (para elegir). */
  valorAqui: unknown;
  valorOtra: unknown;
  /** Elegir pide la clave de administración. */
  conClave: boolean;
}

export function diferencia(f: FilaComun, nombres: Parameters<typeof tituloDato>[1] = {}): Diferencia {
  const aqui = textoValor(f.clave, f.local, nombres);
  const otra = textoValor(f.clave, f.valor, nombres);
  const quien = f.consola?.trim() ? `En «${f.consola.trim()}»` : "En la otra";
  return {
    clave: f.clave,
    titulo: tituloDato(f, nombres),
    aqui,
    otra,
    frase: `En esta consola: ${aqui} · ${quien}: ${otra}`,
    // Una plantilla «que está aquí» se reparte tal cual (el servidor pone la cifrada de aquí).
    valorAqui: claseDe(f.clave)?.clase === "plantilla" ? (f.local ? true : null) : (f.local ?? null),
    valorOtra: f.valor ?? null,
    conClave: pideClave(f.clave),
  };
}

/** Quién lo cambió por última vez, en palabras (o null si fue esta consola). */
export function cambiadoPor(f: Pick<FilaComun, "esta" | "consola" | "por">): string | null {
  if (f.esta) return null;
  const desde = deQueConsola(f);
  return f.por?.trim() ? `Lo cambió ${f.por.trim()} desde ${desde}` : `Cambiado desde ${desde}`;
}
