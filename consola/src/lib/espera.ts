// Órdenes esperando su turno, de todas las consolas (v1.49, docs/consolas-multiples.md §5).
//
// Las de esta consola las da su servidor (`GET /ordenes?pendientes=1`, se cancelan
// con `POST …/cancelar`). Las de las otras consolas del equipo llegan en su resumen
// (`en_espera`) y se cancelan con la orden `cancelar_espera` (inofensiva, sin clave).
// Aquí se juntan en una sola lista, sin repetir y sin la dirección de ninguna consola.
import type * as T from "./tipos";
import { mensajeOrden } from "./textosEquipo";

export interface FilaEspera {
  id: string;
  tipo: string;
  equipo: string;
  equipoNombre: string;
  /** Qué hace, como lo dice el equipo (sin rutas ni secretos), si lo dice. */
  descripcion: string | null;
  /** Desde qué consola, si no es esta (su nombre en el equipo, o «otra consola»). */
  otraConsola: string | null;
  /** Quién la mandó. */
  por: string | null;
  /** Cuándo se aplicará (RFC 3339). */
  aplica: string;
  /** «servidor»: de esta consola (`cancelarOrden`); «orden»: de otra (`cancelar_espera`). */
  cancelar: "servidor" | "orden";
}

/** El nombre que se enseña de la consola que mandó una orden: el suyo en el equipo o, si no tiene, el de `resumen.consolas` con esa identidad. Nunca la dirección. */
export function nombreConsola(e: T.Equipo | undefined, c: T.OrdenEnEspera["consola"]): string {
  const propio = (c.nombre ?? "").trim();
  if (propio) return propio;
  const otra = e?.resumen?.consolas?.find((x) => x.identidad === c.identidad && !x.esta);
  // `resumen.consolas[].nombre` puede ser el nombre de su dirección: se usa solo si no lo parece.
  const n = (otra?.nombre ?? "").trim();
  return n && !/[.:/]/.test(n) ? n : "otra consola";
}

/** ¿Puede cancelarla esta consola? Las de otra, si el equipo lo admite (`cancelar_espera`). */
export const admiteCancelarEspera = (e: T.Equipo | undefined) => !!e?.resumen?.admite?.includes("ordenes_en_espera");

/**
 * Junta las de esta consola (`propias`, de su servidor) y las que los equipos dicen
 * tener en espera desde otras consolas, sin repetir (el equipo usa el mismo id) y sin
 * las que ya pasaron su caducidad. De la que antes se aplica a la que más tarda.
 */
export function filasEnEspera(propias: T.Orden[], equipos: T.Equipo[], ahora: number): FilaEspera[] {
  const porId = new Map(equipos.map((e) => [e.id, e]));
  const filas: FilaEspera[] = [];
  const vistos = new Set<string>();
  for (const o of propias) {
    const e = porId.get(o.equipo ?? "");
    const delEquipo = e?.resumen?.en_espera?.find((x) => x.id === o.id);
    vistos.add(o.id);
    filas.push({
      id: o.id,
      tipo: o.tipo,
      equipo: o.equipo ?? "",
      equipoNombre: e?.nombre ?? "un equipo",
      descripcion: delEquipo?.descripcion ?? null,
      otraConsola: null,
      por: o.emitida_por?.nombre ?? null,
      aplica: o.not_before ?? delEquipo?.aplica ?? "",
      cancelar: "servidor",
    });
  }
  for (const e of equipos) {
    for (const x of e.resumen?.en_espera ?? []) {
      if (vistos.has(x.id) || x.consola?.esta) continue;
      const caduca = Date.parse(x.caduca);
      if (!Number.isFinite(caduca) || caduca <= ahora) continue;
      vistos.add(x.id);
      filas.push({
        id: x.id,
        tipo: x.tipo,
        equipo: e.id,
        equipoNombre: e.nombre,
        descripcion: x.descripcion ?? null,
        otraConsola: nombreConsola(e, x.consola),
        por: x.por ?? null,
        aplica: x.aplica,
        cancelar: "orden",
      });
    }
  }
  return filas.sort((a, b) => (Date.parse(a.aplica) || 0) - (Date.parse(b.aplica) || 0));
}

