// Tipo y marcas de un destino (0.7.26, bloque 2; docs/regla-3-2-1.md).
//
// Tipo, uno solo: Local · Fuera del sitio · Nube. Marcas, combinables:
// Inmutable (solo añadir, bloqueo de objetos de N días, instantáneas fuera de
// su alcance) y Aislado (un medio que se desconecta y se rota).
//
// Se deduce del destino (carpeta del equipo → Local; zona de un almacén →
// Local + Inmutable; Dropbox/Drive/OneDrive → Nube; B2/S3 → Nube, + Inmutable
// si tiene bloqueo) y siempre se puede cambiar: lo que marca una persona manda
// y se dice («marcado por una persona»).
//
// Compatible hacia atrás: el catálogo sigue llevando `lugar` e `inmutable` (lo
// que entiende una consola anterior); `tipo`, `aislado`, `bloqueo_dias` y
// `aislado_dias` son nuevos y opcionales. `inmutable: "desconectado"` de antes
// se lee como «Aislado». Sin dependencias de Svelte (lo prueban los vectores).
import type { AtributosDestino, ConexionVolumen, InmutableDestino, LugarDestino, TipoDestino } from "./tipos";

export const TIPOS_DESTINO: TipoDestino[] = ["local", "fuera", "nube"];

/** Días sin conectarse tras los que avisa un medio aislado (si no se dice otra cosa). */
export const DIAS_AISLADO = 30;

export const TEXTO_TIPO: Record<TipoDestino, string> = { local: "Local", fuera: "Fuera del sitio", nube: "Nube" };
/** Lo que es cada tipo, en una frase corta (ayuda del selector). */
export const AYUDA_TIPO: Record<TipoDestino, string> = {
  local: "En este equipo o en otro de la oficina.",
  fuera: "En otra sede o un servidor de fuera.",
  nube: "Dropbox, Drive, OneDrive, B2, S3…",
};
export const TEXTO_MARCA = { inmutable: "Inmutable", aislado: "Aislado" } as const;

/** Las formas de ser inmutable (la marca «Inmutable»). */
export const COMO_INMUTABLE: Exclude<InmutableDestino, "no" | "desconectado">[] = ["solo_anadir", "object_lock", "instantaneas"];
export const TEXTO_COMO_INMUTABLE: Record<(typeof COMO_INMUTABLE)[number], string> = {
  solo_anadir: "Solo añadir",
  object_lock: "Bloqueo de objetos",
  instantaneas: "Instantáneas fuera de su alcance",
};

/** El tipo de un `lugar` de antes. */
export const tipoDeLugar = (l: LugarDestino): TipoDestino => (l === "nube" ? "nube" : l === "otra_sede" ? "fuera" : "local");

/** El `lugar` que entiende una consola anterior para un tipo (lo local, como se dedujo si ya era local). */
export const lugarDeTipo = (t: TipoDestino, deducido: LugarDestino): LugarDestino =>
  t === "nube" ? "nube" : t === "fuera" ? "otra_sede" : deducido === "este_equipo" || deducido === "oficina" ? deducido : "oficina";

/** Lo deducido del destino, antes de lo que diga la persona. */
export interface Deducido {
  lugar: LugarDestino;
  inmutable: InmutableDestino;
  /** Días del bloqueo de objetos, si se saben (p. ej. de la copia externa). */
  bloqueoDias?: number | null;
}

export interface Clasificacion {
  tipo: TipoDestino;
  /** Marca «Inmutable». */
  inmutable: boolean;
  /** Cómo es inmutable («no» si no lo es). */
  como: InmutableDestino;
  /** Marca «Aislado». */
  aislado: boolean;
  /** Días del bloqueo de objetos (solo con `como: "object_lock"`), si se saben. */
  bloqueoDias: number | null;
  /** Días sin conectarse tras los que avisa (Aislado). */
  aisladoDias: number;
  /** El tipo lo marcó una persona. */
  tipoPorPersona: boolean;
  /** Alguna marca la marcó una persona. */
  marcasPorPersona: boolean;
}

const diasValidos = (n: unknown, max: number): number | null => (typeof n === "number" && Number.isInteger(n) && n >= 1 && n <= max ? n : null);

/** Tipo y marcas de un destino: lo deducido con lo que dice la persona encima. */
export function clasificar(d: Deducido, a: AtributosDestino | null | undefined): Clasificacion {
  const at = a ?? {};
  const tipo = at.tipo ?? (at.lugar ? tipoDeLugar(at.lugar) : tipoDeLugar(d.lugar));
  const forma = at.inmutable ?? d.inmutable;
  const inmutable = forma !== "no" && forma !== "desconectado";
  const aislado = at.aislado ?? forma === "desconectado";
  const como = inmutable ? forma : "no";
  return {
    tipo,
    inmutable,
    como,
    aislado,
    bloqueoDias: como === "object_lock" ? (diasValidos(at.bloqueo_dias, 36500) ?? diasValidos(d.bloqueoDias, 36500)) : null,
    aisladoDias: diasValidos(at.aislado_dias, 365) ?? DIAS_AISLADO,
    tipoPorPersona: !!(at.tipo || at.lugar),
    marcasPorPersona: at.inmutable !== undefined || at.aislado !== undefined || at.bloqueo_dias !== undefined,
  };
}

/** Lo que se elige en el editor. */
export interface Eleccion {
  tipo: TipoDestino;
  inmutable: boolean;
  como: InmutableDestino;
  aislado: boolean;
  bloqueoDias: number | null;
  aisladoDias: number;
  soporte: string;
}

