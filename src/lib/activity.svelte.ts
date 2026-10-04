// Historial de actividad (copias, verificaciones y copias externas), compartido
// por la vista «Actividad» y las tarjetas de Estado y de cada copia.
import {
  ArchiveRestore,
  CloudCheck,
  CircleAlert,
  CircleCheck,
  CirclePause,
  CirclePlay,
  CloudUpload,
  FolderSync,
  Info,
  KeyRound,
  Settings2,
  Share2,
  ShieldAlert,
  ShieldCheck,
  TriangleAlert,
} from "@lucide/svelte";
import * as api from "$lib/api";
import type { ActivityEntry, Repo } from "$lib/api";
import type { Selection } from "$lib/nav";

/** Cuántas entradas se piden (el backend admite hasta 5000). */
const LIMIT = 2000;

export const activity = $state<{ entries: ActivityEntry[]; loading: boolean; error: string; loadedAt: number }>({
  entries: [],
  loading: false,
  error: "",
  loadedAt: 0,
});

let pending: Promise<void> | null = null;

/** Vuelve a leer el historial (si ya se está leyendo, espera a esa lectura). */
export function refreshActivity(): Promise<void> {
  pending ??= (async () => {
    activity.loading = true;
    try {
      activity.entries = await api.activityHistory(LIMIT);
      activity.error = "";
      activity.loadedAt = Date.now();
    } catch (e) {
      activity.error = String(e);
    } finally {
      activity.loading = false;
      pending = null;
    }
  })();
  return pending;
}

/** Lee el historial ahora (si no es reciente) y cada `ms` mientras la ventana está visible. Devuelve la limpieza. */
export function pollActivity(ms = 30_000) {
  if (Date.now() - activity.loadedAt > 5_000) refreshActivity();
  const t = setInterval(() => document.visibilityState === "visible" && refreshActivity(), ms);
  const onVisible = () => document.visibilityState === "visible" && Date.now() - activity.loadedAt > ms && refreshActivity();
  document.addEventListener("visibilitychange", onVisible);
  return () => {
    clearInterval(t);
    document.removeEventListener("visibilitychange", onVisible);
  };
}

export const KIND_ICON = {
  backup: FolderSync,
  verify: ShieldCheck,
  offsite: CloudUpload,
  verify_offsite: CloudCheck,
  restore_test: ArchiveRestore,
  pause: CirclePause,
  resume: CirclePlay,
  guard: ShieldAlert,
  config: Settings2,
  kit: KeyRound,
  share: Share2,
};
/** "info": pausas y reanudaciones (no son un resultado). */
export const RESULT_ICON = { ok: CircleCheck, warning: TriangleAlert, error: CircleAlert, info: Info };
export const RESULT_LABEL = { ok: "Correcta", warning: "Con avisos", error: "Fallida", info: "Información" };
export const ORIGIN_LABEL: Record<string, string> = { agent: "Automática", retry: "Reintento", manual: "Manual", remote: "A distancia" };

/** «Copia «Laboral»», «Verificación», «Copia externa» o la pausa de las copias automáticas. */
export function kindLabel(e: ActivityEntry) {
  if (e.kind === "config") return e.plan_name ? `Cambio en «${e.plan_name}»` : "Cambio de configuración";
  if (e.kind === "kit") return "Kit de recuperación";
  if (e.kind === "share") return "Destino compartido";
  if (e.kind === "pause") return "Copias automáticas en pausa";
  if (e.kind === "resume") return "Copias automáticas reanudadas";
  if (e.kind === "guard") return e.result === "info" ? "Subida a la nube reanudada" : "Cambio inusual: subida frenada";
  if (e.kind === "verify") return "Verificación";
  if (e.kind === "verify_offsite") return "Verificación de la copia en la nube";
  if (e.kind === "restore_test") return "Prueba de restauración";
  if (e.kind === "offsite") return "Copia externa";
  return e.plan_name ? `Copia «${e.plan_name}»` : "Copia";
}

/** Segundos entre el inicio y el final (null si las fechas no valen o es una pausa). */
export function durationOf(e: ActivityEntry) {
  if (e.result === "info") return null; // una pausa no «dura» como una copia
  const s = (new Date(e.finished).getTime() - new Date(e.started).getTime()) / 1000;
  return Number.isFinite(s) && s >= 0 ? s : null;
}

/** Qué abrir al pulsar una entrada: la copia (si sigue existiendo) o su destino. */
export function entryTarget(e: ActivityEntry, repos: Repo[]): Selection | null {
  const repo = repos.find((r) => r.id === e.repo_id);
  if (!repo) return null;
  if (e.kind === "backup" && e.plan_id && repo.plans.some((p) => p.id === e.plan_id))
    return { kind: "copy", repoId: repo.id, planId: e.plan_id };
  return { kind: "destination", repoId: repo.id };
}

/** Texto sin mayúsculas ni tildes, para buscar. */
export const fold = (s: string) =>
  s
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase();

const pad = (n: number) => String(n).padStart(2, "0");
/** "AAAA-MM-DD HH:MM:SS" en hora local. */
function localStamp(iso: string) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

/** CSV (separado por «;», como lo espera Excel en español) con las entradas dadas. */
export function activityCsv(entries: ActivityEntry[]) {
  const cell = (v: unknown) => {
    let s = v == null ? "" : String(v);
    // Evita que Excel interprete un texto como fórmula (=, +, -, @).
    if (/^[=+\-@]/.test(s)) s = `'${s}`;
    return /[";\n\r]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
  };
  const head = [
    "Inicio",
    "Fin",
    "Duración (s)",
    "Tipo",
    "Origen",
    "Repositorio",
    "Copia",
    "Resultado",
    "Mensaje",
    "Versión",
    "Datos añadidos (bytes)",
    "Archivos nuevos",
    "Archivos cambiados",
    "Usuario",
  ];
  const TYPE = {
    backup: "Copia",
    verify: "Verificación",
    offsite: "Copia externa",
    verify_offsite: "Verificación de la copia en la nube",
    restore_test: "Prueba de restauración",
    pause: "Pausa",
    resume: "Reanudación",
    guard: "Cambio inusual",
    config: "Cambio de configuración",
    kit: "Kit de recuperación",
    share: "Destino compartido",
  };
  const rows = entries.map((e) => {
    const d = durationOf(e);
    return [
      localStamp(e.started),
      localStamp(e.finished),
      d == null ? "" : Math.round(d),
      TYPE[e.kind] ?? e.kind,
      ORIGIN_LABEL[e.origin] ?? e.origin,
      e.repo_name,
      e.plan_name ?? "",
      RESULT_LABEL[e.result] ?? e.result,
      e.message,
      e.snapshot_id ?? "",
      e.data_added ?? "",
      e.files_new ?? "",
      e.files_changed ?? "",
      e.user ?? "",
    ];
  });
  return [head, ...rows].map((r) => r.map(cell).join(";")).join("\r\n");
}

/** Descarga un texto como archivo (con BOM para que Excel reconozca UTF-8). */
export function downloadText(name: string, text: string, type = "text/csv;charset=utf-8") {
  const url = URL.createObjectURL(new Blob(["﻿", text], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
