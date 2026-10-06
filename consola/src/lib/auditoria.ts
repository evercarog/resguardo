// Comprobación de la cadena de auditoría en el navegador, además de la del
// servidor: cada entrada lleva la huella de la anterior y su propia huella es
//   SHA-256(prev_hash | n | creado (segundos Unix) | actor | accion | objetivo | datos)
// (crates/servidor/src/almacen/sqlite.rs, hash_entrada). Si alguien cambia o
// quita una entrada de las que se ven aquí, deja de cuadrar.
import { sha256 } from "@noble/hashes/sha2.js";
import { aHex, utf8 } from "./cripto/bytes";
import type { EntradaAuditoria } from "./tipos";

export const GENESIS = "0".repeat(64);

export const hashEntrada = (e: EntradaAuditoria) =>
  aHex(sha256(utf8(`${e.prev_hash}|${e.n}|${Math.floor(Date.parse(e.creado) / 1000)}|${e.actor}|${e.accion}|${e.objetivo}|${e.datos}`)));

/** Comprueba las entradas dadas (en cualquier orden): devuelve el n de la primera que no cuadra, o null. */
export function primeraRota(entradas: EntradaAuditoria[]): number | null {
  const orden = [...entradas].sort((a, b) => a.n - b.n);
  for (let i = 0; i < orden.length; i++) {
    const e = orden[i];
    if (hashEntrada(e) !== e.hash) return e.n;
    const prev = orden[i - 1];
    if (prev && prev.n === e.n - 1 && prev.hash !== e.prev_hash) return e.n;
    if (e.n === 1 && e.prev_hash !== GENESIS) return e.n;
  }
  return null;
}

// ---------------------------------------------------------------------------
// Ancla externa (plan-mejoras 9b; docs/plataforma.md §7.3.1). El resumen por correo
// lleva la cabeza de la cadena de cada cliente en una línea:
//   resguardo-ancla:1:<cliente>:<n>:<creado, segundos Unix>:<huella>
// (crates/protocolo/src/derivaciones.rs, linea_ancla). Con ella, este navegador
// comprueba que la cadena de hoy, recalculada entera desde el principio, sigue
// teniendo esa huella en esa entrada: si el servidor la rehízo, no cuadra.
// ---------------------------------------------------------------------------

export interface Ancla {
  cliente: string;
  n: number;
  /** La hora de esa entrada (segundos Unix). */
  creado: number;
  hash: string;
}

/** La línea de un ancla, como la escribe el servidor. */
export const lineaAncla = (a: Ancla) => `resguardo-ancla:1:${a.cliente}:${a.n}:${a.creado}:${a.hash}`;

/** El ancla de una entrada de la cadena de este cliente. */
export const anclaDe = (cliente: string, e: EntradaAuditoria): Ancla => ({ cliente, n: e.n, creado: Math.floor(Date.parse(e.creado) / 1000), hash: e.hash });

/**
 * El ancla de un texto pegado (la línea sola o el correo entero). Sin espacios ni
 * saltos de línea: el correo puede partir la línea. `null` si no hay ninguna.
 */
export function leerAncla(texto: string): Ancla | null {
  const limpio = texto.replace(/[\s­​]+/g, "");
  const m = /resguardo-ancla:1:([A-Za-z0-9-]{1,64}):(\d{1,15}):(\d{1,15}):([0-9a-fA-F]{64})/.exec(limpio);
  if (!m) return null;
  const n = Number(m[2]);
  if (!Number.isSafeInteger(n) || n < 1) return null;
  return { cliente: m[1], n, creado: Number(m[3]), hash: m[4].toLowerCase() };
}

/** Lo que dice la cadena de hoy frente a un ancla. */
export type Comprobacion =
  /** La cadena está entera y la entrada del ancla sigue igual: nada de lo anterior cambió. */
  | { estado: "bien"; total: number }
  /** La cadena de hoy no cuadra consigo misma (falta o cambió una entrada) desde `en`. */
  | { estado: "rota"; en: number }
  /** La cadena de hoy es más corta que el ancla: la rehicieron o se restauró una copia anterior. */
  | { estado: "falta"; total: number }
  /** La entrada del ancla tiene hoy otra huella: la rehicieron. */
  | { estado: "distinta"; ahora: EntradaAuditoria };

/**
 * Comprueba la cadena **entera** (desde la primera entrada, sin huecos) y la
 * compara con el ancla. `entradas`, en cualquier orden.
 */
export function comprobarAncla(entradas: EntradaAuditoria[], ancla: Ancla): Comprobacion {
  const orden = [...entradas].sort((a, b) => a.n - b.n);
  let prev = GENESIS;
  for (let i = 0; i < orden.length; i++) {
    const e = orden[i];
    if (e.n !== i + 1) return { estado: "rota", en: i + 1 };
    if (e.prev_hash !== prev || hashEntrada(e) !== e.hash) return { estado: "rota", en: e.n };
    prev = e.hash;
  }
  const e = orden[ancla.n - 1];
  if (!e) return { estado: "falta", total: orden.length };
  if (e.hash !== ancla.hash) return { estado: "distinta", ahora: e };
  return { estado: "bien", total: orden.length };
}
