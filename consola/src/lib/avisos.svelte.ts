// Avisos breves (toasts): abajo a la derecha, 4 s (8 s los errores y los que
// llevan acción), con aria-live.
export type Tono = "ok" | "bad" | "info" | "warn";

export interface Toast {
  id: number;
  tono: Tono;
  texto: string;
  /** Una acción: un enlace (`href`) o algo que hacer. */
  accion?: { texto: string; hacer?: () => void; href?: string };
}

export const toasts = $state<Toast[]>([]);
let siguiente = 1;

export function avisar(texto: string, tono: Tono = "ok", accion?: Toast["accion"]) {
  const id = siguiente++;
  toasts.push({ id, tono, texto, accion });
  // Con una acción, algo más de tiempo para pulsarla.
  setTimeout(() => cerrarToast(id), tono === "bad" || accion ? 8000 : 4000);
}

export function cerrarToast(id: number) {
  const i = toasts.findIndex((t) => t.id === id);
  if (i >= 0) toasts.splice(i, 1);
}

export const fallo = (e: unknown) => avisar(e instanceof Error ? e.message : String(e), "bad");
