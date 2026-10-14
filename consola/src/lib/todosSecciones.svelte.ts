// Plan 0.8.0, D4: lo que las secciones de «Todos los clientes» piden aparte del panel
// (avisos, órdenes con espera de esta consola y restauraciones), con una sola petición
// para todos; con un servidor anterior (404), cliente a cliente (solo los de la cuenta).
import * as api from "./api";
import { todos } from "./todos.svelte";
import type { PanelDe } from "./global";

async function deTodos<X>(agregado: () => Promise<PanelDe<X>>, deUno: (c: string) => Promise<X>): Promise<PanelDe<X>> {
  try {
    return await agregado();
  } catch (e) {
    if (!(e instanceof api.ApiError && e.estado === 404)) throw e;
  }
  const clientes = await Promise.all(todos.clientes.map(async (c) => ({ id: c.id, nombre: c.nombre, rol: c.rol, datos: await deUno(c.id) })));
  return { generado: new Date().toISOString(), clientes, omitidos: 0 };
}

export const avisosDeTodos = () => deTodos(api.panelAvisos, (c) => api.avisos(c, true));
export const ordenesDeTodos = () => deTodos(api.panelOrdenes, (c) => api.ordenesPendientes(c));
export const restauracionesDeTodos = () => deTodos(api.panelRestauraciones, (c) => api.restauraciones(c));
