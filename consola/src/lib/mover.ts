// «Mover a otro sitio…» (v1.41): lo que no es pantalla. Los pasos los lleva
// el navegador que lo empezó (se recuerdan en su almacenamiento local); el
// equipo cuenta a todas sus consolas cuándo trae el historial (v1.4x,
// progreso `historial` con `mover`), y las demás solo lo enseñan.
import { lista } from "./formato";
import type { TareaEnMarcha } from "./tipos";

/** Dónde recuerda este navegador un movimiento a medias. */
export const claveMover = (cliente: string, equipo: string, repo: string) => `resguardo.mover.${cliente}.${equipo}.${repo}`;

/** ¿Este navegador lleva un movimiento de ese repositorio (a medias o recién hecho)? */
export function hayPlanMover(cliente: string, equipo: string, repo: string): boolean {
  try {
    return typeof localStorage !== "undefined" && localStorage.getItem(claveMover(cliente, equipo, repo)) != null;
  } catch {
    return false;
  }
}

/**
 * El paso «las copias pasan al nuevo», según cuántas guardan en el repositorio:
 * 0 → «No hay copias que cambiar» (se da por hecho); 1 → «Cambiar la copia «X»
 * para que guarde en el nuevo»; N → «Cambiar las N copias («X», «Y» y «Z») para
 * que guarden en el nuevo».
 */
export function textoPasoCopias(nombres: string[]): { texto: string; sinCopias: boolean } {
  if (!nombres.length) return { texto: "No hay copias que cambiar", sinCopias: true };
  if (nombres.length === 1) return { texto: `Cambiar la copia «${nombres[0]}» para que guarde en el nuevo`, sinCopias: false };
  return { texto: `Cambiar las ${nombres.length} copias (${lista(nombres.map((n) => `«${n}»`))}) para que guarden en el nuevo`, sinCopias: false };
}

/** Lo que dice el paso de las copias cuando no hacía falta (la configuración no tenía ninguna en él). */
export function textoSinCopias(equipo: string, repo: string): string {
  return `Ninguna copia de ${equipo} guardaba en «${repo}»: no hay nada que cambiar.`;
}

/** El movimiento en marcha de un repositorio (sea el que se mueve o el nuevo), si lo hay. */
export function moviendoDe(tareas: TareaEnMarcha[], repo: string): TareaEnMarcha | null {
  return tareas.find((t) => t.tipo === "historial" && t.mover && (t.origen === repo || t.repo === repo)) ?? null;
}
