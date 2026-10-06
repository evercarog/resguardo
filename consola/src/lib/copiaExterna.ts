// Copia externa (cambiar_copia_externa, api-servidor.md §5, v1.4x): «Usar uno
// que ya existe» y el bloqueo de objetos del destino. Lo que no es pantalla, sin
// dependencias del navegador (lo prueban los vectores, scripts/vectores.ts).
import { destinoCuerpo, partirDireccion, repoExistenteCompleto, repoExistenteVacio, type RepoExistente } from "./direccion";
import type { Equipo } from "./tipos";

/** Agentes que entienden `existente`, `ruta`, `bloqueo_dias` y `solo_probar`. */
export const ADMITE_EXTERNA_EXISTENTE = "externa_existente";
export const admiteExternaExistente = (e: Equipo | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_EXTERNA_EXISTENTE);

/** Días máximos de bloqueo (los del agente: 10 años). */
export const MAX_BLOQUEO = 3650;

/** Lo que se elige en el diálogo además de lo de siempre. */
export interface ExternaExtra {
  /** «Crear uno nuevo» (como siempre) o «Usar uno que ya existe». */
  modo: "nuevo" | "existente";
  existente: RepoExistente;
  /** Nombre del destino nuevo (vacío: «Backblaze B2», «S3»… como al adoptar). */
  nombreExistente: string;
  conBloqueo: boolean;
  /** Del campo numérico: número, o "" si se vacía. */
  bloqueoDias: number | string;
}

export const externaExtraVacia = (): ExternaExtra => ({
  modo: "nuevo",
  existente: { ...repoExistenteVacio(), tipo: "b2" },
  nombreExistente: "",
  conBloqueo: false,
  bloqueoDias: 30,
});

/** Los días de bloqueo, si valen (enteros de 1 a 3650). */
export function diasBloqueo(x: number | string): number | null {
  const t = String(x ?? "").trim();
  if (!/^\d{1,4}$/.test(t)) return null;
  const n = Number(t);
  return n >= 1 && n <= MAX_BLOQUEO ? n : null;
}

export const errorBloqueo = (x: ExternaExtra) => (x.conBloqueo && diasBloqueo(x.bloqueoDias) === null ? `Escribe los días de bloqueo (de 1 a ${MAX_BLOQUEO}).` : null);

/** `bloqueo_dias` para la orden (nada si no hay bloqueo). */
export const cuerpoBloqueo = (x: ExternaExtra) => {
  const n = x.conBloqueo ? diasBloqueo(x.bloqueoDias) : null;
  return n ? { bloqueo_dias: n } : {};
};

/**
 * Campos de la orden para «Usar uno que ya existe»: el destino (sin la
 * carpeta), la carpeta del repositorio dentro de él, `existente` y SU
 * contraseña. Sin certificado propio: la subida usa el del origen.
 */
export function cuerpoExistente(x: ExternaExtra, idDestino: string) {
  const { ruta } = partirDireccion(x.existente.tipo, x.existente.direccion);
  const nombre = x.nombreExistente.trim();
  const { ca_pem: _ca, ...destino } = destinoCuerpo(x.existente, { id: idDestino, ...(nombre ? { nombre } : {}) }) as ReturnType<typeof destinoCuerpo> & { ca_pem?: string };
  return { destino, ruta, existente: true, contrasena_destino: x.existente.contrasena };
}

export const existenteCompleto = (x: ExternaExtra) => repoExistenteCompleto(x.existente);

/** Qué pasa con la retención en el destino (como lo dice el agente al guardarla). */
export function textoRetencionDestino(conRetencion: boolean, bloqueo: number | null, soloAnadir = false): string {
  if (soloAnadir) return "El destino es de solo añadir: desde este equipo no se borra nada allí (la retención la aplica el propio servidor).";
  if (bloqueo && conRetencion)
    return `Con bloqueo de ${bloqueo} días, la retención de allí solo quita versiones de más de ${bloqueo} días (forget, sin prune): no libera espacio, así que el destino guarda todo lo subido y crece.`;
  if (bloqueo) return `Con bloqueo de ${bloqueo} días y sin retención propia, allí no se borra nada: guarda todas las versiones que suba.`;
  return conRetencion ? "La retención propia quita allí las versiones sobrantes y libera su espacio (forget --prune) después de cada subida." : "";
}

/** Lo que se dice de una copia externa en la ficha del repositorio («· a uno que ya existía · bloqueo de 30 días»). */
export function detallesExterna(x: { existente?: boolean | null; bloqueo_dias?: number | null; solo_anadir?: boolean | null } | null | undefined): string[] {
  if (!x) return [];
  return [x.existente ? "a un repositorio que ya existía" : "", x.bloqueo_dias ? `bloqueo de ${x.bloqueo_dias} días (sin prune)` : "", x.solo_anadir ? "destino de solo añadir" : ""].filter(Boolean);
}
