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

export type ClaseDato = "etiqueta.color" | "etiqueta.plantilla" | "destino.regla" | "destino" | "plantilla" | "sede" | "sede.equipo" | "sede.destino" | "etiqueta.categoria" | "orden.equipos" | "exclusiones" | "nota.observacion" | "nota.comentario" | "prueba.funcional" | "prueba.politica" | "inventario";
const CLASES: ClaseDato[] = ["etiqueta.color", "etiqueta.plantilla", "destino.regla", "destino", "plantilla", "sede", "sede.equipo", "sede.destino", "etiqueta.categoria", "orden.equipos", "exclusiones", "nota.observacion", "nota.comentario", "prueba.funcional", "prueba.politica", "inventario"];

/** 0.8.0 (D1, D2): `resumen.admite` de un agente que guarda sedes, categorías y orden. */
export const ADMITE_SEDES = "sedes";
/** ¿Es de las clases de la 0.8.0? (Solo a los agentes con `admite: "sedes"`: uno anterior rechazaría la orden entera.) */
export const esDeSedes = (clave: string) => ["sede", "sede.equipo", "sede.destino", "etiqueta.categoria", "orden.equipos"].includes(claseDe(clave)?.clase ?? "");
/** ¿Las guarda este equipo? */
export const admiteSedes = (e: Pick<Equipo, "resumen"> | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_SEDES);

/** 0.8.1 (B2, 15y): `resumen.admite` de un agente que guarda los grupos de exclusiones (`exclusiones:<id>`). */
export const ADMITE_EXCLUSIONES = "grupos_exclusiones";
/** ¿Es un grupo de exclusiones? (Solo a los agentes con `admite: "grupos_exclusiones"`.) */
export const esDeExclusiones = (clave: string) => claseDe(clave)?.clase === "exclusiones";
/** ¿Guarda este equipo este dato? (Las clases nuevas, solo los agentes que las admiten: uno anterior rechazaría la orden entera.) */
export function admiteClave(e: Pick<Equipo, "resumen"> | null | undefined, clave: string): boolean {
  if (esDeSedes(clave)) return admiteSedes(e);
  if (esDeExclusiones(clave)) return !!e?.resumen?.admite?.includes(ADMITE_EXCLUSIONES);
  if (claseDe(clave)?.clase === "inventario") return !!e?.resumen?.admite?.includes(ADMITE_INVENTARIO);
  if (esDePruebas(clave)) {
    if (!e?.resumen?.admite?.includes(ADMITE_PRUEBAS)) return false;
    // Como los comentarios: con el documento del equipo casi lleno, las pruebas siguen en esta consola.
    const bytes = (e.resumen as { datos_cliente?: { bytes?: number } | null }).datos_cliente?.bytes ?? 0;
    return claseDe(clave)?.clase !== "prueba.funcional" || bytes < LLENO_COMENTARIOS;
  }
  if (esDeNotas(clave)) {
    if (!e?.resumen?.admite?.includes(ADMITE_NOTAS)) return false;
    // Los comentarios pueden ser muchos: con el documento del equipo ya a tres cuartos de su
    // tope (256 KiB), dejan de repartirse (siguen en esta consola) para no llenarlo.
    const bytes = (e.resumen as { datos_cliente?: { bytes?: number } | null }).datos_cliente?.bytes ?? 0;
    return claseDe(clave)?.clase !== "nota.comentario" || bytes < LLENO_COMENTARIOS;
  }
  return true;
}

/** 0.8.1 (H2, 15qq): `resumen.admite` de un agente que guarda el inventario de lo protegido. */
export const ADMITE_INVENTARIO = "inventario_comun";
/** 0.8.1 (H4, 15ss): `resumen.admite` de un agente que guarda las pruebas funcionales y su política. */
export const ADMITE_PRUEBAS = "pruebas_comunes";
/** ¿Es una prueba funcional o su política? (Solo a los agentes con `admite: "pruebas_comunes"`.) */
export const esDePruebas = (clave: string) => ["prueba.funcional", "prueba.politica"].includes(claseDe(clave)?.clase ?? "");
/** ¿Guarda este equipo las pruebas funcionales para todas las consolas? */
export const admitePruebas = (e: Pick<Equipo, "resumen"> | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_PRUEBAS);

