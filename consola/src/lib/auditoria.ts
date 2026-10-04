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
