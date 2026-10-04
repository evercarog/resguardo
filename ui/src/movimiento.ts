// Duraciones de las transiciones de Svelte respetando «reducir movimiento».
import { prefersReducedMotion } from "svelte/motion";

/** Devuelve `ms`, o 0 si el sistema pide reducir el movimiento. */
export const dur = (ms: number) => (prefersReducedMotion.current ? 0 : ms);
