<script lang="ts">
  import { onMount } from "svelte";
  import { ArrowRight, History } from "@lucide/svelte";
  import type { ActivityEntry, Repo } from "$lib/api";
  import { KIND_ICON, ORIGIN_LABEL, RESULT_ICON, RESULT_LABEL, activity, durationOf, entryTarget, kindLabel, pollActivity } from "$lib/activity.svelte";
  import { formatBytes, formatDate, formatDay, formatDuration, formatTime } from "$lib/format";
  import type { Selection } from "$lib/nav";

  // Tarjeta compacta con las últimas entradas del historial de actividad
  // (en Estado: de todo; en una copia: solo de esa copia).
  interface Props {
    title: string;
    repos: Repo[];
    /** Qué entradas se muestran (del historial completo, ya ordenado). */
    select: (e: ActivityEntry) => boolean;
    limit: number;
    /** Mostrar qué fue y en qué destino (en una copia sobra: es siempre ella). */
    showWhat?: boolean;
    emptyText: string;
    onviewall: () => void;
    /** Si se da, cada fila abre su copia o destino. */
    onnavigate?: (sel: Selection) => void;
  }
  let { title, repos, select, limit, showWhat = true, emptyText, onviewall, onnavigate }: Props = $props();

  onMount(() => pollActivity(30_000));

  const items = $derived(activity.entries.filter(select).slice(0, limit));
  const problems = $derived(items.filter((e) => e.result === "error").length);

  /** «Hoy 14:05», «Ayer 09:00» o «3 sept 23:00». */
  function when(iso: string) {
    const day = formatDay(iso);
    if (day === "Hoy" || day === "Ayer") return `${day} ${formatTime(iso)}`;
    return `${new Intl.DateTimeFormat("es", { day: "numeric", month: "short" }).format(new Date(iso))} ${formatTime(iso)}`;
  }
</script>

<section class="card recent">
  <header>
    <h2 class="section-title"><History size={16} /> {title}</h2>
    {#if problems}<span class="bad-count">{problems} {problems === 1 ? "fallo" : "fallos"}</span>{/if}
    <button class="link" onclick={onviewall}>Ver todo <ArrowRight size={13} /></button>
  </header>

  {#if !activity.loadedAt && !activity.error}
    <div class="skeleton" role="status" aria-label="Cargando la actividad">{#each [0, 1, 2] as i}<div style:animation-delay="{i * 120}ms"></div>{/each}</div>
  {:else if activity.error && !activity.entries.length}
    <p class="faint small">No se pudo leer el historial: {activity.error}</p>
  {:else if !items.length}
    <p class="faint small">{emptyText}</p>
  {:else}
    <ul>
      {#each items as e (`${e.started}|${e.finished}|${e.kind}|${e.origin}|${e.repo_id}|${e.plan_id ?? ""}`)}
        {@const sel = onnavigate ? entryTarget(e, repos) : null}
        {@const ResIcon = RESULT_ICON[e.result] ?? RESULT_ICON.ok}
        {@const KindIcon = KIND_ICON[e.kind] ?? KIND_ICON.backup}
        {@const secs = durationOf(e)}
        <li class="r-{e.result}">
          <svelte:element
            this={sel ? "button" : "div"}
            class="row"
            role={sel ? undefined : "group"}
            onclick={sel ? () => onnavigate?.(sel) : undefined}
            title={sel ? (sel.kind === "copy" ? `Abrir la copia «${e.plan_name}»` : `Abrir el repositorio «${e.repo_name}»`) : undefined}
          >
            <span class="res" title={RESULT_LABEL[e.result]}><ResIcon size={15} /><span class="sr-only">{RESULT_LABEL[e.result]}</span></span>
            <span class="when" title={formatDate(e.started)}>{when(e.started)}</span>
            <span class="what">
              {#if showWhat}
                <span class="kind-icon"><KindIcon size={13} /></span>
                <span class="label" title={kindLabel(e)}>{kindLabel(e)}</span>
                <span class="faint dest" title={e.repo_name}>· {e.repo_name}</span>
              {:else}
                <span class="faint">{secs != null ? formatDuration(secs) : ""}</span>
              {/if}
            </span>
            <span class="origin o-{e.origin}" title={e.origin === "remote" ? `Pedida a distancia desde ${e.requested_from ?? "la web"}` : undefined}>{ORIGIN_LABEL[e.origin] ?? e.origin}</span>
            {#if e.unchanged}
              <span class="added same" title={e.message}>Sin cambios</span>
            {:else}
              <span class="added">{e.kind === "backup" && e.data_added != null ? `+${formatBytes(e.data_added)}` : ""}</span>
            {/if}
            {#if e.result !== "ok" && e.message}
              <span class="msg" title={e.message}>{e.message}</span>
            {/if}
          </svelte:element>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .recent {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px 22px 14px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
  }
  h2 :global(svg) {
    color: var(--text-3);
  }
  .bad-count {
    padding: 0 8px;
    font-size: var(--fs-xs);
    font-weight: 650;
    line-height: 20px;
    border-radius: 999px;
    color: var(--bad);
    background: var(--bad-soft);
  }
  .link {
    flex: none;
  }
  ul {
    list-style: none;
    margin: 0 -8px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .row {
    display: grid;
    grid-template-columns: 16px 100px minmax(0, 1fr) auto 76px;
    align-items: center;
    column-gap: 10px;
    row-gap: 1px;
    width: 100%;
    padding: 6px 8px;
    font: inherit;
    font-size: var(--fs-sm);
    text-align: left;
    color: var(--text-1);
    background: none;
    border: none;
    border-radius: var(--radius-sm);
  }
  button.row {
    cursor: pointer;
  }
  button.row:hover {
    background: var(--surface-2);
  }
  .r-error .row {
    background: var(--bad-soft);
  }
  .r-error button.row:hover {
    background: color-mix(in srgb, var(--bad) 16%, transparent);
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
  .when {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .what {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    white-space: nowrap;
  }
  .label {
    flex: 0 1 auto;
    min-width: 0;
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .kind-icon {
    display: grid;
    flex: none;
    color: var(--text-3);
  }
  .dest {
    /* Se recorta antes el destino que el nombre. */
    flex: 0 100 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .origin {
    padding: 0 7px;
    font-size: var(--fs-overline);
    font-weight: 550;
    line-height: 18px;
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
  .added {
    font-size: var(--fs-xs);
    font-weight: 600;
    text-align: right;
    color: var(--accent-text);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  /* Copia sin cambios: correcta, sin versión nueva. */
  .added.same {
    font-weight: 500;
    color: var(--text-3);
  }
  .msg {
    grid-column: 3 / -1;
    font-size: var(--fs-xs);
    color: var(--bad);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .r-warning .msg {
    color: var(--warn);
  }
  .small {
    margin: 0;
    font-size: var(--fs-sm);
  }
</style>
