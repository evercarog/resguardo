// Plan 0.8.1, B5 (15pp): el nombre de un equipo, igual en todas sus consolas.
//
// El nombre que se pone al preparar el alta (o el que tiene en esta consola
// cuando se conecta otra) se quedaba solo aquí: una consola que se conectaba
// después tomaba el de Windows. Ahora se manda también al equipo con su orden
// `nombre_equipo` (v1.56) si el agente la admite y aún no tiene uno puesto.
import * as api from "./api";
import { mandarOrden } from "./ordenar";
import { admiteDatosEquipo } from "./datosEquipo";
import type * as T from "./tipos";

/** El nombre que el propio equipo tiene puesto para todas sus consolas, si lo tiene. */
export function nombreDelEquipo(e: Pick<T.Equipo, "resumen">): string | null {
  const v = e.resumen?.datos_equipo?.nombre?.valor;
  return typeof v === "string" && v.trim() ? v.trim() : null;
}

/**
 * ¿El nombre de esta consola es solo de esta consola? (El agente guarda el nombre para
 * todas, pero aún no tiene ninguno puesto: otra consola puede llamarlo distinto.)
 */
export const nombreSoloAqui = (e: Pick<T.Equipo, "resumen" | "nombre" | "confirmado">) => e.confirmado !== false && admiteDatosEquipo(e) && nombreDelEquipo(e) === null && !!e.nombre.trim();

/** Los equipos (de una lista) cuyo nombre en esta consola difiere del que tiene puesto el equipo, o no tiene. */
export const nombresPorFijar = <E extends Pick<T.Equipo, "resumen" | "nombre" | "confirmado">>(xs: E[]) => xs.filter((e) => nombreSoloAqui(e));

/** Manda el nombre de esta consola al equipo (sin esperar a que conteste). */
export async function fijarNombre(cliente: T.Cliente, equipo: T.Equipo, nombre = equipo.nombre): Promise<void> {
  await mandarOrden({ cliente, equipo, tipo: "nombre_equipo", cuerpo: { nombre: nombre.trim() } });
}

/**
 * Tras el alta: espera (hasta `segundos`) a que el equipo diga lo que admite y, si guarda el
 * nombre para todas sus consolas y aún no tiene uno, le manda el de esta. Si el agente es
 * anterior, no hace nada (el nombre sigue siendo de esta consola, como siempre).
 */
export async function fijarNombreTrasAlta(cliente: T.Cliente, equipoId: string, segundos = 90): Promise<boolean> {
  for (let i = 0; i < segundos / 3; i++) {
    let e: T.Equipo;
    try {
      e = await api.equipo(cliente.id, equipoId);
    } catch {
      await new Promise((r) => setTimeout(r, 3000));
      continue;
    }
    if (e.resumen?.admite) {
      if (!nombreSoloAqui(e)) return false;
      await fijarNombre(cliente, e);
      return true;
    }
    await new Promise((r) => setTimeout(r, 3000));
  }
  return false;
}
