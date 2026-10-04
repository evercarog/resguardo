// Estado del modo agente (copias programadas), compartido por la interfaz.
import * as api from "$lib/api";
import type { AgentInfo, RunningTask, Schedule } from "$lib/api";

export const agent = $state<{ info: AgentInfo | null; error: string }>({ info: null, error: "" });

export async function refreshAgent() {
  try {
    agent.info = await api.agentInfo();
    agent.error = "";
  } catch (e) {
    agent.error = String(e);
  }
}

/** Tarea del agente en curso en este destino (si da señales de vida: un proceso cortado deja de contar a los 15 min). */
export function liveTask(repoId: string): RunningTask | null {
  const r = agent.info?.tasks?.running;
  if (!r || r.repo_id !== repoId) return null;
  return Date.now() - new Date(r.updated ?? r.started).getTime() < 15 * 60_000 ? r : null;
}

/** Destinos cuya copia externa sube a este destino de la app (`destino:<id>`). */
export function offsiteSources(repoId: string) {
  return agent.info?.repos.filter((r) => r.offsite?.provider === `destino:${repoId}`) ?? [];
}

/** Nombre del sitio al que sube la copia externa de un destino («Siigo · Backblaze», «Backblaze B2»…). */
export function offsiteTargetName(repoId: string, repos: { id: string; name: string }[]): string | undefined {
  const o = agent.info?.repos.find((r) => r.id === repoId)?.offsite;
  if (!o) return undefined;
  if (o.provider.startsWith("destino:")) return repos.find((r) => r.id === o.provider.slice("destino:".length))?.name ?? o.target_name ?? undefined;
  return { b2: "Backblaze B2", wasabi: "Wasabi", r2: "Cloudflare R2", aws: "Amazon S3", s3: "S3" }[o.provider] ?? undefined;
}

/** En plural, para «los domingos a las 03:00». */
const WEEKDAYS = ["lunes", "martes", "miércoles", "jueves", "viernes", "sábados", "domingos"];

export function scheduleLabel(s: Schedule) {
  switch (s.kind) {
    case "plans":
      return "según los horarios de sus copias";
    case "monitor":
      return `solo vigilar (se esperan copias ${s.every === 24 ? "cada día" : `cada ${s.every} h`})`;
    case "hours":
      return s.every === 1 ? "cada hora" : `cada ${s.every} horas`;
    case "daily":
      return `todos los días a las ${s.time}`;
    case "weekly":
      return `los ${WEEKDAYS[s.weekday]} a las ${s.time}`;
    case "after_backup":
      return s.min_minutes > 0 ? `después de cada copia con cambios (como mucho cada ${s.min_minutes} min)` : "después de cada copia con cambios";
  }
}

function atTime(day: Date, time: string) {
  const [h, m] = time.split(":").map(Number);
  return new Date(day.getFullYear(), day.getMonth(), day.getDate(), h, m);
}

/**
 * Próxima copia según el horario (misma regla que `Schedule::is_due` en el
 * agente). `since` es la última copia o el momento en que se activó.
 * Si ya tocaba, devuelve una fecha pasada: el agente la hará en ≤ 5 min.
 */
export function nextRun(s: Schedule, since: Date): Date | null {
  // «Después de cada copia» no tiene hora: depende de la próxima copia con cambios.
  if (s.kind === "monitor" || s.kind === "plans" || s.kind === "after_backup") return null;
  if (s.kind === "hours") return new Date(since.getTime() + s.every * 3_600_000);
  let slot = atTime(since, s.time);
  if (s.kind === "weekly") {
    const back = (((since.getDay() + 6) % 7) - s.weekday + 7) % 7; // lunes = 0
    slot = atTime(new Date(since.getFullYear(), since.getMonth(), since.getDate() - back), s.time);
  }
  const step = s.kind === "daily" ? 1 : 7;
  while (slot <= since) slot = new Date(slot.getFullYear(), slot.getMonth(), slot.getDate() + step, slot.getHours(), slot.getMinutes());
  return slot;
}