/** El historial de órdenes que llegaron desde otras consolas (sin las de esta, por su identidad), de la más reciente a la más antigua. */
export function ordenesDeOtras(entradas: { equipo: T.Equipo; historial: T.EntradaHistorial[] }[], identidadPropia: string): (T.EntradaHistorial & { equipoNombre: string; equipoId: string })[] {
  const vistas = new Set<string>();
  const l: (T.EntradaHistorial & { equipoNombre: string; equipoId: string })[] = [];
  for (const { equipo, historial } of entradas) {
    for (const h of historial) {
      if (h.tipo !== "orden" || !h.identidad || h.identidad === identidadPropia || vistas.has(h.id)) continue;
      vistas.add(h.id);
      l.push({ ...h, equipoNombre: equipo.nombre, equipoId: equipo.id });
    }
  }
  return l.sort((a, b) => (Date.parse(b.hora) || 0) - (Date.parse(a.hora) || 0));
}

/** El resultado de una entrada «orden» del historial (su `resultado` no es el de las copias). */
export const resultadoOrden = (h: T.EntradaHistorial): string => String((h as { resultado?: unknown }).resultado ?? "");

/** Lo que pasó con una orden del historial, en palabras. */
export const RESULTADO_ORDEN: Record<string, { texto: string; tono: "ok" | "warn" | "bad" | "info" | "neutral" }> = {
  en_espera: { texto: "En espera", tono: "warn" },
  hecha: { texto: "Hecha", tono: "ok" },
  en_marcha: { texto: "En marcha", tono: "info" },
  fallida: { texto: "Fallida", tono: "bad" },
  rechazada: { texto: "Rechazada", tono: "bad" },
  cancelada: { texto: "Cancelada", tono: "info" },
  caducada: { texto: "Caducada", tono: "neutral" },
};

/**
 * v1.58: por qué una orden no se aplicó, en palabras (para «Órdenes»), o `null` si se
 * aplicó, sigue en camino o se canceló. Nunca desaparece sin decir nada: «Caducó sin
 * aplicarse» o «Rechazada: …» con el motivo.
 */
export function porQueNoSeAplico(o: Pick<T.Orden, "estado" | "mensaje" | "detalle" | "motivo"> & { tipo?: string }): string | null {
  // Sin «[ruta]» a la vista (el equipo quita las rutas de lo que manda).
  const m = mensajeOrden(o.tipo, (o.mensaje ?? "").trim());
  switch (o.estado) {
    case "caducada":
      return o.motivo === "sin_respuesta"
        ? "Caducó sin aplicarse: el equipo la recibió, pero no contestó a tiempo."
        : "Caducó sin aplicarse: no llegó al equipo a tiempo (estaba sin conexión, o esperando detrás de otra orden con espera).";
    case "rechazada":
      if (canceladaEnElEquipo(o)) return null;
      if (m.startsWith("Orden repetida o antigua"))
        return "Rechazada: el equipo ya había aceptado una orden posterior y descartó esta. Con agentes anteriores pasaba si se mandaba otra orden mientras esta esperaba su hora.";
      if (m.startsWith("Todavía no es la hora")) return "Rechazada: el reloj del equipo va atrasado y para él aún no era la hora. Pon el equipo en hora y vuelve a mandarla.";
      return m ? `Rechazada: ${m}` : "Rechazada por el equipo.";
    case "fallida":
      return m ? `No se pudo aplicar: ${m}` : "No se pudo aplicar.";
    default:
      return null;
  }
}

/** La que otra consola canceló en el equipo (v1.49): llega `rechazada` con `detalle.cancelada`. */
export function canceladaEnElEquipo(o: Pick<T.Orden, "estado" | "detalle">): boolean {
  if (o.estado !== "rechazada" || !o.detalle) return false;
  try {
    return (JSON.parse(o.detalle) as { cancelada?: unknown }).cancelada === true;
  } catch {
    return false;
  }
}

/**
 * ¿Ofrecer «Volver a mandar»? Las que no se aplicaron (caducada, rechazada, fallida), salvo
 * las que abren una sesión (se vuelven a abrir solas al usarlas) y las canceladas en el
 * equipo. Va sellada para el equipo, así que no se reenvía tal cual: se vuelve a pedir
 * desde el equipo, con su clave si hace falta.
 */
export function sePuedeVolverAMandar(o: Pick<T.Orden, "estado" | "tipo" | "detalle">): boolean {
  return ["caducada", "rechazada", "fallida"].includes(o.estado) && !["abrir_sesion", "explorar", "elegir_carpetas", "descargar", "cancelar_espera"].includes(o.tipo) && !canceladaEnElEquipo(o);
}
