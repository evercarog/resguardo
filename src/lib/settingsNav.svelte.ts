// Abrir Ajustes (en una sección) desde cualquier parte de la app, también al
// volver de reabrirla como administrador.
import * as api from "$lib/api";

export type SettingsSection = "general" | "equipo" | "bandeja" | "explorador" | "seguridad";

export const SETTINGS_SECTIONS: { id: SettingsSection; label: string }[] = [
  { id: "general", label: "General" },
  { id: "equipo", label: "Este equipo" },
  { id: "bandeja", label: "Bandeja y avisos" },
  { id: "explorador", label: "Explorador de archivos" },
  { id: "seguridad", label: "Seguridad" },
];

/** Sección pedida (la recoge la página principal). */
export const settingsNav = $state<{ request: SettingsSection | null }>({ request: null });

export function openSettings(section: SettingsSection = "general") {
  settingsNav.request = section;
}

const KEY = "resguardo:abrir-ajustes";

/** Reabre Resguardo como administrador y, al volver, abre Ajustes en esa sección. */
export async function relaunchToSettings(section: SettingsSection) {
  try {
    localStorage.setItem(KEY, section);
  } catch {
    /* sin almacenamiento: se abrirá en Estado */
  }
  try {
    await api.relaunchAsAdmin();
  } catch (e) {
    try {
      localStorage.removeItem(KEY);
    } catch {
      /* nada */
    }
    throw e;
  }
}

/** Sección pendiente tras reabrir (y se olvida). */
export function takeSettingsIntent(): SettingsSection | null {
  try {
    const v = localStorage.getItem(KEY);
    localStorage.removeItem(KEY);
    return SETTINGS_SECTIONS.some((s) => s.id === v) ? (v as SettingsSection) : null;
  } catch {
    return null;
  }
}
