// Datos de otro servidor para cambiar_servidor y servidores_respaldo
// (api-servidor.md §11): dirección, identidad Ed25519, autoridad TLS y una
// ficha de ese servidor. Se copian de una consola a otra como un bloque
// JSON. La ficha es un secreto de un solo uso: viaja solo dentro de la orden
// sellada y nunca se guarda en el navegador.
import { deB64 } from "./cripto/bytes";
import type { Ficha } from "./tipos";

export interface DestinoServidor {
  url: string;
  identidad: string;
  ca_pem: string;
  ficha: string;
}

/** El bloque que se copia en el servidor nuevo y se pega en el antiguo. */
export function bloqueServidor(f: Ficha, url = location.origin): string {
  return JSON.stringify({ url, identidad: f.servidor.identidad, ca_pem: f.servidor.ca_pem, ficha: f.ficha }, null, 2);
}

/** Lee un bloque pegado. Devuelve los datos, o un texto con lo que falla. */
export function leerBloque(texto: string): DestinoServidor | string {
  let x: Partial<Record<keyof DestinoServidor, unknown>>;
  try {
    x = JSON.parse(texto.trim());
  } catch {
    return "No es un bloque válido: cópialo entero desde «Recibir un cliente» o «Dar una ficha» del otro servidor.";
  }
  const url = typeof x.url === "string" ? x.url.trim().replace(/\/+$/, "") : "";
  if (!/^https:\/\/[^\s/]+/i.test(url)) return "Falta la dirección del servidor (https://…).";
  if (url === location.origin) return "Esa dirección es la de este mismo servidor.";
  if (typeof x.identidad !== "string" || !esLlave(x.identidad)) return "La identidad del servidor no es válida.";
  if (typeof x.ca_pem !== "string" || !x.ca_pem.includes("-----BEGIN CERTIFICATE-----")) return "Falta la autoridad TLS del servidor (PEM).";
  if (typeof x.ficha !== "string" || x.ficha.trim().length < 16) return "Falta la ficha.";
  return { url, identidad: x.identidad, ca_pem: x.ca_pem, ficha: x.ficha.trim() };
}

function esLlave(b64: string) {
  try {
    return deB64(b64).length === 32;
  } catch {
    return false;
  }
}

/** Huella corta de la identidad, para compararla de palabra con quien lleva el otro servidor. */
export const huellaCorta = (identidad: string) =>
  [...deB64(identidad).slice(0, 8)].map((b) => b.toString(16).padStart(2, "0").toUpperCase()).join(":");
