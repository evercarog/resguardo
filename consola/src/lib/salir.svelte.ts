// Avisar antes de salir de una página con trabajo sin enviar (G2, plan 0.8.1):
// el editor de copias y el asistente de Restaurar. Para salir de la consola
// (cerrar la pestaña, un enlace a otra web) pregunta el navegador; para ir a
// otra página de la consola, el diálogo de `ConfirmarSalida` (en el marco).
import { beforeNavigate, goto } from "$app/navigation";

export const salida = $state<{ abierta: boolean; texto: string; destino: string | null }>({ abierta: false, texto: "", destino: null });
let dejar = false;

/** Llamar al crear el componente. `hayCambios` se mira en cada navegación. */
export function preguntarAlSalir(hayCambios: () => boolean, texto = "Lo que has cambiado aún no se ha enviado y se perderá.") {
  beforeNavigate((nav) => {
    if (dejar || !hayCambios()) return;
    // Cerrar la pestaña o irse a otra web: cancelar hace que el navegador pregunte él.
    if (nav.willUnload || !nav.to) {
      nav.cancel();
      return;
    }
    // Dentro de la misma página (otra pestaña, ?dia=…): no es salir.
    if (nav.from && nav.to.url.pathname === nav.from.url.pathname) return;
    nav.cancel();
    salida.texto = texto;
    salida.destino = nav.to.url.pathname + nav.to.url.search + nav.to.url.hash;
    salida.abierta = true;
  });
}

export async function salirIgual() {
  const d = salida.destino;
  salida.abierta = false;
  salida.destino = null;
  if (!d) return;
  dejar = true;
  try {
    await goto(d);
  } finally {
    dejar = false;
  }
}

export function quedarse() {
  salida.abierta = false;
  salida.destino = null;
}
