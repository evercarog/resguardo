// Las pruebas funcionales de cada cliente (plan 0.8.0, E3, 14k), cargadas una vez por
// cliente y vueltas a pedir al cambiarlas. Las usan la regla 3-2-1-1-0 (el «0») en
// todas las páginas, la página «Pruebas funcionales» e Informes. Un servidor anterior
// no las tiene (404): entonces `null` y la consola no ofrece registrarlas.
import * as api from "./api";
import type { PruebaFuncional, PruebasFuncionales } from "./tiposPruebas";

const porCliente = $state<Record<string, PruebasFuncionales | null>>({});
/** El último fallo al pedirlas (que no sea un servidor anterior): la página lo enseña con «Reintentar». */
const errores = $state<Record<string, string>>({});
const pidiendo = new Map<string, Promise<void>>();

/** Lo ya cargado de un cliente (`undefined` mientras no llega; `null` con un servidor anterior). */
export const funcionalesDe = (c: string | null | undefined): PruebasFuncionales | null | undefined => (c ? porCliente[c] : undefined);

/** Solo la lista (vacía mientras no llega): lo que necesita la regla. */
export const listaFuncionales = (c: string | null | undefined): PruebaFuncional[] => (c ? (porCliente[c]?.pruebas ?? []) : []);

/** Por qué no se pudieron cargar (vacío si no falló). */
export const errorFuncionales = (c: string | null | undefined): string => (c ? (errores[c] ?? "") : "");

/** La carga (una vez; con `forzar`, otra vez). Nunca falla: sin datos, se queda como estaba y el fallo queda en `errorFuncionales`. */
export function cargarFuncionales(c: string, forzar = false): Promise<void> {
  if (!c) return Promise.resolve();
  if (!forzar && (c in porCliente || pidiendo.has(c))) return pidiendo.get(c) ?? Promise.resolve();
  const p = api
    .pruebasFuncionales(c)
    .then((x) => {
      porCliente[c] = x;
      delete errores[c];
    })
    .catch((e: unknown) => void (errores[c] = (e as Error).message || "Sin respuesta del servidor."))
    .finally(() => pidiendo.delete(c));
  pidiendo.set(c, p);
  return p;
}

/** Lo que devuelve el servidor tras un cambio (ya trae todo). */
export function ponerFuncionales(c: string, x: PruebasFuncionales) {
  porCliente[c] = x;
}
