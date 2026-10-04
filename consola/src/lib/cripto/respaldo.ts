// Clave de respaldo de la consola (v1.23, «Copia de la consola»; ver
// crates/protocolo/src/respaldo_consola.rs). La elige el propietario y se queda
// en el navegador: el servidor solo recibe la clave pública X25519 que sale de
// ella (y su sal), con la que sella cada copia nocturna sin poder abrirla.
//
//   secreto = HKDF-SHA256(Argon2id(NFC(clave), sal), salt = "", info = "resguardo-respaldo-consola-v1")
//   publica = X25519(secreto)
import { hkdf } from "@noble/hashes/hkdf.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { x25519 } from "@noble/curves/ed25519.js";
import { aB64, aleatorio, borrar, deB64, utf8 } from "./bytes";
import { bytesClave, type Argon2 } from "./claves";

/** Largo mínimo (como en el servidor y en `resguardo-server restaurar-respaldo`). */
export const MIN_CLAVE_RESPALDO = 12;

/** Una sal nueva (16 bytes, base64). */
export const salRespaldo = () => aB64(aleatorio(16));

/** El secreto X25519 de la clave de respaldo (solo para las pruebas; la consola no lo guarda). */
export async function secretoRespaldo(argon2: Argon2, clave: string, salB64: string): Promise<Uint8Array> {
  const material = await argon2(bytesClave(clave), deB64(salB64));
  const secreto = hkdf(sha256, material, new Uint8Array(0), utf8("resguardo-respaldo-consola-v1"), 32);
  borrar(material);
  return secreto;
}

/** La clave pública (base64) de la clave de respaldo: lo único que se manda al servidor. */
export async function publicaRespaldo(argon2: Argon2, clave: string, salB64: string): Promise<string> {
  const secreto = await secretoRespaldo(argon2, clave, salB64);
  const publica = aB64(x25519.getPublicKey(secreto));
  borrar(secreto);
  return publica;
}
