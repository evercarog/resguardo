// Avisos breves ("toasts") en la esquina inferior derecha. Confirman acciones
// que terminan sin que haga falta mirar el panel donde se lanzaron.

export type ToastKind = "success" | "info" | "error";

export interface Toast {
  id: number;
  message: string;
  kind: ToastKind;
}

export const toasts = $state<Toast[]>([]);

let next = 0;
const timers = new Map<number, ReturnType<typeof setTimeout>>();

export function dismissToast(id: number) {
  clearTimeout(timers.get(id));
  timers.delete(id);
  const i = toasts.findIndex((t) => t.id === id);
  if (i >= 0) toasts.splice(i, 1);
}

/** Muestra un aviso. `timeout` 0 = no se cierra solo. */
export function toast(message: string, kind: ToastKind = "success", timeout = 4000) {
  const id = ++next;
  toasts.push({ id, message, kind });
  // Como mucho unos pocos a la vez: el más antiguo se va.
  if (toasts.length > 4) dismissToast(toasts[0].id);
  if (timeout > 0) timers.set(id, setTimeout(() => dismissToast(id), timeout));
  return id;
}
