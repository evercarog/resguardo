// Copiar al portapapeles (G2, plan 0.8.1). En una consola por http (sin TLS) o
// sin permiso, el navegador no deja escribir en el portapapeles: se prueba la
// forma antigua y, si tampoco, se dice y se deja el texto seleccionado para
// copiarlo con Ctrl+C. Antes, el botón no hacía nada y no decía nada.
import { avisar } from "./avisos.svelte";

/** Selecciona el texto de un elemento (o el de un campo) para copiarlo a mano. */
export function seleccionar(el: Element | null | undefined) {
  if (!el) return;
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    el.focus();
    el.select();
    return;
  }
  const sel = window.getSelection();
  if (!sel) return;
  const r = document.createRange();
  r.selectNodeContents(el);
  sel.removeAllRanges();
  sel.addRange(r);
}

function aLaAntigua(texto: string): boolean {
  const ta = document.createElement("textarea");
  ta.value = texto;
  ta.setAttribute("readonly", "");
  ta.style.position = "fixed";
  ta.style.opacity = "0";
  document.body.appendChild(ta);
  const activo = document.activeElement as HTMLElement | null;
  ta.select();
  let ok = false;
  try {
    ok = document.execCommand("copy");
  } catch {
    ok = false;
  }
  ta.remove();
  activo?.focus?.();
  return ok;
}

/**
 * Copia `texto`. Si no se puede, avisa y selecciona `dondeEsta` (el elemento que
 * enseña el texto), para copiarlo a mano. Devuelve si se copió.
 */
export async function copiar(texto: string, dondeEsta?: Element | null): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(texto);
      return true;
    }
  } catch {
    /* sin permiso: la forma antigua */
  }
  if (aLaAntigua(texto)) return true;
  seleccionar(dondeEsta);
  avisar(dondeEsta ? "El navegador no deja copiar aquí (la consola no va por https). El texto queda seleccionado: cópialo con Ctrl+C." : "El navegador no deja copiar aquí (la consola no va por https). Selecciona el texto y cópialo con Ctrl+C.", "warn");
  return false;
}
