// 0.8.2 (C3, plan-mejoras 15jj punto 2 y 3): lo que estaba en marcha cuando el equipo dejó
// de responder. Sin runas (lo prueba scripts/vectores-082.ts).
//
// - **Lo calcula el servidor** (`progreso.rs`): cuando el progreso de una tarea deja de
//   llegar sin la lista vacía del final (90 s sin noticias), la pasa a `interrumpidas` del
//   equipo en `GET …/progreso`, con la hora de la última noticia. Así lo ven igual todas las
//   pestañas y personas de esa consola, aunque nadie estuviera mirando en ese momento. Solo
//   en memoria (como el progreso): tras reiniciar el servidor se olvida.
// - **Aquí se decide qué se enseña:** manda lo que diga el equipo. Una copia deja de salir
//   como interrumpida en cuanto hay una vuelta suya igual o más nueva que su empiezo (terminó,
//   o el agente 0.8.1 la anotó él mismo como «Interrumpida» al volver). Lo demás (verificar,
//   espejos…) deja de salir cuando el equipo vuelve a mandar un informe después del corte.
import { hora } from "./formato";
import { ultimaVuelta } from "./copia";
import type { Equipo, EquipoDetalle, Informe, TareaEnMarcha } from "./tipos";

type Informes = Record<string, Informe | null | undefined>;

export interface Cortada {
  equipo: Equipo;
  tarea: TareaEnMarcha;
  /** La última noticia de la tarea (cuando el equipo dejó de contar). */
  desde: string;
}

/** Un informe que llegó tanto después del corte dice ya cómo está el equipo. */
const MARGEN_INFORME = 2 * 60_000;

const fmtSemana = new Intl.DateTimeFormat("es", { weekday: "long" });
const fmtDiaMes = new Intl.DateTimeFormat("es", { day: "numeric", month: "long" });

/** Una hora pasada en frase: «a las 12:40», «ayer a las 12:40», «el lunes a las 12:40» o «el 4 de octubre a las 12:40». */
export function aLas(iso: string, ahora = Date.now()): string {
  const d = new Date(iso);
  const inicio = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const dias = Math.round((inicio(new Date(ahora)) - inicio(d)) / 86_400_000);
  const h = hora(iso);
  if (dias <= 0) return `a las ${h}`;
  if (dias === 1) return `ayer a las ${h}`;
  if (dias < 7) return `el ${fmtSemana.format(d)} a las ${h}`;
  return `el ${fmtDiaMes.format(d)} a las ${h}`;
}

/** ¿Sigue valiendo lo cortado, o el equipo ya dijo otra cosa? */
export function sigueCortada(equipo: Equipo, t: TareaEnMarcha, desde: string, informe: Informe | null = (equipo as EquipoDetalle).ultimo_informe ?? null): boolean {
  const corte = Date.parse(desde);
  if (!Number.isFinite(corte)) return false;
  if (t.tipo === "copia" && t.copia) {
    const k = equipo.resumen?.copias?.find((x) => x.id === t.copia);
    if (!k) return false;
    const v = ultimaVuelta(k, informe);
    const empezo = Date.parse(t.empezo ?? "");
    // Con margen: la hora de la vuelta y la del progreso las pone el mismo equipo, pero redondeadas.
    const desdeCuando = (Number.isFinite(empezo) ? empezo : corte) - 60_000;
    return !(v && Date.parse(v.cuando) >= desdeCuando);
  }
  const inf = Date.parse(informe?.recibido ?? "");
  return !(Number.isFinite(inf) && inf > corte + MARGEN_INFORME);
}

/** Lo cortado que sigue valiendo, de todos los equipos (o de uno). */
export function cortadasVigentes(cortadas: Record<string, { desde: string; tareas: TareaEnMarcha[] }>, equipos: Equipo[], informes: Informes = {}, soloEquipo?: string): Cortada[] {
  const out: Cortada[] = [];
  for (const [id, c] of Object.entries(cortadas)) {
    if (soloEquipo && id !== soloEquipo) continue;
    const e = equipos.find((x) => x.id === id);
    if (!e) continue;
    const inf = informes[id] ?? (e as EquipoDetalle).ultimo_informe ?? null;
    for (const t of c.tareas) if (sigueCortada(e, t, c.desde, inf)) out.push({ equipo: e, tarea: t, desde: c.desde });
  }
  return out;
}

/** La interrumpida de una copia (o null). */
export function cortadaDeCopia(cortadas: Record<string, { desde: string; tareas: TareaEnMarcha[] }>, equipo: Equipo, copia: string, informes: Informes = {}): Cortada | null {
  return cortadasVigentes(cortadas, [equipo], informes, equipo.id).find((c) => c.tarea.tipo === "copia" && c.tarea.copia === copia) ?? null;
}

/**
 * «Interrumpida: PC-02 se desconectó a las 12:40». Si el equipo ya volvió pero no ha dicho
 * nada de esa tarea: «… dejó de responder a las 12:40».
 */
export function textoCortada(c: Pick<Cortada, "equipo" | "desde">, ahora = Date.now()): string {
  const cuando = aLas(c.desde, ahora);
  return c.equipo.conectado ? `Interrumpida: ${c.equipo.nombre} dejó de responder ${cuando}` : `Interrumpida: ${c.equipo.nombre} se desconectó ${cuando}`;
}

/**
 * C3 (15jj punto 3): el estado del equipo junto a una copia en «Copias», solo si no está
 * conectado: «PC-02 desconectado desde ayer a las 18:05» (o null).
 */
export function equipoDesconectado(e: Pick<Equipo, "nombre" | "conectado" | "ultimo_contacto">, ahora = Date.now()): string | null {
  if (e.conectado) return null;
  return e.ultimo_contacto ? `${e.nombre} desconectado desde ${aLas(e.ultimo_contacto, ahora).replace(/^a las/, "las")}` : `${e.nombre} sin conectar todavía`;
}
