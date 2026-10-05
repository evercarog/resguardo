// Observaciones y comentarios (v1.40, api-servidor.md §6, «Observaciones y comentarios»).
//
// Lo que escriben las personas sobre un equipo, un repositorio, una copia, un
// destino o el cliente: una observación (texto libre con Markdown ligero,
// lib/markdown.ts) y una bitácora de comentarios. Se guardan en claro en el
// servidor, con el cliente (no dentro de la configuración cifrada de los
// equipos): verlas no pide la clave de administración. Por eso se recuerda
// «no pongas contraseñas aquí».
//
// Aquí: cómo se nombra cada objeto, el índice del cliente abierto (para los
// contadores 💬 y la búsqueda de Ctrl+K) y los límites.
import * as api from "./api";
import { primeraLinea } from "./markdown";
import type { IndiceNota, NotasObjeto, Rol, TipoNota } from "./tipos";

export const MAX_TEXTO = 2000;
export const AVISO_SECRETOS = "No pongas contraseñas ni claves aquí: lo ve cualquiera con acceso a este cliente.";

/** El objeto de un repositorio o de una copia: «<equipo>/<id>». */
export const objetoDe = (equipo: string, id: string) => `${equipo}/${id}`;
export const claveNota = (tipo: TipoNota, objeto: string) => `${tipo}:${objeto}`;

/** Escribir (observaciones y comentarios): técnico o más. */
export const puedeEscribirNotas = (rol: Rol | undefined) => rol === "tecnico" || rol === "administrador" || rol === "propietario";

/** El índice del cliente abierto. */
export const notas = $state({
  cliente: "",
  porClave: {} as Record<string, IndiceNota>,
  cargando: false,
  /** Si el servidor no las tiene (anterior a v1.40), nada de notas en la consola. */
  disponible: true,
});

let pidiendo: Promise<void> | null = null;

/** Lee el índice de ese cliente (una vez; `forzar` para volver a leerlo). */
export function asegurarIndice(cliente: string, forzar = false): Promise<void> {
  if (!cliente) return Promise.resolve();
  if (!forzar && notas.cliente === cliente) return pidiendo ?? Promise.resolve();
  if (notas.cliente !== cliente) {
    notas.cliente = cliente;
    notas.porClave = {};
    notas.disponible = true;
    for (const k of Object.keys(detalles)) delete detalles[k];
  }
  notas.cargando = true;
  const p = api
    .indiceNotas(cliente)
    .then((r) => {
      if (notas.cliente !== cliente) return;
      notas.porClave = Object.fromEntries(r.objetos.map((o) => [claveNota(o.tipo, o.objeto), o]));
    })
    .catch((e) => {
      // Un servidor anterior no tiene la ruta: sin notas (y sin molestar).
      if (e instanceof api.ApiError && e.codigo === "no_existe" && notas.cliente === cliente) notas.disponible = false;
    })
    .finally(() => {
      if (pidiendo === p) pidiendo = null;
      notas.cargando = false;
    });
  pidiendo = p;
  return p;
}

/** Lo que tiene ese objeto (o undefined). */
export const notaDe = (tipo: TipoNota, objeto: string): IndiceNota | undefined => notas.porClave[claveNota(tipo, objeto)];

/** Tras leer o cambiar las notas de un objeto: su entrada del índice, al día. */
export function actualizarIndice(cliente: string, tipo: TipoNota, objeto: string, observacion: string | null, comentarios: number) {
  if (notas.cliente !== cliente) return;
  const k = claveNota(tipo, objeto);
  const copia = { ...notas.porClave };
  if (!observacion && comentarios === 0) delete copia[k];
  else copia[k] = { tipo, objeto, titulo: observacion ? primeraLinea(observacion) || null : null, observacion: !!observacion, comentarios, actualizada: new Date().toISOString() };
  notas.porClave = copia;
}

/** «Observaciones de la copia», etc. */
export const NOMBRE_TIPO: Record<TipoNota, string> = {
  cliente: "del cliente",
  equipo: "del equipo",
  repositorio: "del repositorio",
  copia: "de la copia",
  destino: "del destino",
};

// --- Las notas de un objeto (lo que enseñan Observaciones y Comentarios) -------

/** Las notas leídas de cada objeto del cliente abierto (por `claveNota`). */
export const detalles = $state({} as Record<string, NotasObjeto>);
const enCurso = new Map<string, Promise<NotasObjeto | null>>();

/** Lee las notas de un objeto (dos componentes que las piden a la vez comparten la petición). */
export function cargarNotas(cliente: string, tipo: TipoNota, objeto: string): Promise<NotasObjeto | null> {
  const k = `${cliente}|${claveNota(tipo, objeto)}`;
  const ya = enCurso.get(k);
  if (ya) return ya;
  const p = api
    .notasDe(cliente, tipo, objeto)
    .then((d) => {
      ponerDetalle(cliente, d);
      return d;
    })
    .catch((e) => {
      if (e instanceof api.ApiError && e.codigo === "no_existe") {
        if (notas.cliente === cliente) notas.disponible = false;
        return null;
      }
      throw e;
    })
    .finally(() => enCurso.delete(k));
  enCurso.set(k, p);
  return p;
}

/** Guarda en la caché lo leído (o cambiado) y pone al día el índice. */
export function ponerDetalle(cliente: string, d: NotasObjeto) {
  if (notas.cliente && notas.cliente !== cliente) return;
  detalles[claveNota(d.tipo, d.objeto)] = d;
  actualizarIndice(cliente, d.tipo, d.objeto, d.observacion?.texto ?? null, d.comentarios.length);
}

/** Cambia la observación si es otra (vacía: se quita). Devuelve si cambió. */
export async function guardarObservacion(cliente: string, tipo: TipoNota, objeto: string, texto: string): Promise<boolean> {
  const limpio = texto.replace(/\r\n?/g, "\n").trim();
  const conocida = detalles[claveNota(tipo, objeto)];
  // Sin leerla antes: si el índice dice que no hay, «nada» es lo mismo que nada.
  const previa = conocida ? (conocida.observacion?.texto ?? "") : notaDe(tipo, objeto)?.observacion ? null : "";
  if (previa !== null && previa === limpio) return false;
  const obs = await api.ponerObservacion(cliente, tipo, objeto, limpio);
  const d = detalles[claveNota(tipo, objeto)];
  if (d) ponerDetalle(cliente, { ...d, observacion: obs });
  else actualizarIndice(cliente, tipo, objeto, obs?.texto ?? null, notaDe(tipo, objeto)?.comentarios ?? 0);
  return true;
}

/** Lo que falla en un texto (o null). */
export function errorTextoNota(texto: string): string | null {
  const n = [...texto.trim()].length;
  return n > MAX_TEXTO ? `Como mucho ${MAX_TEXTO} caracteres (llevas ${n}).` : null;
}
