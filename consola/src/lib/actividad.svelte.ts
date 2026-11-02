// Lo que la consola está haciendo con el servidor, para decirlo sin palabras:
// - `primerPlano`: peticiones que la persona espera (cargar una pantalla,
//   mandar algo). Con ellas, la barra fina de arriba (BarraProgreso).
// - `fondo`: refrescos automáticos (cada 8–15 s). Solo un «Actualizando…»
//   discreto, sin mover nada.
// - `sinConexion`: la última petición no llegó al servidor. Se avisa con una
//   franja y se reintenta sola hasta que vuelve.
// Las esperas largas de las sesiones (hasta 25 s) no cuentan: no son «cargar».

import { untrack } from "svelte";
import { esPrimerPlano } from "./primerPlano";

export const actividad = $state({ primerPlano: 0, fondo: 0, sinConexion: false });

let modoFondo = 0;

/**
 * 16l: lo que pide una carga después de su primer `await` (en serie) ya no está dentro de
 * `enFondo`. Para que un refresco automático no encienda la barra por esas peticiones, una
 * petición solo cuenta como de primer plano si la persona hizo algo hace poco (cambiar de
 * pantalla, pulsar, escribir) o si ya hay otra de primer plano en marcha (la misma carga en
 * serie). Lo demás (temporizadores, cambios en vivo) es de fondo.
 */
let ultimaAccion = Date.now();

/** La persona hizo algo (o cambió de pantalla): lo que se pida ahora es de primer plano. */
export function marcarAccion(ahora = Date.now()) {
  ultimaAccion = ahora;
}

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
  const fondo = !esPrimerPlano({ enFondo: modoFondo > 0, ahora: Date.now(), ultimaAccion, enMarcha: untrack(() => actividad.primerPlano) });
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
  // 16l: pulsar, tocar o escribir abre la ventana de primer plano (en captura: antes que el resto).
  for (const ev of ["pointerdown", "keydown", "submit", "popstate"]) window.addEventListener(ev, () => marcarAccion(), { capture: true, passive: true });
  window.addEventListener("offline", () => conexionPerdida());
  window.addEventListener("online", () => {
    if (actividad.sinConexion) {
      if (reintento) clearTimeout(reintento);
      reintento = null;
      conexionPerdida(); // prueba enseguida (dentro de 3 s) en vez de esperar al siguiente intento
    }
  });
}
