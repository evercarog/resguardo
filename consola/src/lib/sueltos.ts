// 0.8.1 (D6, plan-mejoras 15gg): los repositorios **sueltos** del disco de un almacén
// (los que no están en la carpeta de ninguno de sus equipos; p. ej. los de un rest-server
// viejo) y «Dar a un equipo». Sin runas (lo prueba scripts/vectores-copias-dia.ts).
import { zonaPorPuerto } from "./almacenUnico";
import { usuarioEnAlmacen } from "./direccion";
import { MAX_REPO, normalizar } from "./nombres";
import { PRINCIPAL } from "./destinos";
import type { Equipo } from "./tipos";

export const ADMITE_SUELTOS = "repos_sueltos";

/** Uno suelto, como lo dice el almacén (`guarda_copias.sueltos`). */
export interface Suelto {
  /** La zona (sin ella, la principal). */
  zona?: string | null;
  /** Su carpeta, relativa a la de la zona (`rest-antiguo/ventas`); fuera del almacén, la ruta entera. */
  ruta: string;
  /** Fuera de la carpeta del almacén (en una carpeta que se le indicó): solo se enseña. */
  fuera?: boolean;
  versiones: number;
  ultima?: string | null;
}

/** Los sueltos de un almacén (lo que no venga como debe, fuera). */
export function sueltosDe(a: Pick<Equipo, "resumen"> | null | undefined): Suelto[] {
  const l: unknown = a?.resumen?.guarda_copias?.sueltos;
  if (!Array.isArray(l)) return [];
  return l.flatMap((x) => {
    const o = x as Record<string, unknown>;
    if (typeof o?.ruta !== "string" || !o.ruta) return [];
    return [{ zona: typeof o.zona === "string" ? o.zona : null, ruta: o.ruta, fuera: o.fuera === true, versiones: Number.isFinite(o.versiones) ? Number(o.versiones) : 0, ultima: typeof o.ultima === "string" ? o.ultima : null }];
  });
}

/** Los equipos con acceso a esa zona del almacén: su usuario allí y el id de su destino. */
export function equiposConAcceso(almacen: Pick<Equipo, "id" | "resumen">, equipos: Equipo[], zona: string | null | undefined): { equipo: Equipo; usuario: string; destino: string }[] {
  const z = zona || PRINCIPAL;
  return equipos.flatMap((e) => {
    if (e.id === almacen.id) return [];
    const d = (e.resumen?.destinos ?? []).find((x) => x.equipo_almacen === almacen.id && x.tipo === "rest" && zonaPorPuerto(almacen, x.donde) === z && !!usuarioEnAlmacen(x.donde));
    return d ? [{ equipo: e, usuario: usuarioEnAlmacen(d.donde), destino: d.id }] : [];
  });
}

/** La carpeta que se propone en la del equipo: el último trozo, legible y que no esté ya. */
export function carpetaPropuesta(ruta: string, ocupadas: string[]): string {
  const base = normalizar(ruta.split("/").filter(Boolean).at(-1) ?? "", MAX_REPO, "repositorio");
  if (!ocupadas.includes(base)) return base;
  for (let i = 2; ; i++) {
    const x = `${base.slice(0, MAX_REPO - 3)}-${i}`;
    if (!ocupadas.includes(x)) return x;
  }
}

/** El cuerpo de «Dar a un equipo» (al almacén, con la clave de administración). */
export const cuerpoDar = (s: Suelto, usuario: string, carpeta: string) => ({ dar_suelto: { ...(s.zona ? { zona: s.zona } : {}), ruta: s.ruta, usuario, carpeta } });