/** 0.8.1 (B5, 15pp): `resumen.admite` de un agente que guarda observaciones y comentarios comunes. */
export const ADMITE_NOTAS = "notas_comunes";
/** ¿Es una observación o un comentario común? (Solo a los agentes con `admite: "notas_comunes"`.) */
export const esDeNotas = (clave: string) => ["nota.observacion", "nota.comentario"].includes(claseDe(clave)?.clase ?? "");
/** Lo que puede ocupar el documento de un equipo antes de dejar de repartir comentarios. */
export const LLENO_COMENTARIOS = 192 * 1024;
/** Tipos con observación común (la del equipo va con `observacion_equipo`) y con comentarios comunes. */
export const TIPOS_OBSERVACION_COMUN = ["repositorio", "copia", "destino"];
export const TIPOS_COMENTARIO_COMUN = ["equipo", "repositorio", "copia", "destino"];
const parteObjeto = (x: string) => x.length > 0 && x.length <= 128 && x !== "." && x !== ".." && /^[A-Za-z0-9_.:@-]+$/.test(x);
/** El objeto de una nota según su tipo (como `objeto_nota_valido` del protocolo). */
export function objetoNotaValido(tipo: string, objeto: string): boolean {
  if (tipo === "repositorio" || tipo === "copia") {
    const i = objeto.indexOf("/");
    return i > 0 && parteObjeto(objeto.slice(0, i)) && parteObjeto(objeto.slice(i + 1));
  }
  return (tipo === "equipo" || tipo === "destino") && parteObjeto(objeto);
}

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
      : clase === "plantilla" || clase === "sede" || clase === "sede.equipo" || clase === "etiqueta.categoria" || clase === "exclusiones"
        ? idSimple(sujeto)
        : clase === "orden.equipos"
          ? sujeto === "todos"
          : clase === "nota.observacion"
            ? ((i) => i > 0 && TIPOS_OBSERVACION_COMUN.includes(sujeto.slice(0, i)) && objetoNotaValido(sujeto.slice(0, i), sujeto.slice(i + 1)))(sujeto.indexOf(":"))
            : clase === "nota.comentario" || clase === "prueba.funcional"
              ? idSimple(sujeto)
              : clase === "prueba.politica"
                ? sujeto === "cliente"
                : clase === "inventario"
                  ? ((i) => i > 0 && ["equipo", "repositorio"].includes(sujeto.slice(0, i)) && objetoNotaValido(sujeto.slice(0, i), sujeto.slice(i + 1)))(sujeto.indexOf(":"))
                  : idDestinoValido(sujeto);
  return ok ? { clase, sujeto } : null;
}

/** ¿Cambiarlo pide la clave de administración? (Lo que cuenta en la regla 3-2-1: el tipo y las marcas de un destino y la sede de un equipo o de un destino.) */
export const pideClave = (clave: string) => ["destino.regla:", "sede.equipo:", "sede.destino:"].some((p) => clave.startsWith(p));

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
  const conClave = [...p.porEnviarConClave, ...p.sinCompartirConClave];
  const k = conClave.filter((x) => x.clave.startsWith("destino.regla:")).length;
  if (k) partes.push(k === 1 ? "1 tipo de destino por repartir (pide la clave)" : `${k} tipos de destino por repartir (piden la clave)`);
  const ks = conClave.length - k;
  if (ks) partes.push(ks === 1 ? "1 sede de un equipo o destino por repartir (pide la clave)" : `${ks} sedes de equipos o destinos por repartir (piden la clave)`);
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
    case "sede":
      return typeof v?.nombre === "string" ? `«${v.nombre}»` : "quitada";
    case "sede.equipo":
    case "sede.destino":
      return typeof v?.sede === "string" ? `la sede ${v.sede}` : "la de por defecto";
    case "etiqueta.categoria":
      return Array.isArray(v?.etiquetas) ? (v.etiquetas as string[]).join(", ") || "sin etiquetas" : "quitada";
    case "orden.equipos":
      return v ? "un orden a mano" : "sin orden a mano";
    case "exclusiones":
      return Array.isArray(v?.reglas) ? (v.reglas as string[]).join(", ") || "sin reglas" : "quitado";
    case "nota.observacion":
    case "nota.comentario": {
      const t = typeof v?.texto === "string" ? (v.texto as string).replace(/\s+/g, " ").trim() : "";
      return t ? (t.length > 80 ? `«${t.slice(0, 79)}…»` : `«${t}»`) : "sin observación";
    }
    case "prueba.funcional": {
      if (!v) return "borrada";
      const r = v.resultado === "ok" ? "bien" : v.resultado === "parcial" ? "con incidencias" : v.resultado === "fallo" ? "falló" : "";
      return typeof v.fecha === "string" ? `del ${v.fecha}${r ? `, ${r}` : ""}` : "la tiene";
    }
    case "inventario":
      if (!v) return "nada anotado";
      return [v.responsable ? `responsable ${v.responsable}` : "", v.criticidad ? `criticidad ${v.criticidad}` : "", v.datos_personales === true ? "con datos personales" : v.datos_personales === false ? "sin datos personales" : ""].filter(Boolean).join(", ") || "anotado";
    case "prueba.politica":
      if (!v) return "la de por defecto";
      return v.activo === false ? "apagada" : `cada ${v.meses} ${v.meses === 1 ? "mes" : "meses"}, aviso ${v.aviso_dias} días antes`;
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
    case "sede":
      return `Sede «${nombre(f.valor) ?? nombre(f.local) ?? c.sujeto}»`;
    case "sede.equipo":
      return "Sede de un equipo";
    case "sede.destino":
      return `Sede de ${nombres.destino?.(c.sujeto) ? `«${nombres.destino(c.sujeto)}»` : c.sujeto}`;
    case "etiqueta.categoria":
      return `Categoría «${nombre(f.valor) ?? nombre(f.local) ?? c.sujeto}»`;
    case "orden.equipos":
      return "Orden de los equipos";
    case "exclusiones":
      return `Grupo de exclusiones «${nombre(f.valor) ?? nombre(f.local) ?? c.sujeto}»`;
    case "nota.observacion": {
      const i = c.sujeto.indexOf(":");
      const tipo = c.sujeto.slice(0, i);
      const obj = c.sujeto.slice(i + 1);
      const quien = tipo === "destino" ? (nombres.destino?.(obj) ?? obj) : obj.slice(obj.indexOf("/") + 1);
      return `Observación de ${tipo === "copia" ? "la copia" : tipo === "repositorio" ? "el repositorio" : "el destino"} «${quien}»`;
    }
    case "nota.comentario":
      return "Un comentario";
    case "prueba.funcional":
      return "Una prueba funcional";
    case "prueba.politica":
      return "Política de pruebas funcionales";
    case "inventario":
      return "Inventario de lo protegido";


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
