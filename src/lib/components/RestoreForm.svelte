<script lang="ts">
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { desktopDir, downloadDir } from "@tauri-apps/api/path";
  import { open } from "@tauri-apps/plugin-dialog";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import {
    ArrowLeft,
    ArrowRight,
    ChevronDown,
    CircleAlert,
    CircleCheck,
    FolderInput,
    FolderOpen,
    FolderPlus,
    History,
    Lock,
    PackageOpen,
    RotateCcw,
    Square,
    TriangleAlert,
  } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Repo, Snapshot } from "$lib/api";
  import { formatBytes, formatDate, formatDuration, formatNumber } from "$lib/format";
  import { displayPath, isWindowsSnapshot } from "$lib/paths";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { cancelRestore, clearRestore, restores, startRestore } from "$lib/restores.svelte";
  import HelpLink from "./HelpLink.svelte";

  // Dónde y cómo restaurar, el progreso y el resultado. Lo usan el diálogo
  // «Restaurar» del explorador de versiones y el asistente de restauración.
  interface Props {
    repo: Repo;
    snapshot: Snapshot;
    dir: string;
    /** Nombres dentro de `dir`; vacío = todo el contenido. */
    names: string[];
    /** Descripción de lo que se restaura («informe.docx», «3 elementos de Documentos»…). */
    label: string;
    /** Asistente: primero las opciones y después un resumen para confirmar. */
    guided?: boolean;
    /** true mientras se restaura (para no cerrar la ventana). */
    running?: boolean;
    /** Etapa actual, para el indicador de pasos del asistente. */
    onstage?: (stage: "options" | "review" | "run") => void;
    /** Volver a elegir los archivos (asistente). */
    onback?: () => void;
    /** Restaurar otra cosa al terminar (asistente). */
    onagain?: () => void;
    /** Seguir en segundo plano: se cierra la ventana y se avisa al terminar. */
    onbackground: () => void;
    onclose: () => void;
    ondone: () => void;
  }
  let {
    repo,
    snapshot,
    dir,
    names,
    label,
    guided = false,
    running = $bindable(false),
    onstage,
    onback,
    onagain,
    onbackground,
    onclose,
    ondone,
  }: Props = $props();

  let where = $state<"other" | "original">("other");
  let otherTarget = $state("");
  let targetInfo = $state<{ exists: boolean; empty: boolean } | null>(null);
  let overwrite = $state(false);
  let reviewing = $state(false);
  let starting = $state(false);
  let error = $state("");
  let showErrors = $state(false);
  let started = $state(false);
  let cancelling = $state(false);

  /**
   * Ubicación original: la carpeta del snapshot convertida en ruta del sistema.
   * El backend restaura los elementos de `dir` directamente dentro del destino,
   * así que con destino = esa carpeta cada elemento vuelve a su sitio. Solo si
   * la versión es del mismo sistema (Windows/otros) y, en Windows, si la carpeta
   * está dentro de una unidad (desde la raíz `/` no hay una ruta equivalente).
   */
  const originalTarget = $derived.by(() => {
    const windowsStyle = isWindowsSnapshot(snapshot.paths);
    const onWindows = typeof navigator !== "undefined" && /windows/i.test(navigator.userAgent);
    if (windowsStyle !== onWindows) return null;
    if (windowsStyle) return /^\/[A-Za-z](\/|$)/.test(dir) ? displayPath(dir, true) : null;
    // En macOS/Linux, nunca restaurar directamente en la raíz del sistema.
    return dir === "/" ? null : dir;
  });
  const originalReason = $derived(
    originalTarget
      ? ""
      : isWindowsSnapshot(snapshot.paths) && dir === "/"
        ? "No disponible desde la raíz de la versión: entra antes en una unidad o carpeta."
        : "Esta versión se hizo en otro sistema operativo: restaura en otra carpeta.",
  );
  const target = $derived(where === "original" ? (originalTarget ?? "") : otherTarget.trim());

  // Solo mostramos el estado de la restauración que se lanzó desde aquí.
  const run = $derived(started ? restores[repo.id] : undefined);
  const percent = $derived(Math.min(100, Math.max(0, (run?.status?.percent ?? 0) * 100)));
  const hasExisting = $derived(!!targetInfo?.exists && !targetInfo.empty);
  const replace = $derived(hasExisting && overwrite);
  const busyElsewhere = $derived(!started && !!restores[repo.id]?.running);

  $effect(() => {
    running = run?.running ?? false;
  });
  $effect(() => {
    onstage?.(started ? "run" : reviewing ? "review" : "options");
  });

  const pad = (n: number) => String(n).padStart(2, "0");
  /** «Restaurado 2026-09-28 14.30»: la fecha de la versión, sin caracteres que Windows no admite. */
  function defaultFolderName() {
    const d = new Date(snapshot.time);
    return `Restaurado ${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}.${pad(d.getMinutes())}`;
  }

  onMount(async () => {
    clearRestore(repo.id); // resultado de una restauración anterior
    for (const base of [desktopDir, downloadDir]) {
      try {
        const folder = await base();
        const sep = folder.includes("\\") ? "\\" : "/";
        otherTarget = `${folder.replace(/[\\/]+$/, "")}${sep}${defaultFolderName()}`;
        return;
      } catch {
        /* se prueba la siguiente; si ninguna, el usuario elige */
      }
    }
  });

  // Revisa el destino (con una pequeña espera mientras se escribe).
  $effect(() => {
    const path = target;
    targetInfo = null;
    if (!path) return;
    const t = setTimeout(async () => {
      try {
        targetInfo = await api.inspectTarget(path);
      } catch {
        targetInfo = null;
      }
    }, 250);
    return () => clearTimeout(t);
  });

  async function browse() {
    const picked = await open({ directory: true, title: "Carpeta donde restaurar" });
    if (typeof picked === "string") {
      otherTarget = picked;
      where = "other";
    }
  }

  async function start() {
    error = "";
    let password: string | null = null;
    if (replace) {
      password = await withPassword({
        title: "Reemplazar archivos existentes",
        message: `Los archivos de «${target}» que coincidan con los de la versión se sobrescribirán. Esta acción no se puede deshacer.`,
        repoName: repo.name,
        confirmLabel: "Reemplazar y restaurar",
        danger: true,
        action: (pw) => api.checkPassword(repo.id, pw),
      });
      if (!password) return;
    }
    starting = true;
    started = true;
    const done = startRestore(repo.id, repo.name, { snapshot: snapshot.id, dir, names, target, overwrite: replace, password });
    starting = false;
    await done;
    // Si se siguió en segundo plano, esta ventana ya no está: no se toca la selección.
    if (!closed && restores[repo.id]?.result) ondone();
  }

  /** Tras un fallo: volver a las opciones para corregir y reintentar. */
  function retry() {
    clearRestore(repo.id);
    started = false;
    cancelling = false;
    showErrors = false;
  }

  async function cancel() {
    cancelling = true;
    try {
      await cancelRestore(repo.id);
    } catch (e) {
      error = String(e);
      cancelling = false;
    }
  }

  let closed = false;
  function background() {
    closed = true;
    onbackground();
  }

  /** Abre el Explorador en lo restaurado (el primer elemento, seleccionado; o la carpeta). */
  function reveal(path: string) {
    const sep = path.includes("\\") ? "\\" : "/";
    const item = names.length ? `${path.replace(/[\\/]+$/, "")}${sep}${names[0]}` : path;
    revealItemInDir(item).catch((e) => (error = String(e)));
  }

  const canContinue = $derived(!!target && !busyElsewhere && !starting);
  /** Por qué no se puede seguir (tooltip del botón desactivado). */
  const disabledWhy = $derived(
    busyElsewhere ? "Ya hay una restauración en curso en este repositorio" : starting ? "Empezando…" : "Elige la carpeta donde dejar los archivos",
  );
