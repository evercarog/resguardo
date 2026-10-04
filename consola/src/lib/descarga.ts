// Descarga por el relé (api-servidor.md §9): el equipo cifra los archivos con
// una clave que solo conocen él y este navegador (va dentro de la orden
// sellada), los sube por trozos al servidor y aquí se bajan, se descifran y
// se guardan. Al terminar se borran del servidor.
import * as api from "./api";
import { borrar } from "./cripto/bytes";
import { descifrarTrozo } from "./cripto/simetrico";

export interface Progreso {
  fase: "esperando" | "subiendo" | "bajando" | "listo";
  trozos: number;
  bytes: number;
  bajados: number;
}

const espera = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Espera a que el equipo termine de subir, baja y descifra los trozos, y borra el relé. */
export async function bajarRelevo(opts: {
  cliente: string;
  relevo: string;
  clave: Uint8Array;
  alProgreso: (p: Progreso) => void;
  signal: AbortSignal;
  /** Como mucho (sin noticias del equipo) antes de rendirse. */
  paciencia?: number;
}): Promise<Blob> {
  const { cliente, relevo, clave, alProgreso, signal } = opts;
  const limite = Date.now() + (opts.paciencia ?? 30 * 60_000);
  let estado: Awaited<ReturnType<typeof api.relevo>> | null = null;
  // 1. Esperar a que la subida termine.
  while (!signal.aborted) {
    try {
      estado = await api.relevo(cliente, relevo);
    } catch {
      estado = null; // aún no existe o hubo un corte: se reintenta
    }
    if (estado?.estado === "caducado") throw new Error("La descarga caducó antes de bajarla. Vuelve a pedirla.");
    if (estado?.estado === "listo") break;
    alProgreso({ fase: estado ? "subiendo" : "esperando", trozos: estado?.trozos ?? 0, bytes: estado?.bytes ?? 0, bajados: 0 });
    if (Date.now() > limite) throw new Error("El equipo no ha terminado de preparar la descarga. Vuelve a intentarlo más tarde.");
    await espera(1500);
  }
  if (signal.aborted || !estado) throw new DOMException("Cancelado", "AbortError");
  // 2. Bajar y descifrar en orden (el índice y «último» van autenticados).
  const partes: Uint8Array<ArrayBuffer>[] = [];
  let bajados = 0;
  for (let n = 0; n < estado.trozos; n++) {
    const cifrado = await api.trozoRelevo(cliente, relevo, n, signal);
    let plano: Uint8Array;
    try {
      plano = descifrarTrozo(clave, relevo, n, n === estado.trozos - 1, cifrado);
    } catch {
      throw new Error("Un trozo de la descarga no es auténtico (alterado o fuera de orden). Se ha descartado todo.");
    }
    partes.push(new Uint8Array(plano));
    bajados += plano.length;
    alProgreso({ fase: "bajando", trozos: estado.trozos, bytes: estado.bytes, bajados });
  }
  borrar(clave);
  try {
    await api.borrarRelevo(cliente, relevo);
  } catch {
    /* caduca solo en 1 h */
  }
  alProgreso({ fase: "listo", trozos: estado.trozos, bytes: estado.bytes, bajados });
  return new Blob(partes);
}

/** Guarda un Blob: con el diálogo del sistema si el navegador lo permite, si no como descarga normal. */
export async function guardar(blob: Blob, nombre: string) {
  const w = window as unknown as { showSaveFilePicker?: (o: unknown) => Promise<{ createWritable: () => Promise<{ write: (b: Blob) => Promise<void>; close: () => Promise<void> }> }> };
  if (w.showSaveFilePicker) {
    try {
      const h = await w.showSaveFilePicker({ suggestedName: nombre });
      const f = await h.createWritable();
      await f.write(blob);
      await f.close();
      return;
    } catch (e) {
      if ((e as Error).name === "AbortError") return;
    }
  }
  const url = URL.createObjectURL(blob);
  Object.assign(document.createElement("a"), { href: url, download: nombre }).click();
  setTimeout(() => URL.revokeObjectURL(url), 60_000);
}
