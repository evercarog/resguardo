// Verificación automática de un repositorio (v1.28, `config.verificaciones`):
// cada N días, `restic check` leyendo un porcentaje de los datos, rotativo
// (como `VerificacionAuto` del agente, gestion_v2.rs).
import type { Equipo, VerificacionAuto } from "./tipos";

/** Lo que dice el agente que entiende (v1.28). */
export const ADMITE_VERIFICACION = "verificacion_auto";
export const admiteVerificacion = (e: Equipo | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_VERIFICACION);

/** Lo que se propone al encenderla: cada semana, un 10 % (todo en 10 semanas). */
export const VERIFICACION_POR_DEFECTO: VerificacionAuto = { cada_dias: 7, porcentaje: 10 };
export const PORCENTAJES = [0, 2, 5, 10, 20, 25, 50, 100];

/** En cuántas vueltas se lee todo (como el agente: 100/porcentaje, de 2 a 52); 0 % o 100 %: null. */
export function partesVerificacion(porcentaje: number): number | null {
  if (porcentaje <= 0 || porcentaje >= 100) return null;
  return Math.min(52, Math.max(2, Math.round(100 / porcentaje)));
}

export function errorVerificacion(v: VerificacionAuto): string | null {
  if (!Number.isInteger(v.cada_dias) || v.cada_dias < 1 || v.cada_dias > 31) return "De cada día a cada 31 días.";
  if (!Number.isInteger(v.porcentaje) || v.porcentaje < 0 || v.porcentaje > 100) return "El porcentaje va de 0 a 100.";
  return null;
}

/** «Cada 7 días, el 10 % de los datos: todo el repositorio en 10 verificaciones (unos 70 días)». */
export function fraseVerificacion(v: VerificacionAuto): string {
  const cada = v.cada_dias === 1 ? "Cada día" : `Cada ${v.cada_dias} días`;
  if (v.porcentaje <= 0) return `${cada}, solo la estructura (rápida: no lee los datos).`;
  if (v.porcentaje >= 100) return `${cada}, todos los datos (lenta en repositorios grandes).`;
  const n = partesVerificacion(v.porcentaje)!;
  return `${cada}, el ${Math.round(100 / n)} % de los datos: todo el repositorio en ${n} verificaciones (unos ${n * v.cada_dias} días).`;
}
