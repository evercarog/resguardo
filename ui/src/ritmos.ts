// Los ritmos de lo que está en marcha (lectura, escritura o subida, archivos
// por segundo) a lo largo de los últimos minutos, para `GraficaOndas`. Lo
// usan la consola (con el progreso que manda el agente, v1.36) y la ventana
// del agente (con la serie que escribe el servicio).

import type { SerieOnda } from "./componentes/GraficaOndas.svelte";

/** Lo que dice una tarea en marcha (consola, v1.25 y v1.36). */
export interface MuestraRitmo {
  tipo: string;
  /** Lo que procesa restic (bytes/s). */
  velocidad?: number | null;
  /** Lectura real del disco (bytes/s). */
  lectura?: number | null;
  /** Lo que sube o escribe en el destino (bytes/s). */
  subida?: number | null;
  archivos_s?: number | null;
  bytes?: number | null;
}

/** Puntos `[ms, valor]` de cada medida. */
export interface Ritmo {
  lectura: [number, number][];
  escritura: [number, number][];
  archivos: [number, number][];
  /** Bytes y momento de la muestra anterior (para las tareas que no dan ritmo). */
  previo?: [number, number];
}

export const VENTANA_S = 300;
export const MAX_PUNTOS = 150;

/** Tipos que leen (su ritmo deducido de los bytes es lectura). */
const LEEN = new Set(["verificacion", "verificar", "verificar_externa", "prueba_restauracion"]);

function recortar(xs: [number, number][], ahora: number) {
  while (xs.length && xs[0][0] < ahora - VENTANA_S * 1000) xs.shift();
  while (xs.length > MAX_PUNTOS) xs.shift();
}

function poner(xs: [number, number][], t: number, v: number) {
  if (xs.length && xs[xs.length - 1][0] >= t) xs.pop();
  xs.push([t, v]);
}

/** Añade una muestra de una tarea al ritmo `r` (lo crea si no hay). */
export function anotar(r: Ritmo | undefined, t: number, m: MuestraRitmo): Ritmo {
  const x: Ritmo = r ?? { lectura: [], escritura: [], archivos: [] };
  let lectura = m.lectura ?? m.velocidad ?? null;
  let escritura = m.subida ?? null;
  if (lectura == null && escritura == null && m.bytes != null) {
    if (x.previo && t > x.previo[0] && m.bytes >= x.previo[1]) {
      const v = ((m.bytes - x.previo[1]) * 1000) / (t - x.previo[0]);
      if (LEEN.has(m.tipo)) lectura = v;
      else escritura = v;
    }
  }
  if (m.bytes != null) x.previo = [t, m.bytes];
  if (lectura != null) poner(x.lectura, t, lectura);
  if (escritura != null) poner(x.escritura, t, escritura);
  if (m.archivos_s != null) poner(x.archivos, t, m.archivos_s);
  recortar(x.lectura, t);
  recortar(x.escritura, t);
  recortar(x.archivos, t);
  return x;
}

/** Qué se llama cada medida según la tarea. */
export function nombres(tipo: string): { lectura: string; escritura: string } {
  switch (tipo) {
    case "restauracion":
      return { lectura: "Lectura del almacén", escritura: "Escritura en el disco" };
    case "verificacion":
    case "verificar":
    case "verificar_externa":
    case "prueba_restauracion":
      return { lectura: "Lectura del almacén", escritura: "Escritura" };
    case "copia_externa":
      return { lectura: "Lectura", escritura: "Subida a la copia externa" };
    case "espejo":
      return { lectura: "Lectura", escritura: "Copia al espejo" };
    case "nube":
      return { lectura: "Lectura", escritura: "Subida a la nube" };
    default:
      return { lectura: "Lectura del disco", escritura: "Subida al destino" };
  }
}

/** Las series de bytes/s (lectura en azul, escritura o subida en naranja), solo las que tienen datos. */
export function seriesBytes(tipo: string, r: Ritmo | undefined): SerieOnda[] {
  if (!r) return [];
  const n = nombres(tipo);
  return [
    { id: "lectura", nombre: n.lectura, color: "--onda-1", puntos: r.lectura },
    { id: "escritura", nombre: n.escritura, color: "--onda-2", puntos: r.escritura },
  ].filter((s) => s.puntos.length);
}

/** La serie de archivos por segundo (otra unidad: va en su propia gráfica). */
export function serieArchivos(r: Ritmo | undefined): SerieOnda[] {
  return r?.archivos.length ? [{ id: "archivos", nombre: "Archivos por segundo", color: "--onda-3", puntos: r.archivos }] : [];
}

/** «1.240 archivos/s». */
export const porSegundoArchivos = (v: number) => `${Math.round(v).toLocaleString("es-ES")} archivos/s`;
