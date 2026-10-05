// ¿Se actualizó el servidor mientras esta pestaña seguía abierta? La consola es
// una aplicación de una sola página: una pestaña abierta antes de actualizar
// sigue con el código de antes (con sus fallos) hasta que se recarga. Cada
// 5 minutos, al volver a la pestaña y al recuperar la conexión, se pregunta la
// versión del servidor (`GET /api/servidor`, sin sesión); si ya no es la de
// cuando se cargó, se ofrece «Recargar».

export const versionNueva = $state({ hay: false, version: "" });

const CADA = 5 * 60_000;

/** `cargada`: la versión del servidor con la que se cargó la consola. `preguntar`: la de ahora. */
export function vigilarVersion(cargada: () => string | undefined, preguntar: () => Promise<string>): () => void {
  let ultima = 0;
  const mirar = async () => {
    if (versionNueva.hay || Date.now() - ultima < 30_000) return;
    ultima = Date.now();
    const antes = cargada();
    if (!antes) return;
    try {
      const v = await preguntar();
      if (v && v !== antes) {
        versionNueva.version = v;
        versionNueva.hay = true;
      }
    } catch {
      /* sin conexión: ya se verá */
    }
  };
  const alVolver = () => document.visibilityState === "visible" && void mirar();
  const t = setInterval(() => document.visibilityState === "visible" && void mirar(), CADA);
  document.addEventListener("visibilitychange", alVolver);
  window.addEventListener("online", alVolver);
  return () => {
    clearInterval(t);
    document.removeEventListener("visibilitychange", alVolver);
    window.removeEventListener("online", alVolver);
  };
}
