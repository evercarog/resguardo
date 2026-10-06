// El espejo del almacén por destino (docs/espejo.md): horario, «después de
// cada copia nueva» y lo que se manda al equipo. Sin dependencias de Svelte,
// para las pruebas (scripts/vectores-espejo.ts).
import { horarioEnFrase } from "./formato";
import type { Equipo, Horario } from "./tipos";

/** El agente entiende el espejo por destino (horario, selección, retención y verificación). */
export const ADMITE_FLEXIBLE = "espejo_flexible";

export const admiteEspejoFlexible = (e: Pick<Equipo, "resumen"> | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_FLEXIBLE);

/** Un destino del espejo como lo da el resumen del equipo. */
export interface DestinoEspejoResumen {
  tipo: "carpeta" | "nube";
  carpeta?: string | null;
  nube?: string | null;
  ultima?: string | null;
  resultado?: string | null;
  /** Su horario propio (sin él, cada día a `espejo.hora`). */
  horario?: Horario | null;
  /** También después de cada copia nueva. */
  tras_copia?: boolean | null;
  /** La próxima vuelta por horario. */
  proxima?: string | null;
}

/** Lo que se manda de un destino en `guarda_copias.espejo.destinos` (sin sus resultados). */
export interface DestinoEspejoOrden {
  tipo: "carpeta" | "nube";
  carpeta: string;
  nube?: string;
  horario?: Horario;
  tras_copia?: boolean;
}

/** Un destino del resumen en la forma de la orden: lo que ya tiene, para reenviarlo sin cambios. */
export function destinoParaOrden(d: DestinoEspejoResumen): DestinoEspejoOrden {
  const o: DestinoEspejoOrden = d.tipo === "nube" ? { tipo: "nube", nube: d.nube ?? "", carpeta: d.carpeta ?? "" } : { tipo: "carpeta", carpeta: d.carpeta ?? "" };
  if (d.horario && (d.horario.reglas?.length || d.horario.horas?.length)) o.horario = d.horario;
  if (d.tras_copia) o.tras_copia = true;
  return o;
}

/** El horario «cada día a esa hora» de antes, como horario de las copias. */
export const horarioDiario = (hora: string): Horario => ({ dias: [1, 2, 3, 4, 5, 6, 7], horas: [hora] });

/** «Cada día a las 02:00», «Cada hora de 8:00 a 18:00, … y después de cada copia nueva». */
export function cuandoEspejo(d: Pick<DestinoEspejoResumen, "horario" | "tras_copia">, horaGlobal: string): string {
  const h = d.horario && (d.horario.reglas?.length || d.horario.horas?.length) ? horarioEnFrase(d.horario) : `Cada día a las ${horaGlobal}`;
  return d.tras_copia ? `${h} y después de cada copia nueva` : h;
}

const tieneHorario = (d: Pick<DestinoEspejoResumen, "horario">) => !!(d.horario && (d.horario.reglas?.length || d.horario.horas?.length));

/** Corto, para el mapa y las flechas: «cada noche a las 02:00» (como antes) o «con su horario y tras cada copia». */
export function cuandoCorto(d: Pick<DestinoEspejoResumen, "horario" | "tras_copia">, horaGlobal: string): string {
  const base = tieneHorario(d) ? "con su horario" : `cada noche a las ${horaGlobal}`;
  return d.tras_copia ? `${base} y tras cada copia` : base;
}

/** Lo mismo para todo el espejo: el de sus destinos si todos coinciden; si no, «con el horario de cada destino». */
export function cuandoCortoEspejo(e: { hora: string; destinos?: Pick<DestinoEspejoResumen, "horario" | "tras_copia">[] | null }): string {
  const textos = [...new Set((e.destinos?.length ? e.destinos : [{}]).map((d) => cuandoCorto(d, e.hora)))];
  return textos.length === 1 ? textos[0] : "con el horario de cada destino";
}

/** La hora que se manda en `espejo.hora` (para consolas anteriores): la primera hora del primer destino. */
export function horaParaConsolasAnteriores(destinos: DestinoEspejoOrden[], porDefecto: string): string {
  for (const d of destinos) {
    const h = d.horario?.horas?.[0] ?? d.horario?.reglas?.map((r) => ("horas" in r ? r.horas[0] : "desde" in r ? r.desde : r.hora)).find(Boolean);
    if (h && /^([01]\d|2[0-3]):[0-5]\d$/.test(h)) return h;
  }
  return porDefecto;
}
