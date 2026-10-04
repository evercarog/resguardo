<script lang="ts">
  import type { Snapshot } from "$lib/api";
  import { formatBytes, formatDate, formatDuration } from "$lib/format";

  let { snapshots }: { snapshots: Snapshot[] } = $props();

  const LIMIT = 30;

  /** Últimas copias con resumen (restic 0.17+), de la más antigua a la más reciente. */
  const items = $derived(
    snapshots
      .filter((s) => s.summary?.backup_start && s.summary?.backup_end)
      .sort((a, b) => a.time.localeCompare(b.time))
      .slice(-LIMIT)
      .map((s) => ({
        id: s.id,
        time: s.time,
        seconds: Math.max(0, (new Date(s.summary!.backup_end!).getTime() - new Date(s.summary!.backup_start!).getTime()) / 1000),
        added: s.summary!.data_added_packed ?? 0,
      })),
  );

  const last = $derived(items.at(-1));
  const avg = $derived(items.length ? items.reduce((n, i) => n + i.seconds, 0) / items.length : 0);
  const longest = $derived(items.reduce<(typeof items)[number] | null>((a, i) => (!a || i.seconds > a.seconds ? i : a), null));
  const avgAdded = $derived(items.length ? items.reduce((n, i) => n + i.added, 0) / items.length : 0);
  /** Si la última copia tardó bastante más de lo normal, se señala. */
  const slower = $derived(!!last && items.length >= 4 && last.seconds > avg * 1.5 && last.seconds - avg > 30);

  const CHARTS = [
    { key: "added", title: "Nuevo en disco por copia", format: (v: number) => formatBytes(v) },
    { key: "seconds", title: "Duración por copia", format: (v: number) => formatDuration(v) },
  ] as const;

  let hover = $state<{ chart: string; index: number } | null>(null);
</script>

{#if items.length >= 2}
  <section class="card activity">
    <header>
      <h2>Actividad</h2>
      <span class="faint">últimas {items.length} copias</span>
    </header>

    <dl class="tiles">
      <div>
        <dt>Última copia tardó</dt>
        <dd>{formatDuration(last?.seconds)}</dd>
        {#if slower}<span class="note warn">más lenta de lo habitual</span>{/if}
      </div>
      <div>
        <dt>Duración media</dt>
        <dd>{formatDuration(avg)}</dd>
      </div>
      <div title={longest ? formatDate(longest.time) : ""}>
        <dt>La más larga</dt>
        <dd>{formatDuration(longest?.seconds)}</dd>
        {#if longest}<span class="note">{formatDate(longest.time)}</span>{/if}
      </div>
      <div>
        <dt>Nuevo en disco (media)</dt>
        <dd>{formatBytes(avgAdded)}</dd>
      </div>
    </dl>

    <div class="charts">
      {#each CHARTS as chart}
        {@const values = items.map((i) => i[chart.key])}
        {@const max = Math.max(...values, 1)}
        <figure>
          <figcaption>
            <span>{chart.title}</span>
            <span class="faint">máx. {chart.format(max)}</span>
          </figcaption>
          <div class="plot" role="img" aria-label="{chart.title} en las últimas {items.length} copias" onmouseleave={() => (hover = null)}>
            {#each items as item, i (item.id)}
              <button
                class="col"
                class:on={hover?.chart === chart.key && hover.index === i}
                onmouseenter={() => (hover = { chart: chart.key, index: i })}
                onfocus={() => (hover = { chart: chart.key, index: i })}
                aria-label="{formatDate(item.time)}: {chart.format(values[i])}"
              >
                <span class="bar" style:height="{Math.max(2, (values[i] / max) * 100)}%" style:--i={i}></span>
              </button>
            {/each}
            {#if hover?.chart === chart.key}
              {@const item = items[hover.index]}
              <div class="tip" style:left="{((hover.index + 0.5) / items.length) * 100}%">
                <strong>{chart.format(values[hover.index])}</strong>
                <span>{formatDate(item.time)}</span>
              </div>
            {/if}
          </div>
          <div class="axis faint">
            <span>{formatDate(items[0].time)}</span>
            <span>{formatDate(items.at(-1)!.time)}</span>
          </div>
        </figure>
      {/each}
    </div>
  </section>
{/if}

<style>
  .activity {
    padding: 20px 22px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  h2 {
    font-size: var(--fs-h2);
    font-weight: 650;
  }
  header .faint {
    font-size: var(--fs-sm);
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
    margin: 0;
  }
  .tiles > div {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    padding: 10px 14px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  dt {
    font-size: var(--fs-xs);
    font-weight: 650;
    color: var(--text-3);
  }
  dd {
    margin: 0;
    font-family: var(--font-display);
    font-size: 17px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }
  .note {
    font-size: var(--fs-xs);
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .note.warn {
    color: var(--warn);
  }
  .charts {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 24px;
  }
  figure {
    margin: 0;
    min-width: 0;
  }
  figcaption {
    display: flex;
    justify-content: space-between;
    margin-bottom: 8px;
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--text-2);
  }
  figcaption .faint {
    font-weight: 400;
    font-variant-numeric: tabular-nums;
  }
  .plot {
    position: relative;
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 88px;
    border-bottom: 1px solid var(--border-strong);
  }
  .col {
    flex: 1;
    display: flex;
    align-items: flex-end;
    height: 100%;
    padding: 0;
    background: none;
    border: none;
    cursor: default;
  }
  .bar {
    width: 100%;
    border-radius: 4px 4px 0 0;
    background: var(--accent);
    opacity: 0.75;
    transform-origin: bottom;
    animation: grow 0.45s cubic-bezier(0.2, 0.8, 0.2, 1) both;
    animation-delay: calc(var(--i) * 12ms);
    transition: opacity 0.12s;
  }
  .col.on .bar,
  .col:focus-visible .bar {
    opacity: 1;
  }
  .plot:hover .col:not(.on) .bar {
    opacity: 0.45;
  }
  @keyframes grow {
    from {
      transform: scaleY(0);
    }
  }
  .tip {
    position: absolute;
    bottom: calc(100% + 6px);
    translate: -50% 0;
    z-index: 2;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 5px 9px;
    font-size: var(--fs-xs);
    white-space: nowrap;
    color: var(--text-1);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-md);
    pointer-events: none;
  }
  .tip strong {
    font-variant-numeric: tabular-nums;
  }
  .tip span {
    color: var(--text-3);
    font-size: var(--fs-xs);
  }
  .axis {
    display: flex;
    justify-content: space-between;
    margin-top: 5px;
    font-size: var(--fs-xs);
  }
  @media (max-width: 900px) {
    .tiles {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .charts {
      grid-template-columns: 1fr;
    }
  }
</style>
