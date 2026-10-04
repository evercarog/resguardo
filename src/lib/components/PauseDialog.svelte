<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { CircleAlert, CirclePause, Info, Lock, ShieldCheck, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Repo } from "$lib/api";
  import { agent } from "$lib/agent.svelte";
  import { forgetIntent, rememberIntent } from "$lib/intent.svelte";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { MAX_PAUSE_DAYS, PAUSE_CHOICES, pauseOf, pauseUntil, toLocalInput, whenInWords, type PauseChoice } from "$lib/pause.svelte";
  import { toast } from "$lib/toast.svelte";
  import Modal from "./Modal.svelte";

  // «Pausar copias automáticas» de un destino: cuánto tiempo y cuándo se
  // reanudarán solas. Pide la contraseña del destino y administrador.
  // Desde la bandeja se abre con «1 hora» y, si hay varios destinos con
  // copias automáticas, se elige cuál (`repos`).
  interface Props {
    repo: Repo;
    /** Destinos entre los que elegir (desde la bandeja). */
    repos?: Repo[];
    /** Duración elegida al abrir. */
    initial?: PauseChoice;
    onclose: () => void;
  }
  let { repo: firstRepo, repos: pickable = [], initial = "4h", onclose }: Props = $props();

  let selectedId = $state(untrack(() => firstRepo.id));
  const repo = $derived(pickable.find((r) => r.id === selectedId) ?? firstRepo);
  let choice = $state<PauseChoice>(untrack(() => initial));
  let custom = $state("");
  let error = $state("");

  // "Ahora" avanza para que la hora de reanudación sea la real al confirmar.
  let now = $state(new Date());
  onMount(() => {
    const t = setInterval(() => (now = new Date()), 15_000);
    return () => clearInterval(t);
  });

  const info = $derived(agent.info);
  const current = $derived(pauseOf(repo.id, now.getTime()));
  const until = $derived(pauseUntil(choice, custom, now));
  const invalid = $derived(!!until && !(until instanceof Date));
  const minInput = $derived(toLocalInput(new Date(now.getTime() + 60_000)));
  const maxInput = $derived(toLocalInput(new Date(now.getTime() + MAX_PAUSE_DAYS * 86_400_000)));

  /** «Se reanudarán solas el jueves 2 de octubre a las 06:00.» */
  const summary = $derived.by(() => {
    if (until === null) return "No se reanudarán solas: tendrás que pulsar «Reanudar ahora».";
    if (until instanceof Date) return `Se reanudarán solas ${whenInWords(until, now)}.`;
    return until.error;
  });

  function pick(c: PauseChoice) {
    choice = c;
    error = "";
    // Al elegir una fecha, se propone mañana a esta hora.
    if (c === "date" && !custom) custom = toLocalInput(new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1, now.getHours(), 0));
  }

  async function confirm() {
    error = "";
    if (invalid) return;
    const phrase = until instanceof Date ? `hasta ${whenInWords(until, new Date())}` : "hasta que las reanudes";
    const done = await withPassword({
      title: current ? "Cambiar la pausa" : "Pausar copias automáticas",
      message: `Las copias automáticas de «${repo.name}» quedarán en pausa ${phrase}: ni copias programadas, ni reintentos, ni verificaciones, ni copias externas. Las copias en curso terminan normalmente.`,
      repoName: repo.name,
      confirmLabel: "Pausar",
      action: async (password) => {
        // Se recalcula al confirmar: «1 hora» cuenta desde ahora, no desde que se abrió el diálogo.
        const end = pauseUntil(choice, custom, new Date());
        if (end && !(end instanceof Date)) throw end.error;
        agent.info = await api.agentPause(repo.id, end ? end.toISOString() : null, password);
      },
    });
    if (done) {
      const p = pauseOf(repo.id);
      toast(`Copias automáticas de «${repo.name}» en pausa ${p?.until ? `hasta ${whenInWords(new Date(p.until))}` : "hasta que las reanudes"}`);
      onclose();
    }
  }

  /** Reabre como administrador y, al volver, abre este diálogo. */
  async function elevate() {
    rememberIntent(repo.id, "pause");
    try {
      await api.relaunchAsAdmin();
    } catch (e) {
      forgetIntent();
      error = String(e);
    }
  }
</script>

<Modal {onclose} labelledby="pause-title" width={500}>
  <header class="dlg-head">
    <div class="dlg-title">
      <span class="ticon neutral"><CirclePause size={18} /></span>
      <div>
        <h2 id="pause-title">{current ? "Cambiar la pausa" : "Pausar copias automáticas"}</h2>
        <p>«{repo.name}»</p>
      </div>
    </div>
    <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={onclose}><X size={17} /></button>
  </header>
  <div class="body">
    <p class="muted">
      Durante la pausa no se hacen copias programadas, reintentos, verificaciones ni copias externas. Útil mientras haces mantenimiento en el
      repositorio.
    </p>

    {#if pickable.length > 1}
      <label class="field">
        <span class="faint">Repositorio</span>
        <select class="input" bind:value={selectedId}>
          {#each pickable as r (r.id)}<option value={r.id}>{r.name}</option>{/each}
        </select>
      </label>
    {/if}

    <div class="choices" role="radiogroup" aria-label="Duración de la pausa">
      {#each PAUSE_CHOICES as c (c.id)}
        <label class="choice" class:on={choice === c.id}>
          <input type="radio" name="pause-choice" value={c.id} checked={choice === c.id} onchange={() => pick(c.id)} />
          <span>{c.label}</span>
        </label>
      {/each}
    </div>

    {#if choice === "date"}
      <label class="field">
        <span class="faint">Reanudar el</span>
        <input class="input" type="datetime-local" bind:value={custom} min={minInput} max={maxInput} oninput={() => (error = "")} />
      </label>
    {/if}

    <p class="summary" class:bad={invalid} aria-live="polite">{summary}</p>
    <p class="faint icon-note"><Info size={14} /> Las copias en curso terminan normalmente. No se borra nada ni cambia la programación.</p>

    {#if info && !info.elevated}
      <div class="notice notice-info">
        <Info size={16} />
        <p>Pausar cambia la configuración del agente, así que hace falta abrir Resguardo como administrador. Windows lo pedirá y volverás aquí.</p>
      </div>
    {/if}
    {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}

    <footer>
      <button class="btn btn-ghost" onclick={onclose}>Cancelar</button>
      {#if info?.elevated}
        <button class="btn btn-primary" onclick={confirm} disabled={invalid} title={invalid ? "Elige una fecha válida" : undefined}><Lock size={12} /> Pausar</button>
      {:else}
        <button class="btn btn-primary" onclick={elevate}><ShieldCheck size={14} /> Abrir como administrador</button>
      {/if}
    </footer>
  </div>
</Modal>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  p {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.55;
  }
  .choices {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }
  .choice {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 11px;
    font-size: var(--fs-sm);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    transition:
      border-color 0.15s,
      background 0.15s;
  }
  .choice:hover {
    background: var(--surface-2);
  }
  .choice.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .choice input {
    margin: 0;
    accent-color: var(--accent);
  }
  .field {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: var(--fs-sm);
  }
  .field .input {
    width: auto;
    height: 34px;
  }
  .summary {
    font-weight: 600;
  }
  .summary.bad {
    color: var(--bad);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
  @media (max-width: 520px) {
    .choices {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
