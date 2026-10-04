// La barra lateral plegada a iconos (docs/diseno.md §5). Es una preferencia de
// este navegador: si la persona nunca la tocó, se pliega sola en ventanas de
// escritorio estrechas (menos de 1100 px). En el móvil siempre es un cajón.

const CLAVE = "rg.barra.plegada";

function leer(): boolean | null {
  try {
    const v = localStorage.getItem(CLAVE);
    return v === "1" ? true : v === "0" ? false : null;
  } catch {
    return null;
  }
}

const elegida = leer();

export const barra = $state({
  plegada: elegida ?? (typeof window !== "undefined" && window.innerWidth < 1100),
});

export function plegarBarra(si: boolean) {
  barra.plegada = si;
  try {
    localStorage.setItem(CLAVE, si ? "1" : "0");
  } catch {
    /* sin almacenamiento: dura lo que la pestaña */
  }
}
