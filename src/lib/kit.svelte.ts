// Kit de recuperación: qué destinos lo tienen guardado y la vista para
// imprimirlo (ver src-tauri/src/kit.rs).
import type { Repo } from "$lib/api";

/** Vista del kit abierta: destinos que incluye (null: todos). */
export const kitView = $state<{ open: boolean; ids: string[] | null }>({ open: false, ids: null });

export function openKit(ids: string[] | null = null) {
  kitView.ids = ids;
  kitView.open = true;
}

/**
 * Estado del kit de un destino:
 * - ok: guardado y para la ubicación actual;
 * - stale: guardado, pero la ubicación cambió (ya no sirve);
 * - missing: nunca se guardó.
 */
export function kitState(repo: Repo): "ok" | "stale" | "missing" {
  if (!repo.kit) return "missing";
  return repo.kit.location === repo.location ? "ok" : "stale";
}

const DISMISS_KEY = "resguardo:kit-recordatorio";

/** Recordatorio descartado en esta sesión (vuelve a salir al reabrir la app). */
export const kitReminder = $state<{ dismissed: boolean }>({ dismissed: readDismissed() });

function readDismissed() {
  try {
    return sessionStorage.getItem(DISMISS_KEY) === "1";
  } catch {
    return false;
  }
}

export function dismissKitReminder() {
  kitReminder.dismissed = true;
  try {
    sessionStorage.setItem(DISMISS_KEY, "1");
  } catch {
    /* sin almacenamiento: solo en esta vista */
  }
}
