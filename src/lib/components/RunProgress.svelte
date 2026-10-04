<script lang="ts">
  import { slide } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { ChevronDown, CircleAlert, CircleCheck, Info, ShieldCheck, Square, TriangleAlert } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Repo } from "$lib/api";
  import { agent } from "$lib/agent.svelte";
  import { runs, cancelBackup } from "$lib/backups.svelte";
  import { formatBytes, formatDuration, formatNumber } from "$lib/format";
  import RepoErrorActions from "./RepoErrorActions.svelte";

  // Progreso y resultado de la copia a mano de un destino (una a la vez por
  // destino). Con `planId`, solo se muestra si es de esa copia.
  interface Props {
    repo: Repo;
    planId?: string;
    /** Mostrar qué copia se está haciendo («Copiando «Laboral»…»). */
    showName?: boolean;
    /** Ocultar el resultado final (solo el progreso). */
    progressOnly?: boolean;
  }
  let { repo, planId, showName = false, progressOnly = false }: Props = $props();

  let showErrors = $state(false);
  let cancelling = $state(false);
  let error = $state("");

  const run = $derived(runs[repo.id] && (!planId || runs[repo.id].planId === planId) ? runs[repo.id] : null);
  const running = $derived(run?.running ?? false);
  const percent = $derived(Math.min(100, Math.max(0, (run?.status?.percent ?? 0) * 100)));

  // Al empezar otra copia se reinicia el estado del botón.
  $effect(() => {
    if (running) {
      cancelling = false;
      error = "";
      showErrors = false;
    }
  });

  // Qué hacer con los archivos que no se pudieron leer, según el motivo.
  const IN_USE = /utilizado por otro proceso|used by another process|being used|bloqueado|locked a portion/i;
  const DENIED = /acceso denegado|access is denied|permission denied/i;
  const errorHint = $derived.by(() => {
    if (!run || run.running || !run.errorCount) return null;
    const admin = !!agent.info?.elevated;
    const inUse = run.errors.some((e) => IN_USE.test(e));
    const denied = run.errors.some((e) => DENIED.test(e));
    if (inUse && !admin)
      return {
        text: "Algún archivo estaba abierto en otro programa (por ejemplo, Outlook). Ciérralo y vuelve a copiar, o abre Resguardo como administrador: así se copian también los archivos abiertos.",
        admin: true,
      };
    if (inUse) return { text: "Algún archivo estaba abierto en otro programa y no se pudo leer ni con la instantánea del disco. Ciérralo y vuelve a copiar.", admin: false };
    if (denied && !admin)
      return {
        text: "Windows no deja leer algunos archivos (de otro usuario o del sistema). Si los necesitas, abre Resguardo como administrador; si no, exclúyelos de la copia.",
        admin: true,
      };
    return { text: "El resto se guardó bien. Si vuelve a pasar con los mismos archivos, revisa que existan y se puedan abrir, o exclúyelos de la copia.", admin: false };
  });
  let relaunchError = $state("");
  async function relaunch() {
    relaunchError = "";
    try {
      await api.relaunchAsAdmin();
    } catch (e) {
      relaunchError = String(e);
    }
  }

  async function cancel() {
    cancelling = true;
    try {
      await cancelBackup(repo.id);
    } catch (e) {
      error = String(e);
      cancelling = false;
    }
  }
</script>

