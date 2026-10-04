// Lo que la consola está haciendo con el servidor, para decirlo sin palabras:
// - `primerPlano`: peticiones que la persona espera (cargar una pantalla,
//   mandar algo). Con ellas, la barra fina de arriba (BarraProgreso).
// - `fondo`: refrescos automáticos (cada 8–15 s). Solo un «Actualizando…»
//   discreto, sin mover nada.
// - `sinConexion`: la última petición no llegó al servidor. Se avisa con una
//   franja y se reintenta sola hasta que vuelve.
// Las esperas largas de las sesiones (hasta 25 s) no cuentan: no son «cargar».

import { untrack } from "svelte";

export const actividad = $state({ primerPlano: 0, fondo: 0, sinConexion: false });

let modoFondo = 0;

/**
 * Lo que se pide dentro de `fn` (de forma síncrona, hasta su primer `await`)
 * cuenta como refresco de fondo: p. ej. `enFondo(() => cargar())`.
 */
export function enFondo<T>(fn: () => T): T {
  modoFondo++;
  try {
    return fn();
  } finally {
    modoFondo--;
  }
}

/** Una petición empieza; devuelve cómo marcar que terminó. `invisible`: no cuenta (esperas largas). */
export function empezar(invisible = false): () => void {
  if (invisible) return () => {};
  const fondo = modoFondo > 0;
  // Sin seguir la lectura: una petición lanzada desde un $effect no debe
  // depender del contador (si no, cada petición volvería a lanzar el efecto).
  untrack(() => {
    if (fondo) actividad.fondo++;
    else actividad.primerPlano++;
  });
  let hecho = false;
  return () => {
    if (hecho) return;
    hecho = true;
    untrack(() => {
      if (fondo) actividad.fondo = Math.max(0, actividad.fondo - 1);
      else actividad.primerPlano = Math.max(0, actividad.primerPlano - 1);
    });
  };
}

let reintento: ReturnType<typeof setTimeout> | null = null;

/** El servidor respondió (con lo que sea): hay conexión. */
export function conexionOk() {
  actividad.sinConexion = false;
  if (reintento) clearTimeout(reintento);
  reintento = null;
}

/** No se llegó al servidor: se avisa y se prueba cada pocos segundos hasta que vuelva. */
export function conexionPerdida() {
  actividad.sinConexion = true;
  if (reintento || typeof window === "undefined") return;
  let espera = 3000;
  const probar = async () => {
    try {
      const r = await fetch("/api/servidor", { cache: "no-store", credentials: "same-origin" });
      if (r.status > 0) return conexionOk();
    } catch {
      /* sigue sin conexión */
    }
    espera = Math.min(espera * 2, 30_000);
    reintento = setTimeout(probar, espera);
  };
  reintento = setTimeout(probar, espera);
}

if (typeof window !== "undefined") {
  window.addEventListener("offline", () => conexionPerdida());
  window.addEventListener("online", () => {
    if (actividad.sinConexion) {
      if (reintento) clearTimeout(reintento);
      reintento = null;
      conexionPerdida(); // prueba enseguida (dentro de 3 s) en vez de esperar al siguiente intento
    }
  });
}
