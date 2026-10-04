// Pausa de las copias automáticas de un destino (ver `agent::Pause`): mientras
// dura, el agente no empieza copias programadas, reintentos, verificaciones ni
// copias externas de ese destino. Lo que esté en marcha termina normalmente.
import type { AgentPause } from "$lib/api";
import { agent } from "$lib/agent.svelte";

/** Duración máxima de una pausa con fecha de fin (la misma que comprueba el agente). */
export const MAX_PAUSE_DAYS = 30;

/**
 * Pausa vigente de un destino. Una que ya terminó no cuenta: el agente la
 * quita (y reanuda las copias) en su próxima vuelta, en ≤ 5 minutos.
 */
export function pauseOf(repoId: string, now = Date.now()): AgentPause | null {
  const p = agent.info?.repos.find((r) => r.id === repoId)?.pause;
  if (!p) return null;
  if (p.until && !(new Date(p.until).getTime() > now)) return null;
  return p;
}

const DAYS = ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"];
const MONTHS = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];
const pad = (n: number) => String(n).padStart(2, "0");

/** «hoy a las 14:00», «mañana a las 06:00» o «el jueves 2 de octubre a las 06:00». */
export function whenInWords(d: Date, now = new Date()) {
  const startOf = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const days = Math.round((startOf(d) - startOf(now)) / 86_400_000);
  const time = `a las ${pad(d.getHours())}:${pad(d.getMinutes())}`;
  if (days === 0) return `hoy ${time}`;
  if (days === 1) return `mañana ${time}`;
  const year = d.getFullYear() !== now.getFullYear() ? ` de ${d.getFullYear()}` : "";
  return `el ${DAYS[d.getDay()]} ${d.getDate()} de ${MONTHS[d.getMonth()]}${year} ${time}`;
}

/** «hasta mañana a las 06:00» o «hasta que las reanudes». */
export function untilWords(p: AgentPause, now = new Date()) {
  return p.until ? `hasta ${whenInWords(new Date(p.until), now)}` : "hasta que las reanudes";
}

/** Duraciones que ofrece el diálogo «Pausar copias automáticas». */
export type PauseChoice = "1h" | "4h" | "morning" | "24h" | "3d" | "date" | "manual";

export const PAUSE_CHOICES: { id: PauseChoice; label: string }[] = [
  { id: "1h", label: "1 hora" },
  { id: "4h", label: "4 horas" },
  { id: "morning", label: "Hasta mañana a las 6:00" },
  { id: "24h", label: "24 horas" },
  { id: "3d", label: "3 días" },
  { id: "date", label: "Hasta una fecha y hora…" },
  { id: "manual", label: "Hasta que la reanude" },
];

/**
 * Cuándo terminaría la pausa: una fecha, `null` (hasta reanudarla a mano) o
 * un error en palabras. Los días se cuentan en hora local (un cambio de hora
 * no mueve «mañana a las 6:00»); las horas, como tiempo transcurrido.
 * `custom`: valor de un `<input type="datetime-local">` ("2026-10-02T06:00").
 */
export function pauseUntil(choice: PauseChoice, custom: string, now = new Date()): Date | null | { error: string } {
  const at = (days: number, h = now.getHours(), m = now.getMinutes()) =>
    new Date(now.getFullYear(), now.getMonth(), now.getDate() + days, h, m, now.getSeconds());
  switch (choice) {
    case "1h":
      return new Date(now.getTime() + 3_600_000);
    case "4h":
      return new Date(now.getTime() + 4 * 3_600_000);
    case "24h":
      return new Date(now.getTime() + 24 * 3_600_000);
    case "morning":
      return new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1, 6, 0);
    case "3d":
      return at(3);
    case "manual":
      return null;
    case "date": {
      if (!custom) return { error: "Elige el día y la hora." };
      const d = new Date(custom); // sin zona: hora local
      if (Number.isNaN(d.getTime())) return { error: "La fecha no es válida." };
      if (d.getTime() <= now.getTime()) return { error: "Elige un momento futuro." };
      if (d.getTime() > now.getTime() + MAX_PAUSE_DAYS * 86_400_000)
        return { error: `Como mucho ${MAX_PAUSE_DAYS} días. Para más tiempo, elige «Hasta que la reanude».` };
      return d;
    }
  }
}

/** Valor para `<input type="datetime-local">` en hora local ("2026-10-02T06:00"). */
export function toLocalInput(d: Date) {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