</script>

{#if !started && !reviewing}
  <div class="body">
    <fieldset class="choices">
      <legend class="field-label legend-row">¿Dónde quieres dejar los archivos? <HelpLink topic="restaurar-donde" label="dónde restaurar" /></legend>
      <div class="choice-box" class:on={where === "other"}>
        <label class="choice">
          <input type="radio" name="where" checked={where === "other"} onchange={() => (where = "other")} />
          <span class="choice-body">
            <span class="choice-title"><FolderPlus size={15} /> <strong>En otra carpeta</strong> <span class="rec">Recomendado</span></span>
            <span class="faint">No se toca nada de lo que ya tienes; después copias lo que necesites.</span>
          </span>
        </label>
        {#if where === "other"}
          <div class="target" transition:slide={{ duration: dur(140) }}>
            <span class="with-button">
              <input
                class="input mono"
                bind:value={otherTarget}
                spellcheck="false"
                placeholder="C:\Users\...\Restaurado"
                aria-label="Carpeta donde restaurar"
              />
              <button type="button" class="btn" onclick={browse}><FolderOpen size={15} /> Examinar</button>
            </span>
            <span class="field-hint">
              {#if !otherTarget.trim()}
                Elige una carpeta.
              {:else if !targetInfo}
                &nbsp;
              {:else if !targetInfo.exists}
                Se creará la carpeta.
              {:else if targetInfo.empty}
                La carpeta existe y está vacía.
              {:else}
                La carpeta ya tiene contenido.
              {/if}
            </span>
          </div>
        {/if}
      </div>
      <label class="choice choice-box" class:on={where === "original"} class:disabled={!originalTarget}>
        <input type="radio" name="where" checked={where === "original"} disabled={!originalTarget} onchange={() => (where = "original")} />
        <span class="choice-body">
          <span class="choice-title"><FolderInput size={15} /> <strong>En su ubicación original</strong></span>
          {#if originalTarget}
            <span class="faint">Vuelven a <span class="mono path" title={originalTarget}>{originalTarget}</span></span>
            {#if where === "original" && targetInfo && !targetInfo.exists}
              <span class="faint">Esa carpeta ya no existe en este equipo: se volverá a crear.</span>
            {/if}
          {:else}
            <span class="faint">{originalReason}</span>
          {/if}
        </span>
      </label>
    </fieldset>

    {#if hasExisting}
      <fieldset class="choices" transition:slide={{ duration: dur(160) }}>
        <legend class="field-label legend-row">Si un archivo ya existe allí <HelpLink topic="restaurar-existentes" label="los archivos que ya existen" /></legend>
        <label class="choice choice-box" class:on={!overwrite}>
          <input type="radio" name="ow" checked={!overwrite} onchange={() => (overwrite = false)} />
          <span class="choice-body">
            <span class="choice-title"><strong>Conservarlo</strong> <span class="rec">Recomendado</span></span>
            <span class="faint">Solo se crean los archivos que falten. ¿Quieres las dos versiones? Restaura en otra carpeta.</span>
          </span>
        </label>
        <label class="choice choice-box danger" class:on={overwrite}>
          <input type="radio" name="ow" checked={overwrite} onchange={() => (overwrite = true)} />
          <span class="choice-body">
            <span class="choice-title"><strong>Reemplazarlo con el de la versión</strong></span>
            <span class="faint"><Lock size={11} /> No se puede deshacer. Pide la contraseña del repositorio.</span>
          </span>
        </label>
      </fieldset>
    {/if}

    {#if busyElsewhere}
      <div class="notice notice-info"><CircleAlert size={16} /><p>Ya hay una restauración en curso en este repositorio. Espera a que termine.</p></div>
    {/if}
    {#if error}
      <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>
    {/if}

    <footer>
      {#if onback}
        <button class="btn btn-ghost back" onclick={onback}><ArrowLeft size={15} /> Atrás</button>
      {:else}
        <button class="btn btn-ghost" onclick={onclose}>Cancelar</button>
      {/if}
      {#if guided}
        <button class="btn btn-primary" onclick={() => (reviewing = true)} disabled={!canContinue} title={canContinue ? undefined : disabledWhy}>Revisar <ArrowRight size={15} /></button>
      {:else}
        <button class="btn {replace ? 'btn-danger' : 'btn-primary'}" onclick={start} disabled={!canContinue} title={canContinue ? undefined : disabledWhy}>
          <RotateCcw size={15} />
          {replace ? "Reemplazar y restaurar" : "Restaurar"}
        </button>
      {/if}
    </footer>
  </div>
{:else if !started}
  <!-- Resumen antes de empezar (asistente) -->
  <div class="body">
    <dl class="summary">
      <div>
        <dt><PackageOpen size={15} /> Qué</dt>
        <dd>{label}</dd>
      </div>
      <div>
        <dt><History size={15} /> Versión</dt>
        <dd>{formatDate(snapshot.time)} <span class="faint mono">· {snapshot.short_id}</span></dd>
      </div>
      <div>
        <dt><FolderOpen size={15} /> Dónde</dt>
        <dd>
          <span class="mono path" title={target}>{target}</span>
          <span class="faint">{where === "original" ? "Su ubicación original" : targetInfo && !targetInfo.exists ? "Carpeta nueva" : "Otra carpeta"}</span>
        </dd>
      </div>
      <div>
        <dt><Lock size={15} /> Si ya existen</dt>
        <dd>
          {#if !hasExisting}
            La carpeta está vacía o es nueva: no se sobrescribe nada.
          {:else if replace}
            <strong class="danger-text">Se reemplazan</strong> con los de la versión (te pediremos la contraseña).
          {:else}
            Se conservan: solo se crean los que falten.
          {/if}
        </dd>
      </div>
    </dl>
    {#if busyElsewhere}
      <div class="notice notice-info"><CircleAlert size={16} /><p>Ya hay una restauración en curso en este repositorio. Espera a que termine.</p></div>
    {/if}
    {#if error}
      <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>
    {/if}
    <footer>
      <button class="btn btn-ghost back" onclick={() => (reviewing = false)}><ArrowLeft size={15} /> Cambiar opciones</button>
      <button class="btn {replace ? 'btn-danger' : 'btn-primary'}" onclick={start} disabled={!canContinue} title={canContinue ? undefined : disabledWhy}>
        <RotateCcw size={15} />
        {replace ? "Reemplazar y restaurar" : "Restaurar"}
      </button>
    </footer>
  </div>
{:else if run}
  <div class="body">
    {#if run.running}
      <div class="progress-top">
        <span class="percent">{#if run.status}{percent.toFixed(0)}<small>%</small>{:else}<span class="preparing">Preparando…</span>{/if}</span>
        {#if run.status?.seconds_remaining != null}<span class="faint">Quedan {formatDuration(run.status.seconds_remaining)}</span>{/if}
      </div>
      <div class="track" role="progressbar" aria-label="Progreso de la restauración" aria-valuenow={Math.round(percent)} aria-valuemin={0} aria-valuemax={100}>
        <div class="fill" class:indeterminate={!run.status} style:width={run.status ? `${percent}%` : undefined}></div>
      </div>
      {#if run.status}
        <p class="faint stats">
          {formatNumber(run.status.files_done)} de {formatNumber(run.status.total_files)} archivos ·
          {formatBytes(run.status.bytes_done)} de {formatBytes(run.status.total_bytes)}
        </p>
      {/if}
      <p class="faint dest mono" title={run.request.target}>→ {run.request.target}</p>
      <footer>
        <button class="btn btn-ghost" onclick={background} title="Cierra esta ventana; te avisaremos al terminar">Seguir en segundo plano</button>
        <button class="btn" onclick={cancel} disabled={cancelling}><Square size={13} fill="currentColor" /> {cancelling ? "Deteniendo…" : "Detener"}</button>
      </footer>
    {:else if run.result}
      {@const s = run.result.summary}
      <div class="notice {run.result.error_count ? 'notice-warn' : 'notice-success'}" aria-live="polite" transition:slide={{ duration: dur(180) }}>
        {#if run.result.error_count}<TriangleAlert size={17} />{:else}<CircleCheck size={17} />{/if}
        <div class="result">
          <strong>{run.result.error_count ? "Restauración terminada con avisos" : "Restauración completada"}</strong>
          {#if s}
            <p class="muted">
              {formatNumber(s.files_restored)} {s.files_restored === 1 ? "archivo restaurado" : "archivos restaurados"} ({formatBytes(s.bytes_restored)}){#if s.files_skipped}
                · {formatNumber(s.files_skipped)} ya existían y no se tocaron{/if}.
            </p>
          {/if}
          <p class="muted mono path" title={run.result.target}>{run.result.target}</p>
        </div>
      </div>
      {#if run.errorCount > 0}
        <button class="toggle" onclick={() => (showErrors = !showErrors)} aria-expanded={showErrors}>
          <span class="chev" class:open={showErrors}><ChevronDown size={15} /></span>
          {run.errorCount} {run.errorCount === 1 ? "elemento no se pudo restaurar" : "elementos no se pudieron restaurar"}
        </button>
        {#if showErrors}
          <ul class="errors mono selectable" transition:slide={{ duration: dur(160) }}>
            {#each run.errors as err}<li>{err}</li>{/each}
          </ul>
        {/if}
      {/if}
      {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if onagain}
          <button class="btn btn-ghost" onclick={onagain}>Restaurar otra cosa</button>
        {/if}
        <button class="btn" onclick={onclose}>{onagain ? "Terminar" : "Cerrar"}</button>
        <button class="btn btn-primary" onclick={() => reveal(run.result!.target)}><FolderOpen size={15} /> Abrir carpeta</button>
      </footer>
    {:else if run.failure}
      <div class="notice notice-danger" role="alert">
        <CircleAlert size={17} />
        <div class="result">
          <strong>{run.failure.includes("cancelada") ? "Restauración detenida" : "No se pudo restaurar"}</strong>
          <p>{run.failure}</p>
        </div>
      </div>
      <footer>
        <button class="btn btn-ghost" onclick={onclose}>Cerrar</button>
        <button class="btn btn-primary" onclick={retry}><RotateCcw size={15} /> Volver a intentarlo</button>
      </footer>
    {/if}
  </div>
{/if}

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .with-button {
    display: flex;
    gap: 8px;
  }
  .with-button .btn {
    height: 36px;
  }
  .choices {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    margin: 0;
    padding: 0;
    border: none;
  }
  .choices legend {
    margin-bottom: 8px;
  }
  .legend-row {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .choice-box {
    border: 1.5px solid var(--border);
    border-radius: var(--radius);
    transition:
      border-color 0.15s,
      background 0.15s;
  }
  .choice {
    display: flex;
    gap: 11px;
    align-items: flex-start;
    padding: 11px 13px;
    cursor: pointer;
  }
  .target {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0 13px 11px 39px;
  }
  .choice.disabled {
    cursor: default;
    opacity: 0.6;
  }
  .choice-body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
    font-size: var(--fs-sm);
  }
  .choice-title {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .choice-title :global(svg) {
    color: var(--text-3);
  }
  .choice .faint {
    font-size: var(--fs-sm);
  }
  .choice > input {
    margin-top: 3px;
    accent-color: var(--accent);
  }
  .choice-box.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .choice-box.danger.on {
    border-color: var(--bad);
    background: var(--bad-soft);
  }
  .choice.danger > input {
    accent-color: var(--bad);
  }
  .target .input {
    background: var(--surface);
  }
  .rec {
    display: inline-block;
    padding: 0 7px;
    font-size: var(--fs-overline);
    font-weight: 600;
    line-height: 18px;
    border-radius: 999px;
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .choice-box.on .rec {
    background: var(--surface);
  }
  .path {
    font-size: var(--fs-xs);
    word-break: break-all;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  footer .back {
    margin-right: auto;
  }

  .summary {
    display: flex;
    flex-direction: column;
    margin: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .summary > div {
    display: grid;
    grid-template-columns: 140px minmax(0, 1fr);
    gap: 12px;
    padding: 11px 14px;
    border-bottom: 1px solid var(--border);
  }
  .summary > div:last-child {
    border-bottom: none;
  }
  .summary dt {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--text-2);
  }
  .summary dd {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    font-size: var(--fs-sm);
    min-width: 0;
  }
  .summary dd .faint {
    font-size: var(--fs-sm);
  }

  .progress-top {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  .percent {
    font-family: var(--font-display);
    font-size: 34px;
    font-weight: 650;
    line-height: 1;
    font-variant-numeric: tabular-nums;
  }
  .percent small {
    font-size: 17px;
    margin-left: 2px;
    color: var(--text-2);
  }
  .preparing {
    font-size: var(--fs-h2);
    color: var(--text-2);
  }
  .track {
    height: 8px;
    border-radius: 999px;
    background: var(--surface-3);
    overflow: hidden;
    margin-top: -6px;
  }
  .fill {
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, color-mix(in srgb, var(--accent) 70%, white), var(--accent));
    transition: width 0.25s ease-out;
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
  .stats,
  .dest {
    margin: -6px 0 0;
    font-size: var(--fs-sm);
    font-variant-numeric: tabular-nums;
  }
  .dest {
    font-size: var(--fs-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .result {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .result p {
    font-size: var(--fs-sm);
  }
  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0;
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 550;
    color: var(--text-2);
    background: none;
    border: none;
    cursor: pointer;
    align-self: flex-start;
  }
  .chev {
    display: grid;
    transform: rotate(-90deg);
    transition: transform 0.2s;
  }
  .chev.open {
    transform: none;
  }
  .errors {
    margin: -8px 0 0;
    padding: 10px 12px 10px 28px;
    max-height: 160px;
    overflow: auto;
    font-size: var(--fs-xs);
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
</style>
