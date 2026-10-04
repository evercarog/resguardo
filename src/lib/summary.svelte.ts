// Resumen en una o dos frases, arriba de cada destino y de cada copia: cómo
// está, lo último que pasó y, si falta algo, solo lo más importante. Sale de
// los mismos datos que la salud de la protección y el estado (nada nuevo que
// mantener a mano).
import type { AgentRun, Plan, Repo } from "$lib/api";
import { agent, offsiteSources, offsiteTargetName } from "$lib/agent.svelte";
import { copyStatus } from "$lib/copies.svelte";
import { formatRelative } from "$lib/format";
import { untilWords, whenInWords } from "$lib/pause.svelte";
import { FIX_PHRASE, pendingItems, protections, protectionTone } from "$lib/protection.svelte";
import { elapsedLabel, frequencyLabel, health, statusOf } from "$lib/status.svelte";

export type SummaryTone = "ok" | "warn" | "bad" | "info" | "muted";

export interface Summary {
  tone: SummaryTone;
  /** Primera frase: cómo está. */
  head: string;
  /** Lo último que pasó (puede ir vacío). */
  facts: string;
  /** Lo más importante que falta (puede ir vacío). */
  missing: string;
}

const DAY = 86_400_000;
const WEEKDAY = new Intl.DateTimeFormat("es", { weekday: "long" });
const SHORT_DATE = new Intl.DateTimeFormat("es", { day: "numeric", month: "short" });

/** «hoy», «ayer», «el domingo» (esta semana) o «el 3 sept». */
export function dayPhrase(iso: string, now = Date.now()) {
  const d = new Date(iso);
  const start = (t: Date) => new Date(t.getFullYear(), t.getMonth(), t.getDate()).getTime();
  const days = Math.round((start(new Date(now)) - start(d)) / DAY);
  if (days <= 0) return "hoy";
  if (days === 1) return "ayer";
  if (days < 7) return `el ${WEEKDAY.format(d)}`;
  return `el ${SHORT_DATE.format(d).replace(".", "")}`;
}

/** «a, b y c». */
export function joinAnd(parts: string[]) {
  const list = parts.filter(Boolean);
  if (list.length <= 1) return list[0] ?? "";
  return `${list.slice(0, -1).join(", ")} y ${list.at(-1)}`;
}

const capital = (s: string) => (s ? s.charAt(0).toUpperCase() + s.slice(1) : s);
const sentence = (parts: string[]) => {
  const s = joinAnd(parts);
  return s ? `${capital(s)}.` : "";
};

function taskRun(key: string): AgentRun | null {
  return agent.info?.tasks?.runs[key] ?? null;
}

/**
 * Lo más importante que falta en un destino: algo configurado que falla
 * («review»: se dice qué pasa) o algo sin configurar («fix»: qué falta).
 */
function missingOf(repo: Repo): { kind: "review" | "fix"; text: string } | null {
  const first = pendingItems(protections[repo.id])[0];
  if (!first) return null;
  if (first.state === "bad" && !/^(sin|todas)/i.test(first.detail))
    return { kind: "review", text: `${first.label.toLowerCase()}: ${first.detail.replace(/\.$/, "")}` };
  return { kind: "fix", text: FIX_PHRASE[first.id] };
}

function missingFor(repo: Repo) {
  const m = missingOf(repo);
  if (!m) return "";
  return m.kind === "review" ? `Revisa ${m.text}.` : `Falta: ${m.text}.`;
}

