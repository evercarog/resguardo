// «Buscar un archivo»: búsquedas por destino, compartidas por la app. Los
// resultados se quedan aquí al cerrar el diálogo (y la búsqueda sigue).
import { listen } from "@tauri-apps/api/event";
import * as api from "$lib/api";
import type { SearchMatch, SearchOutcome, SearchProgress, SearchRequest, Snapshot } from "$lib/api";
import { parentDir } from "$lib/paths";

export interface SearchRun {
  running: boolean;
  request: SearchRequest;
  /** Texto del alcance («todas las versiones», «las 10 más recientes»…). */
  scopeLabel: string;
  /** Versiones en las que se busca, de la más reciente a la más antigua. */
  scope: Snapshot[];
  /** Coincidencias por ID completo de versión. */
  hits: Record<string, SearchMatch[]>;
  outcome: SearchOutcome | null;
  failure: string | null;
  started: number;
}

export const searches = $state<Record<string, SearchRun>>({});

/** Destino cuya búsqueda hay que abrir (desde Ctrl+K). */
export const searchUi = $state<{ open: string | null }>({ open: null });

/** Abrir una versión de un destino en una carpeta (desde «Ver versiones»). */
export const browseUi = $state<{ pending: { repoId: string; snapshotId: string; dir: string } | null }>({ pending: null });

let listening: Promise<unknown> | null = null;
function ensureListening() {
  listening ??= listen<SearchProgress>(api.SEARCH_PROGRESS_EVENT, ({ payload }) => {
    const run = searches[payload.repo_id];
    if (!run?.running) return;
    run.hits[payload.snapshot] = payload.matches;
  });
  return listening;
}

const cancelled = new Set<string>();

export async function startSearch(repoId: string, request: SearchRequest, scope: Snapshot[], scopeLabel: string) {
  await ensureListening();
  cancelled.delete(repoId);
  searches[repoId] = { running: true, request, scope, scopeLabel, hits: {}, outcome: null, failure: null, started: Date.now() };
  const run = searches[repoId];
  try {
    run.outcome = await api.searchFiles(repoId, request);
  } catch (e) {
    run.failure = cancelled.has(repoId) ? null : String(e);
    if (cancelled.has(repoId)) run.outcome = null;
  } finally {
    run.running = false;
  }
}

export function cancelSearch(repoId: string) {
  cancelled.add(repoId);
  return api.cancelSearch(repoId);
}

export function clearSearch(repoId: string) {
  if (!searches[repoId]?.running) delete searches[repoId];
}

/** Una variante del archivo: las versiones seguidas en las que tenía el mismo tamaño y fecha. */
export interface Variant {
  size: number | null;
  mtime: string | null;
  /** Versiones con esta variante, de la más reciente a la más antigua. */
  snapshots: Snapshot[];
}

export interface FileGroup {
  path: string;
  name: string;
  /** Carpeta dentro de la versión. */
  dir: string;
  isDir: boolean;
  /** Versiones en las que está, de la más reciente a la más antigua. */
  count: number;
  first: Snapshot;
  last: Snapshot;
  /** No está en la versión más reciente buscada: se borró o se movió después de `last`. */
  gone: boolean;
  variants: Variant[];
}

/**
 * Agrupa las coincidencias por ruta. `scope` va de la más reciente a la más
 * antigua; las versiones sin datos (aún no recorridas) no cuentan para `gone`.
 */
export function groupResults(run: SearchRun): FileGroup[] {
  const byPath = new Map<string, { match: SearchMatch; snap: Snapshot }[]>();
  for (const snap of run.scope) {
    for (const m of run.hits[snap.id] ?? []) {
      let list = byPath.get(m.path);
      if (!list) byPath.set(m.path, (list = []));
      list.push({ match: m, snap });
    }
  }
  const newest = run.scope[0];
  const groups: FileGroup[] = [];
  for (const [path, list] of byPath) {
    const variants: Variant[] = [];
    for (const { match, snap } of list) {
      const prev = variants.at(-1);
      const size = match.size ?? null;
      const mtime = match.mtime ?? null;
      if (prev && prev.size === size && prev.mtime === mtime) prev.snapshots.push(snap);
      else variants.push({ size, mtime, snapshots: [snap] });
    }
    groups.push({
      path,
      name: path.slice(path.lastIndexOf("/") + 1),
      dir: parentDir(path),
      isDir: list[0].match.type === "dir",
      count: list.length,
      last: list[0].snap,
      first: list[list.length - 1].snap,
      gone: !!newest && list[0].snap.id !== newest.id && !run.running,
      variants,
    });
  }
  // Primero lo más reciente; luego por nombre.
  return groups.sort((a, b) => b.last.time.localeCompare(a.last.time) || a.path.localeCompare(b.path));
}
