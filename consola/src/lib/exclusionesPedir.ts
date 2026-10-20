// 15y: la vista previa de «No copiar» (orden `vista_exclusiones`, con la clave de
// administración: la prueba que ya calculó el editor de copias al abrirse). La respuesta
// llega sellada para este navegador (ver exclusionesCopia.ts).
import * as api from "./api";
import { aB64, deB64, deUtf8 } from "./cripto/bytes";
import { abrir, parEfimero } from "./cripto/sobre";
import { mandarOrden } from "./ordenar";
import type { Cliente, Equipo } from "./tipos";
import { leerVista, type VistaPrevia } from "./exclusionesCopia";

/** Pide la vista previa y espera la respuesta (el equipo tiene como mucho 60 s; aquí, unos 2 min). */
export async function pedirVista(cliente: Cliente, equipo: Equipo, cuerpo: Record<string, unknown>, prueba: Uint8Array, alPaso?: (t: string) => void): Promise<VistaPrevia> {
  const eph = parEfimero();
  const o = await mandarOrden({ cliente, equipo, tipo: "vista_exclusiones", cuerpo, secretos: { prueba }, responderA: aB64(eph.publica), alPaso });
  alPaso?.(`${equipo.nombre} está revisando las carpetas…`);
  for (let i = 0; i < 80; i++) {
    const x = (await api.ordenesEquipo(cliente.id, equipo.id, 10)).find((y) => y.id === o.id);
    if (x && ["fallida", "rechazada", "cancelada", "caducada"].includes(x.estado)) throw new Error(x.mensaje ?? `${equipo.nombre} no pudo revisar las carpetas.`);
    if (x?.estado === "hecha") {
      const sellado = x.detalle ? (JSON.parse(x.detalle) as { sellado?: string }).sellado : undefined;
      if (!sellado) throw new Error("La respuesta no trae la vista previa.");
      return leerVista(JSON.parse(deUtf8(abrir(eph.secreta, deB64(sellado)))));
    }
    await new Promise((r) => setTimeout(r, 1500));
  }
  throw new Error(`${equipo.nombre} no ha respondido todavía. Vuelve a intentarlo en un momento.`);
}
