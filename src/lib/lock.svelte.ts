// Bloqueo de la app con Windows Hello: estado en la interfaz y bloqueo tras
// un rato sin usarla. Quien manda es el backend (rechaza los comandos con la
// app bloqueada); aquí solo se decide qué se muestra.
import { listen } from "@tauri-apps/api/event";
import * as api from "$lib/api";
import type { AppLockStatus } from "$lib/api";

export const appLock = $state<{ ready: boolean; status: AppLockStatus | null; locked: boolean }>({
  ready: false,
  status: null,
  locked: false,
});

/** Al abrir la app: ¿empieza bloqueada? */
export async function initLock() {
  try {
    const s = await api.appLockStatus();
    appLock.status = s;
    appLock.locked = s.locked;
  } catch {
    // Sin respuesta del backend: nada que bloquear desde aquí.
    appLock.locked = false;
  } finally {
    appLock.ready = true;
  }
  startIdleWatch();
  // Al ocultarse en la bandeja con el bloqueo activado, el backend la bloquea.
  void listen("app-locked", () => (appLock.locked = true));
}

export async function refreshLockStatus() {
  appLock.status = await api.appLockStatus();
  return appLock.status;
}

// ---------- Volver a bloquear tras un rato sin usarla ----------

let lastActivity = Date.now();
let watching = false;

function startIdleWatch() {
  if (watching) return;
  watching = true;
  const touch = () => (lastActivity = Date.now());
  for (const ev of ["pointerdown", "pointermove", "keydown", "wheel", "touchstart"]) {
    window.addEventListener(ev, touch, { passive: true, capture: true });
  }
  setInterval(() => {
    const s = appLock.status;
    if (!s?.enabled || !s.idle_minutes || appLock.locked) return;
    if (Date.now() - lastActivity >= s.idle_minutes * 60_000) void lockNow();
  }, 15_000);
}

export async function lockNow() {
  try {
    await api.appLockLock();
  } catch {
    // Ya estaba bloqueada.
  }
  appLock.locked = true;
}

export function markUnlocked() {
  lastActivity = Date.now();
  appLock.locked = false;
}
