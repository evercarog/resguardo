// Textos que manda el equipo, listos para enseñarlos en la consola.
//
// El agente quita las rutas de lo que manda al servidor (privacidad) y pone
// «[ruta]» en su lugar (`web::public_message`). Ese marcador no debe verse:
// donde la ruta solo decía «dónde» (« en [ruta].», «: [ruta]» al final) se
// quita; si queda alguno en mitad de la frase, se deja «…».
//
//   npm run test:vectores (scripts/vectores-textos-equipo.ts)

const MARCA = "\\[ruta\\]";
const UNA = `«?${MARCA}»?`;
/** « en [ruta]», « en «[ruta]», «[ruta]»» al final de una frase. */
const EN_RUTA = new RegExp(`\\s+en\\s+${UNA}(?:\\s*,\\s*${UNA})*\\s*(?=[.;]|$)`, "g");
/** «…: [ruta]» al final de una frase. */
const DOS_PUNTOS_RUTA = new RegExp(`\\s*:\\s*${UNA}\\s*(?=[.;]|$)`, "g");
const SUELTA = new RegExp(UNA, "g");

/** Un mensaje del equipo sin marcadores «[ruta]» a la vista. */
export function sinMarcadores(m: string): string {
  if (!m.includes("[ruta]")) return m;
  return m
    .replace(EN_RUTA, "")
    .replace(DOS_PUNTOS_RUTA, "")
    .replace(SUELTA, "«…»")
    .replace(/\s+([.,;:])/g, "$1")
    .replace(/\s{2,}/g, " ")
    .trim();
}

/** Lo que se enseña cuando el equipo no puede restaurar una unidad entera. */
export const UNIDAD_ENTERA = "No se puede restaurar una unidad entera junto al original. Elige las carpetas de dentro.";

/**
 * El resultado de «restaurar» para la persona. `carpetaAbajo`: la consola ya
 * enseña debajo la carpeta donde quedó (calculada en el navegador).
 * Compatible con los agentes anteriores (que mandan la ruta quitada) y los nuevos.
 */
export function textoRestaurar(m: string, carpetaAbajo = false): string {
  const t = m.trim();
  // Agentes anteriores: «… una unidad o la raíz entera: [ruta]»; nuevos: «… una unidad entera …».
  if (/^No se puede restaurar una unidad\b/i.test(t)) return UNIDAD_ENTERA;
  const hecho = /^Restaurado \((\d+) elementos?\)/.exec(t);
  if (hecho) {
    const n = Number(hecho[1]);
    return `Restaurado (${n} ${n === 1 ? "elemento" : "elementos"})${carpetaAbajo ? " en la carpeta de abajo" : ""}.`;
  }
  return sinMarcadores(t);
}

/** La respuesta del equipo a una orden, sin marcadores (la de «restaurar», con su texto propio). */
export function mensajeOrden(tipo: string | null | undefined, m: string): string {
  return tipo === "restaurar" ? textoRestaurar(m) : sinMarcadores(m);
}
