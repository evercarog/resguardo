// «Equipos que no están en todas las consolas» (docs/plan-mejoras.md, tarea 2;
// docs/consolas-multiples.md §2.5). Un equipo dado de alta en una consola no
// llega solo a las demás: hay que conectarlo también («Conectar también a otra
// consola…»). Esta consola sabe en qué otras consolas están los equipos del
// cliente por su resumen (`consolas`, v1.36: nombre, dirección e identidad; nada
// secreto) y, con eso, cuáles faltan en alguna.
//
// Reglas:
// - Solo cuentan los equipos gestionados desde aquí y ya confirmados.
// - Un equipo sin resumen todavía (recién dado de alta) no se sabe dónde está:
//   no cuenta, salvo que se pida (`nuevo`), porque uno recién llegado solo
//   puede estar en esta.
// - Un agente que no admite `consolas_multiples` solo puede estar en esta: falta
//   en todas las demás (hay que actualizarlo para conectarlo).
// - Una consola que ninguno de sus equipos ve hace mucho (más de 30 días sin
//   contacto, o añadida hace más de 7 y nunca contactada) no se sugiere: puede
//   que ya no exista, y conectar más equipos a ella no ayuda.
import type { ConsolaDelEquipo, Equipo } from "./tipos";
import { hostDe } from "./conexion";

/** Otra consola en la que está al menos un equipo del cliente. */
export interface OtraConsola {
  identidad: string;
  nombre: string;
  url: string;
  /** Los equipos que ya están en ella. */
  con: Equipo[];
  /** Los que no. */
  sin: Equipo[];
}

const DIA = 24 * 3600_000;
/** Sin contacto de ningún equipo en este tiempo, la consola se da por abandonada. */
export const VIVA_DIAS = 30;
/** Añadida y aún sin contacto: se sigue esperando este tiempo. */
export const NUEVA_DIAS = 7;

/** Gestionado desde aquí y confirmado: se le pueden mandar órdenes. */
export const gestionable = (e: Equipo) => e.confirmado && e.modo === "gestionado";
/** ¿Admite estar en varias consolas a la vez (v1.35)? */
export const admiteVarias = (e: Equipo) => !!e.resumen?.admite?.includes("consolas_multiples");

function viva(x: ConsolaDelEquipo, ahora: number) {
  if (x.ultimo_contacto) return ahora - Date.parse(x.ultimo_contacto) <= VIVA_DIAS * DIA;
  return !!x.desde && ahora - Date.parse(x.desde) <= NUEVA_DIAS * DIA;
}

/**
 * Las otras consolas del cliente (las que no son esta), con qué equipos están
 * ya en cada una y cuáles no. Primero las que tienen más equipos.
 * `nuevos`: ids de equipos recién dados de alta que aún no han mandado su
 * resumen (cuentan como que solo están aquí).
 */
export function otrasConsolas(equipos: Equipo[], ahora: number, nuevos: string[] = []): OtraConsola[] {
  const activos = equipos.filter(gestionable);
  const porId = new Map<string, { x: ConsolaDelEquipo; con: Equipo[]; viva: boolean }>();
  for (const e of activos)
    for (const x of e.resumen?.consolas ?? []) {
      if (x.esta || !x.identidad) continue;
      const v = porId.get(x.identidad);
      if (!v) porId.set(x.identidad, { x, con: [e], viva: viva(x, ahora) });
      else {
        if (!v.con.includes(e)) v.con.push(e);
        v.viva ||= viva(x, ahora);
        // El nombre y la dirección, los del contacto más reciente.
        if ((x.ultimo_contacto ?? "") > (v.x.ultimo_contacto ?? "")) v.x = x;
      }
    }
  const sabido = (e: Equipo) => !!e.resumen || nuevos.includes(e.id);
  return [...porId.values()]
    .filter((v) => v.viva)
    .map(({ x, con }) => ({
      identidad: x.identidad,
      nombre: x.nombre?.trim() || hostDe(x.url),
      url: x.url,
      con,
      sin: activos.filter((e) => !con.includes(e) && sabido(e)),
    }))
    .sort((a, b) => b.con.length - a.con.length || a.nombre.localeCompare(b.nombre));
}

/** Las consolas del cliente en las que falta este equipo. */
export const faltaEn = (equipo: Equipo, consolas: OtraConsola[]) => consolas.filter((c) => c.sin.some((e) => e.id === equipo.id));

/** Los equipos que faltan en alguna consola (sin repetir), en el orden de la lista. */
export function equiposQueFaltan(consolas: OtraConsola[]): Equipo[] {
  const out: Equipo[] = [];
  for (const c of consolas) for (const e of c.sin) if (!out.includes(e)) out.push(e);
  return out;
}

/** «Consola en línea (consola.ejemplo.com)», o solo el host si el nombre ya lo es. */
export const nombreConsola = (c: { nombre: string; url: string }) => {
  const h = hostDe(c.url);
  return c.nombre && c.nombre !== h ? `${c.nombre} (${h})` : h;
};

/**
 * Lo que se le dice a un equipo que falta en `c`: «Este equipo solo está en
 * esta consola; los demás también están en «X»». Si el equipo ya está en
 * alguna otra, o si no todos los demás están en X, se dice así.
 */
export function fraseEquipo(equipo: Equipo, c: OtraConsola, equipos: Equipo[]): string {
  const otros = equipos.filter((e) => gestionable(e) && e.id !== equipo.id);
  const quien = c.con.length >= otros.length ? "los demás" : c.con.length === 1 ? `${c.con[0].nombre}` : `otros ${c.con.length} equipos de este cliente`;
  const verbo = c.con.length === 1 && c.con.length < otros.length ? "está" : "están";
  const soloAqui = !equipo.resumen?.consolas?.some((x) => !x.esta);
  return `${soloAqui ? "Este equipo solo está en esta consola" : `Este equipo no está en «${c.nombre}»`}; ${quien} también ${verbo} en «${c.nombre}».`;
}
