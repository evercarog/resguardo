// La cola del instalador «listo» armada en el navegador (v1.48). Mismo formato y mismas
// comprobaciones que crates/protocolo/src/instalador.rs (`cola`, `DatosInstalador::validar`):
//
//   "RESGUARDO-COLA-1" ‖ u32 BE n ‖ JSON (n bytes) ‖ u32 BE n ‖ "RESGUARDO-FIN-01"
//
// El servidor da el instalador genérico (`GET …/instalador-agente`) y la consola le añade la
// cola con el código que generó ella: el servidor nunca lo ve. Los vectores de
// crates/protocolo/vectors/instalador.json comprueban que los bytes son los mismos que en Rust.
import { utf8 } from "./cripto/bytes";

export interface DatosInstalador {
  v: 1;
  /** `https://servidor:puerto` (sin ruta). */
  servidor: string;
  /** SHA-256 de la autoridad TLS del servidor, `AB:CD:…` (32 pares). */
  huella_ca: string;
  cliente: string;
  /** Nombre del equipo en la consola. */
  nombre: string;
  /** Código de emparejamiento (un solo uso, caduca). */
  codigo: string;
}

const INICIO = "RESGUARDO-COLA-1";
const FIN = "RESGUARDO-FIN-01";
export const MAX_JSON = 4096;

/** Lo que no cuadre, fuera (como `DatosInstalador::validar`): `null` si vale, si no, el motivo. */
export function validarDatos(d: DatosInstalador): string | null {
  if (d.v !== 1) return "Versión de la cola no admitida.";
  if (!d.servidor.startsWith("https://")) return "La dirección del servidor tiene que empezar por https://.";
  const resto = d.servidor.slice("https://".length);
  if (!resto || resto.length > 200 || !/^[A-Za-z0-9.:[\]-]+$/.test(resto) || resto.startsWith(":") || resto.endsWith(":"))
    return "Dirección del servidor no válida (https://servidor:puerto, sin ruta).";
  if (!/^[0-9A-Fa-f]{2}(:[0-9A-Fa-f]{2}){31}$/.test(d.huella_ca)) return "Huella de la autoridad TLS no válida.";
  if (!d.cliente || d.cliente.length > 64 || !/^[A-Za-z0-9-]+$/.test(d.cliente)) return "Cliente no válido.";
  const n = [...d.nombre].length;
  if (n === 0 || n > 80 || /^\p{White_Space}|\p{White_Space}$/u.test(d.nombre) || /[\p{Cc}"]/u.test(d.nombre)) return "Nombre del equipo no válido.";
  if (d.codigo.length < 8 || d.codigo.length > 20 || !/^[A-Za-z0-9-]+$/.test(d.codigo)) return "Código de emparejamiento no válido.";
  return null;
}

/** Los bytes de la cola (para añadirla al final del instalador genérico). */
export function cola(d: DatosInstalador): Uint8Array {
  const mal = validarDatos(d);
  if (mal) throw new Error(mal);
  // El mismo orden de campos que serde (y sin espacios): los mismos bytes que en Rust.
  const json = utf8(JSON.stringify({ v: d.v, servidor: d.servidor, huella_ca: d.huella_ca, cliente: d.cliente, nombre: d.nombre, codigo: d.codigo }));
  if (json.length > MAX_JSON) throw new Error("Datos del instalador demasiado largos.");
  const out = new Uint8Array(16 + 4 + json.length + 4 + 16);
  const v = new DataView(out.buffer);
  out.set(utf8(INICIO), 0);
  v.setUint32(16, json.length, false);
  out.set(json, 20);
  v.setUint32(20 + json.length, json.length, false);
  out.set(utf8(FIN), 24 + json.length);
  return out;
}

/** «Resguardo-Agente_<cliente>_<equipo>.exe», solo con letras y cifras ASCII, `-` y `_` (como `nombre_archivo` en el servidor). */
export function nombreArchivo(cliente: string, equipo: string): string {
  const tildes: Record<string, string> = { á: "a", à: "a", ä: "a", Á: "a", À: "a", Ä: "a", é: "e", è: "e", ë: "e", É: "e", È: "e", Ë: "e", í: "i", ì: "i", ï: "i", Í: "i", Ì: "i", Ï: "i", ó: "o", ò: "o", ö: "o", Ó: "o", Ò: "o", Ö: "o", ú: "u", ù: "u", ü: "u", Ú: "u", Ù: "u", Ü: "u", ñ: "n", Ñ: "N" };
  const limpio = (s: string) => {
    const t = [...s].map((c) => tildes[c] ?? (/^[A-Za-z0-9_-]$/.test(c) ? c : "-")).join("");
    return [
      ...t
        .split("-")
        .filter((x) => x)
        .join("-"),
    ]
      .slice(0, 40)
      .join("");
  };
  const [c, e] = [limpio(cliente), limpio(equipo)];
  if (c && e) return `Resguardo-Agente_${c}_${e}.exe`;
  if (e) return `Resguardo-Agente_${e}.exe`;
  return "Resguardo-Agente.exe";
}
