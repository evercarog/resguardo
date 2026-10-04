// Argon2id en un worker: 64 MiB y 3 pasadas tardan un momento y no deben
// congelar la página.
//
// Primero se usa hash-wasm (WASM, la implementación de referencia en C): es
// varias veces más rápido, sobre todo en móviles, y por eso la CSP permite
// 'wasm-unsafe-eval'. Si el navegador o una CSP más estricta no dejan usar
// WASM, se recurre a @noble/hashes (JavaScript puro, ~1 s en un PC). Las dos
// dan lo mismo: scripts/vectores.ts lo comprueba.
// La clave entra como bytes y el worker los borra al terminar.
import { argon2id } from "hash-wasm";
import { argon2id as argon2idJs } from "@noble/hashes/argon2.js";
import { ARGON2 } from "./claves";

let sinWasm = false;

async function calcular(clave: Uint8Array, sal: Uint8Array): Promise<Uint8Array> {
  if (!sinWasm) {
    try {
      return (await argon2id({
        password: clave,
        salt: sal,
        parallelism: ARGON2.hilos,
        iterations: ARGON2.pasadas,
        memorySize: ARGON2.memoriaKiB,
        hashLength: ARGON2.salida,
        outputType: "binary",
      })) as Uint8Array;
    } catch {
      sinWasm = true;
    }
  }
  return argon2idJs(clave, sal, { t: ARGON2.pasadas, m: ARGON2.memoriaKiB, p: ARGON2.hilos, dkLen: ARGON2.salida });
}

self.onmessage = async (e: MessageEvent<{ id: number; clave: Uint8Array; sal: Uint8Array }>) => {
  const { id, clave, sal } = e.data;
  try {
    const out = await calcular(clave, sal);
    (self as unknown as Worker).postMessage({ id, out }, [out.buffer]);
  } catch (err) {
    (self as unknown as Worker).postMessage({ id, error: String(err) });
  } finally {
    clave.fill(0);
  }
};
