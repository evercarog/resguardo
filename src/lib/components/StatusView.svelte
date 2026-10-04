<script lang="ts">
  // «Estado» (inicio): un resumen grande primero —¿está todo protegido? y, si
  // no, qué hay que hacer, por orden de urgencia— y debajo cada destino.
  import { fade } from "svelte/transition";
  import { dur } from "$lib/motion";
  import {
    ArrowRight,
    CircleAlert,
    CircleCheck,
    CircleDashed,
    CirclePause,
    Clock,
    Cloud,
    CloudOff,
    Feather,
    KeyRound,
    LoaderCircle,
    Plus,
    RefreshCw,
    ShieldAlert,
    ShieldCheck,
    TriangleAlert,
    WifiOff,
  } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Repo } from "$lib/api";
  import { formatBytes, formatDate, formatRelative, formatShortDay } from "$lib/format";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { repoKind } from "$lib/repoKind";
  import WebLinkCard from "./WebLinkCard.svelte";
  import TaskProgress from "./TaskProgress.svelte";
  import { dismissKitReminder, kitReminder, kitState, openKit } from "$lib/kit.svelte";
  import { agent, liveTask, offsiteSources, offsiteTargetName } from "$lib/agent.svelte";
  import RecentActivity from "./RecentActivity.svelte";
  import type { Selection } from "$lib/nav";
  import { COPY_LEVEL_LABEL, copyStatus } from "$lib/copies.svelte";
  import { runs } from "$lib/backups.svelte";
  import { pauseOf, untilWords } from "$lib/pause.svelte";
  import { LEVEL_ORDER, elapsedLabel, frequencyLabel, health, refreshAll, statusOf, type Level } from "$lib/status.svelte";
  import RelTime from "./RelTime.svelte";
  import Num from "./Num.svelte";
  import InfoTip from "./InfoTip.svelte";
  import { protections, refreshProtection } from "$lib/protection.svelte";
  import OffsiteHoldBanner from "./OffsiteHoldBanner.svelte";
  import Onboarding from "./Onboarding.svelte";
  import { discreetActive, discreetSummary, uploadLabel } from "$lib/discreet";

  interface Props {
    repos: Repo[];
    /** Abrir un destino. */
    onopen: (repo: Repo) => void;
    onopencopy: (repoId: string, planId: string) => void;
    /** Nueva copia (en un destino concreto, si se da). */
    onnewcopy: (repoId?: string) => void;
    onchange: (repo: Repo) => void;
    onadddestination: () => void;
    /** Abrir el historial de actividad. */
    onopenactivity: () => void;
  }
  let { repos, onopen, onopencopy, onnewcopy, onchange, onadddestination, onopenactivity }: Props = $props();

  /** Abrir la copia o el destino de una entrada de la actividad reciente. */
  function navigate(sel: Selection) {
    if (sel.kind === "copy") onopencopy(sel.repoId, sel.planId);
    else if (sel.kind === "destination") {
      const repo = repos.find((r) => r.id === sel.repoId);
      if (repo) onopen(repo);
    }
  }

  // "Ahora" se actualiza cada minuto para que los retrasos avancen solos.
  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 60_000);
    return () => clearInterval(t);
  });

  const rows = $derived(
    repos
      .map((repo) => ({ repo, status: statusOf(repo, health[repo.id], now), h: health[repo.id] }))
      .sort((a, b) => LEVEL_ORDER[a.status.level] - LEVEL_ORDER[b.status.level] || a.repo.name.localeCompare(b.repo.name)),
  );
  const anyLoading = $derived(rows.some((r) => r.h?.loading));
  const firstLoad = $derived(rows.length > 0 && rows.every((r) => r.status.level === "loading"));
  const lastRefresh = $derived(Math.max(0, ...rows.map((r) => r.h?.loadedAt ?? 0)));
  /** Destinos sin kit de recuperación (o con uno que ya no vale). */
  const withoutKit = $derived(repos.filter((r) => kitState(r) !== "ok"));

  // Salud de la protección de cada destino (para el resumen y las tarjetas).
  $effect(() => {
    void agent.info;
    for (const r of repos) void refreshProtection(r);
  });

  // ---------- Resumen ----------

  /** Un punto de la lista del resumen: qué pasa y qué hacer. */
  interface Issue {
    key: string;
    tone: "bad" | "warn" | "paused" | "info";
    /** Lo urgente cuenta para el titular; las mejoras recomendadas, no. */
    urgent: boolean;
    icon: typeof CircleAlert;
    title: string;
    detail?: string;
    action?: { label: string; run: () => void };
    secondary?: { label: string; run: () => void };
    /** Subida frenada por un cambio inusual: se muestra el aviso completo, en rojo. */
    hold?: Repo;
  }

  const copying = $derived(
    repos.some((r) => runs[r.id]?.running) || !!agent.info?.state.running || repos.some((r) => liveTask(r.id)),
  );

  const issues = $derived.by<Issue[]>(() => {
    const out: Issue[] = [];
    const open = (r: Repo) => () => onopen(r);
    // 1. Subida a la nube frenada por un cambio inusual.
    for (const r of repos) {
      if (agent.info?.offsite_holds?.[r.id]) {
        out.push({ key: `hold-${r.id}`, hold: r, tone: "bad", urgent: true, icon: ShieldAlert, title: `Subida a la nube frenada en «${r.name}»`, detail: "Una copia cambió mucho más de lo normal. Revisa qué cambió antes de reanudarla.", action: { label: "Revisar", run: open(r) } });
      }
    }
    // 2. Destinos sin conexión, atrasados o con retraso.
    for (const { repo: r, status, h } of rows) {
      if (status.level === "error") {
        out.push({ key: `err-${r.id}`, tone: "bad", urgent: true, icon: WifiOff, title: `«${r.name}» no responde`, detail: h?.error?.split("\n")[0], action: { label: "Abrir", run: open(r) } });
      } else if (status.level === "overdue") {
        out.push({ key: `late-${r.id}`, tone: "bad", urgent: true, icon: TriangleAlert, title: `«${r.name}» lleva ${elapsedLabel(status.since ?? 0)} sin copias`, detail: `Se espera una copia ${frequencyLabel(status.expected)}.`, action: { label: "Abrir", run: open(r) } });
      }
    }
    // 3. Copias que fallaron.
    for (const r of repos) {
      for (const p of r.plans ?? []) {
        const cs = copyStatus(r, p, now);
        if (cs.level === "error") {
          out.push({ key: `copy-${r.id}-${p.id}`, tone: "bad", urgent: true, icon: CircleAlert, title: `Falló la copia «${p.name}»`, detail: cs.lastRun?.message || `Se guarda en «${r.name}».`, action: { label: "Ver", run: () => onopencopy(r.id, p.id) } });
        }
      }
    }
    // 4. Retrasos leves.
    for (const { repo: r, status } of rows) {
      if (status.level === "late") {
        out.push({ key: `slow-${r.id}`, tone: "warn", urgent: true, icon: Clock, title: `«${r.name}» va con retraso`, detail: `Última copia hace ${elapsedLabel(status.since ?? 0)}; se espera ${frequencyLabel(status.expected)}.`, action: { label: "Abrir", run: open(r) } });
      }
    }
    // 5. Kit de recuperación.
    if (withoutKit.length && !kitReminder.dismissed) {
      out.push({
        key: "kit",
        tone: "info",
        urgent: false,
        icon: KeyRound,
        title: withoutKit.length === 1 ? `Guarda el kit de recuperación de «${withoutKit[0].name}»` : `Guarda el kit de recuperación de ${withoutKit.length} repositorios`,
        detail: "Sin él, si pierdes este equipo no podrás abrir las copias.",
        action: { label: "Preparar el kit", run: () => openKit(withoutKit.map((r) => r.id)) },
        secondary: { label: "Ahora no", run: dismissKitReminder },
      });
    }
    // 6. Huecos de protección (sin contar el kit, que va aparte).
    const gaps = repos
      .map((r) => ({ r, bad: (protections[r.id]?.items ?? []).filter((i) => i.state === "bad" && i.id !== "kit") }))
      .filter((g) => g.bad.length);
    if (gaps.length) {
      const labels = [...new Set(gaps.flatMap((g) => g.bad.map((i) => i.label.toLowerCase())))];
      const worst = gaps.sort((a, b) => b.bad.length - a.bad.length)[0].r;
      out.push({
        key: "gaps",
        tone: "warn",
        urgent: false,
        icon: ShieldCheck,
        title: gaps.length === 1 ? `Refuerza la protección de «${gaps[0].r.name}»` : `Refuerza la protección de ${gaps.length} repositorios`,
        detail: `Falta: ${labels.slice(0, 3).join(", ")}${labels.length > 3 ? "…" : ""}.`,
        action: { label: "Revisar", run: open(worst) },
      });
    }
    // 7. En pausa (a propósito: informativo).
    for (const { repo: r, status } of rows) {
      if (status.level === "paused") {
        const p = pauseOf(r.id, now);
        out.push({ key: `pause-${r.id}`, tone: "paused", urgent: false, icon: CirclePause, title: `Copias automáticas de «${r.name}» en pausa`, detail: p ? `Se reanudarán ${untilWords(p, new Date(now))}.` : undefined, action: { label: "Abrir", run: open(r) } });
      }
    }
    return out;
  });

  const urgent = $derived(issues.filter((i) => i.urgent));
  /** «Modo discreto» en este momento (el agente usa el mismo horario). */
  const discreetNow = $derived(discreetActive(agent.info?.discreet, new Date(now)));
  const worstTone = $derived(urgent.some((i) => i.tone === "bad") ? "bad" : urgent.length ? "warn" : copying ? "info" : "ok");

  const headline = $derived(
    firstLoad
      ? "Comprobando tus copias…"
      : urgent.length
        ? `${urgent.length} ${urgent.length === 1 ? "cosa necesita" : "cosas necesitan"} atención`
        : copying
          ? "Copiando…"
          : issues.some((i) => i.key === "gaps")
            ? "Copias al día"
            : "Todo protegido",
  );

  /** «Última copia hace 12 min · 4 destinos · 2,1 TB protegidos». */
  const lastCopy = $derived(rows.reduce<string | null>((a, r) => (r.status.last && (!a || r.status.last.time > a) ? r.status.last.time : a), null));
  const protectedBytes = $derived(rows.reduce((n, r) => n + (r.status.last?.summary?.total_bytes_processed ?? 0), 0));

  // ---------- Tarjetas ----------

  const LEVEL_ICON = { ok: CircleCheck, late: Clock, overdue: TriangleAlert, empty: CircleDashed, error: WifiOff, loading: LoaderCircle, paused: CirclePause };
  const LEVEL_TONE: Record<Level, string> = { ok: "ok", late: "warn", overdue: "bad", empty: "neutral", error: "bad", loading: "neutral", paused: "paused" };

  /** Últimos 14 días: número de copias por día (hora local), del más antiguo a hoy. */
  function lastDays(repo: Repo) {
    const snaps = health[repo.id]?.snapshots ?? [];
    const today = new Date(now);
    return Array.from({ length: 14 }, (_, i) => {
      const d = new Date(today.getFullYear(), today.getMonth(), today.getDate() - (13 - i));
      const next = new Date(d.getFullYear(), d.getMonth(), d.getDate() + 1);
      const n = snaps.filter((s) => {
        const t = new Date(s.time);
        return t >= d && t < next;
      }).length;
      return { date: d, n };
    });
  }

  /** Copia externa (nube) de un destino: estado de la última subida. */
  function cloudOf(repo: Repo) {
    const o = agent.info?.repos.find((r) => r.id === repo.id)?.offsite;
    if (!o) return null;
    const run = agent.info?.tasks?.runs[`offsite:${repo.id}`] ?? null;
    return { target: offsiteTargetName(repo.id, repos) ?? "la nube", run, held: !!agent.info?.offsite_holds?.[repo.id] };
  }

  const FREQUENCIES = [1, 6, 12, 24, 48, 168];

  async function setFrequency(repo: Repo, value: string, select: HTMLSelectElement) {
    const hours = value === "auto" ? null : Number(value);
    const done = await withPassword({
      title: "Cambiar la frecuencia esperada",
      message: `Resguardo avisará si «${repo.name}» pasa ${hours ? frequencyLabel(hours).replace("cada", "más de") : "más de lo habitual"} sin copias.`,
      repoName: repo.name,
      confirmLabel: "Guardar",
      action: async (password) => onchange(await api.setExpectedInterval(repo.id, hours, password)),
    });
    if (!done) select.value = repo.expected_hours == null ? "auto" : String(repo.expected_hours);
  }

  const C = 2 * Math.PI * 7;
