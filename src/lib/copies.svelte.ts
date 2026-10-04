// Copias (planes de copia vistos como trabajos): qué versiones guardó cada
// una, si el agente la tiene programada y su estado para la barra lateral.
// Las funciones leen estado reactivo (agente, copias en curso, snapshots):
// usadas dentro de $derived se actualizan solas.
import type { AgentPause, AgentPlan, AgentRun, AgentRunning, Plan, Repo, Snapshot } from "$lib/api";
import { agent } from "$lib/agent.svelte";
import { runs } from "$lib/backups.svelte";
import { health } from "$lib/status.svelte";
import { latestPlanSlot, nextPlanSlot, samePlan } from "$lib/plans";
import { pauseOf } from "$lib/pause.svelte";

/** Una copia: el plan y el destino que lo contiene. */
export interface CopyRef {
  repo: Repo;
  plan: Plan;
}

/** Todas las copias, en el orden de los destinos y de sus planes. */
export const allCopies = (repos: Repo[]): CopyRef[] => repos.flatMap((repo) => (repo.plans ?? []).map((plan) => ({ repo, plan })));

/** Ruta comparable: barras unificadas, sin barra final y, si es de Windows, en minúsculas. */
function normPath(p: string) {
  const windows = /^[A-Za-z]:([\\/]|$)/.test(p) || p.includes("\\");
  let x = p.trim().replace(/\\/g, "/").replace(/\/+$/, "");
  if (/^[A-Za-z]:$/.test(x)) x += "/";
  return windows ? x.toLowerCase() : x;
}

/** ¿Esta versión la guardó esta copia? Mismas carpetas (sin importar el orden) y todas sus etiquetas. */
export function isPlanSnapshot(s: Snapshot, plan: Plan) {
  const a = new Set(s.paths.map(normPath));
  const b = new Set(plan.paths.map(normPath));
  if (!b.size || a.size !== b.size || [...b].some((x) => !a.has(x))) return false;
  return plan.tags.every((t) => s.tags?.includes(t));
}

export const planSnapshots = (snapshots: Snapshot[], plan: Plan) => snapshots.filter((s) => isPlanSnapshot(s, plan));

/** Copia que el agente está haciendo ahora; si lleva más de 10 min sin dar señales, se ignora. */
export function agentRunningNow(): AgentRunning | null {
  const r = agent.info?.state.running;
  if (!r) return null;
  return Date.now() - new Date(r.updated ?? r.started).getTime() < 10 * 60_000 ? r : null;
}

/** "paused": copia automática con el destino en pausa (estado neutro, no es un problema). */
export type CopyLevel = "running" | "paused" | "error" | "warning" | "late" | "ok" | "never" | "loading";

/**
 * Relación con el agente:
 * - manual: sin horario, solo se copia a mano.
 * - scheduled: el agente la tiene tal cual.
 * - stale: el agente tiene una versión anterior (cambios sin aplicar).
 * - unscheduled: tiene horario pero el agente no la programa.
 * - monitor: el destino está en «Solo vigilar».
 * - unsupported: el agente no existe en este sistema.
 */
export type AutoState = "loading" | "unsupported" | "manual" | "scheduled" | "stale" | "unscheduled" | "monitor";

export const COPY_LEVEL_LABEL: Record<CopyLevel, string> = {
  running: "Copiando",
  paused: "En pausa",
  error: "Falló la última copia",
  warning: "Última copia con avisos",
  late: "Con retraso",
  ok: "Al día",
  never: "Sin copias todavía",
  loading: "Comprobando…",
};

export interface CopyStatus {
  level: CopyLevel;
  label: string;
  auto: AutoState;
  /** Plan tal como lo tiene el agente (si lo programa). */
  agentPlan: AgentPlan | null;
  /** Última copia automática según el agente. */
  lastRun: AgentRun | null;
  /** Versión más reciente de esta copia en el destino. */
  lastSnapshot: Snapshot | null;
  /**
   * Momento de la última copia terminada (versión guardada o copia correcta,
   * automática o a mano). Una copia sin cambios («Solo guardar si hay
   * cambios») también cuenta: la copia está al día aunque no haya versión nueva.
   */
  last: Date | null;
  /** La última copia terminada no encontró cambios: no se guardó versión (la última es `lastSnapshot`). */
  unchanged: boolean;
  /** Próxima copia automática (null en pausa). */
  next: Date | null;
  /** Pausa de las copias automáticas del destino (si esta copia es automática). */
  paused: AgentPause | null;
  /** Copia en marcha de esta copia: a mano (desde la app) o del agente. */
  manualRunning: boolean;
  agentRunning: AgentRunning | null;
}

const time = (iso: string | null | undefined) => (iso ? new Date(iso).getTime() : 0);

