<script lang="ts">
  import { slide } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { Archive, CalendarDays, ChartBar, Check, ChevronRight, CircleAlert, Copy, Monitor, RefreshCw, SearchX, X } from "@lucide/svelte";
  import SnapshotCalendar from "./SnapshotCalendar.svelte";
  import type { Snapshot } from "$lib/api";
  import { formatBytes, formatDate, formatDay, formatDuration, formatTime } from "$lib/format";

  interface Props {
    snapshots: Snapshot[];
    loading: boolean;
    error: string;
    onrefresh: () => void;
    /** Abrir el snapshot para explorar y restaurar. */
    onopen: (snapshot: Snapshot) => void;
    /** Título de la tarjeta. */
    title?: string;
    /** Qué hacer para tener la primera versión (cuando no hay ninguna). */
    emptyHint?: string;
    /** Ver lo que más ocupa en una versión (sin él no se ofrece). */
    onlargest?: (snapshot: Snapshot) => void;
    /**
     * Fin (ISO) de las copias correctas sin cambios («Solo guardar si hay
     * cambios»): no dejaron versión, pero ese día se revisó y estaba al día.
     */
    checks?: string[];
  }
  let {
    snapshots,
    loading,
    error,
    onrefresh,
    onopen,
    onlargest,
    checks = [],
    title = "Versiones guardadas",
    emptyHint = "Cada vez que se hace una copia se guarda aquí una versión que puedes explorar y restaurar.",
  }: Props = $props();

  // Filtro por fechas (días locales "AAAA-MM-DD", ambos incluidos).
  let showCalendar = $state(false);
  let from = $state<string | null>(null);
  let to = $state<string | null>(null);

  const pad = (n: number) => String(n).padStart(2, "0");
  const keyOf = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;

  const filtered = $derived.by(() => {
    if (!from) return snapshots;
    const last = to ?? from;
    return snapshots.filter((s) => {
      const k = keyOf(new Date(s.time));
      return k >= from! && k <= last;
    });
  });

  const dayLabel = (key: string, withYear = false) =>
    new Intl.DateTimeFormat("es", { day: "numeric", month: "short", ...(withYear ? { year: "numeric" } : {}) }).format(
      new Date(`${key}T12:00:00`),
    );
  const rangeLabel = $derived(
    !from ? "" : !to || to === from ? dayLabel(from, true) : `${dayLabel(from)} – ${dayLabel(to, true)}`,
  );

  function setRange(start: string | null, end: string | null) {
    from = start;
    to = end;
  }

  function preset(days: number | null) {
    if (days === null) return setRange(null, null);
    const end = new Date();
    const start = new Date(end.getFullYear(), end.getMonth(), end.getDate() - (days - 1));
    setRange(keyOf(start), keyOf(end));
  }

  /** Últimos 60 días (de más antiguo a hoy): cuántas copias hubo cada día. */
  const DAYS = 60;
  const days = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const s of snapshots) {
      const k = keyOf(new Date(s.time));
      counts.set(k, (counts.get(k) ?? 0) + 1);
    }
    const checked = new Map<string, number>();
    for (const c of checks) {
      const k = keyOf(new Date(c));
      checked.set(k, (checked.get(k) ?? 0) + 1);
    }
    const today = new Date();
    return Array.from({ length: DAYS }, (_, i) => {
      const d = new Date(today.getFullYear(), today.getMonth(), today.getDate() - (DAYS - 1 - i));
      const key = keyOf(d);
      return { key, n: counts.get(key) ?? 0, checked: checked.get(key) ?? 0, weekday: d.getDay() };
    });
  });
  const anyChecked = $derived(days.some((d) => !d.n && d.checked));
  /** «2 versiones», «sin versiones · revisada 3 veces sin cambios» o «sin versiones». */
  function dayText(d: { n: number; checked: number }) {
    if (d.n) return `${d.n} ${d.n === 1 ? "versión" : "versiones"}`;
    if (d.checked) return `sin versiones nuevas · revisada ${d.checked === 1 ? "1 vez" : `${d.checked} veces`}, sin cambios`;
    return "sin versiones";
  }
  const maxPerDay = $derived(Math.max(1, ...days.map((d) => d.n)));
  const inRange = (key: string) => !!from && key >= from && key <= (to ?? from);
  const longLabel = (key: string) =>
    new Intl.DateTimeFormat("es", { weekday: "long", day: "numeric", month: "long" }).format(new Date(`${key}T12:00:00`));

  /** Un día: filtra por él; pulsarlo otra vez quita el filtro. */
  function pickDay(key: string) {
    if (from === key && (!to || to === key)) setRange(null, null);
    else setRange(key, key);
  }

  /** Snapshots del más reciente al más antiguo, agrupados por día. */
  const groups = $derived.by(() => {
    const sorted = [...filtered].sort((a, b) => b.time.localeCompare(a.time));
    const out: { day: string; items: Snapshot[] }[] = [];
    for (const s of sorted) {
      const day = formatDay(s.time);
      if (out.at(-1)?.day === day) out.at(-1)!.items.push(s);
      else out.push({ day, items: [s] });
    }
    return out;
  });

  let copied = $state<string | null>(null);
  async function copyId(id: string) {
    try {
      await navigator.clipboard.writeText(id);
      copied = id;
      setTimeout(() => copied === id && (copied = null), 1400);
    } catch {
      /* el portapapeles puede no estar disponible */
    }
  }

  function duration(s: Snapshot) {
    const a = s.summary?.backup_start, b = s.summary?.backup_end;
    return a && b ? (new Date(b).getTime() - new Date(a).getTime()) / 1000 : null;
  }

  /** Versión más reciente de las que se ven (para «Lo que más ocupa» de la cabecera). */
  const latest = $derived(filtered.reduce<Snapshot | null>((a, s) => (!a || s.time > a.time ? s : a), null));

  const baseName = (p: string) => p.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || p;
