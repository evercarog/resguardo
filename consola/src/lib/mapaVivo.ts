// Plan 0.8.1 (C2, 15cc): lo que está en marcha, para el mapa (lib/mapa.ts
// `enVivo`): copias, la copia externa antigua, las copias derivadas (por su id)
// y los espejos (por su trabajo), con el mismo texto en el mapa de un cliente y
// en el de todos. Sin runas: se prueba en scripts/vectores-mapa.ts.
import type { TareaEnMarcha } from "./tipos";
import type { TipoVivo } from "./mapa";

/** La tarea en marcha que corresponde a un trazo del mapa (o undefined). */
export function tareaDelTrazo(tareas: readonly TareaEnMarcha[], repo: string, tipo: TipoVivo, id?: string): TareaEnMarcha | undefined {
  switch (tipo) {
    case "copia":
      return tareas.find((t) => t.tipo === "copia" && t.repo === repo);
    case "copia_externa":
      // La copia externa antigua: una subida del repositorio que no es de ninguna derivada.
      return tareas.find((t) => t.tipo === "copia_externa" && t.repo === repo && !t.derivada);
    case "derivada":
      return tareas.find((t) => t.tipo === "copia_externa" && t.repo === repo && !!id && t.derivada === id);
    case "espejo":
      return tareas.find((t) => t.tipo === "espejo" && (t.espejo === repo || t.repo === repo));
  }
}

/** «Copiando 40 %», «Subiendo 12 %», «Subiendo…», «En cola» (con el porcentaje que se pinta). */
export function textoDelTrazo(t: TareaEnMarcha | undefined, tipo: TipoVivo, pct: number | null): string | null {
  if (!t) return null;
  if (t.fase === "en_cola") return "En cola";
  return `${tipo === "copia" ? "Copiando" : "Subiendo"}${pct != null ? ` ${pct} %` : "…"}`;
}
