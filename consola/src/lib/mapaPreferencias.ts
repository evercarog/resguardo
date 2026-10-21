// Plan 0.8.1 (C2: 15rr, 15l, 15b): cómo quiere ver cada persona el mapa de la
// protección, recordado en este navegador **por persona** (su cuenta) y por
// mapa (un cliente o «todos»): cómo se agrupan los equipos, qué grupos tiene
// desplegados y qué capas lleva encendidas (0.8.2: también los grupos plegados
// de los recuadros de Equipos, C4, y el filtro por infraestructura, A4). Solo preferencias de pantalla:
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

/** 0.8.2 (A4, 15xx): lo anotado en la infraestructura se ve por defecto en cada tarjeta (sin nada anotado, no sale). */
export const CAPAS_POR_DEFECTO: Capas = { sedes: false, copias: true, estado: true, infra: true, almacenamiento: true };

export interface PrefsMapa {
  agrupar: Agrupar;
  desplegados: string[];
  capas: Capas;
  /** 0.8.2 (C4): los grupos plegados de los recuadros (Equipos). */
  plegados: string[];
  /** 0.8.2 (A4): el filtro por infraestructura («srv:hv-01», «cj:hv-01:tank»; vacío: todo). */
  infra: string;
}

export const PREFS_POR_DEFECTO: PrefsMapa = { agrupar: AGRUPAR_POR_DEFECTO, desplegados: [], capas: CAPAS_POR_DEFECTO, plegados: [], infra: "" };

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
  const lista = (v: unknown) => (Array.isArray(v) ? v.filter((y): y is string => typeof y === "string" && y.length <= 300).slice(0, 200) : []);
  const d = lista(o.desplegados);
  const c = o.capas && typeof o.capas === "object" ? (o.capas as Record<string, unknown>) : {};
  const capa = (k: keyof Capas) => (typeof c[k] === "boolean" ? (c[k] as boolean) : porDefecto.capas[k]);
  return {
    agrupar: { modo, n },
    desplegados: d,
    capas: { sedes: capa("sedes"), copias: capa("copias"), estado: capa("estado"), infra: capa("infra"), almacenamiento: capa("almacenamiento") },
    plegados: lista(o.plegados),
    infra: typeof o.infra === "string" && /^(srv|cj|dk):[a-z0-9:-]{1,90}$/.test(o.infra) ? o.infra : "",
  };
}

/** Desplegar o plegar un grupo. */
export function alternarGrupo(p: PrefsMapa, id: string): PrefsMapa {
  const ya = p.desplegados.includes(id);
  return { ...p, desplegados: ya ? p.desplegados.filter((x) => x !== id) : [...p.desplegados, id].slice(-200) };
}

/** 0.8.2 (C4): plegar o desplegar un recuadro. */
export function alternarPlegado(p: PrefsMapa, id: string): PrefsMapa {
  const ya = p.plegados.includes(id);
  return { ...p, plegados: ya ? p.plegados.filter((x) => x !== id) : [...p.plegados, id].slice(-200) };
}

/** «No agrupar», «Por sede», «Con más de 8». */
export function textoAgrupar(a: Agrupar, conSedes: boolean): string {
  if (a.modo === "no") return "No";
  if (a.modo === "sede") return conSedes ? "Por sede" : "Por grupo";
  if (a.modo === "recuadros") return "Por grupos";
  return `Con más de ${a.n}`;
}