{#if running && run}
  <div class="progress-block" transition:slide={{ duration: dur(200) }}>
    <div class="progress-top">
      <span class="percent">
        {#if run.status}{percent.toFixed(0)}<small>%</small>{:else}<span class="preparing">Preparando…</span>{/if}
      </span>
      <span class="right">
        {#if run.status?.seconds_remaining != null}
          <span class="faint">Quedan {formatDuration(run.status.seconds_remaining)}</span>
        {/if}
        <button class="btn btn-sm" onclick={cancel} disabled={cancelling}>
          <Square size={11} fill="currentColor" />
          {cancelling ? "Deteniendo…" : "Detener"}
        </button>
      </span>
    </div>
    {#if showName && run.planName}<p class="copying">Copiando «{run.planName}»…</p>{/if}
    <div class="track" role="progressbar" aria-label="Progreso de la copia" aria-valuenow={Math.round(percent)} aria-valuemin={0} aria-valuemax={100}>
      <div class="fill" class:indeterminate={!run.status} style:width={run.status ? `${percent}%` : undefined}></div>
    </div>
    {#if run.status}
      <dl class="stats">
        <div>
          <dt>Archivos</dt>
          <dd>{formatNumber(run.status.files_done)} <span class="faint">/ {formatNumber(run.status.total_files)}</span></dd>
        </div>
        <div>
          <dt>Datos</dt>
          <dd>{formatBytes(run.status.bytes_done)} <span class="faint">/ {formatBytes(run.status.total_bytes)}</span></dd>
        </div>
        <div>
          <dt>Errores</dt>
          <dd class:has-errors={run.errorCount > 0}>{run.errorCount}</dd>
        </div>
      </dl>
      {#if run.status.current}
        <p class="current mono" title={run.status.current}>{run.status.current}</p>
      {/if}
    {/if}
  </div>
{/if}

{#if error}
  <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>
{/if}

{#if !progressOnly}
  <div class="live" aria-live="polite">
    {#if run && !running}
      {#if run.result}
        {@const s = run.result.summary}
        <div class="notice {run.result.incomplete ? 'notice-warn' : 'notice-success'}" transition:slide={{ duration: dur(200) }}>
          {#if run.result.incomplete}<TriangleAlert size={17} />{:else}<CircleCheck size={17} />{/if}
          <div class="result">
            <strong>{showName && run.planName ? `Copia de «${run.planName}»` : "Copia"} {run.result.incomplete ? "terminada con avisos" : "completada"}</strong>
            {#if run.result.unchanged}
              <!-- «Solo guardar si hay cambios»: todo correcto, pero no hacía falta una versión nueva. -->
              <p class="muted">
                No había cambios: no hizo falta guardar una versión nueva.{#if s}&nbsp;Se revisaron {formatNumber(s.total_files_processed)}
                  archivos en {formatDuration(s.total_duration)}.{/if}
              </p>
            {:else if s}
              <p class="muted">
                {formatNumber(s.files_new)} nuevos · {formatNumber(s.files_changed)} modificados ·
                {formatNumber(s.files_unmodified)} sin cambios — {formatBytes(s.data_added)} añadidos en
                {formatDuration(s.total_duration)}{#if s.snapshot_id}&nbsp;· versión <span class="mono">{s.snapshot_id.slice(0, 8)}</span>{/if}
              </p>
            {/if}
          </div>
        </div>
      {:else if run.failure}
        <div class="notice {run.cancelled ? 'notice-info' : 'notice-danger'}" role="alert" transition:slide={{ duration: dur(200) }}>
          <CircleAlert size={17} />
          <div class="failure">
            <p>{run.cancelled ? "Copia detenida: no se guardó ninguna versión nueva." : run.failure}</p>
            {#if !run.cancelled}<RepoErrorActions repoId={repo.id} repoName={repo.name} message={run.failure} />{/if}
          </div>
        </div>
      {/if}
    {/if}
  </div>

  {#if run && run.errorCount > 0 && (!running || showErrors)}
    <div class="errors-block">
      <button class="toggle" onclick={() => (showErrors = !showErrors)} aria-expanded={showErrors}>
        <span class="chev" class:open={showErrors}><ChevronDown size={15} /></span>
        {run.errorCount}
        {run.errorCount === 1 ? "archivo no se pudo leer" : "archivos no se pudieron leer"}
      </button>
      {#if showErrors}
        <ul class="errors mono selectable" transition:slide={{ duration: dur(180) }}>
          {#each run.errors as err}<li>{err}</li>{/each}
          {#if run.errorCount > run.errors.length}<li class="faint">… y {run.errorCount - run.errors.length} más</li>{/if}
        </ul>
      {/if}
      {#if errorHint}
        <p class="hint">
          <Info size={14} />
          <span>
            {errorHint.text}
            {#if errorHint.admin}
              <button class="link" onclick={relaunch}><ShieldCheck size={13} /> Abrir como administrador</button>
            {/if}
            {#if relaunchError}<span class="err">{relaunchError}</span>{/if}
          </span>
        </p>
      {/if}
    </div>
  {/if}
{/if}

<style>
  .failure {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  /* Región viva para lectores de pantalla; no ocupa hueco en el flex. */
  .live {
    display: contents;
  }
  .copying {
    margin: 0;
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--accent-text);
  }
  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 2px 4px 2px 0;
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 550;
    color: var(--text-2);
    background: none;
    border: none;
    cursor: pointer;
  }
  .hint {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 6px 0 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .hint :global(svg) {
    flex: none;
    margin-top: 2px;
  }
  .hint .link {
    margin-left: 4px;
    vertical-align: baseline;
  }
  .hint .err {
    display: block;
    color: var(--bad);
  }
  .toggle:hover {
    color: var(--text-1);
  }
  .chev {
    display: grid;
    transition: transform 0.2s;
    transform: rotate(-90deg);
  }
  .chev.open {
    transform: none;
  }
  .progress-block {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .progress-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .right {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: var(--fs-sm);
  }
  .percent {
    font-family: var(--font-display);
    font-size: 34px;
    font-weight: 650;
    line-height: 1;
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
  }
  .percent small {
    font-size: 17px;
    font-weight: 600;
    margin-left: 2px;
    color: var(--text-2);
  }
  .preparing {
    font-size: var(--fs-h2);
    font-weight: 600;
    color: var(--text-2);
  }
  .track {
    position: relative;
    height: 8px;
    border-radius: 999px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .fill {
    position: relative;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, color-mix(in srgb, var(--accent) 70%, white), var(--accent));
    transition: width 0.25s ease-out;
    overflow: hidden;
  }
  .fill::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, transparent, rgb(255 255 255 / 0.35), transparent);
    transform: translateX(-100%);
    animation: shine 1.8s ease-in-out infinite;
  }
  @keyframes shine {
    to {
      transform: translateX(100%);
    }
  }
  .fill.indeterminate {
    width: 35%;
    animation: slide 1.3s ease-in-out infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(290%);
    }
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
    margin: 0;
  }
  .stats div {
    padding: 10px 12px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  dt {
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  dd {
    margin: 2px 0 0;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  dd .faint {
    font-weight: 400;
  }
  .has-errors {
    color: var(--warn);
  }
  .current {
    margin: 0;
    font-size: var(--fs-xs);
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .result {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .result p {
    font-size: var(--fs-sm);
  }
  .errors {
    margin: 8px 0 0;
    padding: 10px 12px 10px 28px;
    max-height: 180px;
    overflow: auto;
    font-size: var(--fs-xs);
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .errors li + li {
    margin-top: 4px;
  }
</style>
