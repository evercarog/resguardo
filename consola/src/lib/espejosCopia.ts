// Lo que cuelga del repositorio de una copia (espejos y copias derivadas),
// agrupado por quién lo hace: el almacén donde está el repositorio (sus
// espejos) o el propio equipo (sus copias derivadas y la copia externa).
// Lo usa la lista de copias del editor (plan 0.7.26, bloque 3.1: «debajo sus
// espejos y derivadas, agrupados por almacén»). Con vectores en
// scripts/vectores-editor-guiado.ts.
//
// TODO(bloque 4): cuando el agente mande `espejo.trabajos[]` (espejos como
// trabajos, varios por repositorio y hechos también por el propio equipo), que
// esta función los lea de ahí (el helper «espejos agrupados por almacén» de esa
// rama) en lugar de `pasosDelRepo`. La forma de lo que devuelve no cambia: el
// editor de copias solo pinta grupos con sus pasos.
import { pasosDelRepo, repoEnAlmacen, type PasoCadena } from "./cadenas";
import type { Equipo } from "./tipos";

export interface GrupoEspejos {
  /** Clave estable del grupo (`almacen:<id>` o `equipo:<id>`). */
  clave: string;
  /** Quién lo hace. */
  quien: { id: string; nombre: string; almacen: boolean };
  /** Sus pasos (espejos o copias derivadas), en el orden de siempre. */
  pasos: PasoCadena[];
}

/**
 * Espejos y derivadas del repositorio `repoId` de `equipo`, agrupados: primero
 * el almacén (sus espejos), después el propio equipo (derivadas y copia
 * externa). Vacío si no hay nada o el repositorio no está en el resumen.
 */
export function espejosDeCopia(equipo: Equipo, repoId: string, equipos: Equipo[]): GrupoEspejos[] {
  const pasos = pasosDelRepo(equipo, repoId, equipos).filter((p) => p.nivel > 0);
  if (!pasos.length) return [];
  const repo = equipo.resumen?.repositorios?.find((r) => r.id === repoId);
  const en = repo ? repoEnAlmacen(equipo, repo, equipos) : null;
  const grupos: GrupoEspejos[] = [];
  const espejos = pasos.filter((p) => p.clase === "espejo");
  if (espejos.length) {
    const a = en?.almacen;
    grupos.push({ clave: `almacen:${a?.id ?? "?"}`, quien: { id: a?.id ?? "", nombre: a?.nombre ?? "El almacén", almacen: true }, pasos: espejos });
  }
  const derivadas = pasos.filter((p) => p.clase !== "espejo");
  if (derivadas.length) grupos.push({ clave: `equipo:${equipo.id}`, quien: { id: equipo.id, nombre: equipo.nombre, almacen: false }, pasos: derivadas });
  return grupos;
}
