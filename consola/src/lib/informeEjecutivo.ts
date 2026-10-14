// Informe ejecutivo (plan 0.8.0, G1): lo que la consola le pasa al servidor de la
// regla 3-2-1-1-0 (la calcula ella: ve todos los equipos y el catálogo de destinos).
// Solo ids y el estado de cada parte, en el orden 3 · 2 · 1 · 1 · 0; los nombres y
// el texto de lo que falta los pone el servidor al hacer el PDF.
import type { IdParte, ReglaCopia } from "./regla321";
import type { EstadoParteInforme } from "./tipos";

export const ORDEN_PARTES: IdParte[] = ["copias", "soportes", "fuera", "inmutable", "errores"];

export interface CopiaReglaInforme {
  equipo: string;
  copia: string;
  partes: EstadoParteInforme[];
}

/** Cumple, atrasado (la configuración cumple pero algo no está al día) o falta. */
export function estadoParte(p: { cumple: boolean; cumple_config: boolean } | undefined): EstadoParteInforme {
  if (!p) return "falta";
  return p.cumple ? "cumple" : p.cumple_config ? "atrasado" : "falta";
}

/** La regla de cada copia, como la guarda el servidor (ordenada, para comparar sin repetir envíos). */
export function reglaParaInforme(rs: ReglaCopia[]): CopiaReglaInforme[] {
  return rs
    .map((r) => ({
      equipo: r.equipo.id,
      copia: r.copia.id,
      partes: ORDEN_PARTES.map((id) => estadoParte(r.regla.partes.find((p) => p.id === id))),
    }))
    .sort((a, b) => (a.equipo + a.copia).localeCompare(b.equipo + b.copia));
}
