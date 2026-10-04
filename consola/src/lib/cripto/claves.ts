// Derivaciones del protocolo que comparten la consola y los agentes
// (docs/api-servidor.md §1, crates/protocolo). Todas son funciones puras; el
// Argon2id se inyecta (un worker en el navegador, hash-wasm directo en Node).
import { ed25519 } from "@noble/curves/ed25519.js";
import { hkdf } from "@noble/hashes/hkdf.js";
import { hmac } from "@noble/hashes/hmac.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { aB64, aHex, borrar, deB64, iguales, utf8 } from "./bytes";

/** Parámetros de Argon2id del protocolo: 64 MiB, 3 pasadas, 1 hilo, 32 bytes. */
export const ARGON2 = { memoriaKiB: 65536, pasadas: 3, hilos: 1, salida: 32 } as const;

/** Argon2id(clave, sal) con los parámetros de ARGON2. */
export type Argon2 = (clave: Uint8Array, sal: Uint8Array) => Promise<Uint8Array>;

// ---------------------------------------------------------------------------
// Emparejamiento
// ---------------------------------------------------------------------------

/** Código normalizado: solo letras y cifras, en mayúsculas («abcd efgh-jk» → «ABCDEFGHJK»). */
export const normalizarCodigo = (codigo: string) => codigo.replace(/[^0-9a-z]/gi, "").toUpperCase();

/** SHA-256 (hex) del código normalizado (protocolo::mensajes::code_hash). */
export const hashCodigo = (codigo: string) => aHex(sha256(utf8(normalizarCodigo(codigo))));

/** Los 4 primeros bytes big-endian mod 1 000 000, como «NNN NNN». */
function seisCifras(h: Uint8Array): string {
  const n = new DataView(h.buffer, h.byteOffset, 4).getUint32(0, false) % 1_000_000;
  const s = String(n).padStart(6, "0");
  return `${s.slice(0, 3)} ${s.slice(3)}`;
}

/** SAS v1 (fase 5, consola de escritorio): solo para los vectores de v1.json. */
export const sasV1 = (consoleSignPub: string, endpointBoxPub: string) =>
  seisCifras(sha256(utf8(`resguardo-sas-v1|${consoleSignPub}|${endpointBoxPub}`)));

/** SAS v2: lo que la consola y el equipo muestran al emparejar. */
export const sasV2 = (identidadServidor: string, boxPub: string, signPub: string) =>
  seisCifras(sha256(utf8(`resguardo-sas-v2|${identidadServidor}|${boxPub}|${signPub}`)));

/** La huella de la autoridad TLS tal como entra en el SAS v3: solo las cifras hexadecimales, en mayúsculas. */
export const huellaParaSas = (huella: string) => huella.replace(/[^0-9a-fA-F]/g, "").toUpperCase();

/**
 * SAS v3 (v1.26, agentes ≥ 0.7.10): el de v2 más la huella de la autoridad TLS.
 * El equipo pone la que fijó al vincular; la consola, `huella_ca` de `GET /api/servidor`.
 * Si alguien en medio hizo fijar su autoridad al equipo, los números no coinciden.
 */
export const sasV3 = (identidadServidor: string, boxPub: string, signPub: string, huellaCa: string) =>
  seisCifras(sha256(utf8(`resguardo-sas-v3|${identidadServidor}|${boxPub}|${signPub}|${huellaParaSas(huellaCa)}`)));

// ---------------------------------------------------------------------------
// Clave de administración
// ---------------------------------------------------------------------------

/**
 * La clave de administración en bytes: UTF-8 de su forma Unicode NFC. Así una
 * «ñ» o una tilde dan lo mismo escritas desde cualquier sistema (lo hacen igual
 * los dos lados).
 */
export const bytesClave = (claveAdmin: string) => utf8(claveAdmin.normalize("NFC"));

/** prueba_e = Argon2id(clave_admin, sal_equipo): la autorización de nivel 3 para el equipo `e`. */
export async function pruebaAdmin(argon2: Argon2, claveAdmin: string, salEquipoB64: string): Promise<Uint8Array> {
  return argon2(bytesClave(claveAdmin), deB64(salEquipoB64));
}

