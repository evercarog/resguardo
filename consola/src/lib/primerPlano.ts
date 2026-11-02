// 16l: ¿una petición es de primer plano (enciende la barra de arriba) o de fondo? Sin runas,
// para probarlo en scripts/vectores.ts (lo usa lib/actividad.svelte.ts).

/** Cuánto dura lo que la persona hizo (cambiar de pantalla, pulsar, escribir) como «está esperando». */
export const VENTANA_ACCION_MS = 4000;

/**
 * Lo marcado con `enFondo` es de fondo. Lo demás cuenta como de primer plano si la persona hizo
 * algo hace poco o si ya hay otra petición de primer plano en marcha (la misma carga, en serie);
 * si no, es un refresco automático (un temporizador, un cambio en vivo) aunque no lo marcara nadie.
 */
export function esPrimerPlano(o: { enFondo: boolean; ahora: number; ultimaAccion: number; enMarcha: number }): boolean {
  if (o.enFondo) return false;
  return o.ahora - o.ultimaAccion < VENTANA_ACCION_MS || o.enMarcha > 0;
}
