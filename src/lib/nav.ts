// Qué está abierto en la ventana principal.
export type Selection =
  | { kind: "status" }
  /** Historial de actividad; `filter` limita a un destino ("repoId") o a una copia ("repoId#planId"). */
  | { kind: "activity"; filter?: string }
  /** Todos los equipos de la cuenta de Resguardo Web. */
  | { kind: "equipos" }
  /** Equipos gestionados por esta consola (fase 5). */
  | { kind: "gestionados" }
  /** Ajustes, abiertos en una sección. */
  | { kind: "ajustes"; section?: import("$lib/settingsNav.svelte").SettingsSection }
  /** Una copia (plan) de un destino. */
  | { kind: "copy"; repoId: string; planId: string }
  /** Un repositorio de restic (el antiguo «destino»: el id de la selección no cambia). */
  | { kind: "destination"; repoId: string }
  /** Un destino (lugar) con sus repositorios. */
  | { kind: "place"; placeId: string };
