// Plan 0.8.1, B1: los números del menú (barra lateral y pestañas), en un solo
// sitio para que digan lo mismo: órdenes en espera, avisos sin revisar y el
// plazo de las pruebas funcionales.
import { actual } from "./estado.svelte";
import { funcionalesDe } from "./funcionales.svelte";
import { sinDuplicados } from "./salud";

export type Cuenta = { cuenta?: number; tono?: "warn" | "bad" };

/** E3 (14k): la prueba funcional se acerca (ámbar) o venció (rojo); si no, nada. */
export function avisoPruebas(c: string): "warn" | "bad" | null {
  const f = funcionalesDe(c)?.estado?.cliente.fase;
  return f === "vencida" ? "bad" : f === "pronto" ? "warn" : null;
}

/** El número de una pestaña (por su ruta dentro del cliente). */
export function cuentaPestana(c: string, ruta: string): Cuenta | null {
  if (ruta === "/actividad") return actual.pendientes ? { cuenta: actual.pendientes, tono: "warn" } : null;
  if (ruta === "/avisos") return actual.avisosAbiertos ? { cuenta: actual.avisosAbiertos, tono: "bad" } : null;
  if (ruta === "/equipos") return { cuenta: sinDuplicados(actual.equipos).length || undefined };
  if (ruta === "/recuperar/pruebas") {
    const t = avisoPruebas(c);
    return t ? { cuenta: 1, tono: t } : null;
  }
  return null;
}

/** El número de una sección: el de su pestaña con número (las órdenes en espera en Actividad…). */
export function cuentaSeccion(c: string, rutas: string[]): Cuenta | null {
  for (const r of rutas) {
    const n = cuentaPestana(c, r);
    if (n?.cuenta) return n;
  }
  return null;
}
