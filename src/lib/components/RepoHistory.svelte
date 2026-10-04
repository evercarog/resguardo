<script lang="ts">
  import { onMount } from "svelte";
  import { CircleAlert, History, LoaderCircle, RefreshCw } from "@lucide/svelte";
  import type { ActivityEntry, Repo } from "$lib/api";
  import { activity, KIND_ICON, kindLabel, ORIGIN_LABEL, pollActivity, refreshActivity, RESULT_ICON, RESULT_LABEL } from "$lib/activity.svelte";
  import { formatBytes, formatDate, formatDay, formatTime } from "$lib/format";

  // «Historia» de un destino: todo lo que le pasó en una sola línea de tiempo
  // (copias a mano, automáticas y a distancia, subidas, verificaciones,
  // pruebas de restauración, pausas, frenos, kits y cambios de configuración).
  interface Props {
    repo: Repo;
  }
  let { repo }: Props = $props();

  type Group = "all" | "backups" | "uploads" | "checks" | "pauses" | "changes";
  const GROUPS: { id: Group; label: string; kinds: ActivityEntry["kind"][] }[] = [
    { id: "all", label: "Todo", kinds: [] },
    { id: "backups", label: "Copias", kinds: ["backup"] },
    { id: "uploads", label: "Subidas", kinds: ["offsite"] },
    { id: "checks", label: "Comprobaciones", kinds: ["verify", "verify_offsite", "restore_test"] },
    { id: "pauses", label: "Pausas y frenos", kinds: ["pause", "resume", "guard"] },
    { id: "changes", label: "Cambios", kinds: ["config", "kit"] },
  ];
  const PAGE = 40;

  let group = $state<Group>("all");
  let problems = $state(false);
  let limit = $state(PAGE);

  onMount(() => pollActivity());

  const mine = $derived(activity.entries.filter((e) => e.repo_id === repo.id));
  const inGroup = (e: ActivityEntry, g: Group) => g === "all" || GROUPS.find((x) => x.id === g)!.kinds.includes(e.kind);
  const shown = $derived(mine.filter((e) => inGroup(e, group) && (!problems || e.result === "warning" || e.result === "error")));
  const counts = $derived(Object.fromEntries(GROUPS.map((g) => [g.id, mine.filter((e) => inGroup(e, g.id)).length])) as Record<Group, number>);
  const page = $derived(shown.slice(0, limit));
  /** Por días, como en «Actividad». */
  const days = $derived.by(() => {
    const out: { day: string; items: ActivityEntry[] }[] = [];
    for (const e of page) {
      const day = formatDay(e.finished);
      const last = out.at(-1);
      if (last?.day === day) last.items.push(e);
      else out.push({ day, items: [e] });
    }
    return out;
  });

  /** Quién o qué lo hizo. */
  function by(e: ActivityEntry) {
    if (e.kind === "config" || e.kind === "kit") return e.user ? `por ${e.user}` : "";
    if (e.origin === "remote") return `pedida desde ${e.requested_from ?? "la web"}`;
    if (e.origin === "manual") return e.user ? `a mano, por ${e.user}` : "a mano";
    return (ORIGIN_LABEL[e.origin] ?? e.origin).toLowerCase();
  }

  function choose(g: Group) {
    group = g;
    limit = PAGE;
  }
</script>

