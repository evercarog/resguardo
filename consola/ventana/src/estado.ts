// Lo que se deduce del estado del servicio (mismas reglas que la bandeja, bandeja.rs).
import type { SerieOnda } from "$ui/componentes/GraficaOndas.svelte";
import { nombres } from "$ui/ritmos";
import type { Actividad, EstadoBandeja, Punto, TipoActividad } from "./tipos";

export type Tono = "ok" | "info" | "warn" | "bad" | "neutral";

const SERVICIO_CALLADO_MS = 15 * 60_000;

/** ¿El servicio dejó de escribir hace tiempo? */
export const servicioCallado = (b: EstadoBandeja | null, ahora?: string) =>
  !!b?.escrito && Date.parse(ahora ?? new Date().toISOString()) - Date.parse(b.escrito) > SERVICIO_CALLADO_MS;

export function tono(b: EstadoBandeja | null, ahora?: string): Tono {
  if (!b || !b.vinculado) return "neutral";
  if (servicioCallado(b, ahora)) return "warn";
  if (b.actividades?.length) return "info";
  if (b.copias.some((c) => c.resultado === "error")) return "bad";
  if (b.aviso || b.copias.some((c) => c.resultado === "warning")) return "warn";
  return b.copias.some((c) => c.resultado === "ok") ? "ok" : "neutral";
}

export const TIPOS: Record<TipoActividad, string> = {
  copia: "Copia de seguridad",
  restauracion: "Restauración",
  verificacion: "Verificación",
  copia_externa: "Copia externa",
  espejo: "Espejo",
  nube: "Subida a la nube",
};

export function textoFase(a: Actividad): string {
  switch (a.fase) {
    case "antes_de_copiar":
      return "Preparando los datos («Antes de copiar»)…";
    case "preparando":
      return "Preparando…";
    case "escaneando":
      return "Leyendo los archivos…";
    case "subiendo":
      return a.tipo === "copia" ? "Copiando…" : "En marcha…";
    case "terminando":
      return "Guardando la versión…";
    default:
      return a.tipo === "restauracion" ? "Restaurando…" : a.tipo === "verificacion" ? "Verificando…" : "Subiendo…";
  }
}

/** «quedan 4 min», «quedan 1 h 20 min». */
export function quedan(s: number | undefined): string | null {
  if (s == null) return null;
  if (s < 60) return "queda menos de un minuto";
  const m = Math.round(s / 60);
  if (m < 60) return `quedan ${m} min`;
  return `quedan ${Math.floor(m / 60)} h ${m % 60} min`;
}

/** Las series de la ventana (`gestionado-ventana.json`) para `GraficaOndas`. */
export function seriesDe(serie: Punto[] | undefined): { bytes: SerieOnda[]; archivos: SerieOnda[] } {
  const xs = serie ?? [];
  if (!xs.length) return { bytes: [], archivos: [] };
  const tipo = xs[xs.length - 1][4];
  const n = nombres(tipo);
  const lectura = xs.map((p) => [p[0] * 1000, p[1]] as [number, number]);
  const escritura = xs.map((p) => [p[0] * 1000, p[2]] as [number, number]);
  const archivos = xs.map((p) => [p[0] * 1000, p[3]] as [number, number]);
  const con = (s: [number, number][]) => s.some((p) => p[1] > 0);
  return {
    bytes: [
      ...(con(lectura) ? [{ id: "lectura", nombre: n.lectura, color: "--onda-1", puntos: lectura }] : []),
      ...(con(escritura) ? [{ id: "escritura", nombre: n.escritura, color: "--onda-2", puntos: escritura }] : []),
    ],
    archivos: con(archivos) ? [{ id: "archivos", nombre: "Archivos por segundo", color: "--onda-3", puntos: archivos }] : [],
  };
}