/** Resumen de un destino. */
export function destinationSummary(repo: Repo, repos: Repo[], now = Date.now()): Summary {
  const status = statusOf(repo, health[repo.id], now);
  const protection = protections[repo.id];
  const tone = protectionTone(protection);
  const agentRepo = agent.info?.repos.find((r) => r.id === repo.id) ?? null;
  const name = `«${repo.name}»`;

  if (status.level === "loading" || (!protection && status.level !== "error")) return { tone: "muted", head: `Comprobando ${name}…`, facts: "", missing: "" };
  if (status.level === "error")
    return {
      tone: "bad",
      head: `No se puede conectar con ${name}.`,
      facts: "Revisa que el disco esté conectado o que el servidor esté encendido y accesible.",
      missing: "",
    };
  if (status.level === "empty")
    return {
      tone: "info",
      head: `${name} aún no tiene versiones.`,
      facts: repo.plans.length ? "Pulsa «Copiar ahora» en una de sus copias para guardar la primera." : "Crea una copia que se guarde aquí para empezar.",
      missing: "",
    };

  let head: string;
  let headTone: SummaryTone = tone === "muted" ? "muted" : tone;
  if (status.level === "paused" && agentRepo?.pause) {
    head = `Las copias automáticas de ${name} están en pausa ${untilWords(agentRepo.pause, new Date(now))}.`;
    headTone = "info";
  } else if (status.level === "overdue" || status.level === "late") {
    head = `Las copias de ${name} van con retraso: la última fue hace ${elapsedLabel(status.since ?? 0)}.`;
    headTone = status.level === "overdue" ? "bad" : "warn";
  } else if (tone === "ok") head = `${name} está protegido.`;
  else if (tone === "bad") head = `${name} necesita atención.`;
  else head = `${name} está protegido, aunque se puede mejorar.`;

  const facts: string[] = [];
  if (status.last && status.level !== "late" && status.level !== "overdue") facts.push(`última copia ${formatRelative(status.last.time)}`);
  if (status.level === "late" || status.level === "overdue")
    facts.push(`se espera una ${frequencyLabel(status.expected)}${status.auto ? " (según su historial)" : ""}`);
  // Copia externa: va al día si la última subida es posterior a la última versión.
  if (agentRepo?.offsite) {
    const run = taskRun(`offsite:${repo.id}`);
    const where = offsiteTargetName(repo.id, repos);
    const held = !!agent.info?.offsite_holds?.[repo.id];
    if (held) facts.push("la subida a la nube está frenada por un cambio inusual");
    else if (run?.result === "error") facts.push(`la última subida${where ? ` a «${where}»` : ""} falló`);
    else if (run && status.last && run.finished >= status.last.time) facts.push(`la copia externa${where ? ` en «${where}»` : ""} va al día`);
    else if (run) facts.push(`la copia externa se subió ${formatRelative(run.finished)}`);
    else facts.push("la copia externa aún no se ha subido");
  } else {
    const from = offsiteSources(repo.id);
    if (from.length) facts.push(`recibe la copia externa de ${joinAnd(from.map((s) => `«${s.name}»`))}`);
  }
  const verify = taskRun(`verify:${repo.id}`);
  if (verify?.result === "error") facts.push("la última verificación falló");
  else if (verify) facts.push(`se verificó ${dayPhrase(verify.finished, now)}`);

  return { tone: headTone, head, facts: sentence(facts), missing: missingFor(repo) };
}

/** Resumen de una copia (y de lo más importante que le falta a su destino). */
export function copySummary(repo: Repo, plan: Plan, now = Date.now()): Summary {
  const s = copyStatus(repo, plan, now);
  const name = `«${plan.name}»`;
  let tone: SummaryTone = "ok";
  let head: string;
  switch (s.level) {
    case "loading":
      return { tone: "muted", head: `Comprobando ${name}…`, facts: "", missing: "" };
    case "running":
      tone = "info";
      head = `${name} se está copiando ahora.`;
      break;
    case "error":
      tone = "bad";
      head = `La última copia de ${name} falló.`;
      break;
    case "warning":
      tone = "warn";
      head = `La última copia de ${name} terminó con avisos: algunos archivos no se pudieron leer.`;
      break;
    case "late":
      tone = "warn";
      head = `${name} va con retraso.`;
      break;
    case "paused":
      tone = "info";
      head = `${name} está en pausa con las copias automáticas de «${repo.name}».`;
      break;
    case "never":
      tone = "info";
      head = `${name} aún no se ha copiado nunca.`;
      break;
    default:
      head = `${name} está al día.`;
  }

  const facts: string[] = [];
  if (s.last && s.level !== "running") facts.push(`última copia ${formatRelative(s.last.toISOString())}${s.unchanged ? " (sin cambios)" : ""}`);
  if (s.level !== "running" && s.level !== "paused") {
    if ((s.auto === "scheduled" || s.auto === "stale") && s.next) facts.push(`la próxima, ${whenInWords(s.next, new Date(now))}`);
    else if (s.auto === "manual") facts.push("se copia solo cuando pulsas «Copiar ahora»");
    else if (s.auto === "unscheduled") facts.push("tiene horario, pero aún no está programada");
  }
  if (s.level === "never" && !s.last) facts.push("pulsa «Copiar ahora» para guardar la primera versión");

  // Del destino, solo lo más importante que falta.
  const m = missingOf(repo);
  const missing = !m ? "" : m.kind === "review" ? `En «${repo.name}», revisa ${m.text}.` : `A su repositorio, «${repo.name}», le falta ${m.text}.`;
  return { tone, head, facts: sentence(facts), missing };
}
