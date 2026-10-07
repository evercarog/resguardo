// La actualización automática de los agentes en la consola (docs/actualizaciones.md).
// El estado lo dice el propio agente en su informe (`actualizacion`): aquí solo se
// pone en palabras. La versión disponible es la que sirve este servidor.
// Vectores: scripts/vectores-actualizaciones.ts (con los de crates/protocolo/vectors/publicacion.json).
import type { Tono } from "./salud";

/** `informe.actualizacion` de un agente (v1.57). */
export interface EstadoActualizacion {
  estado: string;
  motivo?: string | null;
  mensaje?: string | null;
  version_disponible?: string | null;
  hasta?: string | null;
  version_objetivo?: string | null;
  version_fallida?: string | null;
  anillo?: string | null;
  modo?: string | null;
  origen?: string | null;
  ultima_busqueda?: string | null;
  cuando?: string | null;
}

interface Version {
  numeros: [number, number, number];
  pre: string | null;
}

const VERSION = /^(0|[1-9]\d{0,8})\.(0|[1-9]\d{0,8})\.(0|[1-9]\d{0,8})(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/;

export function leerVersion(s: string | null | undefined): Version | null {
  const m = s ? VERSION.exec(s) : null;
  return m ? { numeros: [Number(m[1]), Number(m[2]), Number(m[3])], pre: m[4] ?? null } : null;
}

function compararPre(a: string, b: string): number {
  const x = a.split(".");
  const y = b.split(".");
  for (let i = 0; ; i++) {
    if (i >= x.length && i >= y.length) return 0;
    if (i >= x.length) return -1;
    if (i >= y.length) return 1;
    const [p, q] = [x[i], y[i]];
    const [np, nq] = [/^\d+$/.test(p), /^\d+$/.test(q)];
    const o = np && nq ? Math.sign(Number(p) - Number(q)) : np ? -1 : nq ? 1 : p < q ? -1 : p > q ? 1 : 0;
    if (o) return o;
  }
}

/** -1, 0 o 1 como `publicacion::comparar` (null si alguna no es una versión). */
export function compararVersiones(a: string | null | undefined, b: string | null | undefined): number | null {
  const [x, y] = [leerVersion(a), leerVersion(b)];
  if (!x || !y) return null;
  for (let i = 0; i < 3; i++) if (x.numeros[i] !== y.numeros[i]) return x.numeros[i] < y.numeros[i] ? -1 : 1;
  if (x.pre === y.pre) return 0;
  if (x.pre === null) return 1;
  if (y.pre === null) return -1;
  return compararPre(x.pre, y.pre);
}

/** Por qué espera, en palabras. */
export const MOTIVOS: Record<string, string> = {
  sin_paquete: "No hay paquete para su sistema en esta versión.",
  necesita_intermedia: "Necesita pasar antes por una versión intermedia: instálala a mano.",
  espera_aprobacion: "Espera a que la apruebes («Actualizar ahora»).",
  retenida: "Retenida: falló en algún equipo del cliente. «Actualizar ahora» la libera.",
  espera_anillo: "Espera su turno (anillo general).",
  espera_ventana: "Fuera de su ventana de mantenimiento.",
  ventanas_sin_coincidir: "Sus consolas tienen ventanas que no coinciden nunca: no se actualiza solo.",
  en_marcha: "Hay una copia o una restauración en marcha: espera a que termine.",
  almacen_sin_ventana: "Es un almacén: actualizarlo corta un momento las copias de los demás. Pon una ventana de mantenimiento o usa «Actualizar ahora».",
  sin_consola: "No pudo hablar con ninguna consola en ese momento.",
  version_desconocida: "No sabe comparar su versión.",
};

export interface Vista {
  /** Para agrupar y filtrar. */
  clave: "al_dia" | "pendiente" | "actualizando" | "fallo" | "vuelta_atras" | "pausada" | "desactivada" | "anterior" | "sin_datos";
  texto: string;
  tono: Tono;
  detalle?: string;
}

/**
 * El estado de un equipo, en palabras: lo que dice su informe y, si no dice nada (un
 * agente anterior a esta función), si su versión es anterior a la que hay.
 */
export function vistaActualizacion(info: EstadoActualizacion | null | undefined, version: string | null | undefined, disponible: string | null | undefined): Vista {
  const masNueva = !!disponible && compararVersiones(version, disponible) === -1;
  if (!info || typeof info !== "object" || !info.estado) {
    return masNueva
      ? { clave: "anterior", texto: "Versión anterior", tono: "warn", detalle: `Este agente no se actualiza solo: instala la ${disponible} a mano una vez y desde ahí se actualizará solo.` }
      : { clave: "sin_datos", texto: "Sin datos", tono: "neutral", detalle: "Este agente aún no dice nada de las actualizaciones." };
  }
  const hasta = info.hasta ? ` Hasta el ${fechaCorta(info.hasta)}.` : "";
  switch (info.estado) {
    case "desactivada":
      return { clave: "desactivada", texto: "Sin actualización automática", tono: "neutral", detalle: "Este agente se compiló sin llave de publicación: se actualiza a mano." };
    case "al_dia":
    case "actualizada":
      return masNueva
        ? { clave: "pendiente", texto: "Pendiente", tono: "info", detalle: "La verá en su próxima búsqueda (cada 6 horas, o ya con «Actualizar ahora»)." }
        : { clave: "al_dia", texto: "Al día", tono: "ok", detalle: info.estado === "actualizada" && info.version_objetivo ? `Se actualizó solo a la ${info.version_objetivo}.` : undefined };
    case "pendiente": {
      const motivo = info.motivo ?? "";
      const grave = ["retenida", "necesita_intermedia", "ventanas_sin_coincidir", "almacen_sin_ventana", "sin_paquete"].includes(motivo);
      return { clave: "pendiente", texto: "Pendiente", tono: grave ? "warn" : "info", detalle: `${MOTIVOS[motivo] ?? "Esperando."}${hasta}`.trim() };
    }
    case "pausada":
      return { clave: "pausada", texto: "En pausa", tono: "paused", detalle: "Una de sus consolas tiene las actualizaciones en pausa." };
    case "descargando":
      return { clave: "actualizando", texto: "Actualizando", tono: "info", detalle: `Bajando la ${info.version_objetivo ?? "versión nueva"}.` };
    case "actualizando":
      return { clave: "actualizando", texto: "Actualizando", tono: "info", detalle: `Instalando la ${info.version_objetivo ?? "versión nueva"}; si no está sana en 10 minutos, vuelve sola a la anterior.` };
    case "vuelta_atras":
      return {
        clave: "vuelta_atras",
        texto: "Volvió a la anterior",
        tono: "bad",
        detalle: `La ${info.version_fallida ?? "versión nueva"} no estuvo sana y el equipo volvió a la que tenía.${info.mensaje ? ` ${info.mensaje}` : ""}`,
      };
    case "fallida":
      return { clave: "fallo", texto: "Falló", tono: "bad", detalle: `No se pudo actualizar${info.version_objetivo ? ` a la ${info.version_objetivo}` : ""}.${info.mensaje ? ` ${info.mensaje}` : ""}` };
    default:
      return { clave: "sin_datos", texto: info.estado, tono: "neutral" };
  }
}

function fechaCorta(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getDate())}/${p(d.getMonth() + 1)} a las ${p(d.getHours())}:${p(d.getMinutes())}`;
}

/** ¿Se puede pedir «Actualizar ahora» a este equipo? (sabe actualizarse solo y no está en pausa ni compilado sin llave) */
export const seActualizaSolo = (vista: Vista) => !["anterior", "sin_datos", "desactivada", "pausada"].includes(vista.clave);

/** «HH:MM» válido (la ventana de mantenimiento). */
export const horaValida = (s: string) => /^([01]\d|2[0-3]):[0-5]\d$/.test(s);

/** El texto de la política del cliente. */
export function frasePolitica(p: { modo: string; dias_general: number; ventana: { desde: string; hasta: string } | null }): string {
  const ventana = p.ventana ? `, entre las ${p.ventana.desde} y las ${p.ventana.hasta}` : "";
  if (p.modo === "pausada") return "En pausa: ningún equipo se actualiza.";
  if (p.modo === "manual") return "Solo cuando la apruebes con «Actualizar ahora».";
  const dias = p.dias_general === 0 ? "a la vez que los de prueba" : p.dias_general === 1 ? "1 día después" : `${p.dias_general} días después`;
  return `Automática: los equipos de prueba en cuanto sale; los demás, ${dias}${ventana}.`;
}