/** verificador_e = SHA-256(prueba_e): lo único que guarda el equipo. */
export const verificador = (prueba: Uint8Array) => sha256(prueba);

/** Material del cliente: Argon2id(clave_admin, sal_cliente). Con él salen K_cfg y K_exp. */
export async function materialCliente(argon2: Argon2, claveAdmin: string, salClienteB64: string): Promise<Uint8Array> {
  return argon2(bytesClave(claveAdmin), deB64(salClienteB64));
}

/**
 * prueba_codigo (solo en `alta`): HMAC-SHA256(código normalizado,
 * "resguardo-alta-v1|" + equipo + "|" + verificador_b64), en base64. Demuestra
 * al equipo que el alta viene de quien vio el código (el servidor solo tiene su hash).
 */
export const pruebaCodigo = (codigo: string, equipoId: string, verificadorB64: string) =>
  aB64(hmac(sha256, utf8(normalizarCodigo(codigo)), utf8(`resguardo-alta-v1|${equipoId}|${verificadorB64}`)));

const derivar = (material: Uint8Array, info: string) => hkdf(sha256, material, new Uint8Array(0), utf8(info), 32);

/** K_cfg: cifra la configuración de los equipos y firma sus etiquetas. */
export const kCfg = (material: Uint8Array) => derivar(material, "resguardo-kcfg-v1");
/** K_exp: cifra el paquete de exportación del cliente. */
export const kExp = (material: Uint8Array) => derivar(material, "resguardo-kexp-v1");

/** Etiqueta del equipo: HMAC-SHA256(K_cfg, "resguardo-etiqueta-v1|" + id + "|" + box_pub + "|" + sign_pub), en base64. */
export const etiquetaEquipo = (kcfg: Uint8Array, equipoId: string, boxPub: string, signPub: string) =>
  aB64(hmac(sha256, kcfg, utf8(`resguardo-etiqueta-v1|${equipoId}|${boxPub}|${signPub}`)));

/**
 * Comprueba que las claves públicas que da el servidor son las que el
 * administrador confirmó al emparejar. Si no coinciden, el servidor (o
 * alguien en medio) intenta desviar los secretos: no se sella nada.
 */
export function etiquetaValida(kcfg: Uint8Array, equipo: { id: string; box_pub: string; sign_pub: string; etiqueta: string | null }): boolean {
  if (!equipo.etiqueta) return false;
  const esperada = deB64(etiquetaEquipo(kcfg, equipo.id, equipo.box_pub, equipo.sign_pub));
  let real: Uint8Array;
  try {
    real = deB64(equipo.etiqueta);
  } catch {
    return false;
  }
  return iguales(esperada, real);
}

// ---------------------------------------------------------------------------
// Firmas Ed25519 (servidor y agentes)
// ---------------------------------------------------------------------------

function verificarEd25519(publicaB64: string, firmaB64: string, mensaje: string): boolean {
  try {
    return ed25519.verify(deB64(firmaB64), utf8(mensaje), deB64(publicaB64));
  } catch {
    return false;
  }
}

/** Prueba de identidad del servidor: Ed25519 sobre "resguardo-servidor-v1|" + reto + "|" + equipo_id. */
export const mensajeIdentidad = (reto: string, equipoId: string) => `resguardo-servidor-v1|${reto}|${equipoId}`;
export const identidadServidorValida = (identidadB64: string, reto: string, equipoId: string, firmaB64: string) =>
  verificarEd25519(identidadB64, firmaB64, mensajeIdentidad(reto, equipoId));

/** Lo que firma el agente al informar del resultado de una orden. */
export const mensajeResultado = (o: { id: string; seq: number; estado: string; mensaje: string | null; detalle: string | null }) =>
  `resguardo-resultado-v1|${o.id}|${o.seq}|${o.estado}|${o.mensaje ?? ""}|${o.detalle ?? ""}`;

/** ¿El resultado lo firmó de verdad el equipo (con su sign_pub)? */
export const resultadoFirmado = (
  signPubB64: string,
  o: { id: string; seq: number; estado: string; mensaje: string | null; detalle: string | null; firma_agente: string | null },
) => !!o.firma_agente && verificarEd25519(signPubB64, o.firma_agente, mensajeResultado(o));

export { borrar };
