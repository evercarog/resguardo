// Estado de la interfaz que se recuerda entre sesiones (pestaña elegida, secciones plegadas).
// Se guarda en este equipo (localStorage del webview); si no se puede leer o escribir, se usan
// los valores por defecto sin más.

const KEY = "resguardo.ui";

interface UiState {
  /** Última pestaña de la vista de un destino. */
  repoTab: "snapshots" | "retention" | "history";
  /** Tarjeta «Mantenimiento» plegada. */
  maintCollapsed: boolean;
  /** Tarjeta «Copias automáticas» plegada. */
  scheduleCollapsed: boolean;
  /** «Primeros pasos» ocultado a mano (el progreso se calcula siempre de lo que hay). */
  onboardingHidden: boolean;
  /** «Opciones avanzadas» desplegadas o plegadas a mano, por editor (ver Advanced.svelte). */
  advanced: Record<string, boolean>;
  /** Destinos plegados en la barra lateral. */
  foldedPlaces: string[];
}

const defaults: UiState = { repoTab: "snapshots", maintCollapsed: false, scheduleCollapsed: false, onboardingHidden: false, advanced: {}, foldedPlaces: [] };

function load(): UiState {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return {
      repoTab: saved.repoTab === "retention" || saved.repoTab === "history" ? saved.repoTab : "snapshots",
      maintCollapsed: saved.maintCollapsed === true,
      scheduleCollapsed: saved.scheduleCollapsed === true,
      onboardingHidden: saved.onboardingHidden === true,
      advanced:
        saved.advanced && typeof saved.advanced === "object"
          ? Object.fromEntries(Object.entries(saved.advanced).filter(([, v]) => typeof v === "boolean")) as Record<string, boolean>
          : {},
      foldedPlaces: Array.isArray(saved.foldedPlaces) ? saved.foldedPlaces.filter((x: unknown) => typeof x === "string") : [],
    };
  } catch {
    return { ...defaults, advanced: {}, foldedPlaces: [] };
  }
}

export const ui = $state<UiState>(load());

/** Cambia un valor y lo recuerda. */
export function setUi<K extends keyof UiState>(key: K, value: UiState[K]) {
  ui[key] = value;
  try {
    localStorage.setItem(KEY, JSON.stringify(ui));
  } catch {
    // Sin almacenamiento: se recuerda solo mientras la app siga abierta.
  }
}
