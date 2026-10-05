// Tras un 429 de los límites generales del servidor (peticiones por cuenta o por
// IP y minuto), la consola deja de preguntar (GET) el tiempo que dice
// `retry_after`, en vez de volver a intentarlo enseguida y alargar el bloqueo.
// Los límites de una acción concreta (contraseña, códigos para añadir equipos…)
// no paran nada más: solo esa acción. scripts/vectores-emparejar.ts lo prueba.

/** Límites que afectan a todas las peticiones de la cuenta o de la red. */
const GENERALES = new Set(["cuenta", "ip"]);
/** Sin `retry_after` (servidor anterior): esto. */
const POR_DEFECTO_S = 30;
/** Como mucho (un reloj raro no deja la consola parada). */
const MAXIMO_S = 120;

/** Hasta cuándo (ms, como Date.now()) no hacer más GET, o `null` si este error no pide pausa. */
export function pausaTras(estado: number, metodo: string, cuerpo: Record<string, unknown> | undefined, ahora = Date.now()): number | null {
  if (estado !== 429) return null;
  const limite = typeof cuerpo?.limite === "string" ? cuerpo.limite : null;
  // Un servidor anterior no dice cuál: si fue un GET, es el general (las rutas con límite propio son POST).
  if (limite ? !GENERALES.has(limite) : metodo !== "GET") return null;
  const s = Number(cuerpo?.retry_after);
  const espera = Number.isFinite(s) && s > 0 ? Math.min(s, MAXIMO_S) : POR_DEFECTO_S;
  return ahora + espera * 1000;
}

/** El texto mientras la consola está en pausa. */
export function mensajePausa(hasta: number, ahora = Date.now()): string {
  const s = Math.max(1, Math.ceil((hasta - ahora) / 1000));
  return `El servidor pidió esperar: demasiadas peticiones de esta cuenta o red en el último minuto (por seguridad hay un máximo). La consola vuelve a preguntar en ${s} s.`;
}
