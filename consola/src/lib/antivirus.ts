// 0.8.3 (16z): un equipo que deja de responder justo después de una copia correcta. En la
// prueba real, Microsoft Defender puso el agente en cuarentena nada más terminar una copia:
// quitó el programa y el servicio, y la consola solo vio «desconectado». No se puede saber
// desde aquí si fue un antivirus o si alguien apagó el equipo, así que es solo una pista.
// Sin runas (lo prueba scripts/vectores.ts).
import type { Equipo } from "./tipos";

const MIN = 60_000;
/** Sin contacto al menos esto antes de decir nada (un corte de red corto no cuenta). */
export const ESPERA_PISTA = 10 * MIN;
/** La última copia correcta terminó, como mucho, esto antes del último contacto. */
export const VENTANA_COPIA = 30 * MIN;

type EquipoPista = Pick<Equipo, "confirmado" | "modo" | "conectado" | "ultimo_contacto" | "estado_servicio" | "resumen">;

/**
 * ¿Sugerir mirar el antivirus? Solo si el equipo está desconectado desde hace un rato, no
 * dijo que lo paraban (servicio detenido por un administrador, dejó la consola, trasladado),
 * no es un equipo que sale de la oficina, y su última copia correcta terminó justo antes de
 * perder el contacto. Devuelve la hora de esa copia, o `null`.
 */
export function pistaAntivirus(e: EquipoPista, ahora = Date.now()): string | null {
  if (!e.confirmado || e.modo !== "gestionado" || e.conectado) return null;
  if (e.estado_servicio === "detenido_por_admin") return null;
  if (e.resumen?.movil?.sale_oficina) return null;
  const contacto = Date.parse(e.ultimo_contacto ?? "");
  if (!Number.isFinite(contacto) || ahora - contacto < ESPERA_PISTA) return null;
  let ultima: { cuando: string; t: number } | null = null;
  for (const c of e.resumen?.copias ?? []) {
    const u = c.ultima;
    if (!u || (u.estado !== "ok" && u.estado !== "aviso")) continue;
    const t = Date.parse(u.cuando);
    if (Number.isFinite(t) && (!ultima || t > ultima.t)) ultima = { cuando: u.cuando, t };
  }
  // Algo más nuevo que no fue bien (fallo, interrumpida, parada) ya cuenta otra historia.
  for (const c of e.resumen?.copias ?? []) {
    const u = c.ultima;
    if (u && u.estado !== "ok" && u.estado !== "aviso" && ultima && Date.parse(u.cuando) > ultima.t) return null;
  }
  if (!ultima) return null;
  // El contacto se anota con algo de redondeo: se admite que la copia «termine» un poco después.
  const diferencia = contacto - ultima.t;
  if (diferencia < -15 * MIN || diferencia > VENTANA_COPIA) return null;
  return ultima.cuando;
}
