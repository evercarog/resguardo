// Atajos de teclado de la consola: Ctrl+K (⌘K en Mac) abre la paleta para
// ir a cualquier sitio, «?» enseña los atajos y «g» seguida de una letra
// salta a una sección del cliente abierto. Nunca mientras se escribe en un
// campo ni con un diálogo abierto.
import { goto } from "$app/navigation";
import { anyModalOpen } from "$ui/componentes/Modal.svelte";
import { actual } from "./estado.svelte";

export const atajos = $state({ paleta: false, ayuda: false });

export const esMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
/** Cómo se escribe el modificador en esta máquina. */
export const MOD = esMac ? "⌘" : "Ctrl";

/** «g» + letra: a qué sección del cliente lleva. */
export const IR_A: { tecla: string; texto: string; ruta: string }[] = [
  { tecla: "s", texto: "Estado", ruta: "" },
  { tecla: "e", texto: "Equipos", ruta: "/equipos" },
  { tecla: "r", texto: "Repositorios y destinos", ruta: "/repositorios" },
  { tecla: "t", texto: "Restaurar", ruta: "/restaurar" },
  { tecla: "o", texto: "Órdenes", ruta: "/ordenes" },
  { tecla: "a", texto: "Avisos", ruta: "/avisos" },
  { tecla: "i", texto: "Informes", ruta: "/informes" },
];

function escribiendo(e: KeyboardEvent) {
  const t = e.target as HTMLElement | null;
  return !!t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName));
}

let prefijo = 0;

export function alPulsar(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === "k") {
    // Ctrl+K abre (o cierra) la paleta también desde un campo: es el atajo que se espera.
    if (anyModalOpen() && !atajos.paleta) return;
    e.preventDefault();
    atajos.paleta = !atajos.paleta;
    return;
  }
  if (e.ctrlKey || e.metaKey || e.altKey || escribiendo(e) || anyModalOpen() || atajos.paleta || atajos.ayuda) return;
  if (e.key === "?") {
    e.preventDefault();
    atajos.ayuda = true;
    return;
  }
  if (e.key === "g") {
    prefijo = Date.now();
    return;
  }
  if (prefijo && Date.now() - prefijo < 1200 && actual.id) {
    prefijo = 0;
    const x = IR_A.find((a) => a.tecla === e.key.toLowerCase());
    if (x) {
      e.preventDefault();
      void goto(`/c/${actual.id}${x.ruta}`);
    }
  }
}
