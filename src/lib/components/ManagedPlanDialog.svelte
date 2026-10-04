<script lang="ts">
  // Una copia de un equipo gestionado: qué carpetas (de ese equipo), qué
  // excluir y cuándo. Se manda firmada al equipo al guardar.
  import { CalendarClock, CircleAlert, FolderSync, LoaderCircle, Plus, Trash2, X } from "@lucide/svelte";
  import type { ManagedEndpoint, Plan, PlanSchedule } from "$lib/api";
  import { DAY_LETTERS, DAY_NAMES, MAX_TIMES, defaultSchedule, planScheduleSentence, validatePlan } from "$lib/plans";
  import Modal from "./Modal.svelte";

  interface Props {
    endpoint: ManagedEndpoint;
    /** null: copia nueva. */
    plan: Plan | null;
    onclose: () => void;
    /** Guarda la lista completa de copias del equipo. */
    onsave: (plans: Plan[]) => Promise<void>;
  }
  let { endpoint, plan, onclose, onsave }: Props = $props();

  // svelte-ignore state_referenced_locally
  const isNew = !plan;
  // svelte-ignore state_referenced_locally
  let name = $state(plan?.name ?? (endpoint.plans.length ? "" : "Documentos"));
  // svelte-ignore state_referenced_locally
  let pathsText = $state((plan?.paths ?? []).join("\n"));
  // svelte-ignore state_referenced_locally
  let excludesText = $state((plan?.excludes ?? []).join("\n"));
  // svelte-ignore state_referenced_locally
  let scheduled = $state(plan ? !!plan.schedule : true);
  // svelte-ignore state_referenced_locally
  let sched = $state<PlanSchedule>(plan?.schedule ? structuredClone($state.snapshot(plan.schedule)) : defaultSchedule());
  // svelte-ignore state_referenced_locally
  let skipUnchanged = $state(plan ? !!plan.skip_unchanged : true);
  let busy = $state(false);
  let error = $state("");

  const lines = (t: string) =>
    t
      .split(/\r?\n/)
      .map((l) => l.trim())
      .filter(Boolean);

  /** Carpetas típicas de un equipo de Windows (rutas de ese equipo). */
  const SUGGESTIONS: [string, string[], string[]][] = [
    ["Todos los usuarios", ["C:\\Users"], ["C:\\Users\\*\\AppData", "C:\\Users\\Default", "C:\\Users\\Public\\Libraries"]],
    ["Carpeta pública", ["C:\\Users\\Public"], []],
  ];
  function suggest(paths: string[], excludes: string[]) {
    const p = new Set(lines(pathsText));
    paths.forEach((x) => p.add(x));
    pathsText = [...p].join("\n");
    const e = new Set(lines(excludesText));
    excludes.forEach((x) => e.add(x));
    excludesText = [...e].join("\n");
  }

  function slug(s: string) {
    const base = s
      .normalize("NFD")
      .replace(/[\u0300-\u036f]/g, "")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "")
      .slice(0, 30);
    let id = base || "copia";
    for (let i = 2; endpoint.plans.some((p) => p.id === id); i++) id = `${base || "copia"}-${i}`;
    return id;
  }

  const draft = $derived<Plan>({
    id: plan?.id ?? "",
    name: name.trim(),
    paths: lines(pathsText),
    excludes: lines(excludesText),
    tags: [],
    schedule: scheduled ? { ...sched, days: [...sched.days].sort((a, b) => a - b), times: [...sched.times] } : null,
    skip_unchanged: skipUnchanged,
  });
  const others = $derived(endpoint.plans.filter((p) => p.id !== plan?.id));
  const problem = $derived(validatePlan(draft, others));

  function toggleDay(d: number) {
    sched.days = sched.days.includes(d) ? sched.days.filter((x) => x !== d) : [...sched.days, d].sort((a, b) => a - b);
  }

  async function save() {
    if (problem) return;
    busy = true;
    error = "";
    try {
      const p = { ...$state.snapshot(draft), id: plan?.id ?? slug(draft.name) } as Plan;
      await onsave(isNew ? [...endpoint.plans, p] : endpoint.plans.map((x) => (x.id === p.id ? p : x)));
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function remove() {
    if (!plan) return;
    busy = true;
    error = "";
    try {
      await onsave(endpoint.plans.filter((x) => x.id !== plan.id));
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Modal {onclose} labelledby="mplan-title" width={580} dismissible={false}>
  <header class="dlg-head">
    <div class="dlg-title">
      <span class="ticon"><FolderSync size={18} /></span>
      <div>
        <h2 id="mplan-title">{isNew ? "Nueva copia" : `Copia «${plan?.name}»`}</h2>
        <p>En <strong>{endpoint.name}</strong> · se guarda en este Servidor de copias</p>
      </div>
    </div>
    <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={onclose}><X size={17} /></button>
  </header>

  <div class="body">
    <label class="field">
      <span class="field-label">Nombre</span>
      <input class="input" maxlength="60" bind:value={name} placeholder="Documentos" />
    </label>

    <div class="field">
      <label class="field-label" for="mplan-paths">Carpetas de ese equipo</label>
      <textarea id="mplan-paths" class="input mono area" rows="3" spellcheck="false" placeholder={"C:\\Users\\Laura\\Documents"} bind:value={pathsText}></textarea>
      <span class="field-hint">Una por línea, tal como se llaman en ese equipo. «Todos los usuarios» copia sus carpetas sin los archivos de programas (AppData).</span>
      <div class="presets">
        {#each SUGGESTIONS as [label, p, e]}
          <button class="preset" onclick={() => suggest(p, e)}><Plus size={12} /> {label}</button>
        {/each}
      </div>
    </div>

    <div class="field">
      <label class="field-label" for="mplan-excl">No copiar (opcional)</label>
      <textarea id="mplan-excl" class="input mono area" rows="2" spellcheck="false" placeholder="*.tmp" bind:value={excludesText}></textarea>
    </div>

    <label class="switch-row">
      <input type="checkbox" class="switch" bind:checked={scheduled} />
      <span>
        <strong>Copiar con horario</strong>
        <span class="faint">Sin horario, solo se copia cuando pulses «Copiar ahora».</span>
      </span>
    </label>

    {#if scheduled}
      <div class="sched">
        <div class="days" role="group" aria-label="Días">
          {#each DAY_LETTERS as letter, i}
            <button class="day" class:on={sched.days.includes(i)} aria-pressed={sched.days.includes(i)} aria-label={DAY_NAMES[i]} title={DAY_NAMES[i]} onclick={() => toggleDay(i)}>
              {letter}
            </button>
          {/each}
        </div>
        <div class="segmented" role="group" aria-label="Horas">
          <button class:on={sched.mode === "at"} aria-pressed={sched.mode === "at"} onclick={() => (sched.mode = "at")}>A horas concretas</button>
          <button class:on={sched.mode === "every"} aria-pressed={sched.mode === "every"} onclick={() => (sched.mode = "every")}>Cada N horas</button>
        </div>
        {#if sched.mode === "at"}
          <div class="times">
            {#each sched.times as _, i}
              <span class="time-item">
                <input class="input time" type="time" aria-label={`Hora ${i + 1}`} bind:value={sched.times[i]} />
                <button class="icon-btn" title="Quitar hora" aria-label={`Quitar la hora ${sched.times[i]}`} onclick={() => (sched.times = sched.times.filter((_, j) => j !== i))}><X size={14} /></button>
              </span>
            {/each}
            <button class="btn btn-sm" onclick={() => (sched.times = [...sched.times, "19:00"])} disabled={sched.times.length >= MAX_TIMES}><Plus size={13} /> Añadir hora</button>
          </div>
        {:else}
          <div class="every">
            <label>Cada <input class="input num-in" type="number" min="1" max="24" bind:value={sched.every_hours} /> {sched.every_hours === 1 ? "hora" : "horas"}</label>
            <label>desde las <input class="input time" type="time" bind:value={sched.from} /></label>
            <label>hasta las <input class="input time" type="time" bind:value={sched.to} /></label>
          </div>
        {/if}
        {#if !problem && draft.schedule}
          <p class="preview"><CalendarClock size={14} /> {planScheduleSentence(draft.schedule)}</p>
        {/if}
      </div>
    {/if}

    <label class="switch-row">
      <input type="checkbox" class="switch" bind:checked={skipUnchanged} />
      <span>
        <strong>Solo guardar versión si hay cambios</strong>
        <span class="faint">Así no se acumulan versiones iguales.</span>
      </span>
    </label>

    {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}
  </div>

  <footer>
    {#if !isNew}
      <button class="btn btn-ghost danger-text left" onclick={remove} disabled={busy} title="El equipo deja de hacer esta copia (lo ya copiado se queda en el servidor)"><Trash2 size={14} /> Quitar</button>
    {/if}
    <button class="btn btn-ghost" onclick={onclose}>Cancelar</button>
    <button class="btn btn-primary" onclick={save} disabled={busy || !!problem} title={problem ?? undefined}>
      {#if busy}<span class="spin"><LoaderCircle size={15} /></span>{/if}
      {isNew ? "Crear y enviar" : "Guardar y enviar"}
    </button>
  </footer>
</Modal>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .area {
    height: auto;
    padding-top: 8px;
    padding-bottom: 8px;
    resize: vertical;
    font-size: var(--fs-sm);
  }
  .presets {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 6px;
  }
  .preset {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 0 10px;
    font: inherit;
    font-size: var(--fs-sm);
    line-height: 24px;
    color: var(--text-2);
    background: none;
    border: 1px solid var(--border);
    border-radius: 999px;
    cursor: pointer;
  }
  .preset:hover {
    color: var(--text-1);
    border-color: var(--border-strong);
  }
  .sched {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: var(--sp-4);
    border-radius: var(--radius-lg);
    background: var(--surface-2);
  }
  .days {
    display: flex;
    gap: 6px;
  }
  .day {
    width: 36px;
    height: 34px;
    font: inherit;
    font-weight: 650;
    color: var(--text-2);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .day.on {
    color: var(--accent-contrast);
    background: var(--accent);
    border-color: var(--accent);
  }
  .segmented {
    align-self: flex-start;
  }
  .times,
  .every {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .every label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .time-item {
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }
  .time {
    width: auto;
    height: 32px;
  }
  .num-in {
    width: 64px;
    height: 32px;
  }
  .preview {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .danger-text {
    color: var(--bad);
  }
  .left {
    margin-right: auto;
  }
  .spin {
    display: inline-grid;
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
