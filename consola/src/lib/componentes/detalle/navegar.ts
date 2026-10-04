// Abrir y recorrer el panel de detalle cambiando la URL de la página (sin
// recargarla): así «Atrás» del navegador vuelve a lo anterior y un enlace
// copiado abre lo mismo. En la URL nunca van nombres de archivo.
import { goto } from "$app/navigation";
import { page } from "$app/state";
import { conSeleccion, leerSeleccion, type Seleccion } from "$lib/detalle";

/** Cuántos pasos se han dado dentro del panel (para que «Atrás» vuelva y no salga de la página). */
let pasos = 0;

export const seleccion = () => leerSeleccion(page.url.searchParams);

/** Va a otra selección. `reemplazar`: sin añadir un paso al historial (p. ej. al escribir en un filtro). */
export function ir(sel: Partial<Seleccion> & { cerrar?: boolean }, reemplazar = false) {
  const destino = page.url.pathname + conSeleccion(page.url.searchParams, sel);
  if (destino === page.url.pathname + page.url.search) return;
  if (sel.cerrar) pasos = 0;
  else if (!reemplazar) pasos++;
  void goto(destino, { keepFocus: true, noScroll: true, replaceState: reemplazar });
}

/**
 * Atrás dentro del panel: el paso anterior del historial; si se llegó con un
 * enlace (sin pasos dados aquí), lo de arriba en las migas (`padre`) o cerrar.
 */
export function atras(padre?: () => void) {
  if (pasos > 0) {
    pasos--;
    history.back();
  } else if (padre) padre();
  else ir({ cerrar: true });
}

export const cerrar = () => ir({ cerrar: true });

/** Abrir el detalle de una versión (con un filtro: directamente «Qué cambió»). */
export function abrirVersion(id: string, filtro?: "nuevos" | "cambiados" | "borrados") {
  ir(filtro ? { vista: "cambios", version: id, con: null, filtro, vuelta: null } : { vista: "version", version: id, con: null, filtro: "todos", vuelta: null });
}

export const abrirVuelta = (hora: string) => ir({ vista: "vuelta", vuelta: hora, version: null, con: null, filtro: "todos" });
export const abrirEspacio = () => ir({ vista: "espacio", version: null, con: null, vuelta: null, filtro: "todos" });
export const abrirEstado = () => ir({ vista: "estado", version: null, con: null, vuelta: null, filtro: "todos" });
/** Filtrar las listas a un día (o a todos, con `null`). */
export const elegirDia = (dia: string | null) => ir({ dia });
