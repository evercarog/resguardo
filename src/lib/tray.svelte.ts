// Icono de la bandeja del sistema: la interfaz calcula el estado general (con
// las mismas reglas que el titular del panel «Estado») y se lo manda al
// backend, que pinta el escudo y monta el menú.
import * as api from "$lib/api";
import type { Repo, TrayStatus } from "$lib/api";
import { agent } from "$lib/agent.svelte";
import { runs } from "$lib/backups.svelte";
import { agentRunningNow, copyStatus } from "$lib/copies.svelte";
import { health, statusOf } from "$lib/status.svelte";

/** Lo urgente, como en «Estado»: subidas frenadas, destinos sin conexión o atrasados, copias que fallaron y retrasos leves. */
function attention(repos: Repo[], now: number) {
  let bad = 0;
  let warn = 0;
  for (const r of repos) {
    if (agent.info?.offsite_holds?.[r.id]) bad++;
    const level = statusOf(r, health[r.id], now).level;
    if (level === "error" || level === "overdue") bad++;
    else if (level === "late") warn++;
    for (const p of r.plans ?? []) if (copyStatus(r, p, now).level === "error") bad++;
  }
  return { bad, warn };
}

const pct = (p: number | null | undefined) => (p != null ? ` · ${Math.round(p * 100)} %` : "");

/** Lo que está en marcha, en una línea. */
function progressLine(repos: Repo[], now: number): string | null {
  const name = (id: string) => repos.find((r) => r.id === id)?.name ?? "un repositorio";
  const manual = repos.find((r) => runs[r.id]?.running);
  if (manual) {
    const run = runs[manual.id];
    return `Copiando «${run.planName || manual.name}»${pct(run.status?.percent)}`;
  }
  const auto = agentRunningNow();
  if (auto) {
    const repo = repos.find((r) => r.id === auto.repo_id);
    const plan = repo?.plans.find((p) => p.id === auto.plan_id);
    return `Copiando «${plan?.name ?? name(auto.repo_id)}»${pct(auto.percent)}`;
  }
  const task = agent.info?.tasks?.running;
  if (task && now - new Date(task.updated ?? task.started).getTime() < 15 * 60_000) {
    const where = name(task.repo_id);
    switch (task.kind) {
      case "offsite":
        return `Subiendo a la nube «${where}»${pct(task.percent)}`;
      case "verify":
        return `Verificando «${where}»`;
      case "verify_offsite":
        return `Verificando la copia en la nube de «${where}»`;
      case "restore_test":
        return `Probando a restaurar «${where}»`;
    }
  }
  return null;
}

/** Estado para la bandeja. `loaded`: ya se leyó la lista de destinos. */
export function trayStatus(repos: Repo[], loaded: boolean, now = Date.now()): TrayStatus {
  const copies = repos.flatMap((r) => (r.plans ?? []).map((p) => ({ repo_id: r.id, plan_id: p.id, label: `${p.name} · ${r.name}` })));
  const progress = progressLine(repos, now);
  if (!loaded || (repos.length && repos.every((r) => statusOf(r, health[r.id], now).level === "loading"))) {
    return { tone: "neutral", tooltip: "Resguardo: comprobando tus copias…", progress, copies };
  }
  if (!repos.length) return { tone: "neutral", tooltip: "Resguardo: aún no hay copias", progress, copies };
  const { bad, warn } = attention(repos, now);
  const n = bad + warn;
  if (n) {
    return { tone: bad ? "bad" : "warn", tooltip: `Resguardo: ${n} ${n === 1 ? "cosa necesita" : "cosas necesitan"} atención`, progress, copies };
  }
  return { tone: "ok", tooltip: "Resguardo: todo protegido", progress, copies };
}

let last = "";
let pending: TrayStatus | null = null;
let timer: ReturnType<typeof setTimeout> | null = null;

/** Manda el estado a la bandeja: como mucho uno por segundo, y solo si cambió. */
export function syncTray(status: TrayStatus) {
  pending = status;
  if (timer) return;
  timer = setTimeout(() => {
    timer = null;
    const key = JSON.stringify(pending);
    if (!pending || key === last) return;
    api.trayUpdate(pending).then(
      () => (last = key),
      () => {}, // bloqueada: se manda al desbloquear
    );
  }, 1000);
}
