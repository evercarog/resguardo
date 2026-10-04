// Argon2id del navegador: un worker que se crea al primer uso y se reutiliza.
import type { Argon2 } from "./claves";

let worker: Worker | null = null;
let siguiente = 0;
const pendientes = new Map<number, { ok: (b: Uint8Array) => void; mal: (e: Error) => void }>();

function obtenerWorker(): Worker {
  if (worker) return worker;
  worker = new Worker(new URL("./argon2.worker.ts", import.meta.url), { type: "module" });
  worker.onmessage = (e: MessageEvent<{ id: number; out?: Uint8Array; error?: string }>) => {
    const p = pendientes.get(e.data.id);
    if (!p) return;
    pendientes.delete(e.data.id);
    if (e.data.out) p.ok(e.data.out);
    else p.mal(new Error(e.data.error ?? "Argon2id falló"));
  };
  worker.onerror = () => {
    for (const p of pendientes.values()) p.mal(new Error("No se pudo calcular la prueba en este navegador."));
    pendientes.clear();
    worker?.terminate();
    worker = null;
  };
  return worker;
}

export const argon2Navegador: Argon2 = (clave, sal) =>
  new Promise((ok, mal) => {
    const id = ++siguiente;
    pendientes.set(id, { ok, mal });
    // Se envía una copia: el worker la borra al terminar, y quien llama borra la suya.
    obtenerWorker().postMessage({ id, clave: clave.slice(), sal: sal.slice() });
  });
