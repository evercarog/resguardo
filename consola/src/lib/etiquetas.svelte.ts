// Etiquetas de los equipos (v1.18): texto libre para agrupar y filtrar
// («Contabilidad», «Servidores»…). Se guardan en el servidor en claro: son
// metadatos, como el nombre del equipo. No confundir con la `etiqueta` (HMAC).
//
// v1.52 (tarea 6): cada etiqueta puede tener sus ajustes en el servidor (color
// elegido, plantilla por defecto y avisos), que llegan con el resumen del
// cliente (`actual.etiquetas`). Lo que no depende de la pantalla está en
// etiquetasGrupos.ts (con sus pruebas).
import { actual } from "./estado.svelte";
import { colorDe } from "./etiquetasGrupos";
import { guardar, leerTexto } from "./recordar";

export { ajusteDe, colorPorNombre, etiquetasDe, gruposPorEtiqueta, mismaEtiqueta, N_COLORES, NOMBRES_COLOR, pasaFiltro, plantillasPropuestas } from "./etiquetasGrupos";

export const MAX_ETIQUETAS = 10;
export const MAX_LARGO = 32;

/** El color de una etiqueta (índice de la paleta), igual en todas las pantallas del cliente. */
export function colorEtiqueta(nombre: string): number {
  return colorDe(nombre, actual.etiquetas);
}

/** Limpia una etiqueta como el servidor: espacios de más fuera; null si no vale. */
export function limpiarEtiqueta(t: string): string | null {
  const x = t.split(/\s+/).filter(Boolean).join(" ");
  if (!x || [...x].length > MAX_LARGO || /[\u0000-\u001f,]/.test(x)) return null;
  return x;
}

/**
 * El filtro por etiqueta, compartido entre Equipos, Avisos y Estado (por
 * cliente). Se recuerda en este navegador: al volver, sigue puesto.
 */
const filtro = $state({ cliente: "", etiqueta: "" });

export const filtroEtiqueta = {
  get valor(): string {
    if (filtro.cliente !== actual.id) return actual.id ? leerTexto(`etiqueta.${actual.id}`) : "";
    return filtro.etiqueta;
  },
  poner(v: string) {
    filtro.cliente = actual.id;
    filtro.etiqueta = v;
    guardar(`etiqueta.${actual.id}`, v);
  },
};

/** «Agrupar por etiqueta» en Estado y Equipos (por cliente, recordado en este navegador). */
const agrupar = $state({ cliente: "", si: false });

export const agruparPorEtiqueta = {
  get valor(): boolean {
    if (agrupar.cliente !== actual.id) return actual.id ? leerTexto(`agrupar.${actual.id}`) === "si" : false;
    return agrupar.si;
  },
  poner(v: boolean) {
    agrupar.cliente = actual.id;
    agrupar.si = v;
    guardar(`agrupar.${actual.id}`, v ? "si" : "");
  },
};
