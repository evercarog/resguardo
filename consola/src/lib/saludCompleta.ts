// «Salud de la protección» coherente con la regla 3-2-1-1-0 (plan 0.8.0, 14w).
//
// El agente calcula la salud con lo que él sabe (protection.rs): su propio
// repositorio, su copia externa de siempre y sus copias derivadas. Lo que solo
// sabe la consola lo completa aquí, con los mismos datos que la tira 3-2-1
// (los pasos de `reglaDeCopia`): los espejos del almacén y los del propio
// equipo, lo marcado por una persona en cada destino y la retención que aplica
// el almacén. Así no dice «todas las versiones están en un solo sitio» cuando
// el almacén lo espeja a la nube, ni «sin política» cuando la retención la
// aplica el almacén. Sin runas (lo prueban scripts/vectores-camino.ts).
import type { ComprobacionProteccion } from "./tipos";
import { proteccion, type Proteccion } from "./repo";
import { esFuera, pasoAlDia, type ReglaCopia } from "./regla321";
import { lista, relativo } from "./formato";

export const TITULO_FUERA = "Fuera del sitio";

/** 14w: «Fuera del sitio: alguna copia de estos datos está fuera del sitio o en la nube», con los pasos de la regla. */
export function fueraDelSitio(rc: ReglaCopia, ahora: number): ComprobacionProteccion {
  const fuera = rc.pasos.filter((p) => esFuera(p));
  const alDia = fuera.filter((p) => pasoAlDia(p, ahora));
  const nombres = (l: typeof fuera) => lista(l.map((p) => `«${p.nombre}»`));
  if (alDia.length) {
    const ultima = alDia.map((p) => p.ultima_ok ?? "").sort().at(-1);
    const propio = alDia.some((p) => p.tipo === "copia");
    return {
      id: "externa",
      estado: "ok",
      etiqueta: TITULO_FUERA,
      detalle: propio ? "El propio repositorio está fuera del sitio o en la nube." : `Al día: ${nombres(alDia)}${ultima ? ` (la última vez, ${relativo(ultima, ahora)})` : ""}.`,
    };
  }
  if (fuera.length)
    return { id: "externa", estado: "aviso", etiqueta: TITULO_FUERA, detalle: `${nombres(fuera)} ${fuera.length > 1 ? "están" : "está"} fuera del sitio, pero no ${fuera.length > 1 ? "están" : "está"} al día: revísalo.` };
  return {
    id: "externa",
    estado: "fallo",
    etiqueta: TITULO_FUERA,
    detalle: "Ninguna copia de estos datos está fuera del sitio ni en la nube: añade un espejo o una copia derivada a la nube o a otra sede.",
  };
}

/**
 * La salud del agente completada por la consola: «Fuera del sitio» con los
 * pasos de la regla (si el repositorio tiene una copia activa) y la retención
 * del almacén. `extra`: lo de siempre («Fuera de este equipo», lib/dondeGuarda).
 */
export function saludCompleta(
  inf: Parameters<typeof proteccion>[0],
  o: { extra?: ComprobacionProteccion | null; rc?: ReglaCopia | null; retencionAlmacen?: string | null; ahora: number },
): Proteccion | null {
  const p = proteccion(inf, o.extra);
  if (!p) return null;
  const items = p.items.map((i): ComprobacionProteccion => {
    if (i.id === "externa") return o.rc ? fueraDelSitio(o.rc, o.ahora) : { ...i, etiqueta: i.etiqueta === "Copia externa" ? TITULO_FUERA : i.etiqueta };
    if (i.id === "retencion" && i.estado !== "ok" && o.retencionAlmacen) return { ...i, estado: "ok", detalle: `La aplica el almacén «${o.retencionAlmacen}».` };
    return i;
  });
  const puntuacion = items.filter((i) => i.estado === "ok").length;
  const pendientes = items.length - puntuacion;
  return { puntuacion, total: items.length, items, ratio: puntuacion / Math.max(1, items.length), tono: items.some((i) => i.estado === "fallo") ? "bad" : pendientes ? "warn" : "ok", pendientes };
}
