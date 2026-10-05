// El último informe de cada equipo del cliente, para las páginas que listan
// repositorios de varios equipos (versiones, tamaño, verificación…): el
// resumen de un agente anterior no los trae. Una sola petición (GET
// …/informes, v1.8); con un servidor anterior (404), uno por equipo.
import * as api from "./api";
import type { Informe } from "./tipos";

export const ultimos = $state<{ cliente: string; porEquipo: Record<string, Informe | null> }>({ cliente: "", porEquipo: {} });

let cargando = "";

/** Vuelve a pedir los últimos informes del cliente (ya cargados): algo cambió. */
export async function recargarInformes(cliente: string) {
  if (!cliente || ultimos.cliente !== cliente) return;
  try {
    const xs = await api.ultimosInformes(cliente);
    if (ultimos.cliente !== cliente) return;
    const m: Record<string, Informe | null> = { ...ultimos.porEquipo };
    for (const x of xs) m[x.equipo] = { recibido: x.recibido, datos: x.datos };
    ultimos.porEquipo = m;
  } catch {
    /* sin conexión o un servidor anterior (404): se queda lo que había */
  }
}

export async function cargarInformes(cliente: string, equipos: string[]) {
  const clave = `${cliente}|${equipos.join(",")}`;
  if (!cliente || clave === cargando) return;
  cargando = clave;
  if (ultimos.cliente !== cliente) ultimos.porEquipo = {};
  ultimos.cliente = cliente;
  try {
    const xs = await api.ultimosInformes(cliente);
    const m: Record<string, Informe | null> = {};
    for (const x of xs) m[x.equipo] = { recibido: x.recibido, datos: x.datos };
    if (ultimos.cliente === cliente) ultimos.porEquipo = m;
  } catch (err) {
    if (!(err instanceof api.ApiError && err.estado === 404)) {
      cargando = "";
      return;
    }
    for (const id of equipos)
      api
        .equipo(cliente, id)
        .then((x) => ultimos.cliente === cliente && (ultimos.porEquipo[id] = x.ultimo_informe ?? null))
        .catch(() => {});
  }
}
