<script lang="ts">
  import { fade } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { ArrowLeft, ArrowRight, Check, CircleAlert, History, Monitor, RefreshCw, RotateCcw, Archive } from "@lucide/svelte";
  import type { Repo, Snapshot } from "$lib/api";
  import { formatBytes, formatDate, formatNumber, formatRelative } from "$lib/format";
  import { clearRestore, restores } from "$lib/restores.svelte";
  import { anyModalOpen } from "./Modal.svelte";
  import HelpLink from "./HelpLink.svelte";
  import RestoreForm from "./RestoreForm.svelte";
  import SnapshotBrowser from "./SnapshotBrowser.svelte";
  import SnapshotList from "./SnapshotList.svelte";
  import Stepper from "./Stepper.svelte";

  // Asistente «Restaurar archivos»: 1) versión, 2) qué, 3) dónde y cómo,
  // 4) resumen, progreso y resultado. Reutiliza la lista de versiones, el
  // explorador y el formulario del diálogo de restaurar.
  interface Props {
    repo: Repo;
    /** Versiones entre las que elegir (las de la copia o las del destino). */
    snapshots: Snapshot[];
    /** Todas las del destino (para «Qué cambió» en el explorador). */
    all: Snapshot[];
    loading: boolean;
    error: string;
    onrefresh: () => void;
    /** «de la copia «Laboral»», «desde «Disco externo»»… */
    context: string;
    onclose: () => void;
  }
  let { repo, snapshots, all, loading, error, onrefresh, context, onclose }: Props = $props();

  type Phase = "version" | "files" | "where";
  let phase = $state<Phase>("version");
  let formStage = $state<"options" | "review" | "run">("options");
  let chosen = $state<Snapshot | null>(null);
  let selection = $state<{ dir: string; names: string[]; label: string } | null>(null);
  /** Carpeta que se estaba viendo: al cambiar de versión se vuelve a ella. */
  let browseDir = $state<string | null>(null);
  let running = $state(false);

  const latest = $derived(snapshots.reduce<Snapshot | null>((a, s) => (!a || s.time > a.time ? s : a), null));
  const others = $derived(snapshots.filter((s) => s.id !== latest?.id));
  const snapshot = $derived(chosen ?? latest);
  const step = $derived(phase === "version" ? 1 : phase === "files" ? 2 : formStage === "options" ? 3 : 4);
  /** Ya se lanzó la restauración: no se vuelve a pasos anteriores desde el indicador. */
  const locked = $derived(phase === "where" && formStage === "run");

  const STEPS = ["Versión", "Qué restaurar", "Dónde", "Restaurar"];

  function useVersion(s: Snapshot) {
    if (chosen?.id !== s.id) selection = null;
    chosen = s;
    phase = "files";
  }

  function goStep(n: number) {
    if (locked || running || n >= step) return;
    if (n === 1) phase = "version";
    else if (n === 2) phase = "files";
    else if (n === 3) {
      phase = "where";
      formStage = "options";
    }
  }

  function again() {
    clearRestore(repo.id);
    selection = null;
    formStage = "options";
    phase = "files";
  }

  function close() {
    if (running) return;
    if (!restores[repo.id]?.running) clearRestore(repo.id);
    onclose();
  }

  function onkeydown(e: KeyboardEvent) {
    // En el paso 2 el explorador gestiona Escape (quitar selección / volver).
    if (e.key === "Escape" && phase === "version" && !anyModalOpen()) close();
  }
</script>

<!-- En captura: se ve la fase antes de que el explorador reaccione al mismo Escape. -->
<svelte:window onkeydowncapture={onkeydown} />