</script>

<section class="card list">
  <header>
    <div>
      <h2>{title}</h2>
      <p class="faint">
        {#if loading && snapshots.length === 0}Cargando…{:else if from}{filtered.length} de {snapshots.length}
          versiones{:else}{snapshots.length}
          {snapshots.length === 1 ? "versión" : "versiones"}{/if}
      </p>
    </div>
    <div class="head-actions">
      {#if from}
        <span class="range-chip">
          <CalendarDays size={13} />
          {rangeLabel}
          <button class="chip-x" title="Quitar filtro" onclick={() => setRange(null, null)}><X size={12} /></button>
        </span>
      {/if}
      <button
        class="btn btn-ghost btn-sm"
        class:on={showCalendar}
        onclick={() => (showCalendar = !showCalendar)}
        disabled={!snapshots.length}
        aria-expanded={showCalendar}
      >
        <CalendarDays size={14} /> Calendario
      </button>
      {#if onlargest && latest}
        <button
          class="btn btn-ghost btn-sm"
          onclick={() => latest && onlargest(latest)}
          title="Carpetas y archivos más grandes de la versión más reciente{from ? ' del rango elegido' : ''} ({formatDate(latest.time)})"
        >
          <ChartBar size={14} /> Lo que más ocupa
        </button>
      {/if}
      <button class="btn btn-ghost btn-sm" onclick={onrefresh} disabled={loading} title="Actualizar">
        <span class:spin={loading} style="display:grid"><RefreshCw size={14} /></span>
        Actualizar
      </button>
    </div>
  </header>

  {#if snapshots.length && !showCalendar}
    <div class="strip">
      <div class="days" role="group" aria-label="Versiones en los últimos {DAYS} días">
        {#each days as d (d.key)}
          <button
            class="d"
            class:has={d.n > 0}
            class:checked={!d.n && d.checked > 0}
            class:on={inRange(d.key)}
            style:--o={d.n ? 0.35 + 0.65 * (d.n / maxPerDay) : 0}
            title="{longLabel(d.key)}: {dayText(d)}"
            aria-label="{longLabel(d.key)}: {dayText(d)}"
            aria-pressed={inRange(d.key)}
            disabled={!d.n}
            onclick={() => pickDay(d.key)}
          ></button>
        {/each}
      </div>
      <div class="strip-foot faint">
        <span>Últimos {DAYS} días · pulsa un día para ver sus versiones</span>
        <span class="legend">
          {#if anyChecked}<i class="d checked"></i> Sin cambios ·{/if}
          Menos <i class="d"></i><i class="d has" style:--o="0.4"></i><i class="d has" style:--o="0.7"></i><i class="d has" style:--o="1"></i> Más
        </span>
      </div>
    </div>
  {/if}

  {#if showCalendar && snapshots.length}
    <div class="calendar-area" transition:slide={{ duration: dur(180) }}>
      <SnapshotCalendar times={snapshots.map((s) => s.time)} start={from} end={to} onselect={setRange} />
      <div class="cal-side">
        <div class="presets">
          <button class="preset" onclick={() => preset(1)}>Hoy</button>
          <button class="preset" onclick={() => preset(7)}>Últimos 7 días</button>
          <button class="preset" onclick={() => preset(30)}>Últimos 30 días</button>
          <button class="preset" onclick={() => preset(null)} class:active={!from}>Todo</button>
        </div>
        <p class="faint cal-help">
          {#if from && !to}
            Elige el día final del rango, o vuelve a pulsar el mismo día para ver solo ese.
          {:else}
            Pulsa un día para ver sus versiones; pulsa un segundo día para elegir un rango.
          {/if}
        </p>
        <p class="legend faint"><span class="ldot"></span> Días con versiones (más intenso = más versiones)</p>
      </div>
    </div>
  {/if}

  {#if error}
    <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>
  {:else if loading && snapshots.length === 0}
    <div class="skeleton">
      {#each [0, 1, 2] as i}<div class="sk-row" style:animation-delay="{i * 120}ms"></div>{/each}
    </div>
  {:else if snapshots.length === 0}
    <div class="empty-state">
      <Archive size={28} strokeWidth={1.6} />
      <strong>Todavía no hay versiones</strong>
      <span class="faint hint">{emptyHint}</span>
    </div>
  {:else if filtered.length === 0}
    <div class="empty-state">
      <SearchX size={28} strokeWidth={1.6} />
      <strong>No hay versiones en {rangeLabel}</strong>
      <button class="btn btn-ghost btn-sm" onclick={() => setRange(null, null)}>Ver todas</button>
    </div>
  {:else}
    {#each groups as group, g (group.day)}
      <div class="day">{group.day}</div>
      <div class="group">
        {#each group.items as s, i (s.id)}
          <div
            class="row"
            style:--i={Math.min(g * 2 + i, 14)}
            role="button"
            tabindex="0"
            title="Explorar y restaurar"
            onclick={() => onopen(s)}
            onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), onopen(s))}
          >
            <span class="time" title={formatDate(s.time)}>
              {formatTime(s.time)}
              {#if duration(s) !== null}<small title="Duración de la copia">{formatDuration(duration(s))}</small>{/if}
            </span>
            <span class="id-wrap">
              <span class="id mono selectable">{s.short_id}</span>
              <button class="icon-btn copy" title="Copiar el ID completo" aria-label="Copiar el ID completo de la versión {s.short_id}" onclick={(e) => (e.stopPropagation(), copyId(s.id))}>
                {#if copied === s.id}<Check size={13} />{:else}<Copy size={13} />{/if}
              </button>
            </span>
            <span class="paths" title={s.paths.join("\n")}>
              {s.paths.map(baseName).join(", ")}
            </span>
            <span class="tags">
              {#each s.tags as t}<span class="tag">{t}</span>{/each}
            </span>
            <span class="host faint" title="Equipo"><Monitor size={13} />{s.hostname}</span>
            <span class="size">
              <span>{formatBytes(s.summary?.total_bytes_processed)}</span>
              {#if s.summary?.data_added_packed != null}
                <span class="added" title="Espacio nuevo que ocupó en disco esta versión (comprimido y sin duplicados)">
                  +{formatBytes(s.summary.data_added_packed)}
                </span>
              {/if}
            </span>
            <span class="largest-cell">
              {#if onlargest}
                <button
                  class="icon-btn largest"
                  title="Ver lo que más ocupa"
                  aria-label="Ver lo que más ocupa en la versión {s.short_id}"
                  onclick={(e) => (e.stopPropagation(), onlargest(s))}
                  onkeydown={(e) => e.stopPropagation()}
                >
                  <ChartBar size={14} />
                </button>
              {/if}
            </span>
            <span class="go"><ChevronRight size={16} /></span>
          </div>
        {/each}
      </div>
    {/each}
  {/if}
</section>

<style>
  .list {
    padding: 20px 22px 12px;
  }
  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  h2 {
    font-size: var(--fs-h2);
    font-weight: 650;
  }
  header p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .head-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .btn.on {
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .range-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: 4px;
    padding: 0 4px 0 10px;
    height: 28px;
    font-size: var(--fs-sm);
    font-weight: 600;
    border-radius: 999px;
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .chip-x {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    padding: 0;
    color: inherit;
    background: none;
    border: none;
    border-radius: 50%;
    cursor: pointer;
  }
  .chip-x:hover {
    background: color-mix(in srgb, var(--accent) 20%, transparent);
  }
  .calendar-area {
    display: flex;
    gap: 28px;
    padding: 14px 0 16px;
    border-bottom: 1px solid var(--border);
  }
  .cal-side {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding-top: 36px;
    min-width: 0;
  }
  .presets {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .preset {
    height: 28px;
    padding: 0 11px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
    cursor: pointer;
  }
  .preset:hover {
    border-color: var(--accent);
    color: var(--accent-text);
  }
  .preset.active {
    color: var(--accent-text);
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .cal-help,
  .legend {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.5;
    max-width: 320px;
  }
  .legend {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .ldot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
  }
  .day {
    position: sticky;
    top: 0;
    padding: 14px 0 6px;
    font-size: var(--fs-xs);
    font-weight: 650;
    color: var(--text-2);
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }
  .row {
    display: grid;
    grid-template-columns: 70px 118px minmax(0, 1fr) auto 150px 84px 26px 18px;
    cursor: pointer;
    animation: rise 0.3s cubic-bezier(0.2, 0.8, 0.2, 1) both;
    animation-delay: calc(var(--i) * 18ms);
    /* Listas largas: el navegador no dibuja las filas fuera de pantalla. */
    content-visibility: auto;
    contain-intrinsic-size: auto 48px;
    align-items: center;
    gap: 12px;
    padding: 9px 8px;
    margin: 0 -8px;
    border-radius: var(--radius-sm);
    transition: background 0.12s;
  }
  .row + .row {
    border-top: 1px solid var(--border);
  }
  .row:hover {
    background: var(--surface-2);
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
  .row:hover + .row,
  .row:hover {
    border-top-color: transparent;
  }
  .time {
    display: flex;
    flex-direction: column;
    line-height: 1.25;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .time small {
    font-size: var(--fs-xs);
    font-weight: 400;
    color: var(--text-3);
    white-space: nowrap;
  }
  .id-wrap {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .id {
    padding: 1px 7px;
    font-size: var(--fs-xs);
    border-radius: var(--radius-sm);
    background: var(--surface-3);
    color: var(--text-2);
  }
  .copy {
    width: 24px;
    height: 24px;
    opacity: 0;
  }
  .row:hover .copy,
  .copy:focus-visible {
    opacity: 1;
  }
  .largest-cell {
    display: grid;
  }
  .largest {
    width: 26px;
    height: 26px;
    opacity: 0;
  }
  .row:hover .largest,
  .row:focus-within .largest,
  .largest:focus-visible {
    opacity: 1;
  }
  .paths {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tags {
    display: flex;
    gap: 4px;
    justify-content: flex-end;
  }
  .tag {
    padding: 0 8px;
    font-size: var(--fs-xs);
    font-weight: 550;
    line-height: 20px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .host {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .size {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    line-height: 1.25;
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--text-2);
  }
  .added {
    font-size: var(--fs-xs);
    color: var(--accent-text);
  }
  @media (max-width: 980px) {
    .row {
      grid-template-columns: 70px 118px minmax(0, 1fr) 84px 26px 18px;
    }
    .host,
    .tags {
      display: none;
    }
  }

  .hint {
    max-width: 420px;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .skeleton {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 0;
  }
  .sk-row {
    height: 20px;
    border-radius: var(--radius-sm);
    background: var(--surface-3);
    animation: fade 1.2s ease-in-out infinite alternate;
  }
  @keyframes fade {
    to {
      opacity: 0.4;
    }
  }
  /* Últimos 60 días */
  .strip {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 14px;
  }
  .days {
    display: grid;
    grid-template-columns: repeat(30, minmax(0, 1fr));
    gap: 4px;
  }
  .d {
    display: block;
    aspect-ratio: 1;
    min-width: 0;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: var(--surface-3);
  }
  button.d:disabled {
    cursor: default;
  }
  .d.has {
    background: color-mix(in srgb, var(--accent) calc(var(--o) * 100%), var(--surface-3));
  }
  button.d.has {
    cursor: pointer;
    transition: transform 0.1s;
  }
  button.d.has:hover {
    transform: scale(1.25);
  }
  /* Día revisado sin cambios: sin versión nueva, pero al día (no es un hueco). */
  .d.checked {
    box-shadow: inset 0 0 0 1.5px color-mix(in srgb, var(--accent) 60%, transparent);
  }
  .d.on {
    outline: 2px solid var(--text-1);
    outline-offset: 1px;
  }
  .strip-foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: var(--fs-xs);
  }
  .legend {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .legend .d {
    width: 10px;
  }
  @media (prefers-reduced-motion: reduce) {
    button.d.has {
      transition: none;
    }
  }
</style>
