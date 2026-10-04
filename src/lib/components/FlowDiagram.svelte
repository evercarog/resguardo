<script lang="ts">
  import { ChevronRight, CloudUpload, Folders, Plus } from "@lucide/svelte";
  import type { Repo } from "$lib/api";
  import { agent, offsiteSources, offsiteTargetName } from "$lib/agent.svelte";
  import { copyStatus, type CopyLevel } from "$lib/copies.svelte";
  import { formatDate, formatRelative } from "$lib/format";
  import { protections } from "$lib/protection.svelte";
  import { repoKind } from "$lib/repoKind";
  import { health, statusOf } from "$lib/status.svelte";

  // Esquema del destino: de dónde salen los archivos, dónde se guardan y a
  // dónde se suben fuera, con el estado y la hora de cada paso. Cada paso
  // lleva a su sección de la página.
  interface Props {
    repo: Repo;
    repos: Repo[];
    /** Hasta la fecha actual (refresca los «hace X»). */
    now: number;
    onsources: () => void;
    ondestination: () => void;
    oncloud: () => void;
  }
  let { repo, repos, now, onsources, ondestination, oncloud }: Props = $props();

  type Tone = "ok" | "warn" | "bad" | "muted" | "info";
  interface Step {
    title: string;
    sub: string;
    status: string;
    tone: Tone;
    when: string;
    whenTitle?: string;
  }

  const kind = $derived(repoKind(repo.location));
  const status = $derived(statusOf(repo, health[repo.id], now));
  const fromOthers = $derived(offsiteSources(repo.id));
  const agentRepo = $derived(agent.info?.repos.find((r) => r.id === repo.id) ?? null);

  // ---------- 1. Tus archivos ----------
  const WORST: CopyLevel[] = ["error", "warning", "late", "never", "paused", "running", "ok", "loading"];
  const LEVEL_TEXT: Record<CopyLevel, [string, Tone]> = {
    error: ["Falló la última copia", "bad"],
    warning: ["Última copia con avisos", "warn"],
    late: ["Con retraso", "warn"],
    never: ["Sin copias todavía", "info"],
    paused: ["En pausa", "info"],
    running: ["Copiando ahora", "info"],
    ok: ["Al día", "ok"],
    loading: ["Comprobando…", "muted"],
  };
  const sources = $derived.by((): Step => {
    const copies = repo.plans.map((p) => ({ plan: p, s: copyStatus(repo, p, now) }));
    if (!copies.length) {
      if (fromOthers.length)
        return {
          title: fromOthers.map((s) => `«${s.name}»`).join(", "),
          sub: "Sube aquí su copia externa",
          status: "Desde otro repositorio",
          tone: "info",
          when: "",
        };
      return { title: "Tus archivos", sub: "Ninguna copia se guarda aquí", status: "Crea una copia", tone: "muted", when: "" };
    }
    const folders = new Set(repo.plans.flatMap((p) => p.paths.map((x) => x.toLowerCase()))).size;
    const worst = copies.reduce((a, b) => (WORST.indexOf(b.s.level) < WORST.indexOf(a.s.level) ? b : a));
    const [text, tone] = LEVEL_TEXT[worst.s.level];
    const last = copies.reduce<Date | null>((a, c) => (c.s.last && (!a || c.s.last > a) ? c.s.last : a), null);
    return {
      title: "Tus archivos",
      sub: `${folders} ${folders === 1 ? "carpeta" : "carpetas"} · ${copies.length} ${copies.length === 1 ? "copia" : "copias"}`,
      status: copies.length > 1 && worst.s.level !== "ok" ? `${text} («${worst.plan.name}»)` : text,
      tone,
      when: last ? `última copia ${formatRelative(last.toISOString())}` : "",
      whenTitle: last ? formatDate(last.toISOString()) : undefined,
    };
  });

  // ---------- 2. Este destino ----------
  const destination = $derived.by((): Step => {
    const borrado = protections[repo.id]?.items.find((i) => i.id === "borrado");
    const isRest = repo.location.startsWith("rest:");
    let text = "Sin comprobar";
    let tone: Tone = "muted";
    if (borrado?.state === "ok") {
      text = isRest ? "Solo añadir: no se puede borrar" : kind.cloud ? "Con bloqueo de objetos" : "Protegido contra borrado";
      tone = "ok";
    } else if (borrado) {
      text = isRest
        ? borrado.state === "unknown"
          ? "Sin comprobar si es de solo añadir"
          : "Servidor sin «solo añadir»"
        : kind.cloud
          ? "Sin bloqueo de objetos"
          : "Disco: se podría borrar";
      tone = borrado.state === "unknown" ? "muted" : "warn";
    }
    if (status.level === "error") (text = "Sin conexión"), (tone = "bad");
    return {
      title: repo.name,
      sub: kind.label,
      status: text,
      tone,
      when: status.last ? `última versión ${formatRelative(status.last.time)}` : status.level === "empty" ? "sin versiones" : "",
      whenTitle: status.last ? formatDate(status.last.time) : undefined,
    };
  });

  // ---------- 3. Copia externa ----------
  const cloud = $derived.by((): Step | null => {
    const o = agentRepo?.offsite;
    if (!o) return null;
    const run = agent.info?.tasks?.runs[`offsite:${repo.id}`] ?? null;
    const held = !!agent.info?.offsite_holds?.[repo.id];
    const target = o.provider.startsWith("destino:") ? repos.find((r) => r.id === o.provider.slice("destino:".length)) : null;
    const name = offsiteTargetName(repo.id, repos) ?? "Copia externa";
    const lock = target ? (target.object_lock ? "Con bloqueo de objetos" : null) : null;
    let text: string;
    let tone: Tone;
    if (held) (text = "Subida frenada por un cambio inusual"), (tone = "warn");
    else if (run?.result === "error") (text = "Falló la última subida"), (tone = "bad");
    else if (run && status.last && run.finished >= status.last.time) (text = lock ?? "Al día"), (tone = "ok");
    else if (run) (text = lock ? `${lock} · pendiente de subir` : "Pendiente de subir lo último"), (tone = "info");
    else (text = "Aún sin subir"), (tone = "info");
    return {
      title: name,
      sub: target ? repoKind(target.location).label : "Fuera de este equipo",
      status: text,
      tone,
      when: run ? `${run.result === "error" ? "último intento" : "subida"} ${formatRelative(run.finished)}` : "",
      whenTitle: run ? formatDate(run.finished) : undefined,
    };
  });
  /** Este destino ya es la copia externa de otro: no se le pide otra. */
  const isOffsiteTarget = $derived(fromOthers.length > 0 && !repo.plans.length);
