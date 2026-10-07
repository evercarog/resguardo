// Órdenes que se siguen en segundo plano: si alguien cierra el diálogo antes
// de que el equipo responda, la consola sigue mirando y avisa (toast con
// enlace) cuando termina. Así nadie se queda sin saber cómo acabó.
// Solo lee el estado de la orden: no manda nada ni guarda secretos.
import * as api from "./api";
import { enFondo } from "./actividad.svelte";
import { avisar } from "./avisos.svelte";
import { actual, cargarCliente } from "./estado.svelte";
import { ESTADO_ORDEN, nombreOrden } from "./salud";
import { ultimaVuelta } from "./copia";
import { mensajeOrden } from "./textosEquipo";
import type { CopiaResumen, Orden } from "./tipos";

const FINALES = ["hecha", "fallida", "rechazada", "cancelada", "caducada"];
/** Las que se siguen ahora (por id), para no seguir dos veces la misma. */
const siguiendo = new Set<string>();
/** Cuánto se sigue como mucho (después, el resultado está en «Órdenes»). */
const MAXIMO = 15 * 60_000;

export function seguirEnFondo(cliente: string, equipo: { id: string; nombre: string }, orden: Orden, enlace?: { texto: string; href: string }) {
  if (siguiendo.has(orden.id) || FINALES.includes(orden.estado) || orden.not_before) return;
  siguiendo.add(orden.id);
  const inicio = Date.now();
  let espera = 2000;
  const mirar = async () => {
    try {
      const lista = await enFondo(() => api.ordenesEquipo(cliente, equipo.id, 20));
      const o = lista.find((x) => x.id === orden.id);
      if (o && FINALES.includes(o.estado)) {
        siguiendo.delete(orden.id);
        if (o.estado === "cancelada") return;
        const bien = o.estado === "hecha";
        const que = `«${nombreOrden(o.tipo)}» en ${equipo.nombre}`;
        const dijo = o.mensaje ? ` ${mensajeOrden(o.tipo, o.mensaje)}` : "";
        avisar(
          bien ? `${que}: hecha.${dijo}` : `${que}: ${ESTADO_ORDEN[o.estado].texto.toLowerCase()}.${dijo}`,
          bien ? "ok" : "bad",
          enlace ?? { texto: "Ver", href: `/c/${cliente}/equipos/${equipo.id}?tab=ordenes` },
        );
        if (actual.id === cliente) void cargarCliente(cliente, { silencioso: true });
        return;
      }
    } catch {
      /* se reintenta */
    }
    if (Date.now() - inicio > MAXIMO) {
      siguiendo.delete(orden.id);
      return;
    }
    espera = Math.min(espera * 1.5, 15_000);
    setTimeout(mirar, espera);
  };
  setTimeout(mirar, espera);
}

/**
 * «Copiar ahora» con el diálogo ya cerrado: espera a que el equipo informe de
 * esa vuelta (como mucho 6 min) y avisa de cómo terminó, con enlace a la copia.
 */
export function vigilarCopiaEnFondo(cliente: string, equipo: { id: string; nombre: string }, copia: CopiaResumen, desde: number) {
  const clave = `copia|${equipo.id}|${copia.id}|${desde}`;
  if (siguiendo.has(clave)) return;
  siguiendo.add(clave);
  const inicio = Date.now();
  const href = `/c/${cliente}/equipos/${equipo.id}/copias/${encodeURIComponent(copia.id)}`;
  const mirar = async () => {
    try {
      const e = await enFondo(() => api.equipo(cliente, equipo.id));
      const v = ultimaVuelta(copia, e.ultimo_informe);
      if (v && Date.parse(v.cuando) > desde) {
        siguiendo.delete(clave);
        const que = `Copia «${copia.nombre}» de ${equipo.nombre}`;
        const texto =
          v.resultado === "fallo"
            ? `${que}: falló.${v.mensaje ? ` ${v.mensaje}` : ""}`
            : v.resultado === "aviso"
              ? `${que}: terminó con avisos.`
              : v.resultado === "sin_cambios"
                ? `${que}: sin cambios, no hizo falta otra versión.`
                : `${que}: terminada, versión nueva guardada.`;
        avisar(texto, v.resultado === "fallo" ? "bad" : v.resultado === "aviso" ? "warn" : "ok", { texto: "Ver la copia", href });
        if (actual.id === cliente) void cargarCliente(cliente, { silencioso: true });
        return;
      }
    } catch {
      /* se reintenta */
    }
    if (Date.now() - inicio > 6 * 60_000) {
      siguiendo.delete(clave);
      return;
    }
    setTimeout(mirar, 5000);
  };
  setTimeout(mirar, 4000);
}
