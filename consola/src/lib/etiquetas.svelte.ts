// Etiquetas de los equipos (v1.18): texto libre para agrupar y filtrar
// («Contabilidad», «Servidores»…). Se guardan en el servidor en claro: son
// metadatos, como el nombre del equipo. No confundir con la `etiqueta` (HMAC).
//
// Color: cada etiqueta tiene siempre el mismo, de una paleta de 7 pensada
// para distinguirse también con daltonismo (Okabe-Ito, con su versión para el
// tema oscuro en app.css: --et-0 … --et-6). El color va solo en el punto: el
// texto, en tinta neutra, es lo que se lee.
import { actual } from "./estado.svelte";
import { guardar, leerTexto } from "./recordar";
import type { Equipo } from "./tipos";

export const MAX_ETIQUETAS = 10;
export const MAX_LARGO = 32;
export const N_COLORES = 7;

/** El color de una etiqueta (índice de la paleta), igual en todas las pantallas. */
export function colorEtiqueta(nombre: string): number {
  let h = 0;
  for (const c of nombre.trim().toLowerCase()) h = (h * 31 + c.codePointAt(0)!) >>> 0;
  return h % N_COLORES;
}

/** Limpia una etiqueta como el servidor: espacios de más fuera; null si no vale. */
export function limpiarEtiqueta(t: string): string | null {
  const x = t.split(/\s+/).filter(Boolean).join(" ");
  if (!x || [...x].length > MAX_LARGO || /[\u0000-\u001f,]/.test(x)) return null;
  return x;
}

/** Todas las etiquetas del cliente, ordenadas, con cuántos equipos la llevan. */
export function etiquetasDe(equipos: Equipo[]): { nombre: string; n: number }[] {
  const m = new Map<string, { nombre: string; n: number }>();
  for (const e of equipos)
    for (const t of e.etiquetas ?? []) {
      const k = t.toLowerCase();
      const x = m.get(k) ?? { nombre: t, n: 0 };
      x.n++;
      m.set(k, x);
    }
  return [...m.values()].sort((a, b) => a.nombre.localeCompare(b.nombre, "es"));
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

/** ¿Pasa el equipo el filtro? (sin filtro, todos). */
export function pasaFiltro(e: Equipo | undefined, etiqueta: string): boolean {
  if (!etiqueta) return true;
  return !!e?.etiquetas?.some((t) => t.toLowerCase() === etiqueta.toLowerCase());
}
