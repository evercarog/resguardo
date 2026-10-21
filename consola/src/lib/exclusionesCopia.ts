// 0.8.1 (D2, plan-mejoras 15y): los grupos de exclusiones del cliente («Organización»,
// lib/gruposExclusiones.ts) en las copias, «No copiar archivos de más de N MB» con
// excepciones y la vista previa. Sin runas (lo prueba scripts/vectores-copias-dia.ts).
//
// - Una copia guarda las reglas que aplica (`exclusiones`: el equipo no depende de la
//   consola) y, con un agente que lo entiende (`admite: "grupos_en_copias"`), de qué grupos
//   vienen (`grupos_exclusiones: [{ id, nombre, reglas }]`, con las reglas tal como estaban).
// - Activar un grupo añade sus reglas; quitarlo quita las suyas que no ponga otro grupo
//   activo. Las reglas sueltas siguen al lado.
// - Al cambiar un grupo en «Organización», el resumen de cada equipo dice qué copias lo
//   usan y con qué reglas: se ofrece aplicarles el cambio (una orden `config` por equipo).
import type { CopiaConfig, Equipo, GrupoUsado } from "./tipos";

export const ADMITE_GRUPOS = "grupos_en_copias";
/** Como el agente (`excluir::MAX_MB`). */
export const MAX_MB = 1_000_000;
export const MAX_EXCEPCIONES = 50;
/** Las excepciones que se proponen al poner un límite: buzones de correo y volcados. */
export const EXCEPCIONES_HABITUALES = ["*.pst", "*.ost", "*.bak", "*.mdf", "*.ldf"];

type ConGrupos = Pick<CopiaConfig, "exclusiones"> & { grupos_exclusiones?: GrupoUsado[] | null };
type GrupoDato = { id: string; nombre: string; reglas: string[] };

const sinRepetir = (x: string[]) => [...new Set(x)];

/** ¿Tiene la copia este grupo? */
export const usaGrupo = (k: ConGrupos, id: string) => !!k.grupos_exclusiones?.some((g) => g.id === id);

/** Las reglas que ponen los grupos activos de la copia (sin uno, si se dice). */
function reglasDeGrupos(k: ConGrupos, sin?: string): Set<string> {
  return new Set((k.grupos_exclusiones ?? []).filter((g) => g.id !== sin).flatMap((g) => g.reglas));
}

/** Las reglas «sueltas»: las de la copia que no pone ningún grupo activo. */
export function sueltas(k: ConGrupos): string[] {
  const de = reglasDeGrupos(k);
  return k.exclusiones.filter((r) => !de.has(r));
}

/** Pone las reglas sueltas (las de los grupos se quedan). */
export function ponerSueltas(k: ConGrupos, reglas: string[]): void {
  const de = reglasDeGrupos(k);
  k.exclusiones = sinRepetir([...k.exclusiones.filter((r) => de.has(r)), ...reglas.map((r) => r.trim()).filter(Boolean)]);
}

/** Activa (con sus reglas de ahora) o quita un grupo de la copia. */
export function alternarGrupo(k: ConGrupos, g: GrupoDato, activo: boolean): void {
  const grupos = (k.grupos_exclusiones ?? []).filter((x) => x.id !== g.id);
  if (activo) {
    k.grupos_exclusiones = [...grupos, { id: g.id, nombre: g.nombre, reglas: [...g.reglas] }];
    k.exclusiones = sinRepetir([...k.exclusiones, ...g.reglas]);
    return;
  }
  const antes = (k.grupos_exclusiones ?? []).find((x) => x.id === g.id);
  const otros = reglasDeGrupos(k, g.id);
  k.grupos_exclusiones = grupos;
  // Se quitan las suyas que no pone otro grupo activo (las sueltas iguales también: eran del grupo).
  if (antes) k.exclusiones = k.exclusiones.filter((r) => !antes.reglas.includes(r) || otros.has(r));
}

/** Pone al día un grupo que la copia ya usa (sus reglas nuevas en lugar de las de antes). */
export function ponerAlDia(k: ConGrupos, g: GrupoDato): boolean {
  if (!usaGrupo(k, g.id)) return false;
  alternarGrupo(k, g, false);
  alternarGrupo(k, g, true);
  return true;
}

/** ¿Tiene la copia las reglas de ahora de ese grupo? */
export function alDia(usado: Pick<GrupoUsado, "reglas">, g: Pick<GrupoDato, "reglas">): boolean {
  const a = [...usado.reglas].sort();
  const b = [...g.reglas].sort();
  return a.length === b.length && a.every((x, i) => x === b[i]);
}

/**
 * 0.8.2 (C2, 15y): poner o quitar grupos en varias copias a la vez («Varios a la vez» en
 * Equipos). Poner: los que la copia no tiene entran con sus reglas de ahora y los que tiene
 * con reglas de antes se ponen al día. Quitar: salen con sus reglas (las que no ponga otro
 * grupo). Devuelve las copias que cambian (las demás, igual).
 */
export function gruposEnCopias<K extends ConGrupos & { nombre: string }>(copias: K[], grupos: GrupoDato[], quitar = false): K[] {
  const foto = (k: ConGrupos) => JSON.stringify([k.exclusiones, k.grupos_exclusiones ?? []]);
  return copias.filter((k) => {
    const antes = foto(k);
    for (const g of grupos) {
      const u = k.grupos_exclusiones?.find((x) => x.id === g.id);
      if (quitar) {
        if (u) alternarGrupo(k, g, false);
      } else if (!u || !alDia(u, g) || g.reglas.some((r) => !k.exclusiones.includes(r))) alternarGrupo(k, g, true);
    }
    return foto(k) !== antes;
  });
}

