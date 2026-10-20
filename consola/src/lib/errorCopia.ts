// Los errores de las copias, en palabras (plan-mejoras 15oo).
//
// La tabla es la misma que usan el agente y el servidor
// (crates/protocolo/vectors/errores-copia.json, que lee crates/motor/src/errores.rs):
// un agente nuevo ya manda la frase, y aquí se traduce lo que mandaban los anteriores
// (el error del motor de copias tal cual: en inglés, en JSON y cortado a 160 caracteres).
// Las pruebas del mismo archivo las pasan los dos lados (scripts/vectores-errores.ts).

import tabla from "../../../crates/protocolo/vectors/errores-copia.json";

interface Regla {
  caso: string;
  alguna?: string[];
  todas?: string[];
  ninguna?: string[];
  frase: string;
  frase_linux?: string;
}

const REGLAS = tabla.reglas as Regla[];
/** Frase para un error que la tabla no reconoce. */
export const DESCONOCIDO: string = tabla.desconocido;
/** Lo que separa la frase del texto original cuando van en una sola línea. */
export const MARCA_DETALLE: string = tabla.marca_detalle;

export interface ErrorEnPalabras {
  /** El caso de la tabla, `desconocido` o `en_palabras` (ya venía en español). */
  caso: string;
  frase: string;
  /** El texto técnico completo, si lo hay y no es la frase. */
  detalle: string | null;
}

const cumple = (r: Regla, s: string) =>
  (!r.alguna?.length || r.alguna.some((a) => s.includes(a))) && (r.todas ?? []).every((t) => s.includes(t)) && !(r.ninguna ?? []).some((n) => s.includes(n));

/** El `message` de una línea JSON que no se puede leer entera (cortada). */
function mensajeCortado(linea: string): string | null {
  const i = linea.indexOf('"message":"');
  if (i < 0) return null;
  let out = "";
  let escapado = false;
  for (const c of linea.slice(i + 11)) {
    if (escapado) {
      out += c === "n" ? " " : c;
      escapado = false;
    } else if (c === "\\") escapado = true;
    else if (c === '"') break;
    else out += c;
  }
  return out;
}

/** Las líneas útiles del texto técnico (sin JSON alrededor ni el «Falló: » de antes). */
export function textoTecnico(texto: string): string {
  const out: string[] = [];
  for (const cruda of texto.split(/\r?\n/).map((l) => l.trim()).filter(Boolean)) {
    const linea = cruda.startsWith("Falló:") ? cruda.slice(6).trimStart() : cruda;
    let util: string | null = linea;
    if (linea.startsWith("{")) {
      try {
        const v = JSON.parse(linea) as { message_type?: string; message?: string; error?: { message?: string } };
        if (v.message_type === "exit_error") util = v.message ?? null;
        else if (v.message_type === "error") util = v.error?.message ?? v.message ?? null;
        else if (v.message_type) util = null;
      } catch {
        util = mensajeCortado(linea) ?? linea;
      }
    }
    const u = util?.trim();
    if (u && !out.includes(u)) out.push(u);
  }
  return out.join("\n");
}

/** A qué equipo intentó conectar: « (192.0.2.10)» o vacío. */
function destino(tecnico: string): string {
  const host = (s0: string): string | null => {
    let s = s0.replace(/^"+/, "");
    const fin = s.search(/[/"\s]/);
    s = fin < 0 ? s : s.slice(0, fin);
    const arroba = s.lastIndexOf("@");
    if (arroba >= 0) s = s.slice(arroba + 1);
    const h = s.startsWith("[") ? (s.includes("]") ? s.slice(0, s.indexOf("]") + 1) : null) : s.split(":")[0];
    return h && h.length <= 64 && /^[A-Za-z0-9.\-[\]:]+$/.test(h) ? h : null;
  };
  const i = tecnico.indexOf("dial tcp ");
  const j = tecnico.indexOf("://");
  const h = (i >= 0 ? host(tecnico.slice(i + 9)) : null) ?? (j >= 0 ? host(tecnico.slice(j + 3)) : null);
  return h ? ` (${h})` : "";
}

const pareceTecnico = (t: string) => t.startsWith("Falló:") || t.includes("message_type") || t.includes("Fatal:");

/**
 * Cualquier error, en palabras: la frase con qué hacer y, aparte, el texto técnico
 * completo (para enseñarlo plegado). `detalle` es el que manda el agente aparte (v1.8x),
 * si lo hay: gana al que se saca del propio mensaje.
 */
export function enPalabras(mensaje: string | null | undefined, detalle?: string | null): ErrorEnPalabras {
  const t = (mensaje ?? "").trim();
  const aparte = detalle?.trim() || null;
  const i = t.indexOf(MARCA_DETALLE);
  if (i >= 0) {
    const frase = t.slice(0, i).trim();
    return { caso: frase === DESCONOCIDO ? "desconocido" : "en_palabras", frase, detalle: aparte ?? (t.slice(i + MARCA_DETALLE.length).trim() || null) };
  }
  const tecnico = textoTecnico(t);
  const s = tecnico.toLowerCase();
  const r = REGLAS.find((x) => cumple(x, s));
  if (r) {
    const frase = r.frase.replace("{destino}", destino(tecnico));
    return { caso: r.caso, frase, detalle: aparte ?? (tecnico && tecnico !== frase ? tecnico : null) };
  }
  if (pareceTecnico(t)) return { caso: "desconocido", frase: DESCONOCIDO, detalle: aparte ?? (tecnico || null) };
  return { caso: "en_palabras", frase: t, detalle: aparte };
}

/** Solo la frase (para las líneas cortas: listas, globos, títulos). */
export const fraseError = (mensaje: string | null | undefined) => enPalabras(mensaje).frase;