</script>

{#snippet node(step: Step, Icon: typeof Folders, onclick: () => void, label: string)}
  <button class="node tone-{step.tone}" {onclick} aria-label={label} title={label}>
    <span class="nicon"><Icon size={16} /></span>
    <span class="ntext">
      <strong>{step.title}</strong>
      <span class="sub">{step.sub}</span>
      <span class="st"><span class="dot"></span>{step.status}</span>
      {#if step.when}<span class="when" title={step.whenTitle}>{step.when}</span>{/if}
    </span>
  </button>
{/snippet}

<nav class="flow" aria-label="Recorrido de tus copias">
  {@render node(sources, Folders, onsources, "Ver las copias que se guardan aquí")}
  <span class="arrow" aria-hidden="true"><ChevronRight size={16} /></span>
  {@render node(destination, kind.icon, ondestination, "Ver la salud de la protección de este repositorio")}
  {#if !isOffsiteTarget}
    <span class="arrow" class:dashed={!cloud} aria-hidden="true"><ChevronRight size={16} /></span>
    {#if cloud}
      {@render node(cloud, CloudUpload, oncloud, "Ver la copia externa")}
    {:else}
      <button class="node empty" onclick={oncloud} aria-label="Configurar una copia externa">
        <span class="nicon"><Plus size={16} /></span>
        <span class="ntext">
          <strong>Copia externa</strong>
          <span class="sub">En la nube u otro disco</span>
          <span class="st"><span class="dot"></span>Sin configurar</span>
          <span class="when cta">Configurar</span>
        </span>
      </button>
    {/if}
  {/if}
</nav>

<style>
  .flow {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: stretch;
    gap: 6px;
  }
  .node {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    min-width: 0;
    padding: 12px 14px;
    font: inherit;
    text-align: left;
    color: var(--text-1);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    transition:
      border-color 0.15s,
      background 0.15s;
  }
  .node:hover {
    border-color: var(--border-strong);
    background: var(--surface-2);
  }
  .node:focus-visible {
    outline: none;
    box-shadow: var(--focus);
  }
  .node.empty {
    border-style: dashed;
    background: transparent;
  }
  .nicon {
    display: grid;
    place-items: center;
    flex: none;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    color: var(--text-2);
    background: var(--surface-2);
  }
  .tone-ok .nicon {
    color: var(--ok);
    background: var(--ok-soft);
  }
  .tone-warn .nicon {
    color: var(--warn);
    background: var(--warn-soft);
  }
  .tone-bad .nicon {
    color: var(--bad);
    background: var(--bad-soft);
  }
  .tone-info .nicon {
    color: var(--info);
    background: var(--info-soft);
  }
  .ntext {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    font-size: var(--fs-sm);
  }
  /* Nombre y tipo en una línea; el estado y la hora pueden ocupar dos. */
  .ntext > strong,
  .ntext > .sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  strong {
    font-weight: 600;
  }
  .sub,
  .when {
    color: var(--text-3);
  }
  .st {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin-top: 4px;
    color: var(--text-2);
  }
  .dot {
    flex: none;
    margin-top: 6px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--neutral);
  }
  .tone-ok .dot {
    background: var(--ok);
  }
  .tone-warn .dot {
    background: var(--warn);
  }
  .tone-bad .dot {
    background: var(--bad);
  }
  .tone-info .dot {
    background: var(--info);
  }
  .empty .dot {
    background: transparent;
    border: 1.5px solid var(--neutral);
  }
  .cta {
    color: var(--accent-text);
    font-weight: 550;
  }
  .arrow {
    display: grid;
    place-items: center;
    color: var(--text-3);
  }
  .arrow.dashed {
    opacity: 0.55;
  }
  /* Estrecho: un paso debajo de otro. */
  @media (max-width: 860px) {
    .flow {
      grid-template-columns: minmax(0, 1fr);
    }
    .arrow {
      transform: rotate(90deg);
      height: 16px;
    }
  }
</style>
