// El puente con el agente (crates/agente/src/ventana.rs): el agente pasa los
// datos con `window.__resguardo.datos(…)` cada vez que cambian y contesta las
// peticiones (`window.ipc.postMessage`) con `window.__resguardo.respuesta(id, …)`.
// Fuera del agente (npm run dev:ventana), datos simulados.

import type { Datos } from "./tipos";

declare global {
  interface Window {
    ipc?: { postMessage(m: string): void };
    __resguardo?: { datos(d: Datos): void; respuesta(id: number, r: { ok: boolean; datos?: unknown; error?: string }): void };
    __resguardoVersion?: string;
  }
}

export const vivo = $state({ datos: null as Datos | null, recibido: 0 });

let siguiente = 1;
const pendientes = new Map<number, { ok: (v: unknown) => void; mal: (e: Error) => void }>();

window.__resguardo = {
  datos(d) {
    vivo.datos = d;
    vivo.recibido = Date.now();
  },
  respuesta(id, r) {
    const p = pendientes.get(id);
    if (!p) return;
    pendientes.delete(id);
    if (r.ok) p.ok(r.datos);
    else p.mal(new Error(r.error ?? "Error desconocido."));
  },
};

/** ¿Dentro del agente? (Si no, simulado.) */
export const enAgente = typeof window.ipc?.postMessage === "function";

/** Pide algo al agente (la ventana: copiar, desbloquear… o `servicio` con la clave). */
export async function pedir<T = unknown>(op: string, cuerpo: Record<string, unknown> = {}): Promise<T> {
  if (!enAgente) {
    if (!import.meta.env.DEV) throw new Error("Esta página solo funciona dentro de Resguardo Agente.");
    const { simular } = await import("./simulado");
    return (await simular(op, cuerpo)) as T;
  }
  const id = siguiente++;
  return new Promise<T>((ok, mal) => {
    pendientes.set(id, { ok: ok as (v: unknown) => void, mal });
    window.ipc!.postMessage(JSON.stringify({ id, op, ...cuerpo }));
  });
}

/** Una operación del servicio con la clave de administración (ventana desbloqueada). */
export function servicio<T = unknown>(que: string, cuerpo: Record<string, unknown> = {}): Promise<T> {
  return pedir<T>("servicio", { que, cuerpo });
}

/** «bloqueada»: hay que volver a escribir la clave. */
export const esBloqueo = (e: unknown) => e instanceof Error && (e.message === "bloqueada" || e.message === "La clave de administración no es correcta.");

if (import.meta.env.DEV && !enAgente) void import("./simulado").then((m) => m.empezar());
