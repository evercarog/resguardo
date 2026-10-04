// Acciones que quedan pendientes al reabrir Resguardo como administrador:
// se guardan antes de reiniciar y la nueva ventana las retoma al arrancar.
// También sirve para pedir, desde fuera, que se abra el editor de un panel.

const KEY = "resguardo:pendiente";

/**
 * - schedule: abrir el editor de copias automáticas de un destino.
 * - apply: abrir la copia `planId` y programarla en el agente.
 * - pause: abrir «Pausar copias automáticas» de un destino.
 * - resume: reanudar las copias automáticas de un destino.
 */
export type Intent = "schedule" | "apply" | "pause" | "resume";

export interface SavedIntent {
  repoId: string;
  intent: Intent;
  planId?: string;
}

/**
 * Peticiones pendientes: editor de copias automáticas, pausar o reanudar (id
 * del destino) y programar una copia (id del plan).
 */
export const pendingEditor = $state<{
  schedule: string | null;
  apply: string | null;
  pause: string | null;
  resume: string | null;
  maintenance: string | null;
  maintEditor: { repoId: string; which: "verify" | "offsite" | "restore_test" } | null;
  restore: string | null;
  improve: string | null;
  historyTab: string | null;
}>({
  schedule: null,
  apply: null,
  pause: null,
  resume: null,
  /** Mostrar «Mantenimiento» de este destino (p. ej. desde el destino de su copia externa). */
  maintenance: null,
  /** Abrir el editor de una tarea de «Mantenimiento» (desde el esquema o el asistente). */
  maintEditor: null,
  /** Abrir «Restaurar archivos» de un destino (desde Ctrl+K). */
  restore: null,
  /** Abrir «Mejorar la protección» de un destino (desde Ctrl+K). */
  improve: null,
  /** Mostrar la pestaña «Historia» de un destino (desde Ctrl+K). */
  historyTab: null,
});

export function rememberIntent(repoId: string, intent: Intent, planId?: string) {
  try {
    localStorage.setItem(KEY, JSON.stringify({ repoId, intent, planId }));
  } catch {
    /* sin almacenamiento: al volver no se abrirá el editor solo */
  }
}

export function forgetIntent() {
  try {
    localStorage.removeItem(KEY);
  } catch {
    /* nada que borrar */
  }
}

/** Lee y borra la acción pendiente (si la hay). */
export function takeIntent(): SavedIntent | null {
  try {
    const raw = localStorage.getItem(KEY);
    localStorage.removeItem(KEY);
    const saved = raw ? JSON.parse(raw) : null;
    if (!saved || typeof saved.repoId !== "string") return null;
    if (saved.intent === "schedule" || saved.intent === "pause" || saved.intent === "resume") return { repoId: saved.repoId, intent: saved.intent };
    if (saved.intent === "apply" && typeof saved.planId === "string") return { repoId: saved.repoId, intent: "apply", planId: saved.planId };
    return null;
  } catch {
    return null;
  }
}
