// Paquete de exportación de un cliente (`.resguardo-cliente`, api-servidor.md
// §11, crates/protocolo/src/paquete.rs). Se cifra y se descifra aquí, con
// K_exp; los servidores solo guardan el cifrado.
//
//   "RESGUARDO-CLIENTE-1\n" ‖ sal_cliente (b64) ‖ "\n" ‖ trozo₀ ‖ trozo₁ ‖ …
//   trozoₙ = u32 big-endian (longitud) ‖ nonce(24) ‖ XChaCha20-Poly1305(K_exp, nonce, datosₙ,
//            aad = "resguardo-cliente-v1|" + n + "|" + (último ? "1" : "0"))
import { concat, utf8 } from "./bytes";
import { cifrarTrozo, descifrarTrozo, TROZO } from "./simetrico";

const MAGIA = utf8("RESGUARDO-CLIENTE-1\n");
const ID = "resguardo-cliente-v1";

/** Cifra el paquete. `nonce(n)` solo en las pruebas (aleatorio en uso normal). */
export function cifrarPaquete(kexp: Uint8Array, salClienteB64: string, json: Uint8Array, nonce?: (n: number) => Uint8Array): Uint8Array {
  const partes: Uint8Array[] = [MAGIA, utf8(salClienteB64), utf8("\n")];
  const total = Math.max(1, Math.ceil(json.length / TROZO));
  for (let n = 0; n < total; n++) {
    const c = cifrarTrozo(kexp, ID, n, n + 1 === total, json.subarray(n * TROZO, (n + 1) * TROZO), nonce?.(n));
    const len = new Uint8Array(4);
    new DataView(len.buffer).setUint32(0, c.length, false);
    partes.push(len, c);
  }
  return concat(...partes);
}

/** La sal del cliente (para derivar K_exp) y dónde empiezan los trozos. */
export function cabeceraPaquete(p: Uint8Array): { sal: string; inicio: number } {
  if (p.length < MAGIA.length || !MAGIA.every((b, i) => p[i] === b)) throw new Error("No es un paquete de cliente de Resguardo.");
  const fin = p.indexOf(10, MAGIA.length);
  if (fin < 0 || fin - MAGIA.length > 128) throw new Error("Paquete dañado (cabecera).");
  return { sal: new TextDecoder().decode(p.subarray(MAGIA.length, fin)), inicio: fin + 1 };
}

/** Descifra el paquete. Falla si falta o sobra algún trozo, o si la clave no es la del cliente. */
export function descifrarPaquete(kexp: Uint8Array, p: Uint8Array): { sal: string; json: Uint8Array } {
  const { sal, inicio } = cabeceraPaquete(p);
  const partes: Uint8Array[] = [];
  let i = inicio;
  for (let n = 0; ; n++) {
    if (i + 4 > p.length) throw new Error("Paquete incompleto.");
    const len = new DataView(p.buffer, p.byteOffset + i, 4).getUint32(0, false);
    i += 4;
    if (i + len > p.length) throw new Error("Paquete incompleto.");
    const trozo = p.subarray(i, i + len);
    i += len;
    const ultimo = i === p.length;
    try {
      partes.push(descifrarTrozo(kexp, ID, n, ultimo, trozo));
    } catch {
      throw new Error("No se pudo descifrar el paquete: la clave de administración no es la de este cliente, o el archivo está dañado.");
    }
    if (ultimo) return { sal, json: concat(...partes) };
  }
}
