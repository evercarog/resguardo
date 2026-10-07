// El catálogo de destinos de cada cliente (tarea 7a), cargado una vez por
// cliente y vuelto a pedir al cambiarlo. Un servidor anterior no lo tiene
// (404): entonces está vacío y los destinos salen solo de los equipos.
import * as api from "./api";
import type { AtributosDestino, DestinoCatalogo } from "./tipos";

const porCliente = $state<Record<string, DestinoCatalogo[]>>({});
const pidiendo = new Map<string, Promise<void>>();

/** El catálogo ya cargado de un cliente (vacío mientras no llega). */
export const catalogoDe = (c: string | null | undefined): DestinoCatalogo[] => (c ? (porCliente[c] ?? []) : []);

/** Lo carga (una vez; con `forzar`, otra vez). Nunca falla: sin catálogo, vacío. */
export function cargarCatalogo(c: string, forzar = false): Promise<void> {
  if (!forzar && (c in porCliente || pidiendo.has(c))) return pidiendo.get(c) ?? Promise.resolve();
  const p = api
    .destinosCatalogo(c)
    .then((l) => void (porCliente[c] = l))
    .catch(() => void (porCliente[c] ??= []))
    .finally(() => pidiendo.delete(c));
  pidiendo.set(c, p);
  return p;
}

/**
 * Pone (o cambia) el nombre de un destino, o crea uno suelto. Tarea 8: con
 * `atributos` (o `null` para quitarlos) cambia también lo de la regla 3-2-1;
 * sin el campo, el servidor deja los que había.
 */
export async function guardarEnCatalogo(c: string, id: string, d: { nombre: string; tipo: DestinoCatalogo["tipo"]; donde?: string | null; atributos?: AtributosDestino | null }) {
  await api.ponerDestino(c, id, { nombre: d.nombre.trim(), tipo: d.tipo, ...(d.donde ? { donde: d.donde.trim() } : {}), ...(d.atributos !== undefined ? { atributos: d.atributos } : {}) });
  await cargarCatalogo(c, true);
  repartir();
}

/** 0.7.26 (bloque 8): el nombre va a las demás consolas (lo que no pide clave se manda solo). */
function repartir() {
  void import("./datosComunes.svelte").then((m) => m.sincronizar()).catch(() => {});
}

/** Lo quita del catálogo (vuelve al nombre de siempre; no toca ningún equipo). */
export async function quitarDelCatalogo(c: string, id: string) {
  await api.borrarDestino(c, id);
  await cargarCatalogo(c, true);
  repartir();
}
