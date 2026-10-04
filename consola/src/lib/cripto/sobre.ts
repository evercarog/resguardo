// Sobre sellado compatible con crypto_box_seal de libsodium (y con el crate
// crypto_box que usa crates/protocolo):
//
//   epk, esk  = X25519 efímera
//   k         = HSalsa20(X25519(esk, pk), 0¹⁶)       (crypto_box_beforenm)
//   nonce     = BLAKE2b-24(epk ‖ pk)
//   sobre     = epk ‖ XSalsa20-Poly1305(k, nonce, m)  (etiqueta delante, 16 bytes)
//
// Se usa @noble (JavaScript puro, auditado, sin WASM) para no ampliar la CSP
// con más WASM del imprescindible y para que el resultado sea idéntico en
// todos los navegadores. scripts/vectores.ts lo contrasta con libsodium.
import { x25519 } from "@noble/curves/ed25519.js";
import { hsalsa, xsalsa20poly1305 } from "@noble/ciphers/salsa.js";
import { blake2b } from "@noble/hashes/blake2.js";
import { aB64, borrar, concat, deB64 } from "./bytes";

const SIGMA = new Uint32Array([0x61707865, 0x3320646e, 0x79622d32, 0x6b206574]); // "expand 32-byte k"

const u32 = (b: Uint8Array) => {
  const out = new Uint32Array(b.length / 4);
  const v = new DataView(b.buffer, b.byteOffset, b.byteLength);
  for (let i = 0; i < out.length; i++) out[i] = v.getUint32(i * 4, true);
  return out;
};
const deU32 = (w: Uint32Array) => {
  const out = new Uint8Array(w.length * 4);
  const v = new DataView(out.buffer);
  w.forEach((x, i) => v.setUint32(i * 4, x, true));
  return out;
};

/** crypto_box_beforenm: la clave simétrica de un par X25519. */
function claveCaja(publica: Uint8Array, secreta: Uint8Array): Uint8Array {
  const compartida = x25519.getSharedSecret(secreta, publica);
  const out = new Uint32Array(8);
  hsalsa(SIGMA, u32(compartida), new Uint32Array(4), out);
  borrar(compartida);
  return deU32(out);
}

const nonceSellado = (epk: Uint8Array, pk: Uint8Array) => blake2b(concat(epk, pk), { dkLen: 24 });

/**
 * Sella `mensaje` para la clave pública X25519 `publica` (32 bytes).
 * `efimeraSecreta` solo se pasa en las pruebas, para resultados fijos.
 */
export function sellar(publica: Uint8Array, mensaje: Uint8Array, efimeraSecreta?: Uint8Array): Uint8Array {
  if (publica.length !== 32) throw new Error("La clave pública del equipo no es válida");
  const esk = efimeraSecreta ?? x25519.utils.randomSecretKey();
  const epk = x25519.getPublicKey(esk);
  const k = claveCaja(publica, esk);
  const caja = xsalsa20poly1305(k, nonceSellado(epk, publica)).encrypt(mensaje);
  borrar(k);
  if (!efimeraSecreta) borrar(esk);
  return concat(epk, caja);
}

/** crypto_box_seal_open: abre un sobre con el par X25519 del destinatario. */
export function abrir(secreta: Uint8Array, sobre: Uint8Array): Uint8Array {
  if (sobre.length < 48) throw new Error("Sobre demasiado corto");
  const pk = x25519.getPublicKey(secreta);
  const epk = sobre.subarray(0, 32);
  const k = claveCaja(epk, secreta);
  try {
    return xsalsa20poly1305(k, nonceSellado(epk, pk)).decrypt(sobre.subarray(32));
  } finally {
    borrar(k);
  }
}

/** Atajos en base64 (como viajan en JSON). */
export const sellarB64 = (publicaB64: string, mensaje: Uint8Array) => aB64(sellar(deB64(publicaB64), mensaje));
export const abrirB64 = (secreta: Uint8Array, sobreB64: string) => abrir(secreta, deB64(sobreB64));

/** Par X25519 efímero (para `responder_a` y las sesiones). */
export function parEfimero() {
  const secreta = x25519.utils.randomSecretKey();
  return { secreta, publica: x25519.getPublicKey(secreta) };
}
