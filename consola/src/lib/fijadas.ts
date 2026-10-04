// Llaves fijadas de cada equipo en este navegador (IndexedDB).
//
// Una contraseña de repositorio se sella para la X25519 del equipo que da el
// servidor. Sin la clave de administración no se puede comprobar su etiqueta,
// así que la consola recuerda las llaves de cada equipo la primera vez que se
// comprueban con la etiqueta (con la clave de administración) y, a partir de
// ahí, exige que coincidan:
//   - sin fijar   → la orden pide también la clave de administración (y se fijan);
//   - cambiadas   → alto: alerta de seguridad y no se envía nada.
// Solo se guardan claves públicas (nada secreto).
export type EstadoLlaves = "fijada" | "sin_fijar" | "cambiada";

interface Fijada {
  id: string;
  box_pub: string;
  sign_pub: string;
  fijada: string;
}

const BD = "resguardo-consola";
const ALMACEN = "llaves";
/** Si IndexedDB no está disponible (navegación privada estricta), solo en memoria. */
const memoria = new Map<string, Fijada>();
const clave = (cliente: string, equipo: string) => `${cliente}|${equipo}`;

let bd: Promise<IDBDatabase | null> | null = null;
/**
 * Abre la base, sin colgarse nunca: `indexedDB.open` puede quedarse sin
 * respuesta si otra pestaña tiene la base abierta mientras se borra o se
 * actualiza. Si en 2 s no contesta, esta pestaña sigue solo en memoria (en el
 * peor caso se vuelve a pedir la clave de administración para fijar las llaves).
 */
function abrirBd(): Promise<IDBDatabase | null> {
  bd ??= new Promise((ok) => {
    let hecho = false;
    const fin = (x: IDBDatabase | null) => {
      if (hecho) return x?.close();
      hecho = true;
      clearTimeout(t);
      // Si no se pudo abrir, el resto de la sesión va en memoria (sin volver a esperar).
      ok(x);
    };
    const t = setTimeout(() => fin(null), 2000);
    try {
      const r = indexedDB.open(BD, 1);
      r.onupgradeneeded = () => r.result.createObjectStore(ALMACEN, { keyPath: "id" });
      r.onsuccess = () => {
        const db = r.result;
        // Si otra pestaña borra o actualiza la base, se cierra aquí para no bloquearla.
        db.onversionchange = () => {
          db.close();
          bd = null;
        };
        db.onclose = () => (bd = null);
        fin(db);
      };
      r.onerror = () => fin(null);
      r.onblocked = () => fin(null);
    } catch {
      fin(null);
    }
  });
  return bd;
}

async function leer(id: string): Promise<Fijada | null> {
  const db = await abrirBd();
  if (!db) return memoria.get(id) ?? null;
  return new Promise((ok) => {
    try {
      const r = db.transaction(ALMACEN, "readonly").objectStore(ALMACEN).get(id);
      r.onsuccess = () => ok((r.result as Fijada | undefined) ?? memoria.get(id) ?? null);
      r.onerror = () => ok(memoria.get(id) ?? null);
    } catch {
      // La base se cerró entre medias.
      bd = null;
      ok(memoria.get(id) ?? null);
    }
  });
}

/** Fija (o vuelve a fijar) las llaves de un equipo. Llamar solo tras comprobar su etiqueta con la clave de administración. */
export async function fijar(cliente: string, equipo: { id: string; box_pub: string; sign_pub: string }) {
  const f: Fijada = { id: clave(cliente, equipo.id), box_pub: equipo.box_pub, sign_pub: equipo.sign_pub, fijada: new Date().toISOString() };
  memoria.set(f.id, f);
  const db = await abrirBd();
  if (!db) return;
  await new Promise<void>((ok) => {
    try {
      const t = db.transaction(ALMACEN, "readwrite");
      t.objectStore(ALMACEN).put(f);
      t.oncomplete = () => ok();
      t.onerror = () => ok();
      t.onabort = () => ok();
    } catch {
      bd = null;
      ok();
    }
  });
}

/** ¿Las llaves que da ahora el servidor son las fijadas? */
export async function comprobarLlaves(cliente: string, equipo: { id: string; box_pub: string; sign_pub: string }): Promise<EstadoLlaves> {
  const f = await leer(clave(cliente, equipo.id));
  if (!f) return "sin_fijar";
  return f.box_pub === equipo.box_pub && f.sign_pub === equipo.sign_pub ? "fijada" : "cambiada";
}

export async function fijadaEl(cliente: string, equipo: string): Promise<string | null> {
  return (await leer(clave(cliente, equipo)))?.fijada ?? null;
}

/** Las llaves cambiaron en el servidor respecto a las fijadas: no se envía nada. */
export class ErrorLlavesCambiadas extends Error {
  constructor(readonly equipo: string) {
    super(
      `Alerta de seguridad: las llaves de ${equipo} que da el servidor no son las que este navegador comprobó antes. Puede que alguien haya manipulado el servidor para quedarse con tus contraseñas. No se ha enviado nada.`,
    );
  }
}