/**
 * 0.8.2 (C2, 15y): los grupos de una plantilla (también la de una etiqueta, 6b) con las
 * reglas de ahora de «Organización»: la plantilla guarda las de cuando se hizo. Los grupos
 * que ya no existen se quedan como estaban. Devuelve los nombres de los que se pusieron al día.
 */
export function gruposAlDia(k: ConGrupos, grupos: GrupoDato[]): string[] {
  const out: string[] = [];
  for (const u of [...(k.grupos_exclusiones ?? [])]) {
    const g = grupos.find((x) => x.id === u.id);
    if (g && !alDia(u, g)) {
      ponerAlDia(k, g);
      out.push(g.nombre);
    }
  }
  return out;
}

/** Las copias (del resumen de cada equipo) que usan un grupo, y si tienen sus reglas de ahora. */
export function copiasQueUsan(equipos: Equipo[], g: GrupoDato): { equipo: Equipo; copia: { id: string; nombre: string }; alDia: boolean }[] {
  return equipos.flatMap((e) =>
    (e.resumen?.copias ?? []).flatMap((k) => {
      const u = k.grupos_exclusiones?.find((x) => x.id === g.id);
      return u ? [{ equipo: e, copia: { id: k.id, nombre: k.nombre }, alDia: alDia(u, g) }] : [];
    }),
  );
}

/** «No copiar archivos de más de N MB»: lo que falla, o null. */
export function errorTamano(k: Pick<CopiaConfig, "mayores_mb" | "excepto_tamano" | "nombre">): string | null {
  const n = k.mayores_mb;
  if (n != null && (!Number.isInteger(n) || n < 1 || n > MAX_MB)) return `«${k.nombre}»: el tamaño máximo va de 1 MB a ${MAX_MB.toLocaleString("es")} MB`;
  if ((k.excepto_tamano?.length ?? 0) > MAX_EXCEPCIONES) return `«${k.nombre}»: como mucho ${MAX_EXCEPCIONES} excepciones al tamaño`;
  return null;
}

/** «Más de 500 MB, salvo *.pst y *.bak» (o null sin límite). */
export function tamanoEnFrase(k: Pick<CopiaConfig, "mayores_mb" | "excepto_tamano">): string | null {
  if (!k.mayores_mb) return null;
  const ex = k.excepto_tamano ?? [];
  const mb = k.mayores_mb >= 1024 && k.mayores_mb % 1024 === 0 ? `${k.mayores_mb / 1024} GB` : `${k.mayores_mb.toLocaleString("es")} MB`;
  return `nada de más de ${mb}${ex.length ? `, salvo ${ex.length <= 3 ? ex.join(", ") : `${ex.slice(0, 3).join(", ")} y ${ex.length - 3} más`}` : ""}`;
}

/**
 * 15y: lo que el agente dice de «no copiar más de N MB» en el mensaje de una copia
 * («Fuera por tamaño (más de 500 MB): 3 archivos (12 GB: 2 .mp4 y 1 .iso).» o «No se
 * aplicó…»), o null.
 */
export function fueraPorTamano(mensaje: string | null | undefined): string | null {
  if (!mensaje) return null;
  const i = mensaje.search(/Fuera por tamaño|No se aplicó «no copiar más de/);
  return i < 0 ? null : mensaje.slice(i).trim();
}

// --- Vista previa (`vista_exclusiones`) ---

export interface Cuenta {
  archivos: number;
  bytes: number;
}
export interface VistaPrevia {
  total: Cuenta;
  grupos: ({ id: string } & Cuenta)[];
  sueltas: Cuenta;
  tamano: Cuenta | null;
  quedan: Cuenta;
  completo: boolean;
}

const cuenta = (x: unknown): Cuenta => {
  const o = x as Partial<Cuenta> | null;
  return { archivos: Number.isFinite(o?.archivos) ? Number(o!.archivos) : 0, bytes: Number.isFinite(o?.bytes) ? Number(o!.bytes) : 0 };
};

/** La respuesta del equipo, con lo que no venga como debe a cero. */
export function leerVista(x: unknown): VistaPrevia {
  const o = (x ?? {}) as Record<string, unknown>;
  return {
    total: cuenta(o.total),
    grupos: Array.isArray(o.grupos) ? o.grupos.map((g) => ({ id: String((g as { id?: unknown }).id ?? ""), ...cuenta(g) })) : [],
    sueltas: cuenta(o.sueltas),
    tamano: o.tamano ? cuenta(o.tamano) : null,
    quedan: cuenta(o.quedan),
    completo: o.completo === true,
  };
}

/** El cuerpo de la orden: las carpetas y cada grupo (los activos y los que no) con sus reglas. */
export function cuerpoVista(k: CopiaConfig, grupos: GrupoDato[]): Record<string, unknown> {
  return {
    carpetas: k.carpetas,
    grupos: grupos.map((g) => ({ id: g.id, reglas: (k.grupos_exclusiones?.find((x) => x.id === g.id)?.reglas ?? g.reglas).slice(0, 100) })),
    sueltas: sueltas(k),
    ...(k.mayores_mb ? { mayores_mb: k.mayores_mb, excepto: k.excepto_tamano ?? [] } : {}),
  };
}