/**
 * Los atributos que se guardan en el catálogo: solo lo que se aparta de lo
 * deducido, con `lugar` e `inmutable` también puestos para una consola anterior
 * (Aislado sin Inmutable se guarda además como `inmutable: "desconectado"`).
 */
export function atributosDe(d: Deducido, e: Eleccion): AtributosDestino {
  const a: AtributosDestino = {};
  if (e.tipo !== tipoDeLugar(d.lugar)) {
    a.tipo = e.tipo;
    a.lugar = lugarDeTipo(e.tipo, d.lugar);
  }
  const como: InmutableDestino = e.inmutable ? (e.como === "no" || e.como === "desconectado" ? "instantaneas" : e.como) : "no";
  const forma: InmutableDestino = e.inmutable ? como : e.aislado ? "desconectado" : "no";
  const deducidoAislado = d.inmutable === "desconectado";
  if (forma !== d.inmutable) a.inmutable = forma;
  if (e.aislado !== deducidoAislado) a.aislado = e.aislado;
  else if (e.aislado && e.inmutable) a.aislado = true;
  const bloqueo = diasValidos(e.bloqueoDias, 36500);
  if (e.inmutable && como === "object_lock" && bloqueo && bloqueo !== (d.bloqueoDias ?? null)) a.bloqueo_dias = bloqueo;
  const dias = diasValidos(e.aisladoDias, 365);
  if (e.aislado && dias && dias !== DIAS_AISLADO) a.aislado_dias = dias;
  if (e.soporte.trim()) a.soporte = e.soporte.trim();
  return a;
}

// ---------- Conexión de un medio aislado (lo ve el agente) ----------

const DIA = 86_400_000;

/** «hoy», «ayer», «hace 3 días». */
export function haceDias(iso: string, ahora: number): string {
  const n = Math.max(0, Math.floor((ahora - Date.parse(iso)) / DIA));
  return n === 0 ? "hoy" : n === 1 ? "ayer" : `hace ${n} días`;
}

export interface EstadoConexion {
  /** Sin datos (un agente anterior, o aún no lo ha visto): no es un fallo. */
  sinDatos: boolean;
  /** Lleva más de sus N días sin conectarse. */
  tarde: boolean;
  /** Una línea corta para enseñar. */
  texto: string;
  /** Con varios discos que se rotan, uno por disco (el más reciente primero). */
  discos: { id: string; texto: string; tarde: boolean }[];
}

/** Lo que se dice de la conexión de un medio aislado. */
export function estadoConexion(c: ConexionVolumen | null | undefined, dias: number, ahora: number): EstadoConexion {
  const vols = (c?.volumenes ?? []).filter((v) => Number.isFinite(Date.parse(v.visto))).sort((a, b) => Date.parse(b.visto) - Date.parse(a.visto));
  const ultima = [c?.ultima_conexion, vols[0]?.visto].filter((x): x is string => !!x && Number.isFinite(Date.parse(x))).sort().at(-1);
  if (!ultima) return { sinDatos: true, tarde: false, texto: "Sin datos de conexión", discos: [] };
  const tardeDe = (iso: string) => ahora - Date.parse(iso) > dias * DIA;
  const tarde = tardeDe(ultima);
  const texto = tarde ? `Conecta el medio aislado para comprobar la rotación (última vez visto ${haceDias(ultima, ahora)})` : `Conectado por última vez ${haceDias(ultima, ahora)}`;
  const discos = vols.length > 1 ? vols.map((v, i) => ({ id: v.id, texto: `Disco ${i + 1}: visto ${haceDias(v.visto, ahora)}`, tarde: tardeDe(v.visto) })) : [];
  return { sinDatos: false, tarde, texto, discos };
}

/** La última conexión vista (para la regla), o null. */
export function ultimaConexion(c: ConexionVolumen | null | undefined): string | null {
  return [c?.ultima_conexion, ...(c?.volumenes ?? []).map((v) => v.visto)].filter((x): x is string => !!x && Number.isFinite(Date.parse(x))).sort().at(-1) ?? null;
}

/** Lo que se enseña en corto (selectores, cadenas, mapa). */
export type TipoCorto = Pick<Clasificacion, "tipo" | "inmutable" | "aislado" | "bloqueoDias"> & { porPersona?: boolean };

/** De una clasificación, lo que se enseña en corto. */
export const corta = (c: Clasificacion): TipoCorto => ({ tipo: c.tipo, inmutable: c.inmutable, aislado: c.aislado, bloqueoDias: c.bloqueoDias, porPersona: c.tipoPorPersona || c.marcasPorPersona });

/** Una nube conectada (Dropbox, Drive, OneDrive…; un NAS por SMB es local, SFTP fuera del sitio): nunca inmutable. */
export const tipoDeNube = (tipoNube: string): TipoCorto => ({ tipo: tipoNube === "smb" ? "local" : tipoNube === "sftp" ? "fuera" : "nube", inmutable: false, aislado: false, bloqueoDias: null });

/** El nombre accesible del tipo y sus marcas: «Nube, inmutable (bloqueo de 30 días)». */
export function textoClasificacion(c: Pick<Clasificacion, "tipo" | "inmutable" | "aislado" | "bloqueoDias">): string {
  const marcas = [c.inmutable ? `inmutable${c.bloqueoDias ? ` (bloqueo de ${c.bloqueoDias} días)` : ""}` : null, c.aislado ? "aislado" : null].filter(Boolean);
  return [TEXTO_TIPO[c.tipo], ...marcas].join(", ");
}
