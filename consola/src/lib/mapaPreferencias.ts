// Plan 0.8.1 (C2: 15rr, 15l, 15b): cómo quiere ver cada persona el mapa de la
// protección, recordado en este navegador **por persona** (su cuenta) y por
// mapa (un cliente o «todos»): cómo se agrupan los equipos, qué grupos tiene
// desplegados y qué capas lleva encendidas. Solo preferencias de pantalla:
// nunca datos de los equipos ni secretos. Sin runas (se prueba en
// scripts/vectores-mapa.ts); el componente guarda con `recordar.ts`.
import { AGRUPAR_POR_DEFECTO, type Agrupar, type ModoAgrupar } from "./mapa";

/** Las capas que se encienden y apagan. */
export interface Capas {
  /** Recuadros por sede y la nube aparte. */
  sedes: boolean;
  /** Los trazos: copia → repositorio → derivada → espejo. */
  copias: boolean;
  /** Los colores y avisos (estado de cada cosa, «servidor físico compartido»). */
  estado: boolean;
  /** El servidor físico (y su hipervisor) de cada equipo y destino (con infraestructura anotada). */
  infra: boolean;
  /** El conjunto de discos de cada destino (con infraestructura anotada). */
  almacenamiento: boolean;
}

export const CAPAS_POR_DEFECTO: Capas = { sedes: false, copias: true, estado: true, infra: false, almacenamiento: false };

export interface PrefsMapa {
  agrupar: Agrupar;
  desplegados: string[];
  capas: Capas;
}

export const PREFS_POR_DEFECTO: PrefsMapa = { agrupar: AGRUPAR_POR_DEFECTO, desplegados: [], capas: CAPAS_POR_DEFECTO };

const MODOS: readonly ModoAgrupar[] = ["no", "sede", "mas_de"];
/** Los «más de N» que se ofrecen. */
export const OPCIONES_N = [4, 6, 8, 12, 20, 40] as const;

/** La clave en este navegador: por persona y por mapa. */
export const clavePrefs = (persona: string, ambito: string) => `mapa.prefs.${persona || "anon"}.${ambito}`;

/** Lee lo guardado (texto JSON), con lo de por defecto para lo que falte o no cuadre. */
export function leerPrefs(texto: string, porDefecto: PrefsMapa = PREFS_POR_DEFECTO): PrefsMapa {
  let x: unknown = null;
  try {
    x = JSON.parse(texto || "null");
  } catch {
    x = null;
  }
  if (!x || typeof x !== "object") return porDefecto;
  const o = x as Record<string, unknown>;
  const a = o.agrupar && typeof o.agrupar === "object" ? (o.agrupar as Record<string, unknown>) : {};
  const modo = MODOS.includes(a.modo as ModoAgrupar) ? (a.modo as ModoAgrupar) : porDefecto.agrupar.modo;
  const n = typeof a.n === "number" && Number.isInteger(a.n) && a.n >= 2 && a.n <= 200 ? a.n : porDefecto.agrupar.n;
  const d = Array.isArray(o.desplegados) ? o.desplegados.filter((y): y is string => typeof y === "string" && y.length <= 300).slice(0, 200) : [];
  const c = o.capas && typeof o.capas === "object" ? (o.capas as Record<string, unknown>) : {};
  const capa = (k: keyof Capas) => (typeof c[k] === "boolean" ? (c[k] as boolean) : porDefecto.capas[k]);
  return {
    agrupar: { modo, n },
    desplegados: d,
    capas: { sedes: capa("sedes"), copias: capa("copias"), estado: capa("estado"), infra: capa("infra"), almacenamiento: capa("almacenamiento") },
  };
}

/** Desplegar o plegar un grupo. */
export function alternarGrupo(p: PrefsMapa, id: string): PrefsMapa {
  const ya = p.desplegados.includes(id);
  return { ...p, desplegados: ya ? p.desplegados.filter((x) => x !== id) : [...p.desplegados, id].slice(-200) };
}

/** «No agrupar», «Por sede», «Con más de 8». */
export function textoAgrupar(a: Agrupar, conSedes: boolean): string {
  if (a.modo === "no") return "No";
  if (a.modo === "sede") return conSedes ? "Por sede" : "Por grupo";
  return `Con más de ${a.n}`;
}
