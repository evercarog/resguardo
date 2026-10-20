// «Ver las bases de la instancia» (15u): la petición al equipo (ver basesSql.ts).
import * as api from "./api";
import { aB64, deB64, deUtf8 } from "./cripto/bytes";
import { abrir, parEfimero } from "./cripto/sobre";
import { mandarOrden } from "./ordenar";
import type { Cliente, Equipo } from "./tipos";
import { leerLista, type BaseInstancia } from "./basesSql";

/** Pide la lista al equipo y espera su respuesta (como mucho unos 2 min). */
export async function pedirBases(cliente: Cliente, equipo: Equipo, instancia: string, alPaso?: (t: string) => void): Promise<BaseInstancia[]> {
  const eph = parEfimero();
  const o = await mandarOrden({ cliente, equipo, tipo: "bases_sqlserver", cuerpo: { instancia: instancia.trim() || "." }, responderA: aB64(eph.publica), alPaso });
  alPaso?.(`Esperando a ${equipo.nombre}…`);
  for (let i = 0; i < 80; i++) {
    const x = (await api.ordenesEquipo(cliente.id, equipo.id, 10)).find((y) => y.id === o.id);
    if (x && ["fallida", "rechazada", "cancelada", "caducada"].includes(x.estado)) throw new Error(x.mensaje ?? `${equipo.nombre} no pudo leer las bases.`);
    if (x?.estado === "hecha") {
      const sellado = x.detalle ? (JSON.parse(x.detalle) as { sellado?: string }).sellado : undefined;
      if (!sellado) throw new Error("La respuesta no trae la lista.");
      return leerLista(JSON.parse(deUtf8(abrir(eph.secreta, deB64(sellado)))));
    }
    await new Promise((r) => setTimeout(r, 1500));
  }
  throw new Error(`${equipo.nombre} no ha respondido todavía. Vuelve a intentarlo en un momento.`);
}
