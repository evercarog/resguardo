<script lang="ts">
  import { ChevronLeft, ChevronRight } from "@lucide/svelte";

  interface Props {
    /** Fechas (ISO) de los snapshots, para marcar los días con copia. */
    times: string[];
    /** Día inicial y final del filtro, como "AAAA-MM-DD". */
    start: string | null;
    end: string | null;
    onselect: (start: string | null, end: string | null) => void;
  }
  let { times, start, end, onselect }: Props = $props();

  const pad = (n: number) => String(n).padStart(2, "0");
  const keyOf = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  const today = keyOf(new Date());

  /** Número de snapshots por día (hora local). */
  const counts = $derived.by(() => {
    const map = new Map<string, number>();
    for (const t of times) {
      const k = keyOf(new Date(t));
      map.set(k, (map.get(k) ?? 0) + 1);
    }
    return map;
  });
  const maxCount = $derived(Math.max(1, ...counts.values()));

  // Mes visible: el del último snapshot (o el actual).
  const latest = $derived(times.reduce((a, t) => (t > a ? t : a), ""));
  const monthStart = (d: Date) => new Date(d.getFullYear(), d.getMonth(), 1);
  // svelte-ignore state_referenced_locally
  let view = $state(monthStart(latest ? new Date(latest) : new Date()));

  const monthLabel = $derived(
    new Intl.DateTimeFormat("es", { month: "long", year: "numeric" }).format(view).replace(/^./, (c) => c.toUpperCase()),
  );

  /** 6 semanas empezando en lunes. */
  const cells = $derived.by(() => {
    const first = new Date(view.getFullYear(), view.getMonth(), 1);
    const offset = (first.getDay() + 6) % 7; // lunes = 0
    const startDay = new Date(first.getFullYear(), first.getMonth(), 1 - offset);
    return Array.from({ length: 42 }, (_, i) => {
      const d = new Date(startDay.getFullYear(), startDay.getMonth(), startDay.getDate() + i);
      return { key: keyOf(d), day: d.getDate(), inMonth: d.getMonth() === view.getMonth() };
    });
  });

  let hover = $state<string | null>(null);

  // Rango efectivo (incluye la vista previa al pasar el ratón mientras se elige el final).
  const range = $derived.by(() => {
    if (!start) return null;
    const other = end ?? (hover && !end ? hover : start);
    return start <= other ? [start, other] : [other, start];
  });

  function pick(key: string) {
    if (!start || end) onselect(key, null); // empieza una selección nueva
    else if (key === start) onselect(key, key); // mismo día: un solo día
    else onselect(...((key < start ? [key, start] : [start, key]) as [string, string]));
  }

  const move = (months: number) => (view = new Date(view.getFullYear(), view.getMonth() + months, 1));
  const goToday = () => (view = new Date(new Date().getFullYear(), new Date().getMonth(), 1));

  const WEEKDAYS = ["L", "M", "X", "J", "V", "S", "D"];
</script>

<div class="cal" role="group" aria-label="Calendario de versiones">
  <div class="head">
    <button class="icon-btn" onclick={() => move(-1)} title="Mes anterior" aria-label="Mes anterior"><ChevronLeft size={16} /></button>
    <button class="month" onclick={goToday} title="Ir al mes actual">{monthLabel}</button>
    <button class="icon-btn" onclick={() => move(1)} title="Mes siguiente" aria-label="Mes siguiente"><ChevronRight size={16} /></button>
  </div>
  <div class="grid">
    {#each WEEKDAYS as w}<span class="wd">{w}</span>{/each}
    {#each cells as c (c.key)}
      {@const n = counts.get(c.key) ?? 0}
      {@const inRange = !!range && c.key >= range[0] && c.key <= range[1]}
      {@const edge = !!range && (c.key === range[0] || c.key === range[1])}
      <button
        class="day"
        class:out={!c.inMonth}
        class:future={c.key > today}
        class:today={c.key === today}
        class:in-range={inRange}
        class:edge
        class:has={n > 0}
        onclick={() => pick(c.key)}
        onmouseenter={() => (hover = c.key)}
        onmouseleave={() => (hover = null)}
        title={n ? `${n} ${n === 1 ? "versión" : "versiones"}` : "Sin versiones"}
        aria-pressed={inRange}
      >
        <span class="num">{c.day}</span>
        {#if n}
          <span class="dot" style:opacity={0.45 + 0.55 * (n / maxCount)}></span>
        {/if}
      </button>
    {/each}
  </div>
</div>

<style>
  .cal {
    width: 280px;
    flex: none;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 6px;
  }
  .month {
    font: inherit;
    font-weight: 650;
    color: var(--text-1);
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    padding: 4px 8px;
    cursor: pointer;
  }
  .month:hover {
    background: var(--surface-3);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 2px;
  }
  .wd {
    text-align: center;
    font-size: var(--fs-overline);
    font-weight: 650;
    color: var(--text-3);
    padding: 4px 0;
  }
  .day {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    height: 36px;
    padding: 0;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-2);
    background: none;
    border: none;
    border-radius: var(--radius);
    cursor: pointer;
    transition: background 0.1s;
  }
  .day:hover {
    background: var(--surface-3);
  }
  .day.has .num {
    color: var(--text-1);
    font-weight: 650;
  }
  .day.out {
    opacity: 0.35;
  }
  .day.future {
    color: var(--text-3);
  }
  .day.today .num {
    text-decoration: underline;
    text-decoration-color: var(--accent);
    text-decoration-thickness: 2px;
    text-underline-offset: 3px;
  }
  .day.in-range {
    background: var(--accent-soft);
    border-radius: 0;
  }
  .day.edge {
    background: var(--accent);
    border-radius: var(--radius);
  }
  .day.edge .num {
    color: var(--accent-contrast);
  }
  .dot {
    display: grid;
    place-items: center;
    min-width: 6px;
    height: 6px;
    border-radius: 999px;
    background: var(--accent);
  }
  .day.edge .dot {
    background: var(--accent-contrast);
  }
</style>
