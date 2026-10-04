// Buzón de avisos (la campana): lo que conviene saber aunque no estuvieras
// mirando. Sale de dos sitios:
// - el historial (copias que fallaron o con avisos, copias pedidas a
//   distancia, subidas o comprobaciones que fallaron, subidas frenadas por un
//   cambio inusual, reanudaciones solas…), como los avisos de la bandeja;
// - el estado de ahora (un destino sin conexión o atrasado, falta el kit),
//   que sigue en el buzón mientras dure.
// Lo leído y lo borrado se recuerda para cada usuario (almacenamiento local).
import type { ActivityEntry, Repo } from "$lib/api";
import type { Selection } from "$lib/nav";
import { activity, kindLabel } from "$lib/activity.svelte";
import { kitState } from "$lib/kit.svelte";
import { elapsedLabel, health, statusOf } from "$lib/status.svelte";

export type InboxTone = "bad" | "warn" | "ok" | "info";

export interface InboxItem {
  id: string;
  /** Cuándo pasó (RFC 3339). */
  time: string;
  tone: InboxTone;
  title: string;
  text: string;
  /** Lo que hace su botón. */
  action: { label: string; sel?: Selection; kit?: string[] | null };
  /** Sigue pasando ahora (no es un suceso del historial). */
  ongoing?: boolean;
}

const KEY = "resguardo:buzon";
/** Se muestran los sucesos de los últimos 30 días. */
const MAX_AGE = 30 * 86_400_000;

interface Saved {
  read: string[];
  /** Cuándo se vio por primera vez cada aviso de estado («desde cuándo»). */
  firstSeen: Record<string, string>;
  /** «Vaciar»: lo anterior a esta fecha ya no se muestra. */
  clearedBefore: string | null;
}

function load(): Saved {
  try {
    const s = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return {
      read: Array.isArray(s.read) ? s.read.filter((x: unknown) => typeof x === "string").slice(-600) : [],
      firstSeen: s.firstSeen && typeof s.firstSeen === "object" ? s.firstSeen : {},
      clearedBefore: typeof s.clearedBefore === "string" ? s.clearedBefore : null,
    };
  } catch {
    return { read: [], firstSeen: {}, clearedBefore: null };
  }
}

export const inbox = $state<Saved>(load());
export const inboxUi = $state({ open: false });

function save() {
  try {
    localStorage.setItem(KEY, JSON.stringify({ ...inbox, read: inbox.read.slice(-600) }));
  } catch {
    /* sin almacenamiento: se recuerda mientras la app siga abierta */
  }
}

export function markRead(ids: string[]) {
  const set = new Set(inbox.read);
  let changed = false;
  for (const id of ids)
    if (!set.has(id)) {
      inbox.read.push(id);
      changed = true;
    }
  if (changed) save();
}

/** «Vaciar»: deja de mostrar los sucesos hasta ahora (los avisos de estado siguen si siguen pasando). */
export function clearInbox(items: InboxItem[]) {
  inbox.clearedBefore = new Date().toISOString();
  markRead(items.map((i) => i.id));
  save();
}

/** Anota desde cuándo se ve cada aviso de estado (fuera de un $derived). */
export function noteSeen(items: InboxItem[]) {
  let changed = false;
  const live = new Set(items.filter((i) => i.ongoing).map((i) => i.id));
  for (const id of live)
    if (!inbox.firstSeen[id]) {
      inbox.firstSeen[id] = new Date().toISOString();
      changed = true;
    }
  // Lo que dejó de pasar se olvida: si vuelve, es un aviso nuevo.
  for (const id of Object.keys(inbox.firstSeen))
    if (!live.has(id)) {
      delete inbox.firstSeen[id];
      changed = true;
    }
  if (changed) save();
}

const lowerFirst = (t: string) => t.charAt(0).toLowerCase() + t.slice(1);
const entryId = (e: ActivityEntry) => `h|${e.kind}|${e.repo_id}|${e.plan_id ?? ""}|${e.finished}`;