<section class="history">
  <div class="filters" role="group" aria-label="Qué mostrar">
    {#each GROUPS as g (g.id)}
      <button class="chip" class:on={group === g.id} aria-pressed={group === g.id} onclick={() => choose(g.id)} disabled={g.id !== "all" && !counts[g.id]}>
        {g.label} <span class="n">{counts[g.id]}</span>
      </button>
    {/each}
    <label class="only-problems"><input type="checkbox" bind:checked={problems} /> Solo problemas</label>
    <button class="icon-btn" title="Actualizar" aria-label="Actualizar la historia" onclick={() => refreshActivity()} disabled={activity.loading}>
      {#if activity.loading}<span class="spin" style="display:grid"><LoaderCircle size={15} /></span>{:else}<RefreshCw size={15} />{/if}
    </button>
  </div>

  {#if activity.error && !mine.length}
    <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>No se pudo leer el historial: {activity.error}</p></div>
  {:else if !activity.loadedAt && activity.loading}
    <div class="loading">
      {#each [0, 1, 2] as i}<div class="sk" style:animation-delay="{i * 90}ms"></div>{/each}
    </div>
  {:else if !shown.length}
    <div class="empty-state">
      <History size={26} />
      <strong>{mine.length ? "Nada con estos filtros" : "Aún no hay nada en la historia de este repositorio"}</strong>
      <p>
        {mine.length
          ? "Prueba con «Todo» o quita «Solo problemas»."
          : "Aquí irán las copias, subidas, verificaciones, pausas y cambios de configuración de este repositorio. Los cambios se anotan desde esta versión de Resguardo."}
      </p>
    </div>
  {:else}
    {#each days as d (d.day)}
      <h3 class="day">{d.day}</h3>
      <ol class="items">
        {#each d.items as e, i (`${e.finished}|${e.kind}|${e.plan_id ?? ""}|${i}`)}
          {@const KindIcon = KIND_ICON[e.kind] ?? KIND_ICON.backup}
          {@const ResIcon = RESULT_ICON[e.result] ?? RESULT_ICON.ok}
          <li class="item r-{e.result}">
            <span class="kind"><KindIcon size={15} /></span>
            <span class="txt">
              <span class="what">
                <strong>{kindLabel(e)}</strong>
                {#if e.result !== "info"}<span class="res" title={RESULT_LABEL[e.result]}><ResIcon size={13} /></span>{/if}
                {#if e.kind === "backup" && e.unchanged}<span class="faint">sin cambios</span>{:else if e.kind === "backup" && e.data_added != null}<span
                    class="faint">+{formatBytes(e.data_added)}</span
                  >{/if}
              </span>
              {#if e.message && (e.result !== "ok" || e.kind !== "backup")}<span class="msg">{e.message}</span>{/if}
            </span>
            <span class="when">
              <span title={formatDate(e.finished)}>{formatTime(e.finished)}</span>
              {#if by(e)}<span class="faint">{by(e)}</span>{/if}
            </span>
          </li>
        {/each}
      </ol>
    {/each}
    {#if shown.length > limit}
      <button class="btn more" onclick={() => (limit += PAGE)}>Mostrar más ({shown.length - limit} más)</button>
    {/if}
  {/if}
</section>

<style>
  .history {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px 22px 18px;
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-bottom: 4px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-2);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 999px;
    cursor: pointer;
  }
  .chip:hover:not(:disabled) {
    border-color: var(--border-strong);
    color: var(--text-1);
  }
  .chip.on {
    color: var(--accent-text);
    background: var(--accent-soft);
    border-color: color-mix(in srgb, var(--accent) 35%, transparent);
  }
  .chip:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .n {
    font-variant-numeric: tabular-nums;
    color: var(--text-3);
  }
  .only-problems {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .day {
    margin: 10px 0 2px;
    font-size: var(--fs-xs);
    font-weight: 650;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .items {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .item {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) auto;
    gap: 10px;
    align-items: start;
    padding: 7px 0;
    border-bottom: 1px solid var(--border);
    font-size: var(--fs-sm);
  }
  .item:last-child {
    border-bottom: none;
  }
  .kind {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    margin-top: 1px;
    border-radius: 6px;
    color: var(--text-2);
    background: var(--surface-2);
  }
  .r-error .kind {
    color: var(--bad);
    background: var(--bad-soft);
  }
  .r-warning .kind {
    color: var(--warn);
    background: var(--warn-soft);
  }
  .txt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .what {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
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
  .msg {
    color: var(--text-2);
    overflow-wrap: anywhere;
  }
  .when {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 1px;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .more {
    align-self: center;
    margin-top: 6px;
  }
  .loading {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .sk {
    height: 38px;
    border-radius: 8px;
    background: var(--surface-2);
    animation: fade-pulse 1.2s ease-in-out infinite alternate;
  }
</style>
