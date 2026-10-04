// Estado de las copias en curso, compartido por toda la app. Vive fuera de los
// componentes para que el progreso no se pierda al cambiar de repositorio.
import { listen } from "@tauri-apps/api/event";
import * as api from "$lib/api";
import type { BackupProgress, BackupResult } from "$lib/api";
import { formatBytes } from "$lib/format";
import { toast } from "$lib/toast.svelte";

type Status = Extract<BackupProgress, { kind: "status" }>;

export interface BackupRun {
  running: boolean;
  /** Nombre del destino, para el título de la ventana y los avisos. */
  name: string;
  /** Plan que se está copiando (id y nombre). */
  planId: string;
  planName: string;
  status: Status | null;
  /** Errores por archivo recibidos en vivo (limitados). */
  errors: string[];
  errorCount: number;
  result: BackupResult | null;
  failure: string | null;
  /** true si se detuvo a petición (no es un error). */
  cancelled: boolean;
  /** Momento en que terminó (ms), para compararlo con copias posteriores. */
  finishedAt: number | null;
}

const MAX_LIVE_ERRORS = 200;

export const runs = $state<Record<string, BackupRun>>({});

let listening: Promise<unknown> | null = null;

function ensureListening() {
  listening ??= listen<BackupProgress>(api.BACKUP_PROGRESS_EVENT, ({ payload }) => {
    const run = runs[payload.repo_id];
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

/** Copias que se pidió detener: al terminar no se avisa como un error. */
const cancelled = new Set<string>();

/**
 * Lanza la copia de un plan y resuelve cuando termina (bien, con error o
 * cancelada). `planName` es solo para mostrarlo («Copiando «Laboral»…»).
 */
export async function startBackup(repoId: string, name: string, planId: string, planName = "") {
  await ensureListening();
  cancelled.delete(repoId);
  runs[repoId] = { running: true, name, planId, planName, status: null, errors: [], errorCount: 0, result: null, failure: null, cancelled: false, finishedAt: null };
  const run = runs[repoId];
  const what = planName ? `«${planName}» (en «${name}»)` : `«${name}»`;
  try {
    run.result = await api.runBackup(repoId, planId);
    run.errors = run.result.errors;
    run.errorCount = run.result.error_count;
    const added = run.result.summary?.data_added;
    toast(
      run.result.unchanged
        ? `Copia de ${what}: no había cambios, no hizo falta guardar una versión nueva`
        : `Copia de ${what} ${run.result.incomplete ? "terminada con avisos" : "completada"}${added != null ? ` · +${formatBytes(added)}` : ""}`,
      run.result.incomplete ? "info" : "success",
    );
  } catch (e) {
    run.failure = String(e);
    run.cancelled = cancelled.has(repoId);
    if (cancelled.has(repoId)) toast(`Copia de ${what} detenida`, "info");
    else toast(`No se pudo completar la copia de ${what}`, "error", 6000);
  } finally {
    cancelled.delete(repoId);
    run.finishedAt = Date.now();
    run.running = false;
  }
}

export function cancelBackup(repoId: string) {
  cancelled.add(repoId);
  return api.cancelBackup(repoId).catch((e) => {
    cancelled.delete(repoId);
    throw e;
  });
}

export const isRunning = (repoId: string) => runs[repoId]?.running ?? false;