export function copyStatus(repo: Repo, plan: Plan, now = Date.now()): CopyStatus {
  const info = agent.info;
  const current = info?.repos.find((r) => r.id === repo.id) ?? null;
  const agentPlan = current?.schedule.kind === "plans" ? ((current.plans ?? []).find((p) => p.id === plan.id) ?? null) : null;

  let auto: AutoState;
  if (!info) auto = "loading";
  else if (!info.supported) auto = plan.schedule ? "unsupported" : "manual";
  else if (current?.schedule.kind === "monitor") auto = "monitor";
  else if (!plan.schedule) auto = agentPlan ? "stale" : "manual";
  else if (!current) auto = "unscheduled";
  else if (!agentPlan) auto = "stale";
  else auto = samePlan(plan, agentPlan) && current.location === repo.location ? "scheduled" : "stale";

  const lastRun = info?.state.runs[`${repo.id}#${plan.id}`] ?? null;
  const h = health[repo.id];
  const snaps = planSnapshots(h?.snapshots ?? [], plan);
  const lastSnapshot = snaps.reduce<Snapshot | null>((a, s) => (!a || s.time > a.time ? s : a), null);
  const manual = runs[repo.id];
  // Copia a mano correcta de esta copia (sin cambios no deja versión: cuenta igual).
  const manualOk = !!manual && !manual.running && manual.planId === plan.id && !!manual.result ? (manual.finishedAt ?? 0) : 0;
  const agentOk = lastRun && lastRun.result !== "error" ? time(lastRun.finished) : 0;
  const lastMs = Math.max(time(lastSnapshot?.time), agentOk, manualOk);
  // ¿La más reciente fue una revisión sin cambios? (La versión de esa revisión no existe.)
  const unchanged =
    lastMs > time(lastSnapshot?.time) &&
    ((manualOk === lastMs && !!manual?.result?.unchanged) || (agentOk === lastMs && !!lastRun?.unchanged));

  const manualRunning = !!manual?.running && manual.planId === plan.id;
  const live = agentRunningNow();
  const agentRunning = live && live.repo_id === repo.id && live.plan_id === plan.id ? live : null;

  const automatic = !!agentPlan && (auto === "scheduled" || auto === "stale");
  const paused = automatic ? pauseOf(repo.id, now) : null;
  let next: Date | null = null;
  if (agentPlan && automatic && !paused) {
    next = nextPlanSlot(agentPlan.schedule, new Date(Math.max(time(agentPlan.enabled_at), time(lastRun?.started))));
  }

  // Una copia a mano que falló (y no se detuvo a propósito) cuenta como el último
  // resultado solo si es posterior a la última versión o copia automática correcta.
  const manualFailed =
    !!manual &&
    !manual.running &&
    manual.planId === plan.id &&
    !!manual.failure &&
    !manual.cancelled &&
    (manual.finishedAt ?? 0) > lastMs;

  let level: CopyLevel;
  if (manualRunning || agentRunning) level = "running";
  // Un fallo sigue a la vista aunque esté en pausa (como en la web).
  else if (manualFailed) level = "error";
  else if (lastRun?.result === "error" && time(lastRun.finished) >= time(lastSnapshot?.time)) level = "error";
  // En pausa a propósito: no se avisa de retrasos.
  else if (paused) level = "paused";
  else if (isLate(agentPlan, auto, lastRun, lastSnapshot, now)) level = "late";
  else if (lastRun?.result === "warning" && time(lastRun.finished) >= time(lastSnapshot?.time)) level = "warning";
  // La copia a mano más reciente terminó con archivos sin leer.
  else if (manualOk && manualOk === lastMs && !!manual?.result?.incomplete) level = "warning";
  else if (lastMs) level = "ok";
  else if (!h || (h.loading && !h.loadedAt)) level = "loading";
  else level = "never";

  return {
    level,
    label: COPY_LEVEL_LABEL[level],
    auto,
    agentPlan,
    lastRun,
    lastSnapshot,
    last: lastMs ? new Date(lastMs) : null,
    unchanged,
    next,
    paused,
    manualRunning,
    agentRunning,
  };
}

/** Programada y, pasada hora y media de la última hora prevista, sin copia desde entonces. */
function isLate(agentPlan: AgentPlan | null, auto: AutoState, run: AgentRun | null, snap: Snapshot | null, now: number) {
  if (!agentPlan || (auto !== "scheduled" && auto !== "stale")) return false;
  const slot = latestPlanSlot(agentPlan.schedule, new Date(now));
  if (!slot || slot.getTime() <= time(agentPlan.enabled_at)) return false;
  if (now - slot.getTime() < 90 * 60_000) return false;
  return Math.max(time(run?.started), time(snap?.time)) < slot.getTime();
}
