// Conversión de bytes: base64 estándar con relleno (como en todo el protocolo),
// UTF-8 y hexadecimal. Sin dependencias, para el navegador y para Node (pruebas).

const enc = new TextEncoder();
const dec = new TextDecoder("utf-8", { fatal: true });

export const utf8 = (s: string): Uint8Array => enc.encode(s);
export const deUtf8 = (b: Uint8Array): string => dec.decode(b);

export function aB64(b: Uint8Array): string {
  let s = "";
  // En trozos: String.fromCharCode con muchos argumentos desborda la pila.
  for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode(...b.subarray(i, i + 0x8000));
  return btoa(s);
}

export function deB64(s: string): Uint8Array {
  if (!/^[A-Za-z0-9+/]*={0,2}$/.test(s) || s.length % 4 !== 0) throw new Error("Base64 no válido");
  const bin = atob(s);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

export const aHex = (b: Uint8Array): string => Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");

export function concat(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

/** Comparación en tiempo constante (para etiquetas y huellas). */
export function iguales(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  let d = 0;
  for (let i = 0; i < a.length; i++) d |= a[i] ^ b[i];
  return d === 0;
}

/** Borra un búfer con secretos en cuanto deja de hacer falta. */
export function borrar(...bufs: (Uint8Array | undefined | null)[]) {
  for (const b of bufs) b?.fill(0);
}

/** Bytes aleatorios del sistema (WebCrypto, también en Node ≥ 19). */
export function aleatorio(n: number): Uint8Array {
  const b = new Uint8Array(n);
  crypto.getRandomValues(b);
  return b;
}
