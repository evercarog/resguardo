// Lo que está en marcha, para el camino de cada copia (lib/camino.ts) y el
// mapa: «Copiando 40 %», «Subiendo 12 %» o null. La misma cuenta que el mapa
// (MapaProteccion.svelte), en un solo sitio para la página de la copia, la
// tarjeta del repositorio y la página «Copias».
import { pctVisible, tareasDe } from "./progreso.svelte";

export function enVivoCamino(equipo: string, repo: string, tipo: "copia" | "copia_externa" | "espejo"): string | null {
  const t = tareasDe(equipo, { repo, tipos: [tipo] })[0];
  if (!t) return null;
  if (t.fase === "en_cola") return "En cola";
  const p = pctVisible(equipo, t);
  return `${tipo === "copia" ? "Copiando" : "Subiendo"}${p != null ? ` ${p} %` : "…"}`;
}
