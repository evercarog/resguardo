// Cómo se dice el progreso de algo en marcha (v1.25): la fase, las cifras
// («120 de 300 archivos · 4 GB de 10 GB · 12 MB/s · quedan ~3 min») y el
// texto corto de los chips («Copiando… 42 %»).
import { bytes, numero } from "./formato";
import type { TareaEnMarcha, TipoTarea } from "./tipos";

const NBSP = " ";

/** Qué es, en gerundio (chips y lista global). */
export const NOMBRE_TAREA: Record<TipoTarea, string> = {
  copia: "Copiando",
  verificar: "Verificando",
  verificar_externa: "Verificando la copia externa",
  copia_externa: "Subiendo la copia externa",
  prueba_restauracion: "Probando a restaurar",
  historial: "Trayendo el historial",
  retencion: "Aplicando la retención",
  restauracion: "Restaurando",
};

/**
 * v1.4x: «Mover a otro sitio…» en marcha, como se dice en cualquier consola del
 * equipo: «Moviéndose a otro sitio (iniciado desde «Oficina»): trayendo el
 * historial, 56 de 255 versiones». `null` si la tarea no es un paso de un movimiento.
 */
export function textoMover(t: TareaEnMarcha): string | null {
  if (t.tipo !== "historial" || !t.mover) return null;
  const quien = t.otra_consola ? (t.consola ? ` (iniciado desde «${t.consola}»)` : " (iniciado desde otra consola)") : "";
  const que = t.paso === "ultimo" ? "trayendo lo copiado mientras tanto" : "trayendo el historial";
  const cuantas = t.versiones != null && t.versiones_total ? `, ${numero(t.versiones)} de ${numero(t.versiones_total)} ${t.versiones_total === 1 ? "versión" : "versiones"}` : t.fase === "preparando" ? ", preparando…" : "";
  return `Moviéndose a otro sitio${quien}: ${que}${cuantas}`;
}

/** Qué está haciendo ahora (encabezado de la barra). */
export function textoFase(t: TareaEnMarcha): string {
  if (t.tipo !== "copia") return t.etapa?.trim().replace(/…$/, "") || NOMBRE_TAREA[t.tipo];
  switch (t.fase) {
    case "antes_de_copiar":
      return "Antes de copiar: volcados y comprobaciones";
    case "preparando":
      return "Preparando la copia";
    case "escaneando":
      return "Buscando cambios y copiando";
    case "terminando":
      return "Guardando la versión";
    default:
      return "Copiando";
  }
}

/** Porcentaje entero (0–100), o null si aún no se sabe. */
export function porcentaje(t: TareaEnMarcha): number | null {
  return t.porcentaje == null ? null : Math.max(0, Math.min(100, Math.floor(t.porcentaje * 100)));
}

/** «quedan ~3 min», «queda menos de 1 min», «quedan ~1 h 20 min». */
export function textoQuedan(s: number | null | undefined): string | null {
  if (s == null || !Number.isFinite(s) || s < 0) return null;
  if (s < 60) return "queda menos de 1 min";
  const m = Math.round(s / 60);
  if (m < 60) return `quedan ~${m}${NBSP}min`;
  const h = Math.floor(m / 60);
  if (h >= 48) return `quedan ~${Math.round(h / 24)} días`;
  return `quedan ~${h}${NBSP}h${m % 60 ? ` ${m % 60}${NBSP}min` : ""}`;
}

/** «hace 3 min» de un intervalo en segundos (para «lleva …»). */
export function textoDuracion(s: number): string {
  if (s < 60) return `${Math.max(0, Math.floor(s))}${NBSP}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}${NBSP}min`;
  return `${Math.floor(m / 60)}${NBSP}h ${m % 60}${NBSP}min`;
}

/** Las cifras de la línea de debajo de la barra (solo las que se saben). */
export function cifrasTarea(t: TareaEnMarcha): string[] {
  const out: string[] = [];
  if (t.archivos != null && t.archivos_total) out.push(`${numero(t.archivos)} de ${numero(t.archivos_total)} archivos`);
  else if (t.archivos != null && t.archivos > 0) out.push(`${numero(t.archivos)} archivos`);
  if (t.versiones != null && t.versiones_total) out.push(`${numero(t.versiones)} de ${numero(t.versiones_total)} versiones`);
  if (t.bytes != null && t.bytes_total) out.push(`${bytes(t.bytes)} de ${bytes(t.bytes_total)}`);
  else if (t.bytes != null && t.bytes > 0) out.push(bytes(t.bytes));
  if (t.velocidad != null && t.velocidad > 0) out.push(`${bytes(t.velocidad)}/s`);
  const q = textoQuedan(t.quedan_s);
  if (q) out.push(q);
  return out;
}

/** Texto corto: «Copiando… 42 %», «Preparando…», «Verificando… 10 %». */
export function textoCorto(t: TareaEnMarcha, pct?: number | null): string {
  const p = pct === undefined ? porcentaje(t) : pct;
  const que =
    t.tipo === "copia" && (t.fase === "preparando" || t.fase === "antes_de_copiar")
      ? "Preparando"
      : t.tipo === "copia"
        ? "Copiando"
        : t.mover
          ? "Moviendo"
          : (NOMBRE_TAREA[t.tipo] ?? "En marcha").split(" ")[0];
  return p == null ? `${que}…` : `${que}… ${p}${NBSP}%`;
}
