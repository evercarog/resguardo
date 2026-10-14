// 14ii: la contraseña de un repositorio nuevo, la misma en todas partes
// (Nuevo repositorio, Copiar en el almacén, copia derivada con «Otra contraseña»,
// mover un repositorio…): generada en este navegador (lo recomendado) o escrita
// por la persona, con las mismas reglas. Nunca sale del navegador si no es
// sellada para el equipo.
import { aleatorio } from "./cripto/bytes";

/** Mínimo de caracteres de una contraseña escrita (la generada tiene 43). */
export const MIN_ESCRITA = 12;

/** Lo que se elige en el componente `ContrasenaRepo`. */
export interface ContrasenaElegida {
  /** true: «La escribo yo». */
  propia: boolean;
  generada: string;
  escrita: string;
  repetida: string;
}

/** 32 bytes aleatorios en base64url (43 caracteres). */
export function generarContrasena(): string {
  const b = aleatorio(32);
  const s = btoa(String.fromCharCode(...b)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  b.fill(0);
  return s;
}

/** Una elección nueva, con la generada ya puesta. */
export function nuevaContrasena(): ContrasenaElegida {
  return { propia: false, generada: generarContrasena(), escrita: "", repetida: "" };
}

/** La contraseña que se usará: la generada o la escrita. */
export function claveElegida(c: ContrasenaElegida): string {
  return c.propia ? c.escrita : c.generada;
}

/** Qué falta en la escrita (o null si vale). La generada siempre vale. */
export function errorContrasena(c: ContrasenaElegida): string | null {
  if (!c.propia) return c.generada ? null : "Falta la contraseña";
  if (c.escrita.length < MIN_ESCRITA) return `Al menos ${MIN_ESCRITA} caracteres.`;
  if (c.escrita.trim() !== c.escrita) return "Sin espacios al principio ni al final.";
  if (c.escrita !== c.repetida) return "Las dos no coinciden.";
  return null;
}

/** Borra lo escrito y lo generado (al cerrar el diálogo). */
export function olvidarContrasena(c: ContrasenaElegida): void {
  c.generada = c.escrita = c.repetida = "";
}
