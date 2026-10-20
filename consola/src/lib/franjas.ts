// Plan 0.8.0, F4 (14t): límite de velocidad según la hora en los espejos y en las
// copias derivadas (p. ej. 2 MB/s de 8:00 a 18:00 los laborables y sin límite el
// resto). Lo mismo que comprueba el agente (crates/agente/src/franjas.rs): como
// mucho 8 franjas, cada una el mismo día (`desde` antes que `hasta`), de 64 a
// 10 000 000 KiB/s (o 0: sin límite) y sin pisarse. Sin estado: lo prueban los
// vectores (scripts/vectores-versiones.ts).
import type { Equipo, FranjaVelocidad } from "./tipos";
import { bytes } from "./formato";

export const ADMITE_FRANJAS = "limite_franjas";
export const admiteFranjas = (e: Equipo | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_FRANJAS);
export const MAX_FRANJAS = 8;
export const MIN_KIB = 64;
export const MAX_KIB = 10_000_000;

/** La que se propone al añadir una: los laborables de 08:00 a 18:00 a 2 MB/s. */
export const franjaNueva = (): FranjaVelocidad => ({ dias: [1, 2, 3, 4, 5], desde: "08:00", hasta: "18:00", kib: 2048 });

const HORA = /^([01]\d|2[0-3]):[0-5]\d$/;

/** Lo que falla de las franjas (como el agente), o null. */
export function errorFranjas(fs: FranjaVelocidad[] | null | undefined): string | null {
  const l = fs ?? [];
  if (l.length > MAX_FRANJAS) return `Como mucho ${MAX_FRANJAS} franjas de velocidad.`;
  const tramos: [number, string, string][] = [];
  for (const f of l) {
    if (!f.dias.length || f.dias.some((d) => !Number.isInteger(d) || d < 1 || d > 7)) return "Cada franja de velocidad necesita sus días.";
    if (!HORA.test(f.desde) || !HORA.test(f.hasta)) return "Las horas de las franjas de velocidad son HH:MM.";
    if (f.desde >= f.hasta) return `La franja de ${f.desde} a ${f.hasta} tiene que empezar antes de terminar (el mismo día).`;
    if (!Number.isInteger(f.kib) || (f.kib !== 0 && (f.kib < MIN_KIB || f.kib > MAX_KIB))) return `El límite de cada franja va de ${velocidadEnFrase(MIN_KIB)} a ${velocidadEnFrase(MAX_KIB)} (o sin límite).`;
    for (const d of f.dias) {
      if (tramos.some(([e, a, b]) => e === d && f.desde < b && a < f.hasta)) return "Dos franjas de velocidad se pisan el mismo día.";
      tramos.push([d, f.desde, f.hasta]);
    }
  }
  return null;
}

const DIAS = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];
/** «lun–vie», «sáb y dom», «todos los días», «lun, mié y vie». */
export function diasEnFrase(dias: number[]): string {
  const d = [...new Set(dias)].filter((x) => x >= 1 && x <= 7).sort((a, b) => a - b);
  if (d.length === 7) return "todos los días";
  const seguidos = d.length > 2 && d.every((x, i) => i === 0 || x === d[i - 1] + 1);
  if (seguidos) return `${DIAS[d[0] - 1]}–${DIAS[d[d.length - 1] - 1]}`;
  const n = d.map((x) => DIAS[x - 1]);
  return n.length > 1 ? `${n.slice(0, -1).join(", ")} y ${n.at(-1)}` : (n[0] ?? "");
}

/**
 * «2,1 MB/s», «524 KB/s», «sin límite». El agente cuenta el límite en KiB/s; aquí se dice
 * con las mismas unidades que el resto de la consola (`bytes`, de 1000 en 1000, con espacio
 * fino), como la velocidad de una copia en marcha (G3, plan 0.8.1: antes «MB/s» salía de
 * dividir KiB entre 1024 y las etiquetas decían «KiB/s»).
 */
export function velocidadEnFrase(kib: number | null | undefined): string {
  if (!kib) return "sin límite";
  return `${bytes(kib * 1024)}/s`;
}

/** «2 MB/s de 08:00 a 18:00 (lun–vie); el resto, sin límite». */
export function franjasEnFrase(fs: FranjaVelocidad[] | null | undefined, porDefecto?: number | null): string {
  const l = fs ?? [];
  if (!l.length) return porDefecto ? `Hasta ${velocidadEnFrase(porDefecto)}` : "Sin límite";
  const partes = l.map((f) => `${velocidadEnFrase(f.kib)} de ${f.desde} a ${f.hasta} (${diasEnFrase(f.dias)})`);
  return `${partes.join("; ")}; el resto, ${velocidadEnFrase(porDefecto)}`;
}
