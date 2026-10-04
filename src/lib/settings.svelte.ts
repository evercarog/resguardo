// Ajustes de apariencia. Se guardan en este equipo (localStorage del webview):
// son preferencias personales, no datos que haya que proteger.
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

export type ThemeMode = "system" | "light" | "dark" | "black";
export type Accent = "teal" | "blue" | "indigo" | "violet" | "rose" | "amber" | "graphite";

export const THEMES: { id: ThemeMode; label: string }[] = [
  { id: "system", label: "Sistema" },
  { id: "light", label: "Claro" },
  { id: "dark", label: "Oscuro" },
  { id: "black", label: "Negro" },
];

/** `light`/`dark`: los tonos del acento en cada modo (los mismos que en app.css). */
export const ACCENTS: { id: Accent; label: string; light: string; dark: string }[] = [
  { id: "teal", label: "Verde azulado", light: "#0d7a69", dark: "#3cc4ad" },
  { id: "blue", label: "Azul", light: "#1f63d8", dark: "#5b9dff" },
  { id: "indigo", label: "Índigo", light: "#4f46e5", dark: "#8e8cff" },
  { id: "violet", label: "Violeta", light: "#7c3aed", dark: "#b38bff" },
  { id: "rose", label: "Rosa", light: "#cf2f68", dark: "#ff7aa6" },
  { id: "amber", label: "Ámbar", light: "#b45309", dark: "#f2b33d" },
  { id: "graphite", label: "Grafito", light: "#3f4b5c", dark: "#aab6c6" },
];

const KEY = "resguardo:apariencia";

function load(): { theme: ThemeMode; accent: Accent } {
  const fallback = { theme: "system" as ThemeMode, accent: "teal" as Accent };
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return {
      theme: THEMES.some((t) => t.id === saved.theme) ? saved.theme : fallback.theme,
      accent: ACCENTS.some((a) => a.id === saved.accent) ? saved.accent : fallback.accent,
    };
  } catch {
    return fallback;
  }
}

export const appearance = $state(load());

function apply() {
  const root = document.documentElement;
  if (appearance.theme === "system") root.removeAttribute("data-theme");
  else root.dataset.theme = appearance.theme;
  if (appearance.accent === "teal") root.removeAttribute("data-accent");
  else root.dataset.accent = appearance.accent;

  try {
    localStorage.setItem(KEY, JSON.stringify(appearance));
  } catch {
    /* sin almacenamiento: el ajuste dura hasta cerrar la app */
  }

  // La barra de título de la ventana sigue el mismo modo.
  if (isTauri()) {
    const theme = appearance.theme === "system" ? null : appearance.theme === "light" ? "light" : "dark";
    getCurrentWindow()
      .setTheme(theme)
      .catch(() => {});
  }
}

/** Aplica la apariencia guardada al arrancar (antes de pintar la interfaz). */
export const initAppearance = apply;

export function setTheme(theme: ThemeMode) {
  appearance.theme = theme;
  apply();
}

export function setAccent(accent: Accent) {
  appearance.accent = accent;
  apply();
}