</script>

<div class="page" in:fade={{ duration: dur(160) }}>
  <Onboarding variant="card" {repos} onadd={onadddestination} onnewcopy={() => onnewcopy()} {onopen} />

  <!-- Resumen: el titular dice si hace falta hacer algo; debajo, qué y por orden. -->
  <section class="hero tone-{worstTone}" aria-labelledby="hero-title">
    <div class="hero-top">
      <span class="hero-mark" aria-hidden="true">
        {#if worstTone === "bad"}<CircleAlert size={22} />{:else if worstTone === "warn"}<TriangleAlert size={22} />{:else if worstTone === "info"}<span class="spin" style="display:grid"><LoaderCircle size={22} /></span>{:else}<ShieldCheck size={22} />{/if}
      </span>
      <div class="hero-text">
        <h1 id="hero-title">{headline}</h1>
        <p>
          {#if lastCopy}Última copia <RelTime iso={lastCopy} /> ·{/if}
          {repos.length} {repos.length === 1 ? "repositorio" : "repositorios"}
          {#if protectedBytes}· <Num value={protectedBytes} format={(n) => formatBytes(n)} /> protegidos <InfoTip id="total-protegido" />{/if}
          {#if copying && urgent.length}<span class="copying"> · copiando…</span>{/if}
          {#if lastRefresh}<span class="faint"> · comprobado {formatRelative(new Date(lastRefresh).toISOString())}</span>{/if}
        </p>
        {#if discreetNow && agent.info?.discreet}
          {@const d = agent.info.discreet}
          <p class="discreet" title="Modo discreto: {discreetSummary(d)}. Se cambia en Ajustes.">
            <Feather size={13} /> Modo discreto ahora: las copias van con prioridad baja{d.upload_kib ? ` y la subida, limitada a ${uploadLabel(d.upload_kib)}` : ""}.
          </p>
        {/if}
      </div>
      <button class="btn btn-ghost btn-sm" onclick={() => refreshAll(repos)} disabled={anyLoading} title="Volver a consultar todos los repositorios">
        <span class:spin={anyLoading} style="display:grid"><RefreshCw size={14} /></span> Comprobar
      </button>
    </div>

    {#if issues.length}
      <ul class="issues">
        {#each issues as it (it.key)}
          {#if it.hold}
            <!-- Posible ransomware: el aviso completo, con sus acciones, no solo un enlace. -->
            {@const r = it.hold}
            <li class="issue-hold"><OffsiteHoldBanner repo={r} onopen={() => onopen(r)} /></li>
          {:else}
          <li class="issue tone-{it.tone}" in:fade={{ duration: dur(140) }}>
            <span class="issue-ic"><it.icon size={16} /></span>
            <span class="issue-text">
              <strong>{it.title}</strong>
              {#if it.detail}<span>{it.detail}</span>{/if}
            </span>
            <span class="issue-actions">
              {#if it.secondary}<button class="btn btn-ghost btn-sm" onclick={it.secondary.run}>{it.secondary.label}</button>{/if}
              {#if it.action}<button class="btn btn-sm" class:btn-primary={it.key === issues.find((x) => !x.hold)?.key && it.urgent} onclick={it.action.run}>{it.action.label} <ArrowRight size={13} /></button>{/if}
            </span>
          </li>
          {/if}
        {/each}
      </ul>
    {/if}
  </section>


  <section aria-labelledby="dest-title">
    <div class="section-head">
      <h2 id="dest-title">Repositorios <span class="count">· {repos.length}</span></h2>
    </div>
    <div class="grid">
      {#each rows as { repo, status, h }, i (repo.id)}
        {@const kind = repoKind(repo.location)}
        {@const Icon = LEVEL_ICON[status.level]}
        {@const days = lastDays(repo)}
        {@const withCopies = days.filter((d) => d.n > 0).length}
        {@const task = liveTask(repo.id)}
        {@const cloud = cloudOf(repo)}
        {@const prot = protections[repo.id]}
        <article class="card repo" style:--i={i}>
          <button class="repo-head" onclick={() => onopen(repo)} title="Abrir el repositorio «{repo.name}»">
            <span class="repo-icon"><kind.icon size={16} /></span>
            <span class="repo-name">
              <strong>{repo.name}</strong>
              <span>{kind.label}</span>
            </span>
            <span class="badge tone-{LEVEL_TONE[status.level]}">
              <span class:spin={status.level === "loading"} style="display:grid"><Icon size={12} /></span>
              {status.label}
            </span>
          </button>

          <dl class="facts">
            <div>
              <dt>Última versión</dt>
              <dd>
                {#if status.level === "error"}
                  <span class="bad-text" title={h?.error}>Sin conexión</span>
                {:else if status.last}
                  <span title={formatDate(status.last.time)}>{formatRelative(status.last.time)}</span>
                {:else if status.level === "empty"}
                  <span class="faint">ninguna todavía</span>
                {:else if status.level === "loading"}
                  <span class="faint">…</span>
                {:else}
                  <span class="faint">—</span>
                {/if}
              </dd>
            </div>
            <div>
              <dt>Nube</dt>
              <dd>
                {#if cloud?.held}
                  <span class="bad-text">frenada</span>
                {:else if cloud?.run}
                  <span class={cloud.run.result === "ok" ? "" : cloud.run.result === "warning" ? "warn-text" : "bad-text"} title="{cloud.target} · {formatDate(cloud.run.finished)}">
                    {cloud.run.result === "ok" ? formatRelative(cloud.run.finished) : cloud.run.result === "warning" ? "con avisos" : "falló"}
                  </span>
                {:else if cloud}
                  <span class="faint" title={cloud.target}>pendiente</span>
                {:else if offsiteSources(repo.id).length}
                  <span class="faint" title="Este repositorio es la copia externa de otro"><Cloud size={12} /> recibe</span>
                {:else}
                  <span class="faint" title="Sin copia externa: todas las versiones están en un solo sitio"><CloudOff size={12} /> ninguna</span>
                {/if}
              </dd>
            </div>
            <div>
              <dt>Protección</dt>
              <dd class="prot" title={prot ? prot.items.map((x) => `${x.label}: ${x.detail}`).join("\n") : undefined}>
                {#if prot}
                  <svg width="16" height="16" viewBox="0 0 18 18" aria-hidden="true" class="ring tone-{prot.items.some((x) => x.state === 'bad') ? 'bad' : prot.items.some((x) => x.state !== 'ok') ? 'warn' : 'ok'}">
                    <circle cx="9" cy="9" r="7" class="track" />
                    <circle cx="9" cy="9" r="7" class="arc" stroke-dasharray="{(C * prot.score) / prot.total} {C}" transform="rotate(-90 9 9)" />
                  </svg>
                  <span class="num">{prot.score} de {prot.total}</span>
                {:else}
                  <span class="faint">…</span>
                {/if}
              </dd>
            </div>
          </dl>

          {#if task}
            <TaskProgress {task} target={offsiteTargetName(repo.id, repos)} compact />
          {/if}

          {#if repo.plans?.length}
            <ul class="copies">
              {#each repo.plans as plan (plan.id)}
                {@const cs = copyStatus(repo, plan, now)}
                <li>
                  <button class="copy" onclick={() => onopencopy(repo.id, plan.id)} title="Abrir la copia «{plan.name}»">
                    <span class="cdot c-{cs.level}" role="img" aria-label={COPY_LEVEL_LABEL[cs.level]} title={COPY_LEVEL_LABEL[cs.level]}></span>
                    <span class="cname">{plan.name}</span>
                    <span class="cwhen">
                      {#if cs.level === "running"}copiando…{:else if cs.level === "paused"}en pausa{:else if cs.level === "error"}<span class="bad-text">falló</span>{:else if cs.last}<RelTime iso={cs.last.toISOString()} />{cs.unchanged ? " · sin cambios" : ""}{:else}nunca{/if}
                    </span>
                  </button>
                </li>
              {/each}
            </ul>
          {:else if !offsiteSources(repo.id).length}
            <p class="no-copies">
              Ninguna copia guarda aquí.
              <button class="link" onclick={() => onnewcopy(repo.id)}><Plus size={13} /> Crear una</button>
            </p>
          {/if}

          <div class="card-foot">
            <div class="days" role="img" aria-label="Copias en los últimos 14 días: {withCopies} {withCopies === 1 ? 'día' : 'días'} con copias">
              {#each days as d}
                <span class="d" class:has={d.n > 0} title="{formatShortDay(d.date)}: {d.n ? `${d.n} ${d.n === 1 ? 'copia' : 'copias'}` : 'sin copias'}"></span>
              {/each}
            </div>
            <label class="freq" title="Cada cuánto se espera una copia en este repositorio (para avisar de retrasos)">
              <span class="sr-only">Frecuencia esperada</span>
              <select
                value={repo.expected_hours == null ? "auto" : String(repo.expected_hours)}
                onchange={(e) => setFrequency(repo, e.currentTarget.value, e.currentTarget)}
              >
                <option value="auto">{status.detected ? `${frequencyLabel(status.detected)} (detectada)` : "diaria (por defecto)"}</option>
                {#each FREQUENCIES as f}<option value={String(f)}>{frequencyLabel(f)}</option>{/each}
              </select>
            </label>
          </div>
        </article>
      {/each}
    </div>
  </section>

  <RecentActivity
    title="Actividad reciente"
    {repos}
    select={(e) => e.kind !== "config"}
    limit={6}
    emptyText="Todavía no hay actividad. Aquí verás las últimas copias, verificaciones y copias externas."
    onviewall={onopenactivity}
    onnavigate={navigate}
  />

  <WebLinkCard />
</div>

<style>
  /* ---------- Resumen ---------- */
  .hero {
    --tone: var(--ok);
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    padding: var(--sp-6) var(--sp-6) var(--sp-5);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-xl);
  }
  .hero.tone-warn {
    --tone: var(--warn);
  }
  .hero.tone-bad {
    --tone: var(--bad);
  }
  .hero.tone-info {
    --tone: var(--info);
  }
  .hero-top {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
  }
  .hero-mark {
    display: grid;
    place-items: center;
    flex: none;
    width: 48px;
    height: 48px;
    border-radius: 999px;
    color: var(--tone);
    background: color-mix(in srgb, var(--tone) var(--soft), transparent);
  }
  .hero-text {
    flex: 1;
    min-width: 0;
  }
  .hero-text h1 {
    font-size: var(--fs-display);
    line-height: var(--lh-display);
    font-weight: 650;
    letter-spacing: -0.022em;
  }
  .hero-text p {
    margin: 4px 0 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .copying {
    color: var(--info);
  }
  .hero-text p.discreet {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
  }
  .discreet :global(svg) {
    color: var(--info);
  }
  .hero-top > .btn {
    align-self: flex-start;
  }
  .issues {
    list-style: none;
    margin: 0;
    padding: 0;
    border-top: 1px solid var(--border);
  }
  .issue {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: 12px 0;
    border-bottom: 1px solid var(--border);
  }
  .issue-hold {
    padding: 12px 0;
    border-bottom: 1px solid var(--border);
  }
  .issue-hold:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }
  .issue:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }
  .issue-ic {
    display: grid;
    place-items: center;
    flex: none;
    width: 30px;
    height: 30px;
    border-radius: var(--radius);
    color: var(--tone);
    background: color-mix(in srgb, var(--tone) var(--soft), transparent);
  }
  .issue.tone-info .issue-ic {
    color: var(--text-2);
    background: var(--surface-2);
  }
  .issue-text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .issue-text strong {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .issue-text span {
    font-size: var(--fs-sm);
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .issue-actions {
    display: flex;
    gap: 6px;
    flex: none;
  }


  /* ---------- Destinos ---------- */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: var(--sp-4);
  }
  .repo {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-5);
    animation: rise var(--dur-slow) var(--ease-out) both;
    animation-delay: calc(var(--i) * 30ms);
  }
  .repo:hover {
    border-color: var(--border-strong);
  }
  .repo-head {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: 0;
    font: inherit;
    color: inherit;
    text-align: left;
    background: none;
    border: none;
    cursor: pointer;
  }
  .repo-icon {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: var(--radius);
    color: var(--text-2);
    background: var(--surface-2);
  }
  .repo-name {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .repo-name strong {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .repo-name span {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .repo-head:hover strong {
    text-decoration: underline;
    text-underline-offset: 3px;
    text-decoration-color: var(--border-strong);
  }
  .facts {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-3);
    margin: 0;
  }
  .facts dt {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .facts dd {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 2px 0 0;
    font-size: var(--fs-sm);
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .facts dd :global(svg) {
    flex: none;
    vertical-align: -2px;
  }
  .bad-text {
    color: var(--bad);
  }
  .warn-text {
    color: var(--warn);
  }
  .ring {
    --tone: var(--ok);
  }
  .ring.tone-warn {
    --tone: var(--warn);
  }
  .ring.tone-bad {
    --tone: var(--bad);
  }
  .ring .track {
    fill: none;
    stroke: var(--surface-3);
    stroke-width: 3;
  }
  .ring .arc {
    fill: none;
    stroke: var(--tone);
    stroke-width: 3;
    stroke-linecap: round;
    transition: stroke-dasharray var(--dur-slow) var(--ease-out);
  }
  .copies {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
  .copy {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    margin: 0 -8px;
    width: calc(100% + 16px);
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-1);
    text-align: left;
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .copy:hover {
    background: var(--surface-2);
  }
  .cdot {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--neutral);
  }
  .c-ok {
    background: var(--ok);
  }
  .c-warning,
  .c-late {
    background: var(--warn);
  }
  .c-error {
    background: var(--bad);
  }
  .c-running {
    background: var(--info);
  }
  .c-paused {
    background: var(--paused);
  }
  .c-never {
    background: transparent;
    box-shadow: inset 0 0 0 1.5px var(--border-strong);
  }
  .cname {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cwhen {
    flex: none;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .no-copies {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .card-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    margin-top: auto;
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .days {
    display: grid;
    grid-template-columns: repeat(14, 8px);
    gap: 3px;
  }
  .d {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    background: var(--surface-3);
  }
  .d.has {
    background: color-mix(in srgb, var(--ok) 85%, transparent);
  }
  .freq select {
    max-width: 150px;
    height: 24px;
    padding: 0 4px;
    font: inherit;
    font-size: var(--fs-xs);
    color: var(--text-3);
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
    text-align: right;
  }
  .freq select:hover {
    color: var(--text-1);
    border-color: var(--border);
  }
  @media (max-width: 760px) {
    .issue {
      flex-wrap: wrap;
    }
    .issue-actions {
      margin-left: 42px;
    }
  }
</style>
