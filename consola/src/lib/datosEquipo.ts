// v1.4x: lo que el equipo comparte con todas sus consolas y lo que es de cada una
// (docs/consolas-multiples.md §6), y lo que hace falta para quitar un destino que
// ya no se usa. Sin runas: se prueba en scripts/vectores-datos-equipo.ts.
//
// - **Nombre, etiquetas y observación del equipo**: con un agente que lo admite
//   (`admite: "datos_equipo"`), los guarda el equipo (órdenes inofensivas
//   `nombre_equipo`, `etiquetas_equipo`, `observacion_equipo`) y cada consola enseña
//   su valor (`resumen.datos_equipo`). Con uno anterior, como siempre: cada consola
//   el suyo.
// - **Quitar un destino** (`quitar_destino`): solo si ya no lo usa nada en el equipo.
import type { DestinoResumen, Equipo, RepositorioResumen } from "./tipos";
import { destinoDe } from "./repo";

export const ADMITE_DATOS_EQUIPO = "datos_equipo";
export const ADMITE_QUITAR_DESTINO = "quitar_destino";

/** ¿El equipo guarda su nombre, etiquetas y observación para todas sus consolas? */
export const admiteDatosEquipo = (e: Pick<Equipo, "resumen"> | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_DATOS_EQUIPO);
/** ¿Sabe olvidar un destino sin uso (`quitar_destino`, `quitar_repositorio { quitar_destino }`)? */
export const admiteQuitarDestino = (e: Pick<Equipo, "resumen"> | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_QUITAR_DESTINO);

/** ¿Tiene el equipo más de una consola? (Si no, no hace falta decir «en todas sus consolas».) */
export const variasConsolas = (e: Pick<Equipo, "resumen"> | null | undefined) => (e?.resumen?.consolas?.length ?? 0) > 1;

/**
 * El equipo con el nombre y las etiquetas que tiene él puestos (si los tiene). El
 * servidor nuevo ya los copia al recibir el resumen; esto es para un servidor
 * anterior, que no lo hace (y para que la consola no enseñe dos cosas a la vez).
 */
export function conDatosDelEquipo<E extends Equipo>(e: E): E {
  const d = e.resumen?.datos_equipo;
  if (!d) return e;
  const nombre = typeof d.nombre?.valor === "string" && d.nombre.valor.trim() ? d.nombre.valor.trim() : null;
  const etiquetas = Array.isArray(d.etiquetas?.valor) && d.etiquetas.valor.every((x) => typeof x === "string") ? d.etiquetas.valor : null;
  if ((nombre === null || nombre === e.nombre) && (etiquetas === null || JSON.stringify(etiquetas) === JSON.stringify(e.etiquetas ?? []))) return e;
  return { ...e, ...(nombre !== null ? { nombre } : {}), ...(etiquetas !== null ? { etiquetas } : {}) };
}

/** «Lo cambió Ana desde la consola «Oficina»», o null si no lo dice el equipo o lo cambió esta. */
export function cambiadoDesde(campo: { consola?: string | null; esta?: boolean; por?: string | null } | null | undefined): string | null {
  if (!campo || campo.esta) return null;
  const consola = campo.consola?.trim() ? `la consola «${campo.consola.trim()}»` : "otra consola";
  return campo.por?.trim() ? `Lo cambió ${campo.por.trim()} desde ${consola}` : `Cambiado desde ${consola}`;
}

/** Qué hace un cambio de nombre o etiquetas en esta consola, en una frase (para el diálogo). */
export function alcanceDatos(e: Pick<Equipo, "resumen">): string {
  if (!admiteDatosEquipo(e)) return variasConsolas(e) ? "Solo en esta consola: su agente aún no guarda estos datos para todas (actualízalo)." : "";
  return variasConsolas(e) ? "Se guarda en el equipo: lo verán igual todas sus consolas." : "Se guarda en el equipo (y en las consolas que lo gestionen).";
}

/** Lo que usa un destino del equipo (en su resumen): sus repositorios, copias externas y derivadas. */
export function usosDestino(e: Pick<Equipo, "resumen">, destinoId: string): string[] {
  const destinos = e.resumen?.destinos ?? [];
  const usos: string[] = [];
  for (const r of e.resumen?.repositorios ?? []) {
    if (destinoDe(destinos, r)?.id === destinoId) usos.push(`el repositorio «${r.nombre}»`);
    if (r.externa && idDestinoExterna(destinos, r.externa) === destinoId) usos.push(`la copia externa de «${r.nombre}»`);
    if ((r.derivadas ?? []).some((d) => d.destino_id === destinoId)) usos.push(`una copia derivada de «${r.nombre}»`);
  }
  return usos;
}

/** El id del destino de una copia externa (el agente da `destino_id`; uno anterior, solo su nombre). */
function idDestinoExterna(destinos: DestinoResumen[], x: NonNullable<RepositorioResumen["externa"]>): string | null {
  return x.destino_id ?? destinos.find((d) => d.nombre === x.destino)?.id ?? null;
}

/** ¿Se puede ofrecer «Quitar este destino» en este equipo? (Sin uso y con un agente que sabe.) */
export const destinoQuitable = (e: Pick<Equipo, "resumen">, destinoId: string) => admiteQuitarDestino(e) && usosDestino(e, destinoId).length === 0;

/**
 * Al quitar un repositorio: el destino que se quedaría sin uso (para ofrecer quitarlo
 * también), o null si lo usa algo más o el agente no sabe hacerlo.
 */
export function destinoQueQuedaVacio(e: Pick<Equipo, "resumen">, repo: RepositorioResumen): DestinoResumen | null {
  if (!admiteQuitarDestino(e)) return null;
  const d = destinoDe(e.resumen?.destinos, repo);
  if (!d) return null;
  const otros = usosDestino(e, d.id).filter((u) => u !== `el repositorio «${repo.nombre}»`);
  return otros.length ? null : d;
}

export type ProblemaPaso = "sin_destino" | "mismo_equipo";

/**
 * ¿Algo raro en el destino de una copia externa o derivada? `sin_destino`: el equipo
 * ya no lo tiene (se quitó); `mismo_equipo`: una carpeta del propio equipo (ni
 * extraíble ni de la red): no protege si el equipo se pierde.
 */
export function problemaDestinoPaso(e: Pick<Equipo, "resumen">, destinoId: string | null | undefined, nombre?: string | null): ProblemaPaso | null {
  const destinos = e.resumen?.destinos ?? [];
  const d = (destinoId ? destinos.find((x) => x.id === destinoId) : undefined) ?? (nombre ? destinos.find((x) => x.nombre === nombre) : undefined);
  // Un agente que no dice ni el id ni un nombre que se encuentre: no se sabe (nada que avisar).
  if (!d) return destinoId ? "sin_destino" : null;
  if (d.tipo === "local" && !d.red && d.extraible !== true) return "mismo_equipo";
  return null;
}

export const TEXTO_PROBLEMA_PASO: Record<ProblemaPaso, string> = {
  sin_destino: "Su destino ya no está en el equipo: no se hace. Quítala o vuelve a crearla con otro destino.",
  mismo_equipo: "Va a una carpeta de este mismo equipo: si el equipo se daña o lo cifra un ransomware, se pierde con él.",
};
