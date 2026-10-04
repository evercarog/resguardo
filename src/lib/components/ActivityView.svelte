<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { ChevronRight, CircleAlert, CircleCheck, Download, History, ListChecks, RefreshCw, Search, SearchX, TriangleAlert, X } from "@lucide/svelte";
  import type { ActivityEntry, Repo } from "$lib/api";
  import {
    KIND_ICON,
    ORIGIN_LABEL,
    RESULT_ICON,
    RESULT_LABEL,
    activity,
    activityCsv,
    downloadText,
    durationOf,
    entryTarget,
    fold,
    kindLabel,
    pollActivity,
    refreshActivity,
  } from "$lib/activity.svelte";
  import { formatBytes, formatDate, formatDay, formatDuration, formatNumber, formatRelative, formatTime } from "$lib/format";
  import type { Selection } from "$lib/nav";
  import { toast } from "$lib/toast.svelte";

  // Historial de actividad: todo lo que han hecho el agente y la app, con filtros.
  interface Props {
    repos: Repo[];
    /** Filtro inicial por destino ("repoId") o copia ("repoId#planId"). */
    filter?: string;
    /** Abrir una copia o un destino. */
    onnavigate: (sel: Selection) => void;
  }
  let { repos, filter = "", onnavigate }: Props = $props();

  type Period = "today" | "7" | "30" | "all";
  const PERIODS: { id: Period; label: string }[] = [
    { id: "today", label: "Hoy" },
    { id: "7", label: "7 días" },
    { id: "30", label: "30 días" },
    { id: "all", label: "Todo" },
  ];
  const KINDS = [
    { id: "all", label: "Todas" },
    { id: "backup", label: "Copias" },
    { id: "verify", label: "Verificaciones" },
    { id: "offsite", label: "Copias externas" },
    { id: "pause", label: "Pausas" },
    { id: "config", label: "Cambios de configuración" },
  ] as const;

  // Filtros. Al llegar filtrado por una copia se muestra todo su historial.
  // svelte-ignore state_referenced_locally
  let period = $state<Period>(filter ? "all" : "7");
  /** "pause": pausas y reanudaciones de las copias automáticas. */
  let kind = $state<"all" | "backup" | "verify" | "offsite" | "pause" | "config">("all");
  let result = $state<"all" | "problems">("all");
  // svelte-ignore state_referenced_locally
  let target = $state(filter);
  let query = $state("");

  /** Filas visibles (se amplía con «Mostrar más»). */
  const PAGE = 150;
  let shown = $state(PAGE);
  /** Mensajes largos desplegados. */
  let expanded = $state<Record<string, boolean>>({});

  let now = $state(Date.now());
  onMount(() => {
    const stop = pollActivity(30_000);
    const t = setInterval(() => (now = Date.now()), 60_000);
    return () => {
      stop();
      clearInterval(t);
    };
  });

  const since = $derived.by(() => {
    const d = new Date(now);
    if (period === "today") return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
    if (period === "all") return 0;
    return now - Number(period) * 86_400_000;
  });

  /** Destinos y copias para el desplegable (los actuales y los que aparecen en el historial). */
  const targets = $derived.by(() => {
    const map = new Map<string, { name: string; plans: Map<string, string> }>();
    for (const r of repos) map.set(r.id, { name: r.name, plans: new Map(r.plans.map((p) => [p.id, p.name])) });
    for (const e of activity.entries) {
      let d = map.get(e.repo_id);
      if (!d) map.set(e.repo_id, (d = { name: e.repo_name, plans: new Map() }));
      if (e.plan_id && !d.plans.has(e.plan_id)) d.plans.set(e.plan_id, e.plan_name ?? e.plan_id);
    }
    return [...map].map(([id, d]) => ({ id, name: d.name, plans: [...d.plans].map(([pid, name]) => ({ id: pid, name })) }));
  });

  const words = $derived(fold(query.trim()).split(/\s+/).filter(Boolean));

  /** Todo menos el filtro de resultado: de aquí salen los contadores. */
  const base = $derived.by(() => {
    const [repoId, planId] = target.split("#");
    return activity.entries.filter((e) => {
      if (since && new Date(e.started).getTime() < since) return false;
      if (kind === "pause") {
        if (e.kind !== "pause" && e.kind !== "resume") return false;
      } else if (kind === "config") {
        if (e.kind !== "config" && e.kind !== "kit") return false;
      } else if (kind !== "all" && e.kind !== kind) return false;
      if (repoId && e.repo_id !== repoId) return false;
      if (planId && e.plan_id !== planId) return false;
      if (words.length) {
        const text = fold(`${kindLabel(e)} ${e.repo_name} ${e.plan_name ?? ""} ${e.message} ${ORIGIN_LABEL[e.origin] ?? ""}`);
        if (!words.every((w) => text.includes(w))) return false;
      }
      return true;
    });
  });
  const rows = $derived(result === "problems" ? base.filter((e) => e.result === "warning" || e.result === "error") : base);
  const counts = $derived({
    total: base.length,
    ok: base.filter((e) => e.result === "ok").length,
    warning: base.filter((e) => e.result === "warning").length,
    error: base.filter((e) => e.result === "error").length,
  });

  const filtered = $derived(kind !== "all" || result !== "all" || !!target || !!words.length);
  function clearFilters() {
    kind = "all";
    result = "all";
    target = "";
    query = "";
  }

  // Al cambiar los filtros se vuelve a la primera página (no al refrescar
  // los datos cada 30 s, que cerraría lo que se abrió con «Mostrar más»).
  $effect(() => {
    void [period, kind, result, target, query];
    shown = PAGE;
  });

  /** Filas agrupadas por día (del final, que es como vienen ordenadas; así
   *  un mismo día nunca aparece dos veces). */
  const groups = $derived.by(() => {
    const out: { day: string; items: ActivityEntry[] }[] = [];
    const byDay = new Map<string, ActivityEntry[]>();
    for (const e of rows.slice(0, shown)) {
      const day = formatDay(e.finished);
      let items = byDay.get(day);
      if (!items) {
        items = [];
        byDay.set(day, items);
        out.push({ day, items });
      }
      items.push(e);
    }
    return out;
  });

  const keyOf = (e: ActivityEntry) => `${e.started}|${e.finished}|${e.kind}|${e.origin}|${e.repo_id}|${e.plan_id ?? ""}`;
  const LONG = 140;

  function exportCsv() {
    const d = new Date();
    const pad = (n: number) => String(n).padStart(2, "0");
    downloadText(`resguardo-actividad-${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}.csv`, activityCsv(rows));
    toast(`Exportadas ${formatNumber(rows.length)} ${rows.length === 1 ? "entrada" : "entradas"} a CSV.`);
  }

  const firstLoad = $derived(!activity.loadedAt && !activity.error);
