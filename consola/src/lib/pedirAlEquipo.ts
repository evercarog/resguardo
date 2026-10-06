// Una orden inofensiva y corta al equipo (cambiar su nombre, sus etiquetas o su
// observación, quitar un destino sin uso) sin el diálogo de las órdenes: se manda,
// se espera un poco a que conteste y se dice qué pasó. Si tarda (sin conexión), se
// queda pedida: el equipo la aplica al volver y todas sus consolas lo ven entonces.
import * as api from "./api";
import { mandarOrden } from "./ordenar";
import type * as T from "./tipos";

const FINALES = ["hecha", "fallida", "rechazada", "cancelada", "caducada"];

export interface Respuesta {
  /** Ya aplicada en el equipo. */
  hecha: boolean;
  /** Lo que se le dice a la persona. */
  texto: string;
}

export async function pedirAlEquipo(cliente: T.Cliente, equipo: T.Equipo, tipo: string, cuerpo: Record<string, unknown>, segundos = 20): Promise<Respuesta> {
  const o = await mandarOrden({ cliente, equipo, tipo, cuerpo });
  for (let i = 0; i < segundos / 1.5; i++) {
    await new Promise((r) => setTimeout(r, 1500));
    const x = (await api.ordenesEquipo(cliente.id, equipo.id, 10)).find((y) => y.id === o.id);
    if (x && FINALES.includes(x.estado)) {
      if (x.estado === "hecha") return { hecha: true, texto: x.mensaje ?? "Hecho." };
      throw new Error(x.mensaje ?? `${equipo.nombre} no pudo hacerlo.`);
    }
  }
  return { hecha: false, texto: `Pedido a ${equipo.nombre}: se aplicará en cuanto conteste, y entonces lo verán todas sus consolas.` };
}