function fromEntry(e: ActivityEntry, repos: Repo[]): InboxItem | null {
  const repo = repos.find((r) => r.id === e.repo_id);
  const planExists = !!repo && !!e.plan_id && repo.plans.some((p) => p.id === e.plan_id);
  const toCopy: InboxItem["action"] = planExists
    ? { label: "Abrir la copia", sel: { kind: "copy", repoId: e.repo_id, planId: e.plan_id! } }
    : { label: "Abrir el repositorio", sel: { kind: "destination", repoId: e.repo_id } };
  const toDest: InboxItem["action"] = { label: "Abrir el repositorio", sel: { kind: "destination", repoId: e.repo_id } };
  const base = { id: entryId(e), time: e.finished };
  const where = `«${e.repo_name}»`;
  switch (e.kind) {
    case "backup":
      if (e.result === "error") return { ...base, tone: "bad", title: `Falló la ${lowerFirst(kindLabel(e))}`, text: `En ${where}. ${e.message}`, action: toCopy };
      if (e.result === "warning")
        return { ...base, tone: "warn", title: `${kindLabel(e)}: terminó con avisos`, text: `En ${where}. Algunos archivos no se pudieron leer.`, action: toCopy };
      if (e.origin === "remote")
        return { ...base, tone: "ok", title: `${kindLabel(e)}: hecha a distancia`, text: `Pedida desde ${e.requested_from ?? "la web"}.`, action: toCopy };
      return null;
    case "offsite":
    case "verify":
    case "verify_offsite":
    case "restore_test":
      if (e.result === "error") return { ...base, tone: "bad", title: `${kindLabel(e)}: falló`, text: `En ${where}. ${e.message}`, action: toDest };
      if (e.result === "warning") return { ...base, tone: "warn", title: `${kindLabel(e)}: con avisos`, text: `En ${where}. ${e.message}`, action: toDest };
      return null;
    case "guard":
      return {
        ...base,
        tone: e.result === "info" ? "info" : "warn",
        title: kindLabel(e),
        text: e.result === "info" ? `En ${where}.` : `En ${where}: revisa el cambio antes de subirlo. ${e.message}`,
        action: toDest,
      };
    case "share":
      return { ...base, tone: "info", title: "Destino compartido", text: e.message, action: { label: "Ver la actividad", sel: { kind: "activity" } } };
    case "resume":
      return e.origin === "agent" ? { ...base, tone: "info", title: "Copias automáticas reanudadas", text: `En ${where}, al terminar la pausa.`, action: toDest } : null;
    default:
      return null;
  }
}

/** Avisos de ahora, en el orden en que se muestran (lo más reciente primero). */
export function inboxItems(repos: Repo[], now = Date.now()): InboxItem[] {
  const out: InboxItem[] = [];
  const since = (id: string) => inbox.firstSeen[id] ?? new Date(now).toISOString();

  // Estado de ahora.
  for (const r of repos) {
    const s = statusOf(r, health[r.id], now);
    if (s.level === "error") {
      const id = `s|error|${r.id}`;
      out.push({
        id,
        time: since(id),
        tone: "bad",
        title: `No se puede conectar con «${r.name}»`,
        text: "Revisa que el disco esté conectado o que el servidor esté encendido.",
        action: { label: "Abrir el repositorio", sel: { kind: "destination", repoId: r.id } },
        ongoing: true,
      });
    } else if (s.level === "overdue") {
      const id = `s|overdue|${r.id}`;
      out.push({
        id,
        time: since(id),
        tone: "warn",
        title: `«${r.name}» lleva ${elapsedLabel(s.since ?? 0)} sin copias`,
        text: "Mira su historia para saber por qué.",
        action: { label: "Abrir el repositorio", sel: { kind: "destination", repoId: r.id } },
        ongoing: true,
      });
    }
  }
  const noKit = repos.filter((r) => kitState(r) !== "ok");
  if (noKit.length) {
    const id = `s|kit|${noKit.map((r) => r.id).sort().join(",")}`;
    out.push({
      id,
      time: since(id),
      tone: "warn",
      title: noKit.length === 1 ? `Falta el kit de recuperación de «${noKit[0].name}»` : `Falta el kit de recuperación de ${noKit.length} repositorios`,
      text: "Sin él, si pierdes este equipo no podrás abrir las copias.",
      action: { label: "Preparar el kit", kit: noKit.map((r) => r.id) },
      ongoing: true,
    });
  }

  // Sucesos del historial.
  const cleared = inbox.clearedBefore ?? "";
  for (const e of activity.entries) {
    if (now - Date.parse(e.finished) > MAX_AGE) break; // el historial va de lo más reciente a lo más antiguo
    if (e.finished <= cleared) continue;
    const item = fromEntry(e, repos);
    if (item) out.push(item);
  }
  return out.sort((a, b) => b.time.localeCompare(a.time));
}