</script>

<div class="page activity" in:fade={{ duration: dur(160) }}>
  <header class="page-top">
    <div>
      <h1>Actividad</h1>
      <p class="faint">
        Copias, verificaciones, copias externas y pausas, de la más reciente a la más antigua
        {#if activity.loadedAt} · actualizado {formatRelative(new Date(activity.loadedAt).toISOString())}{/if}
      </p>
    </div>
    <div class="page-actions">
      <button class="btn btn-sm" onclick={() => refreshActivity()} disabled={activity.loading} title="Volver a leer el historial">
        <span class:spin={activity.loading} style="display:grid"><RefreshCw size={14} /></span> Actualizar
      </button>
      <button class="btn btn-sm" onclick={exportCsv} disabled={!rows.length} title="Descargar las entradas filtradas como CSV">
        <Download size={14} /> Exportar CSV
      </button>
    </div>
  </header>

  <div class="segmented inline" role="group" aria-label="Periodo">
    {#each PERIODS as p (p.id)}
      <button class:on={period === p.id} aria-pressed={period === p.id} onclick={() => (period = p.id)}>{p.label}</button>
    {/each}
  </div>

  <div class="kpis" role="list" aria-label="Resumen del periodo">
    <div class="kpi" role="listitem" style:--kpi="var(--accent)"><ListChecks size={18} /><strong>{formatNumber(counts.total)}</strong><span>En total</span></div>
    <div class="kpi" role="listitem" style:--kpi="var(--ok)"><CircleCheck size={18} /><strong>{formatNumber(counts.ok)}</strong><span>Correctas</span></div>
    <div class="kpi" role="listitem" style:--kpi="var(--warn)"><TriangleAlert size={18} /><strong>{formatNumber(counts.warning)}</strong><span>Con avisos</span></div>
    <div class="kpi" role="listitem" style:--kpi="var(--bad)"><CircleAlert size={18} /><strong>{formatNumber(counts.error)}</strong><span>Fallidas</span></div>
  </div>

  <section class="card list">
    <div class="filters">
      <label class="search">
        <Search size={15} />
        <input class="input" type="search" placeholder="Buscar por nombre o mensaje…" aria-label="Buscar" bind:value={query} />
      </label>
      <select class="input" aria-label="Tipo" bind:value={kind}>
        {#each KINDS as k (k.id)}<option value={k.id}>{k.label}</option>{/each}
      </select>
      <select class="input" aria-label="Resultado" bind:value={result}>
        <option value="all">Todos los resultados</option>
        <option value="problems">Solo fallos y avisos</option>
      </select>
      <select class="input" aria-label="Repositorio o copia" bind:value={target}>
        <option value="">Todos los repositorios</option>
        {#each targets as t (t.id)}
          <optgroup label={t.name}>
            <option value={t.id}>Todo «{t.name}»</option>
            {#each t.plans as p (p.id)}<option value="{t.id}#{p.id}">Copia «{p.name}»</option>{/each}
          </optgroup>
        {/each}
      </select>
      {#if filtered}
        <button class="btn btn-ghost btn-sm" onclick={clearFilters}><X size={14} /> Quitar filtros</button>
      {/if}
    </div>

    {#if activity.error && !activity.entries.length}
      <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{activity.error}</p></div>
    {:else if firstLoad}
      <div class="skeleton" role="status" aria-label="Cargando la actividad">
        {#each [0, 1, 2, 3, 4] as i}<div style:animation-delay="{i * 120}ms"></div>{/each}
      </div>
    {:else if !activity.entries.length}
      <div class="empty-state">
        <History size={28} strokeWidth={1.6} />
        <strong>Todavía no hay actividad</strong>
        <span class="faint hint">Aquí aparecerán las copias (automáticas y a mano), las verificaciones y las copias externas en cuanto se hagan.</span>
      </div>
    {:else if !rows.length}
      <div class="empty-state">
        <SearchX size={28} strokeWidth={1.6} />
        <strong>No hay actividad {period === "all" ? "con estos filtros" : "en este periodo con estos filtros"}</strong>
        <span class="hint-actions">
          {#if filtered}<button class="btn btn-ghost btn-sm" onclick={clearFilters}>Quitar filtros</button>{/if}
          {#if period !== "all"}<button class="btn btn-ghost btn-sm" onclick={() => (period = "all")}>Ver todo el historial</button>{/if}
        </span>
      </div>
    {:else}
      {#each groups as group, g (group.day)}
        <div class="day">{group.day}</div>
        <div class="group">
          {#each group.items as e, i (keyOf(e))}
            {@const key = keyOf(e)}
            {@const sel = entryTarget(e, repos)}
            {#if sel}
              <div
                class="row clickable r-{e.result}"
                style:--i={Math.min(g * 2 + i, 14)}
                role="button"
                tabindex="0"
                title={sel.kind === "copy" ? `Abrir la copia «${e.plan_name}»` : `Abrir el repositorio «${e.repo_name}»`}
                onclick={() => onnavigate(sel)}
                onkeydown={(ev) => (ev.key === "Enter" || ev.key === " ") && ev.target === ev.currentTarget && (ev.preventDefault(), onnavigate(sel))}
              >
                {@render body(e, key, true)}
              </div>
            {:else}
              <div class="row r-{e.result}" style:--i={Math.min(g * 2 + i, 14)}>
                {@render body(e, key, false)}
              </div>
            {/if}
          {/each}
        </div>
      {/each}
      {#if rows.length > shown}
        <div class="more-rows">
          <button class="btn btn-sm" onclick={() => (shown += PAGE)}>Mostrar más</button>
          <span class="faint">Se muestran {formatNumber(shown)} de {formatNumber(rows.length)}</span>
        </div>
      {/if}
    {/if}
  </section>
</div>

{#snippet body(e: ActivityEntry, key: string, clickable: boolean)}
  {@const KindIcon = KIND_ICON[e.kind] ?? KIND_ICON.backup}
  {@const ResIcon = RESULT_ICON[e.result] ?? RESULT_ICON.ok}
  {@const secs = durationOf(e)}
  {@const showMsg = e.result !== "ok" || (e.kind !== "backup" && !!e.message)}
  {@const long = e.message.length > LONG}
  <span class="time" title={formatDate(e.started)}>{formatTime(e.started)}</span>
  <span class="res" title={RESULT_LABEL[e.result]}><ResIcon size={16} /><span class="sr-only">{RESULT_LABEL[e.result]}</span></span>
  <span class="main">
    <span class="what">
      <span class="kind-icon"><KindIcon size={14} /></span>
      <strong title={kindLabel(e)}>{kindLabel(e)}</strong>
      <span class="dest faint" title={e.repo_name}>· {e.repo_name}</span>
    </span>
    {#if showMsg && e.message}
      <span class="msg" class:clamp={long && !expanded[key]} class:m-ok={e.result === "ok"}>{e.message}</span>
      {#if long}
        <button class="more" onclick={(ev) => (ev.stopPropagation(), (expanded[key] = !expanded[key]))}>
          {expanded[key] ? "Ver menos" : "Ver más"}
        </button>
      {/if}
    {/if}
  </span>
  {#if (e.kind === "config" || e.kind === "kit") && e.user}
    <span class="origin o-manual" title="Usuario de Windows que lo hizo">{e.user}</span>
  {:else}
    <span class="origin o-{e.origin}" title={e.origin === "remote" ? `Pedida a distancia desde ${e.requested_from ?? "la web"}` : e.user ? `Por ${e.user}` : undefined}>{ORIGIN_LABEL[e.origin] ?? e.origin}</span>
  {/if}
  <span class="dur faint" title="Duración">{secs != null ? formatDuration(secs) : ""}</span>
  <span class="data">
    {#if e.unchanged}
      <span class="added same" title={e.message}>Sin cambios</span>
      <small class="faint">no se guardó versión</small>
    {:else if e.kind === "backup" && e.data_added != null}
      <span class="added" title="Datos nuevos añadidos al repositorio">+{formatBytes(e.data_added)}</span>
    {/if}
    {#if e.kind === "backup" && !e.unchanged && (e.files_new != null || e.files_changed != null)}
      <small class="faint">{formatNumber(e.files_new ?? 0)} nuevos · {formatNumber(e.files_changed ?? 0)} cambiados</small>
    {/if}
  </span>
  <span class="go">{#if clickable}<ChevronRight size={16} />{/if}</span>
{/snippet}

<style>
  .activity {
    gap: 16px;
  }

  .list {
    padding: 16px 22px 12px;
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding-bottom: 12px;
  }
  .filters select {
    width: auto;
    max-width: 230px;
    height: 32px;
    padding: 0 8px;
    font-size: var(--fs-sm);
  }
  .search {
    position: relative;
    display: flex;
    align-items: center;
    flex: 1 1 220px;
    min-width: 180px;
    color: var(--text-3);
  }
  .search :global(svg) {
    position: absolute;
    left: 10px;
    pointer-events: none;
  }
  .search input {
    height: 32px;
    padding-left: 32px;
    font-size: var(--fs-sm);
  }

  .day {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: 14px 0 6px;
    font-size: var(--fs-xs);
    font-weight: 650;
    color: var(--text-2);
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }
  .row {
    display: grid;
    grid-template-columns: 48px 18px minmax(0, 1fr) 92px 76px 150px 18px;
    align-items: center;
    gap: 12px;
    padding: 9px 8px;
    margin: 0 -8px;
    border-radius: var(--radius-sm);
    animation: rise 0.3s cubic-bezier(0.2, 0.8, 0.2, 1) both;
    animation-delay: calc(var(--i) * 18ms);
    content-visibility: auto;
    contain-intrinsic-size: auto 46px;
    transition: background 0.12s;
  }
  .row + .row {
    border-top: 1px solid var(--border);
  }
  .row.clickable {
    cursor: pointer;
  }
  .row.clickable:hover {
    background: var(--surface-2);
  }
  .row.clickable:hover,
  .row.clickable:hover + .row {
    border-top-color: transparent;
  }
  .row.r-error {
    background: color-mix(in srgb, var(--bad-soft) 55%, transparent);
  }
  .row.r-error.clickable:hover {
    background: var(--bad-soft);
  }
  .time {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .res {
    display: grid;
    color: var(--ok);
  }
  .r-warning .res {
    color: var(--warn);
  }
  .r-error .res {
    color: var(--bad);
  }
  /* Pausas y reanudaciones: informativas, sin color de resultado. */
  .r-info .res {
    color: var(--text-3);
  }
  .main {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    min-width: 0;
  }
  .what {
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: 100%;
    min-width: 0;
    white-space: nowrap;
  }
  .what strong {
    flex: 0 1 auto;
    min-width: 0;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .kind-icon {
    display: grid;
    flex: none;
    color: var(--text-3);
  }
  .dest {
    flex: 0 100 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: var(--fs-sm);
  }
  .msg {
    font-size: var(--fs-sm);
    line-height: 1.45;
    color: var(--bad);
    overflow-wrap: anywhere;
    user-select: text;
  }
  .r-warning .msg {
    color: var(--warn);
  }
  .msg.m-ok {
    color: var(--text-3);
  }
  .msg.clamp {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .more {
    padding: 0;
    font: inherit;
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--accent-text);
    background: none;
    border: none;
    cursor: pointer;
  }
  .more:hover {
    text-decoration: underline;
  }
  .row:focus-visible {
    outline-offset: -2px;
  }
  .origin {
    justify-self: start;
    padding: 0 8px;
    font-size: var(--fs-xs);
    font-weight: 550;
    line-height: 20px;
    white-space: nowrap;
    border-radius: 999px;
    color: var(--text-2);
    background: var(--surface-3);
  }
  .origin.o-manual {
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .origin.o-remote {
    color: var(--info);
    background: var(--info-soft);
  }
  .origin.o-retry {
    color: var(--warn);
    background: var(--warn-soft);
  }
  .dur {
    font-size: var(--fs-sm);
    text-align: right;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .data {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    line-height: 1.3;
    text-align: right;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .added {
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--accent-text);
  }
  /* Copia sin cambios: correcta, sin versión nueva. */
  .added.same {
    font-weight: 500;
    color: var(--text-3);
  }
  .data small {
    font-size: var(--fs-xs);
  }
  .go {
    display: grid;
    color: var(--text-3);
    opacity: 0;
    transform: translateX(-4px);
    transition:
      opacity 0.15s,
      transform 0.15s;
  }
  .row:hover .go,
  .row:focus-visible .go {
    opacity: 1;
    transform: none;
    color: var(--accent);
  }
  .more-rows {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 0 6px;
    font-size: var(--fs-sm);
  }

  .hint {
    max-width: 440px;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .hint-actions {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }
  @media (max-width: 1240px) {
    .row {
      grid-template-columns: 48px 18px minmax(0, 1fr) 92px 110px 18px;
    }
    .dur {
      display: none;
    }
    .data small {
      display: none;
    }
  }
  @media (max-width: 1150px) {
    .row {
      grid-template-columns: 44px 18px minmax(0, 1fr) 86px 80px 18px;
      gap: 10px;
    }
  }
  /* Ventana estrecha (con la barra lateral): el origen pasa debajo del nombre para que este no se corte. */
  @media (max-width: 1100px) {
    .row {
      grid-template-columns: 44px 18px minmax(0, 1fr) auto 16px;
      row-gap: 4px;
    }
    .time {
      grid-column: 1;
      grid-row: 1 / span 2;
    }
    .res {
      grid-column: 2;
      grid-row: 1 / span 2;
    }
    .main {
      grid-column: 3;
      grid-row: 1;
    }
    .origin {
      grid-column: 3;
      grid-row: 2;
    }
    .data {
      grid-column: 4;
      grid-row: 1 / span 2;
    }
    .go {
      grid-column: 5;
      grid-row: 1 / span 2;
    }
  }
</style>
