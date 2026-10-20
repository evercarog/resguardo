// 0.8.1 (H2, plan-mejoras 15qq): el inventario de lo protegido. No es un inventario
// general de TI (nada de números de serie, usuarios del sistema ni programas): solo lo que
// hace falta como evidencia de qué se protege y cuánto importa. Por equipo y por
// repositorio, opcional: responsable, criticidad, si hay datos personales (sí o no, nunca
// el contenido), el RTO y el RPO objetivo y, de un equipo, su identificador externo (la
// etiqueta del inventario del cliente). Es un dato común del cliente
// (`inventario:<tipo>:<objeto>`, docs/consolas-multiples.md §6.5): igual en todas sus
// consolas con agentes que lo admiten (`inventario_comun`).
import { csv } from "./csv";
import type { Equipo } from "./tipos";

export type Criticidad = "baja" | "media" | "alta" | "critica";
export const CRITICIDADES: { id: Criticidad; texto: string }[] = [
  { id: "baja", texto: "Baja" },
  { id: "media", texto: "Media" },
  { id: "alta", texto: "Alta" },
  { id: "critica", texto: "Crítica" },
];
export const textoCriticidad = (c: string | null | undefined) => CRITICIDADES.find((x) => x.id === c)?.texto ?? "";

export interface FichaInventario {
  responsable?: string | null;
  criticidad?: Criticidad | null;
  datos_personales?: boolean | null;
  rto_min?: number | null;
  rpo_min?: number | null;
  /** Solo de un equipo: la etiqueta del inventario que ya usa el cliente. */
  id_externo?: string | null;
}

export type TipoInventario = "equipo" | "repositorio";
export const claveInventario = (tipo: TipoInventario, objeto: string) => `inventario:${tipo}:${objeto}`;
export const objetoRepo = (equipo: string, repo: string) => `${equipo}/${repo}`;

/** Lo que llega del servidor, limpio. */
export function leerInventario(x: unknown): Record<string, FichaInventario> {
  const out: Record<string, FichaInventario> = {};
  if (!x || typeof x !== "object" || Array.isArray(x)) return out;
  for (const [k, v0] of Object.entries(x as Record<string, unknown>)) {
    if (!/^(equipo|repositorio):/.test(k) || !v0 || typeof v0 !== "object") continue;
    const v = v0 as Record<string, unknown>;
    const f: FichaInventario = {};
    if (typeof v.responsable === "string") f.responsable = v.responsable;
    if (typeof v.criticidad === "string" && CRITICIDADES.some((c) => c.id === v.criticidad)) f.criticidad = v.criticidad as Criticidad;
    if (typeof v.datos_personales === "boolean") f.datos_personales = v.datos_personales;
    if (typeof v.rto_min === "number") f.rto_min = v.rto_min;
    if (typeof v.rpo_min === "number") f.rpo_min = v.rpo_min;
    if (typeof v.id_externo === "string") f.id_externo = v.id_externo;
    out[k] = f;
  }
  return out;
}

/** La ficha de un equipo o un repositorio (vacía si no hay nada anotado). */
export const fichaDe = (inv: Record<string, FichaInventario> | null | undefined, tipo: TipoInventario, objeto: string): FichaInventario => inv?.[`${tipo}:${objeto}`] ?? {};

/** Lo que falla en una ficha (o null). Mismos límites que el protocolo. */
export function errorFicha(f: FichaInventario, tipo: TipoInventario): string | null {
  if ((f.responsable ?? "").trim().length > 80) return "El responsable: como mucho 80 caracteres.";
  if ((f.id_externo ?? "").trim().length > 64) return "El identificador externo: como mucho 64 caracteres.";
  if (tipo !== "equipo" && (f.id_externo ?? "").trim()) return "El identificador externo es de un equipo.";
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u001f\u007f]/.test(`${f.responsable ?? ""}${f.id_externo ?? ""}`)) return "Hay caracteres que no valen.";
  for (const n of [f.rto_min, f.rpo_min]) if (n != null && !(Number.isInteger(n) && n > 0 && n <= 5_256_000)) return "El RTO y el RPO objetivo: un número de minutos mayor que cero.";
  return null;
}

/** El valor para el dato común: solo lo anotado (sin nada, `null`: se quita). */
export function valorFicha(f: FichaInventario): FichaInventario | null {
  const v: FichaInventario = {};
  const r = (f.responsable ?? "").trim();
  if (r) v.responsable = r;
  if (f.criticidad) v.criticidad = f.criticidad;
  if (f.datos_personales != null) v.datos_personales = f.datos_personales;
  if (f.rto_min) v.rto_min = f.rto_min;
  if (f.rpo_min) v.rpo_min = f.rpo_min;
  const id = (f.id_externo ?? "").trim();
  if (id) v.id_externo = id;
  return Object.keys(v).length ? v : null;
}

/** «Sí», «No» o «Sin anotar». */
export const textoDatosPersonales = (b: boolean | null | undefined) => (b === true ? "Sí" : b === false ? "No" : "Sin anotar");

/** Minutos en palabras cortas («4 h», «2 días», «30 min»). */
export function textoMin(n: number | null | undefined): string {
  if (!n) return "";
  if (n % 1440 === 0) return `${n / 1440} ${n === 1440 ? "día" : "días"}`;
  if (n % 60 === 0) return `${n / 60} h`;
  return `${n} min`;
}

/** ¿Lo guarda algún equipo del cliente para todas sus consolas? */
export const admiteInventario = (equipos: Equipo[]) => equipos.some((e) => e.confirmado && e.modo === "gestionado" && e.resumen?.admite?.includes("inventario_comun"));

export const CABECERA_INVENTARIO = [
  "Tipo",
  "Equipo",
  "Identificador externo",
  "Repositorio",
  "Sede",
  "Tipo de equipo",
  "Sistema",
  "Responsable",
  "Criticidad",
  "Datos personales",
  "RTO objetivo (min)",
  "RPO objetivo (min)",
  "Copias",
  "Versiones",
  "Tamaño (bytes)",
  "Última versión",
];

/** Una fila del inventario (un equipo o uno de sus repositorios). */
export interface FilaInventario {
  tipo: TipoInventario;
  equipo: string;
  idExterno: string;
  repositorio: string;
  sede: string;
  tipoEquipo: string;
  sistema: string;
  ficha: FichaInventario;
  copias: number | null;
  versiones: number | null;
  bytes: number | null;
  ultima: string | null;
}

/** El inventario en CSV (15qq, punto 3: para enlazarlo con el inventario del cliente). */
export function csvInventario(filas: FilaInventario[]): string {
  return csv(
    CABECERA_INVENTARIO,
    filas.map((f) => [
      f.tipo === "equipo" ? "Equipo" : "Repositorio",
      f.equipo,
      f.idExterno,
      f.repositorio,
      f.sede,
      f.tipoEquipo,
      f.sistema,
      f.ficha.responsable ?? "",
      textoCriticidad(f.ficha.criticidad),
      f.ficha.datos_personales == null ? "" : f.ficha.datos_personales,
      f.ficha.rto_min ?? "",
      f.ficha.rpo_min ?? "",
      f.copias ?? "",
      f.versiones ?? "",
      f.bytes ?? "",
      f.ultima ?? "",
    ]),
  );
}
