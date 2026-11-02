// 16h: las marcas «Este equipo no llega a esa consola» del cliente abierto (de esta consola).
// Un servidor anterior no las tiene (404): sin marcas, todo como antes.
import * as api from "./api";
import type { MarcasNoLlega } from "./consolasCliente";

export const marcasNoLlega = $state<{ cliente: string; datos: MarcasNoLlega }>({ cliente: "", datos: {} });

export async function cargarNoLlega(cliente: string): Promise<void> {
  try {
    const d = await api.noLlegaCliente(cliente);
    marcasNoLlega.cliente = cliente;
    marcasNoLlega.datos = d && typeof d === "object" ? d : {};
  } catch {
    marcasNoLlega.cliente = cliente;
    marcasNoLlega.datos = {};
  }
}

/** Pone o quita la marca y deja al día lo de aquí. */
export async function marcarNoLlega(cliente: string, equipo: string, consola: string, motivo: string | null, quitar = false): Promise<void> {
  const d = await api.ponerNoLlega(cliente, { equipo, consola, motivo: motivo?.trim() || null, quitar });
  marcasNoLlega.cliente = cliente;
  marcasNoLlega.datos = d && typeof d === "object" ? d : {};
}
