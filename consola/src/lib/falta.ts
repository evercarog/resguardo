// 14aa: «qué falta» de un formulario, para el botón principal apagado
// (BotonFalta.svelte). Lo puro (la primera cosa que falta) se prueba en
// scripts/vectores.ts; `marcarFalta` toca el documento.
import type { Falta } from "./componentes/BotonFalta.svelte";

/** La primera comprobación que no se cumple, o null si está todo. */
export function primeraFalta(comprobaciones: [cumple: unknown, falta: Falta][]): Falta | null {
  for (const [ok, f] of comprobaciones) if (!ok) return f;
  return null;
}

/** Lleva al campo que falta y lo marca un momento (borde de aviso y `aria-invalid`). */
export function marcarFalta(id: string) {
  const el = document.getElementById(id);
  if (!el) return;
  el.scrollIntoView({ block: "nearest" });
  el.focus({ preventScroll: true });
  const yaInvalido = el.getAttribute("aria-invalid");
  el.classList.add("campo-falta");
  if (!yaInvalido) el.setAttribute("aria-invalid", "true");
  setTimeout(() => {
    el.classList.remove("campo-falta");
    if (!yaInvalido) el.removeAttribute("aria-invalid");
  }, 2400);
}
