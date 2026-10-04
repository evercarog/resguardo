// Estado de las copias de todos los repositorios: la última copia de cada
// uno, su ritmo habitual y si va con retraso. Los snapshots se cargan con
// `restic snapshots`, que es una consulta ligera.
import * as api from "$lib/api";
import type { Repo, Snapshot } from "$lib/api";
import { pauseOf } from "$lib/pause.svelte";
import { offsiteSources } from "$lib/agent.svelte";

export interface RepoHealth {
  snapshots: Snapshot[];
  loading: boolean;
  error: string;
  loadedAt: number | null;
}

export const health = $state<Record<string, RepoHealth>>({});

export function setSnapshots(id: string, snapshots: Snapshot[]) {
  health[id] = { snapshots, loading: false, error: "", loadedAt: Date.now() };
}

export async function refresh(id: string) {
  const prev = health[id];
  health[id] = { snapshots: prev?.snapshots ?? [], loading: true, error: "", loadedAt: prev?.loadedAt ?? null };
  try {
    setSnapshots(id, await api.listSnapshots(id));
  } catch (e) {
    health[id].loading = false;
    health[id].error = String(e);
  }
}

export const refreshAll = (repos: Repo[]) => Promise.all(repos.map((r) => refresh(r.id)));

const HOUR = 3_600_000;

/**
 * Ritmo habitual de copias en horas: la mediana de los intervalos entre las
 * últimas copias. null si no hay historial suficiente.
 */
export function detectInterval(snapshots: Snapshot[]): number | null {
  const times = [...new Set(snapshots.map((s) => new Date(s.time).getTime()))].sort((a, b) => b - a).slice(0, 16);
  const gaps = times
    .slice(1)
    .map((t, i) => (times[i] - t) / HOUR)
    .filter((h) => h > 0.1); // copias casi simultáneas (varias carpetas) no cuentan
  if (gaps.length < 3) return null;
  gaps.sort((a, b) => a - b);
  const median = gaps[Math.floor(gaps.length / 2)];
  // Redondeo a valores razonables: minutos → horas enteras; días → días enteros.
  return median < 20 ? Math.max(1, Math.round(median)) : Math.round(median / 24) * 24;
}

/** "paused": copias automáticas en pausa (estado neutro: no avisa de retrasos). */
export type Level = "ok" | "late" | "overdue" | "empty" | "error" | "loading" | "paused";

export interface RepoStatus {
  level: Level;
  label: string;
  last: Snapshot | null;
  /** Horas entre copias esperadas y si vienen de la configuración o del historial. */
  expected: number;
  auto: boolean;
  detected: number | null;
  /** Horas desde la última copia. */
  since: number | null;
}

export const LEVEL_LABEL: Record<Level, string> = {
  ok: "Al día",
  late: "Con retraso",
  overdue: "Atrasado",
  empty: "Sin versiones",
  error: "Sin conexión",
  loading: "Comprobando…",
  paused: "En pausa",
};

/** Prioridad para ordenar: lo que necesita atención primero. */
export const LEVEL_ORDER: Record<Level, number> = { error: 0, overdue: 1, late: 2, empty: 3, paused: 4, loading: 5, ok: 6 };

export function statusOf(repo: Repo, h: RepoHealth | undefined, now = Date.now()): RepoStatus {
  const snapshots = h?.snapshots ?? [];
  const detected = detectInterval(snapshots);
  const expected = repo.expected_hours ?? detected ?? 24;
  const auto = repo.expected_hours == null;
  const last = snapshots.reduce<Snapshot | null>((a, s) => (!a || s.time > a.time ? s : a), null);
  const since = last ? (now - new Date(last.time).getTime()) / HOUR : null;
  const base = { last, expected, auto, detected, since };

  let level: Level;
  if (!h || (h.loading && !h.loadedAt)) level = "loading";
  else if (h.error && !snapshots.length) level = "error";
  else if (!last || since === null) level = "empty";
  // Margen: una hora, o un cuarto del intervalo, para no avisar por minutos.
  else if (since > expected * 2 + 1) level = "overdue";
  else if (since > expected * 1.25 + 1) level = "late";
  else level = "ok";
  if (h?.error && snapshots.length) level = level === "ok" ? "error" : level;
  // Destino de una copia externa: sus versiones llegan cuando sube el origen (p. ej. tras cada
  // copia con cambios), sin un ritmo propio que vigilar; los fallos de subida se ven en el origen.
  if ((level === "late" || level === "overdue") && offsiteSources(repo.id).length) level = "ok";
  // En pausa a propósito (p. ej. por un mantenimiento): ni retrasos ni errores de conexión.
  if (level !== "loading" && pauseOf(repo.id, now)) level = "paused";

  return { ...base, level, label: LEVEL_LABEL[level] };
}

/** "cada hora", "cada 6 h", "diaria", "cada 2 días", "semanal"… */
export function frequencyLabel(hours: number) {
  if (hours === 1) return "cada hora";
  if (hours < 24) return `cada ${hours} h`;
  if (hours === 24) return "diaria";
  if (hours === 168) return "semanal";
  if (hours % 24 === 0) return `cada ${hours / 24} días`;
  return `cada ${hours} h`;
}

/** Tiempo transcurrido en horas → "3 h", "2 días"… */
export function elapsedLabel(hours: number) {
  if (hours < 1) return `${Math.max(1, Math.round(hours * 60))} min`;
  if (hours < 48) return `${Math.round(hours)} h`;
  return `${Math.round(hours / 24)} días`;
}
