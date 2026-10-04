// «Varias consolas a la vez» (docs/consolas-multiples.md): el código de
// conexión que da una consola para que otra le conecte sus equipos, y lo que
// hace falta para mandar `anadir_consola`.
//
// El código es una línea: «RGC1.» y el JSON en base64url. Lleva la dirección
// de esta consola, su identidad Ed25519, la huella de su autoridad TLS, una
// ficha de «Recibir un cliente» (de un solo uso por equipo, con caducidad), la
// sal del cliente aquí y un nombre para enseñar. La ficha es lo único que no es
// público: viaja dentro de la orden sellada para cada equipo, nunca en claro
// por el otro servidor (el código solo se copia y se pega).
import { aB64, deB64, deUtf8, utf8 } from "./cripto/bytes";

export const PREFIJO = "RGC1.";

export interface CodigoConexion {
  /** Dirección para los agentes (https://…, sin barra al final): v1.34 `url_agentes` de esa
   * consola (en una consola en internet, `agentes.<dominio>`, con su autoridad TLS propia), o
   * su propia dirección si no la da. */
  url: string;
  /** Identidad Ed25519 (base64). */
  identidad: string;
  /** Huella SHA-256 de su autoridad TLS: 64 cifras hexadecimales en mayúsculas (sin «:»). */
  huella_ca: string;
  ficha: string;
  /** Sal del cliente en esa consola (base64): con ella se calcula su K_cfg. */
  sal_cliente: string;
  /** Cómo se llama esa consola (para enseñarlo en los equipos). */
  nombre: string;
  /** Nombre del cliente allí (solo para enseñarlo). */
  cliente: string;
  /** Hasta cuándo vale la ficha (RFC 3339). */
  caduca: string;
}

const aB64url = (b: Uint8Array) => aB64(b).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
function deB64url(s: string): Uint8Array {
  const t = s.replace(/-/g, "+").replace(/_/g, "/");
  return deB64(t + "=".repeat((4 - (t.length % 4)) % 4));
}

/** Solo las cifras hexadecimales, en mayúsculas («AB:CD:…» → «ABCD…»). */
export const huellaHex = (h: string) => h.replace(/[^0-9a-fA-F]/g, "").toUpperCase();
/** «ABCD…» → «AB:CD:…», para leerla de palabra. */
export const huellaConPuntos = (h: string) => (huellaHex(h).match(/.{1,2}/g) ?? []).join(":");

/** El código para copiar en la otra consola. */
export function crearCodigo(c: CodigoConexion): string {
  const x = { v: 1, u: c.url, i: c.identidad, h: huellaHex(c.huella_ca), f: c.ficha, s: c.sal_cliente, n: c.nombre, k: c.cliente, c: c.caduca };
  return PREFIJO + aB64url(utf8(JSON.stringify(x)));
}

function esLlave(b64: unknown, min = 32, max = 32) {
  if (typeof b64 !== "string") return false;
  try {
    const n = deB64(b64).length;
    return n >= min && n <= max;
  } catch {
    return false;
  }
}

/** Lee un código pegado. Devuelve los datos o, si no vale, un texto con lo que falla. */
export function leerCodigo(texto: string, ahora = new Date(), origenPropio?: string | string[]): CodigoConexion | string {
  const t = texto.replace(/\s+/g, "");
  if (!t) return "Pega el código de conexión.";
  if (!t.startsWith(PREFIJO)) return "Eso no es un código de conexión: empieza por «RGC1.». Pídelo en la otra consola («Recibir un cliente» o «Dar un código de conexión»).";
  let x: Record<string, unknown>;
  try {
    x = JSON.parse(deUtf8(deB64url(t.slice(PREFIJO.length))));
  } catch {
    return "El código está incompleto o dañado: cópialo entero otra vez.";
  }
  if (x.v !== 1) return "Este código es de una versión más nueva de Resguardo Server: actualiza esta consola.";
  const url = typeof x.u === "string" ? x.u.trim().replace(/\/+$/, "") : "";
  if (!/^https:\/\/[^\s/?#@]+$/i.test(url)) return "La dirección de la otra consola no es válida (https://…).";
  if (origenPropio && [origenPropio].flat().includes(url)) return "Ese código es de esta misma consola: pídelo en la otra.";
  if (!esLlave(x.i)) return "La identidad de la otra consola no es válida.";
  const huella = typeof x.h === "string" ? huellaHex(x.h) : "";
  if (huella.length !== 64) return "Falta la huella de la autoridad TLS de la otra consola.";
  if (typeof x.f !== "string" || x.f.length < 16 || x.f.length > 128) return "Falta la ficha.";
  if (!esLlave(x.s, 16, 64)) return "Falta la sal del cliente de la otra consola.";
  const caduca = typeof x.c === "string" ? x.c : "";
  const fin = Date.parse(caduca);
  if (!Number.isFinite(fin)) return "El código no dice hasta cuándo vale.";
  if (fin <= ahora.getTime()) return "El código caducó: pide otro en la otra consola.";
  const nombre = typeof x.n === "string" && x.n.trim() ? x.n.trim().slice(0, 80) : new URL(url).host;
  return {
    url,
    identidad: x.i as string,
    huella_ca: huella,
    ficha: x.f,
    sal_cliente: x.s as string,
    nombre,
    cliente: typeof x.k === "string" ? x.k.slice(0, 80) : "",
    caduca,
  };
}

/** El host de una dirección (para enseñarla corta). */
export function hostDe(url: string) {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

/** El cuerpo de `anadir_consola` (sin la K_cfg, que se calcula con la clave). */
export const cuerpoAnadir = (c: CodigoConexion, kCfgB64: string, salOrigen: string) => ({
  url: c.url,
  identidad: c.identidad,
  huella_ca: huellaConPuntos(c.huella_ca),
  ficha: c.ficha,
  sal_cliente: c.sal_cliente,
  k_cfg: kCfgB64,
  nombre: c.nombre,
  sal_origen: salOrigen,
});
