// 15hh: parar o pausar lo que está en marcha (orden `parar { clave, pausa? }`, con la clave de
// administración y sin espera). Aquí, las opciones de «Pausar» y cómo se dice una copia pausada.
import type { CopiaResumen, Equipo } from "./tipos";
import { cuandoFrase } from "./formato";

/** ¿El agente sabe parar y pausar? (También manda `parar` en el progreso.) */
export const admiteParar = (e: Pick<Equipo, "resumen"> | null | undefined) => !!e?.resumen?.admite?.includes("parar");

/** 15ll: ¿pide el agente que el equipo no se suspenda mientras copia (y deja apagarlo por copia)? */
export const admiteSinSuspension = (e: Pick<Equipo, "resumen"> | null | undefined) => !!e?.resumen?.admite?.includes("sin_suspension");

export type OpcionPausa = "noche" | "dos_horas" | "manana" | "a_mano";

export const OPCIONES_PAUSA: { id: OpcionPausa; texto: string }[] = [
  { id: "noche", texto: "Esta noche a las 22:00" },
  { id: "dos_horas", texto: "Dentro de 2 horas" },
  { id: "manana", texto: "Mañana a las 08:00" },
  { id: "a_mano", texto: "A mano: cuando pulses «Copiar ahora»" },
];

/** El cuerpo de `pausa` para una opción, contado desde `ahora` (la hora del navegador). */
export function hastaDePausa(o: OpcionPausa, ahora: Date): { hasta: string } | { a_mano: true } {
  if (o === "a_mano") return { a_mano: true };
  if (o === "dos_horas") return { hasta: new Date(ahora.getTime() + 2 * 3600_000).toISOString() };
  const h = new Date(ahora);
  if (o === "noche") {
    h.setHours(22, 0, 0, 0);
    if (h <= ahora) h.setDate(h.getDate() + 1);
  } else {
    h.setDate(h.getDate() + 1);
    h.setHours(8, 0, 0, 0);
  }
  return { hasta: h.toISOString() };
}

/** «En pausa hasta hoy 22:00 · pausada por Ana desde la consola «Oficina»» (o `null` si no lo está). */
export function textoPausa(k: Pick<CopiaResumen, "pausa">, ahora = Date.now()): string | null {
  const p = k.pausa;
  if (!p) return null;
  const hasta = p.hasta ? `En pausa hasta ${cuandoFrase(p.hasta, ahora)}` : "En pausa hasta que pulses «Copiar ahora»";
  return p.quien ? `${hasta} · pausada ${p.quien}` : hasta;
}
