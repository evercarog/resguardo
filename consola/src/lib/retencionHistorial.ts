// Las vueltas de la retención del historial de un equipo («Retención en
// detalle»; aparte de retencionDetalle.ts, que no habla con el servidor y se
// prueba con `npm run test:vectores`).
import * as api from "./api";
import type { EntradaRetencion } from "./retencionDetalle";

/**
 * Las entradas `retencion` del historial de un equipo (v1.45: solo se dan
 * pedidas con `tipo`). Un servidor que no las conoce contesta 422 (tipo no
 * válido) o 404 (sin historial): ninguna.
 */
export async function leerRetenciones(c: string, e: string): Promise<EntradaRetencion[]> {
  try {
    const x = await api.historialEquipo(c, e, { tipo: ["retencion"], limite: 2000 });
    return (x as unknown as EntradaRetencion[]).filter((y) => y.tipo === "retencion");
  } catch (err) {
    if (err instanceof api.ApiError && (err.estado === 422 || err.estado === 404)) return [];
    throw err;
  }
}
