// Restauraciones en curso, compartidas por toda la app (como backups.svelte.ts).
import { listen } from "@tauri-apps/api/event";
import * as api from "$lib/api";
import type { RestoreProgress, RestoreRequest, RestoreResult } from "$lib/api";
import { formatBytes } from "$lib/format";
import { toast } from "$lib/toast.svelte";

type Status = Extract<RestoreProgress, { kind: "status" }>;

export interface RestoreRun {
  running: boolean;
  /** Nombre del repositorio, para los avisos. */
  name: string;
  request: Omit<RestoreRequest, "password">;
  status: Status | null;
  errors: string[];
  errorCount: number;
  result: RestoreResult | null;
  failure: string | null;
}

const MAX_LIVE_ERRORS = 200;

export const restores = $state<Record<string, RestoreRun>>({});

let listening: Promise<unknown> | null = null;

function ensureListening() {
  listening ??= listen<RestoreProgress>(api.RESTORE_PROGRESS_EVENT, ({ payload }) => {
    const run = restores[payload.repo_id];
    if (!run?.running) return;
    if (payload.kind === "status") {
      run.status = payload;
    } else {
      run.errorCount++;
      if (run.errors.length < MAX_LIVE_ERRORS) run.errors.push(payload.message);
    }
  });
  return listening;
}

/** Restauraciones que se pidió detener: al terminar no se avisa como un error. */
const cancelled = new Set<string>();

export async function startRestore(repoId: string, name: string, req: RestoreRequest) {
  await ensureListening();
  cancelled.delete(repoId);
  const { password, ...request } = req;
  restores[repoId] = { running: true, name, request, status: null, errors: [], errorCount: 0, result: null, failure: null };
  const run = restores[repoId];
  try {
    run.result = await api.runRestore(repoId, { ...request, password });
    run.errors = run.result.errors;
    run.errorCount = run.result.error_count;
    const bytes = run.result.summary?.bytes_restored;
    toast(
      `Restauración desde «${name}» ${run.result.error_count ? "terminada con avisos" : "completada"}${bytes != null ? ` · ${formatBytes(bytes)}` : ""}`,
      run.result.error_count ? "info" : "success",
    );
  } catch (e) {
    run.failure = String(e);
    if (cancelled.has(repoId)) toast(`Restauración desde «${name}» detenida`, "info");
    else toast(`No se pudo completar la restauración desde «${name}»`, "error", 6000);
  } finally {
    cancelled.delete(repoId);
    run.running = false;
  }
}

export function cancelRestore(repoId: string) {
  cancelled.add(repoId);
  return api.cancelRestore(repoId).catch((e) => {
    cancelled.delete(repoId);
    throw e;
  });
}

export function clearRestore(repoId: string) {
  if (!restores[repoId]?.running) delete restores[repoId];
}
