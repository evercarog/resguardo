// Cifrado simétrico de las sesiones interactivas y del relé de descargas
// (XChaCha20-Poly1305), como en api-servidor.md §1 (v1.2) y
// crates/protocolo/src/simetrico.rs (vectores: «simetrico» en v1.json).
//
// Sesión:
//   clave_sesion = 32 bytes aleatorios de la consola, dentro del sobre de la orden.
//   k_dir        = HKDF-SHA256(clave_sesion, salt = sesion_id, info = "resguardo-sesion-v1|consola" o "|equipo")
//   mensaje      = nonce(24) ‖ XChaCha20-Poly1305(k_dir, nonce, json, aad = "resguardo-sesion-v1|" + sesion_id)
//   El JSON lleva un contador `i` creciente por emisor: el receptor descarta repeticiones.
//
// Relé:
//   clave_relevo = 32 bytes aleatorios de la consola, dentro del sobre de la orden `descargar`.
//   trozo n      = nonce(24) ‖ XChaCha20-Poly1305(clave_relevo, nonce, datos, aad = relevo_id + "|" + n + "|" + (último ? 1 : 0))
//   Así no se pueden reordenar, repetir ni recortar trozos sin que se note.
import { xchacha20poly1305 } from "@noble/ciphers/chacha.js";
import { hkdf } from "@noble/hashes/hkdf.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { aleatorio, concat, deUtf8, utf8 } from "./bytes";

export type Lado = "consola" | "equipo";

/** Clave de una dirección de la sesión. */
export const claveDireccion = (claveSesion: Uint8Array, sesionId: string, de: Lado) =>
  hkdf(sha256, claveSesion, utf8(sesionId), utf8(`resguardo-sesion-v1|${de}`), 32);

const aadSesion = (sesionId: string) => utf8(`resguardo-sesion-v1|${sesionId}`);

export function cifrarMensaje(k: Uint8Array, sesionId: string, datos: unknown, nonce = aleatorio(24)): Uint8Array {
  return concat(nonce, xchacha20poly1305(k, nonce, aadSesion(sesionId)).encrypt(utf8(JSON.stringify(datos))));
}

export function descifrarMensaje<T = unknown>(k: Uint8Array, sesionId: string, cifrado: Uint8Array): T {
  if (cifrado.length < 24 + 16) throw new Error("Mensaje de sesión demasiado corto");
  const plano = xchacha20poly1305(k, cifrado.subarray(0, 24), aadSesion(sesionId)).decrypt(cifrado.subarray(24));
  return JSON.parse(deUtf8(plano)) as T;
}

const aadTrozo = (relevoId: string, n: number, ultimo: boolean) => utf8(`${relevoId}|${n}|${ultimo ? 1 : 0}`);

export function cifrarTrozo(k: Uint8Array, relevoId: string, n: number, ultimo: boolean, datos: Uint8Array, nonce = aleatorio(24)) {
  return concat(nonce, xchacha20poly1305(k, nonce, aadTrozo(relevoId, n, ultimo)).encrypt(datos));
}

export function descifrarTrozo(k: Uint8Array, relevoId: string, n: number, ultimo: boolean, trozo: Uint8Array): Uint8Array {
  if (trozo.length < 24 + 16) throw new Error("Trozo demasiado corto");
  return xchacha20poly1305(k, trozo.subarray(0, 24), aadTrozo(relevoId, n, ultimo)).decrypt(trozo.subarray(24));
}

/** Tamaño de trozo del relé (sin cifrar): 4 MiB. */
export const TROZO = 4 * 1024 * 1024;

// Configuración del equipo (api-servidor.md §1): la cifra el agente con
// K_cfg y la sube al servidor; la consola la descifra con la clave de
// administración para editarla. Atada al equipo y a su número.
//   cifrado = nonce(24) ‖ XChaCha20-Poly1305(K_cfg, nonce, json, aad = "resguardo-config-v1|" + equipo_id + "|" + seq)
const aadConfig = (equipoId: string, seq: number) => utf8(`resguardo-config-v1|${equipoId}|${seq}`);

export function cifrarConfig(kcfg: Uint8Array, equipoId: string, seq: number, config: unknown, nonce = aleatorio(24)) {
  return concat(nonce, xchacha20poly1305(kcfg, nonce, aadConfig(equipoId, seq)).encrypt(utf8(JSON.stringify(config))));
}

// Plantillas de copia (v1.20): cifradas con una clave derivada de K_cfg, atadas
// al cliente y a su id. El servidor las guarda sin poder leerlas.
//   K_pla   = HKDF-SHA256(ikm = K_cfg, salt = "", info = "resguardo-kplantilla-v1")
//   cifrado = nonce(24) ‖ XChaCha20-Poly1305(K_pla, nonce, json, aad = "resguardo-plantilla-v1|" + cliente + "|" + id)
export const clavePlantillas = (kcfg: Uint8Array) => hkdf(sha256, kcfg, new Uint8Array(0), utf8("resguardo-kplantilla-v1"), 32);
const aadPlantilla = (cliente: string, id: string) => utf8(`resguardo-plantilla-v1|${cliente}|${id}`);

export function cifrarPlantilla(kpla: Uint8Array, cliente: string, id: string, plantilla: unknown, nonce = aleatorio(24)) {
  return concat(nonce, xchacha20poly1305(kpla, nonce, aadPlantilla(cliente, id)).encrypt(utf8(JSON.stringify(plantilla))));
}

export function descifrarPlantilla<T = unknown>(kpla: Uint8Array, cliente: string, id: string, cifrado: Uint8Array): T {
  if (cifrado.length < 24 + 16) throw new Error("Plantilla dañada");
  const plano = xchacha20poly1305(kpla, cifrado.subarray(0, 24), aadPlantilla(cliente, id)).decrypt(cifrado.subarray(24));
  return JSON.parse(deUtf8(plano)) as T;
}

export function descifrarConfig<T = unknown>(kcfg: Uint8Array, equipoId: string, seq: number, cifrado: Uint8Array): T {
  const plano = xchacha20poly1305(kcfg, cifrado.subarray(0, 24), aadConfig(equipoId, seq)).decrypt(cifrado.subarray(24));
  return JSON.parse(deUtf8(plano)) as T;
}
