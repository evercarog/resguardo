// Firmas de minisign en JavaScript (Node), las mismas reglas que
// crates/protocolo/src/publicacion.rs: solo firmas «prehash» (BLAKE2b-512),
// con el comentario de confianza firmado y una llave de la lista.
//
// Lo usan scripts/firmar-publicacion.mjs (para comprobar lo que firmó minisign
// antes de publicarlo) y las pruebas de la consola (vectores y e2e), que además
// firman manifiestos con las llaves de PRUEBAS (`firmarPruebas`): nunca con
// una llave de verdad.
import { createHash, createPrivateKey, createPublicKey, sign, verify } from "node:crypto";

/** Las llaves públicas de un archivo `.pub` de minisign (una o varias). */
export function leerLlaves(texto) {
  return texto
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith("#") && !l.startsWith("untrusted comment:"))
    .map((b64) => {
      const b = Buffer.from(b64, "base64");
      if (b.length !== 42 || b.subarray(0, 2).toString() !== "Ed") throw new Error(`no es una llave pública de minisign: ${b64}`);
      return { b64, id: Buffer.from(b.subarray(2, 10)).reverse().toString("hex").toUpperCase(), clave: b.subarray(10) };
    });
}

/** `null` si la firma vale; si no, por qué. Devuelve también el id de la llave. */
export function comprobarFirma(datos, firma, llaves, revocadas = []) {
  const l = firma.split(/\r?\n/);
  if (firma.length > 4096 || l.length < 4 || !l[2].startsWith("trusted comment: ")) return { error: "la firma no tiene el formato de minisign" };
  const sig = Buffer.from(l[1].trim(), "base64");
  if (sig.length !== 74) return { error: "la firma no tiene el formato de minisign" };
  const id = Buffer.from(sig.subarray(2, 10)).reverse().toString("hex").toUpperCase();
  if (sig.subarray(0, 2).toString() !== "ED") return { error: "firma del formato antiguo (sin prehash): actualiza minisign o usa -H", id };
  if (revocadas.map((r) => r.toUpperCase()).includes(id)) return { error: `la firma es de una llave revocada (${id})`, id };
  const k = llaves.find((x) => x.id === id);
  if (!k) return { error: `la firma es de una llave que no está en la lista (${id})`, id };
  const clave = createPublicKey({ key: Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), k.clave]), format: "der", type: "spki" });
  const hash = createHash("blake2b512").update(datos).digest();
  if (!verify(null, hash, clave, sig.subarray(10))) return { error: "la firma no es válida", id };
  const global = Buffer.from(l[3].trim(), "base64");
  if (global.length !== 64 || !verify(null, Buffer.concat([sig.subarray(10), Buffer.from(l[2].slice(17), "utf8")]), clave, global)) {
    return { error: "el comentario de confianza no es válido", id };
  }
  return { error: null, id };
}

/**
 * SOLO PRUEBAS: firma como `minisign -S` con una semilla Ed25519 en hex (las de
 * crates/protocolo/tests/fixtures/LLAVES-DE-PRUEBAS-LEEME.txt).
 */
export function firmarPruebas(datos, semillaHex, idHex, comentario = "timestamp:1760000000\tfile:manifiesto-agente.json") {
  const privada = createPrivateKey({ key: Buffer.concat([Buffer.from("302e020100300506032b657004220420", "hex"), Buffer.from(semillaHex, "hex")]), format: "der", type: "pkcs8" });
  const numero = Buffer.from(idHex, "hex").reverse();
  const firma = sign(null, createHash("blake2b512").update(datos).digest(), privada);
  const global = sign(null, Buffer.concat([firma, Buffer.from(comentario, "utf8")]), privada);
  return `untrusted comment: signature from minisign secret key\n${Buffer.concat([Buffer.from("ED"), numero, firma]).toString("base64")}\ntrusted comment: ${comentario}\n${global.toString("base64")}\n`;
}

/** SOLO PRUEBAS: la semilla de una llave de pruebas, de su archivo de fixtures. */
export function semillaPruebas(textoFixtures, id) {
  const l = textoFixtures.split(/\r?\n/).find((x) => x.startsWith(`${id}=`));
  if (!l) throw new Error(`no está la llave de pruebas ${id}`);
  return l.slice(id.length + 1).trim();
}
