// Duraciones de las transiciones de Svelte respetando «reducir movimiento»
// del sistema. El CSS ya lo hace en app.css; las transiciones de Svelte van
// por JavaScript y necesitan este ajuste aparte.
import { prefersReducedMotion } from "svelte/motion";

/** Devuelve `ms`, o 0 si el sistema pide reducir el movimiento. */
export const dur = (ms: number) => (prefersReducedMotion.current ? 0 : ms);