<div class="wizard" in:fade={{ duration: dur(160) }}>
  <header class="top">
    <button class="btn btn-ghost btn-sm back" onclick={close} disabled={running} title={running ? "Espera a que termine o síguela en segundo plano" : "Volver"}>
      <ArrowLeft size={15} /> Volver
    </button>
    <div class="title-row">
      <span class="ticon"><RotateCcw size={20} /></span>
      <div>
        <h1>Restaurar archivos <HelpLink topic="restaurar-pasos" label="cómo restaurar" /></h1>
        <p class="faint">{context}</p>
      </div>
    </div>
    <Stepper labels={STEPS} current={step} onstep={goStep} canGo={(n) => n < step && !locked && !running} />
  </header>

  {#if phase === "version"}
    {#if error}
      <section class="card state-card" role="alert">
        <span class="state-icon bad"><CircleAlert size={22} /></span>
        <strong>No se pudieron leer las versiones de «{repo.name}»</strong>
        <p class="muted">{error}</p>
        <div class="state-actions">
          <button class="btn btn-primary" onclick={onrefresh} disabled={loading}>
            <span class:spin={loading} style="display:grid"><RefreshCw size={14} /></span> Reintentar
          </button>
          <HelpLink topic="restaurar-problemas" label="qué hacer si falla" />
        </div>
      </section>
    {:else if !latest && loading}
      <section class="card state-card">
        <span class="state-icon"><span class="spin" style="display:grid"><RefreshCw size={20} /></span></span>
        <strong>Buscando versiones…</strong>
        <p class="muted">Si el repositorio es un disco externo, comprueba que está conectado.</p>
      </section>
    {:else if !latest}
      <section class="card state-card">
        <span class="state-icon"><Archive size={22} /></span>
        <strong>Todavía no hay versiones que restaurar</strong>
        <p class="muted">Cada vez que se hace una copia se guarda una versión. Haz la primera con «Copiar ahora».</p>
      </section>
    {:else}
      <section class="card latest" class:picked={snapshot?.id === latest.id}>
        <div class="latest-info">
          <span class="eyebrow">Versión más reciente</span>
          <strong class="when">{formatRelative(latest.time)}</strong>
          <span class="faint">
            {formatDate(latest.time)} · <Monitor size={12} /> {latest.hostname}
            {#if latest.summary?.total_bytes_processed != null}
              · {formatBytes(latest.summary.total_bytes_processed)}{#if latest.summary.total_files_processed}, {formatNumber(latest.summary.total_files_processed)} archivos{/if}
            {/if}
          </span>
        </div>
        <button class="btn btn-primary" onclick={() => useVersion(latest)}>Usar esta versión <ArrowRight size={15} /></button>
      </section>

      {#if others.length}
        <p class="faint lead"><History size={14} /> ¿Buscas cómo estaba un archivo antes? Elige otra versión (el calendario ayuda a encontrar un día):</p>
        <SnapshotList
          snapshots={others}
          {loading}
          error=""
          title="Versiones anteriores"
          onrefresh={onrefresh}
          onopen={useVersion}
        />
      {/if}
    {/if}
  {:else if phase === "files" && snapshot}
    <div class="chosen">
      <History size={15} />
      <span>
        Versión del <strong>{formatDate(snapshot.time)}</strong>
        <span class="faint">({formatRelative(snapshot.time)}) · <span class="mono">{snapshot.short_id}</span></span>
      </span>
      <button class="link" onclick={() => (phase = "version")}>Cambiar versión</button>
    </div>
    <p class="faint lead">Marca los archivos y carpetas que quieres recuperar, o elige una carpeta entera.</p>
    {#key snapshot.id}
      <SnapshotBrowser
        {repo}
        {snapshot}
        snapshots={all}
        startDir={browseDir}
        ondirchange={(d) => (browseDir = d)}
        onpick={(sel) => {
          selection = sel;
          formStage = "options";
          phase = "where";
        }}
        onclose={() => (phase = "version")}
      />
    {/key}
  {:else if phase === "where" && snapshot && selection}
    <section class="card form-card">
      <h2 class="section-title">{formStage === "options" ? "¿Dónde y cómo?" : formStage === "review" ? "Revisa y restaura" : running ? "Restaurando…" : "Resultado"}</h2>
      <RestoreForm
        {repo}
        {snapshot}
        dir={selection.dir}
        names={selection.names}
        label={selection.label}
        guided
        bind:running
        onstage={(s) => (formStage = s)}
        onback={() => (phase = "files")}
        onagain={again}
        onbackground={onclose}
        onclose={close}
        ondone={() => {}}
      />
    </section>
  {/if}
</div>

<style>
  .wizard {
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 1080px;
    margin: 0 auto;
  }
  .top {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
    margin-bottom: 4px;
  }
  .back {
    margin-left: -8px;
  }
  .title-row {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .title-row .ticon {
    width: 44px;
    height: 44px;
    border-radius: var(--radius-lg);
  }
  h1 {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 21px;
    font-weight: 650;
  }
  .title-row p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }

  .state-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 36px 24px;
    text-align: center;
  }
  .state-card p {
    max-width: 460px;
    margin: 0;
    font-size: var(--fs-sm);
  }
  .state-icon {
    display: grid;
    place-items: center;
    width: 46px;
    height: 46px;
    margin-bottom: 4px;
    border-radius: 50%;
    color: var(--text-3);
    background: var(--surface-3);
  }
  .state-icon.bad {
    color: var(--bad);
    background: var(--bad-soft);
  }
  .state-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
  }

  .latest {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 18px 22px;
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
    background: linear-gradient(135deg, var(--accent-soft), var(--surface) 70%);
  }
  .latest-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .eyebrow {
    font-size: var(--fs-xs);
    font-weight: 650;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--accent-text);
  }
  .when {
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 650;
  }
  .when::first-letter {
    text-transform: uppercase;
  }
  .latest-info .faint {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px;
    font-size: var(--fs-sm);
  }
  .lead {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 6px 0 -4px;
    font-size: var(--fs-sm);
  }

  .chosen {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    font-size: var(--fs-sm);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
  }
  .chosen > :global(svg) {
    color: var(--accent);
  }
  .chosen .link {
    margin-left: auto;
  }

  .form-card {
    width: 100%;
    max-width: 640px;
    padding: 22px 24px 20px;
  }
  .form-card h2 {
    margin-bottom: 16px;
  }
</style>
