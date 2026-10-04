<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { slide } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { ArrowLeft, CalendarClock, CircleAlert, Folder, FolderPlus, FolderSync, Lock, Plus, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Plan, PlanSchedule, Repo } from "$lib/api";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { toast } from "$lib/toast.svelte";
  import { agent } from "$lib/agent.svelte";
  import {
    DAY_LETTERS,
    DAY_NAMES,
    MAX_TAGS,
    MAX_TIMES,
    defaultSchedule,
    nextPlanSlots,
    parseTime,
    planScheduleSentence,
    slotsPreview,
    tagError,
    validatePlan,
    validateSchedule,
  } from "$lib/plans";
  import { splitPath } from "$lib/paths";
  import HelpLink from "./HelpLink.svelte";
  import Modal from "./Modal.svelte";
  import Stepper from "./Stepper.svelte";
  import Advanced from "./Advanced.svelte";

  interface Props {
    /** Destino donde se guarda la copia. */
    repo: Repo;
    /** Copia (plan) a editar. Con `id` vacío es una copia nueva (o un duplicado). */
    plan: Plan;
    onclose: () => void;
    /** Se llama con el destino ya guardado y la copia tal como quedó (con su id). */
    onsaved: (repo: Repo, plan: Plan) => void;
    /** Dentro del asistente de «Nueva copia»: vuelve al paso anterior. */
    onback?: () => void;
    /** Texto del paso («Paso 2 de 2»), si va dentro del asistente. */
    /** Dentro del asistente «Nueva copia»: sus pasos. */
    steps?: { labels: string[]; current: number };
  }
  let { repo, plan, onclose, onsaved, onback, steps }: Props = $props();

  // Copia de trabajo: el componente se crea para cada edición.
  // svelte-ignore state_referenced_locally
  const isNew = !plan.id || !repo.plans.some((p) => p.id === plan.id);
  // svelte-ignore state_referenced_locally
  let name = $state(plan.name);
  // svelte-ignore state_referenced_locally
  let paths = $state<string[]>([...plan.paths]);
  // svelte-ignore state_referenced_locally
  let excludesText = $state(plan.excludes.join("\n"));
  // svelte-ignore state_referenced_locally
  let tags = $state<string[]>([...plan.tags]);
  let tagInput = $state("");
  let tagMsg = $state("");
  // svelte-ignore state_referenced_locally
  let scheduled = $state(!!plan.schedule);
  // svelte-ignore state_referenced_locally
  let sched = $state<PlanSchedule>($state.snapshot(plan.schedule) ?? defaultSchedule());
  // Copia nueva: activado; una existente (o un duplicado) conserva lo que tenía.
  // svelte-ignore state_referenced_locally
  let skipUnchanged = $state(isNew && !plan.paths.length ? true : !!plan.skip_unchanged);
  let error = $state("");
  let tried = $state(false);
  let saving = $state(false);

  const others = $derived(repo.plans.filter((p) => p.id !== plan.id || isNew));
  const excludes = $derived(excludesText.split(/\r?\n/).map((l) => l.trim()).filter(Boolean));

  const draft = $derived<Plan>({
    id: isNew ? "" : plan.id,
    name: name.trim(),
    paths: [...paths],
    excludes: [...excludes],
    tags: [...tags],
    schedule: scheduled ? { ...sched, days: [...sched.days].sort((a, b) => a - b), times: [...sched.times] } : null,
    skip_unchanged: skipUnchanged,
  });

  // Cerrar con cambios sin guardar (Escape, la X o «Cancelar») pide confirmarlo una vez.
  // svelte-ignore state_referenced_locally
  const initialDraft = JSON.stringify(draft);
  const dirty = $derived(JSON.stringify(draft) !== initialDraft);
  let confirmDiscard = $state(false);
  function requestClose() {
    if (saving) return;
    if (dirty && !confirmDiscard) {
      confirmDiscard = true;
      return;
    }
    onclose();
  }
  /** El aviso de descartar queda a la vista y con el foco en «Seguir editando». */
  function reveal(node: HTMLElement) {
    node.scrollIntoView({ block: "nearest" });
    node.querySelector("button")?.focus();
  }

  const nameError = $derived.by(() => {
    const n = name.trim();
    if (!n) return "Ponle un nombre a la copia.";
    if ([...n].length > 60) return "Máximo 60 caracteres.";
    if (others.some((o) => o.name.trim().toLocaleLowerCase() === n.toLocaleLowerCase())) return `Ya hay una copia llamada «${n}» en este repositorio.`;
    return "";
  });
  const scheduleError = $derived(scheduled ? validateSchedule(draft.schedule!) : null);
  const preview = $derived(scheduled && !scheduleError ? nextPlanSlots(draft.schedule!, new Date(), 5) : []);

  // ---------- Carpetas ----------

  async function addFolders() {
    try {
      const picked = await open({ directory: true, multiple: true, title: "Carpetas para copiar" });
      if (!picked) return;
      const list = (Array.isArray(picked) ? picked : [picked]).filter((p) => !paths.includes(p));
      paths = [...paths, ...list];
      // Sin nombre todavía: se propone el de la primera carpeta («Documentos»).
      if (!name.trim() && paths.length) name = suggestName(paths[0]);
    } catch (e) {
      error = String(e);
    }
  }

  /** Nombre de una carpeta que no choque con otra copia del destino. */
  function suggestName(path: string) {
    const drive = /^([A-Za-z]):[\\/]?$/.exec(path.trim());
    const base = drive ? `Disco ${drive[1].toUpperCase()}` : (splitPath(path).name || path).slice(0, 50);
    const taken = (n: string) => others.some((o) => o.name.trim().toLocaleLowerCase() === n.toLocaleLowerCase());
    let n = base;
    for (let i = 2; taken(n); i++) n = `${base} ${i}`;
    return n;
  }

  // ---------- Etiquetas ----------

  function addTag() {
    const t = tagInput.trim().replace(/,+$/, "");
    if (!t) return;
    const e = tagError(t);
    if (e) {
      tagMsg = /\s/.test(t) ? "Las etiquetas no llevan espacios: usa guiones (p. ej. «copia-diaria»)." : e;
      return;
    }
    if (tags.length >= MAX_TAGS) {
      tagMsg = "Máximo 10 etiquetas.";
      return;
    }
    if (!tags.includes(t)) tags = [...tags, t];
    tagInput = "";
    tagMsg = "";
  }

  function onTagKey(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      addTag();
    } else if (e.key === "Backspace" && !tagInput && tags.length) {
      tags = tags.slice(0, -1);
    }
  }

  // ---------- Horario ----------

  const PRESETS: [string, number[]][] = [
    ["Todos los días", [0, 1, 2, 3, 4, 5, 6]],
    ["Lunes a viernes", [0, 1, 2, 3, 4]],
    ["Lunes a sábado", [0, 1, 2, 3, 4, 5]],
    ["Fines de semana", [5, 6]],
  ];
  const sameDays = (a: number[], b: number[]) => a.length === b.length && [...a].sort().every((x, i) => x === [...b].sort()[i]);

  function toggleDay(d: number) {
    sched.days = sched.days.includes(d) ? sched.days.filter((x) => x !== d) : [...sched.days, d].sort((a, b) => a - b);
  }

  function addTime() {
    // Propone una hora después de la última.
    // Última hora válida («7:00» también vale); si ninguna lo es, las 13:00.
    const last = sched.times.map(parseTime).filter((t): t is number => t != null).at(-1);
    const h = last != null ? (Math.floor(last / 60) + 1) % 24 : 13;
    sched.times = [...sched.times, `${String(h).padStart(2, "0")}:00`];
  }

  // ---------- Guardar ----------

  async function save() {
    tried = true;
    error = "";
    const problem = validatePlan(draft, others);
    if (problem) {
      error = problem;
      return;
    }
    const list = isNew ? [...repo.plans, draft] : repo.plans.map((p) => (p.id === plan.id ? draft : p));
    saving = true;
    const before = new Set(repo.plans.map((p) => p.id));
    let saved: { repo: Repo; plan: Plan } | null = null;
    // Si el agente hace las copias de este destino, como administrador se le
    // aplican los cambios enseguida (con la misma contraseña). Sin serlo, la
    // copia muestra «Aplicar cambios».
    const inAgent = !isNew && !!agent.info?.elevated && !!agent.info.repos.some((r) => r.id === repo.id && r.schedule.kind === "plans");
    let agentError = "";
    const done = await withPassword({
      title: isNew ? "Crear copia" : "Guardar copia",
      message: `${isNew ? "Crear" : "Guardar"} la copia «${draft.name}» en «${repo.name}». Para cambiar qué se copia, confirma con la contraseña del repositorio.`,
      repoName: repo.name,
      confirmLabel: isNew ? "Crear copia" : "Guardar cambios",
      // Objetos planos (sin proxies de Svelte) para enviarlos al backend.
      action: async (password) => {
        const updated = await api.setPlans(repo.id, $state.snapshot(list) as Plan[], password);
        // Una copia nueva recibe su id del backend: es la que no estaba antes.
        const found = isNew ? updated.plans.find((p) => !before.has(p.id)) : updated.plans.find((p) => p.id === plan.id);
        saved = { repo: updated, plan: found ?? updated.plans[updated.plans.length - 1] };
        if (inAgent) {
          try {
            agent.info = await api.agentSetSchedule(repo.id, { kind: "plans" }, password);
          } catch (e) {
            // La copia ya se guardó: no se repite, solo se avisa.
            agentError = String(e);
          }
        }
      },
    });
    saving = false;
    if (done && saved) {
      // Ya guardada: que la validación no «vea» la copia recién creada mientras el diálogo se cierra.
      tried = false;
      const { repo: r, plan: p } = saved as { repo: Repo; plan: Plan };
      toast(isNew ? `Copia «${draft.name}» creada` : `Copia «${draft.name}» guardada`);
      if (agentError) toast(`No se pudo actualizar el agente: ${agentError}`, "error", 8000);
      onsaved(r, p);
      onclose();
    }
  }
</script>

<Modal onclose={requestClose} labelledby="plan-title" width={620} dismissible={false}>
  <header class="dlg-head">
    <div class="dlg-title">
      <span class="ticon"><FolderSync size={19} /></span>
      <div>
        <h2 id="plan-title">{isNew ? (plan.paths.length ? "Duplicar copia" : "Nueva copia") : "Editar copia"}</h2>
        <p class="faint">Se guarda en «{repo.name}»</p>
      </div>
    </div>
    <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={requestClose}><X size={17} /></button>
  </header>
  {#if steps}<div class="wiz-steps"><Stepper labels={steps.labels} current={steps.current} /></div>{/if}

  <div class="body">
    <div class="field">
      <label class="field-label" for="plan-name">Nombre</label>
      <input
        id="plan-name"
        class="input"
        maxlength="60"
        placeholder="Laboral, Domingo, Fotos…"
        bind:value={name}
        onkeydown={(e) => e.key === "Enter" && !saving && (e.preventDefault(), save())}
        aria-invalid={tried && !!nameError}
        aria-describedby={tried && nameError ? "plan-name-err" : undefined}
      />
      {#if tried && nameError}<span id="plan-name-err" class="err">{nameError}</span>{/if}
    </div>

    <div class="field">
      <span class="field-label" id="plan-paths-label">Carpetas que se copian</span>
      <ul class="sources" aria-labelledby="plan-paths-label">
        {#each paths as path (path)}
          {@const p = splitPath(path)}
          <li class="source" transition:slide={{ duration: dur(150) }}>
            <span class="source-icon"><Folder size={16} /></span>
            <span class="source-path mono selectable" title={path}>
              <span class="faint">{p.parent}</span><span class="source-name">{p.name}</span>
            </span>
            <button class="icon-btn" title="Quitar carpeta" aria-label={`Quitar ${path}`} onclick={() => (paths = paths.filter((x) => x !== path))}>
              <X size={15} />
            </button>
          </li>
        {/each}
        <li>
          <button class="add-source" onclick={addFolders}><FolderPlus size={16} /> {paths.length ? "Añadir otra carpeta" : "Elegir carpetas"}</button>
        </li>
      </ul>
      {#if tried && paths.length === 0}<span class="err">Añade al menos una carpeta.</span>{/if}
    </div>

    <div class="field">
      <div class="label-row">
        <label class="field-label" for="plan-excludes">Exclusiones <span class="opt">(opcional)</span></label>
        <HelpLink topic="copias-exclusiones" label="las exclusiones" />
      </div>
      <textarea
        id="plan-excludes"
        class="input mono"
        rows="3"
        spellcheck="false"
        placeholder={"*.tmp\nnode_modules\nC:\\Users\\*\\AppData"}
        bind:value={excludesText}
      ></textarea>
      <span class="field-hint">Un patrón por línea. No distingue mayúsculas (como Windows): <code>*.TMP</code> también excluye <code>archivo.tmp</code>.</span>
    </div>

    <fieldset class="sched">
      <legend class="sr-only">Horario</legend>
      <label class="switch-row">
        <input type="checkbox" class="switch" bind:checked={scheduled} />
        <span>
          <strong>Hacerla sola, con horario</strong>
          <span class="faint">{scheduled ? "Se hará en los días y horas que elijas, aunque la app esté cerrada." : "Sin horario: solo se copia cuando pulses «Copiar ahora»."}</span>
        </span>
      </label>

      {#if scheduled}
        <div class="sched-body" transition:slide={{ duration: dur(180) }}>
          <div class="field">
            <div class="label-row">
              <span class="field-label" id="plan-days-label">Días</span>
              <HelpLink topic="copias-horarios" label="los horarios" />
            </div>
            <div class="days" role="group" aria-labelledby="plan-days-label">
              {#each DAY_LETTERS as letter, i}
                <button class="day" class:on={sched.days.includes(i)} aria-pressed={sched.days.includes(i)} aria-label={DAY_NAMES[i]} title={DAY_NAMES[i]} onclick={() => toggleDay(i)}>
                  {letter}
                </button>
              {/each}
            </div>
            <div class="presets">
              {#each PRESETS as [label, days]}
                <button class="preset" class:on={sameDays(sched.days, days)} onclick={() => (sched.days = [...days])}>{label}</button>
              {/each}
            </div>
          </div>

          <div class="field">
            <span class="field-label" id="plan-mode-label">Horas</span>
            <div class="segmented" role="group" aria-labelledby="plan-mode-label">
              <button class:on={sched.mode === "at"} aria-pressed={sched.mode === "at"} onclick={() => (sched.mode = "at")}>A horas concretas</button>
              <button class:on={sched.mode === "every"} aria-pressed={sched.mode === "every"} onclick={() => (sched.mode = "every")}>Cada N horas</button>
            </div>

            {#if sched.mode === "at"}
              <div class="times">
                {#each sched.times as _, i}
                  <span class="time-item">
                    <input class="input time" type="time" aria-label={`Hora ${i + 1}`} bind:value={sched.times[i]} />
                    <button class="icon-btn" title="Quitar hora" aria-label={`Quitar la hora ${sched.times[i]}`} onclick={() => (sched.times = sched.times.filter((_, j) => j !== i))}>
                      <X size={14} />
                    </button>
                  </span>
                {/each}
                <button class="btn btn-sm" onclick={addTime} disabled={sched.times.length >= MAX_TIMES} title={sched.times.length >= MAX_TIMES ? `Máximo ${MAX_TIMES} horas` : undefined}><Plus size={13} /> Añadir hora</button>
              </div>
              <span class="field-hint">¿Cada hora en punto? Usa «Cada N horas».</span>
            {:else}
              <div class="every">
                <label>Cada <input class="input num" type="number" min="1" max="24" bind:value={sched.every_hours} /> {sched.every_hours === 1 ? "hora" : "horas"}</label>
                <label>desde las <input class="input time" type="time" bind:value={sched.from} /></label>
                <label>hasta las <input class="input time" type="time" bind:value={sched.to} /></label>
              </div>
            {/if}
          </div>

          <div class="preview" aria-live="polite">
            <CalendarClock size={15} />
            {#if scheduleError}
              <span class="err">{scheduleError}</span>
            {:else}
              <span>
                <strong>{planScheduleSentence(draft.schedule!)}.</strong>
                {#if preview.length}<span class="muted">Próximas copias: {slotsPreview(preview)}{preview.length === 5 ? " …" : ""}</span>{/if}
              </span>
            {/if}
          </div>
        </div>
      {/if}
    </fieldset>

    <Advanced
      id="plan"
      hint={[tags.length ? `Etiquetas: ${tags.join(", ")}` : "Sin etiquetas", skipUnchanged ? "solo guarda si hay cambios" : "guarda siempre una versión"].join(" · ")}
      custom={tags.length > 0 || !skipUnchanged}
    >
      <label class="switch-row">
        <input type="checkbox" class="switch" bind:checked={skipUnchanged} />
        <span>
          <strong>Solo guardar una versión si hay cambios <HelpLink topic="copias-sin-cambios" label="«Solo guardar si hay cambios»" /></strong>
          <span class="faint">La copia revisa igual a su hora, pero si nada cambió no crea una versión nueva. Útil para copias frecuentes, noches y fines de semana.</span>
        </span>
      </label>

      <div class="field">
        <div class="label-row">
          <label class="field-label" for="plan-tags">Etiquetas <span class="opt">(opcional)</span></label>
          <HelpLink topic="copias-etiquetas" label="las etiquetas" />
        </div>
        <div class="chips input" class:invalid={!!tagMsg}>
          {#each tags as t (t)}
            <span class="chip">
              {t}
              <button class="chip-x" aria-label={`Quitar la etiqueta ${t}`} onclick={() => (tags = tags.filter((x) => x !== t))}><X size={12} /></button>
            </span>
          {/each}
          <input
            id="plan-tags"
            class="chip-input"
            placeholder={tags.length ? "" : "diaria, semanal…"}
            spellcheck="false"
            maxlength="40"
            bind:value={tagInput}
            onkeydown={onTagKey}
            oninput={() => (tagMsg = "")}
            onblur={addTag}
            aria-invalid={!!tagMsg}
            aria-describedby="plan-tags-hint"
          />
        </div>
        {#if tagMsg}
          <span id="plan-tags-hint" class="err" role="alert">{tagMsg}</span>
        {:else}
          <span id="plan-tags-hint" class="field-hint">Pulsa Intro o coma para añadirla. Se guardan en cada versión y sirven para la retención.</span>
        {/if}
      </div>
    </Advanced>

    {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}

    {#if confirmDiscard && dirty}
      <div class="notice notice-warn discard" role="alert" use:reveal>
        <CircleAlert size={16} />
        <p>Tienes cambios sin guardar. ¿Los descartas?</p>
        <button class="btn btn-sm" onclick={() => (confirmDiscard = false)}>Seguir editando</button>
        <button class="btn btn-sm btn-danger" onclick={onclose}>Descartar</button>
      </div>
    {/if}

    <footer>
      {#if onback}
        <button class="btn btn-ghost back" onclick={onback}><ArrowLeft size={14} /> Atrás</button>
      {/if}
      <button class="btn btn-ghost" onclick={requestClose}>Cancelar</button>
      <button class="btn btn-primary" onclick={save} disabled={saving}><Lock size={13} /> {isNew ? "Crear copia" : "Guardar"}</button>
    </footer>
  </div>
</Modal>

<style>
  .discard {
    align-items: center;
  }
  .discard p {
    flex: 1;
  }
  .wiz-steps {
    margin: 0 0 var(--sp-5);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .opt {
    font-weight: 400;
    color: var(--text-3);
  }
  .err {
    font-size: var(--fs-sm);
    color: var(--bad);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .back {
    margin-right: auto;
  }

  /* Carpetas (mismo estilo que el panel) */
  .sources {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .source {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 6px 5px 10px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .source-icon {
    display: grid;
    place-items: center;
    color: var(--accent);
  }
  .source-path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-sm);
  }
  .source-name {
    color: var(--text-1);
    font-weight: 600;
  }
  .add-source {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 100%;
    height: 36px;
    font: inherit;
    font-weight: 550;
    color: var(--text-2);
    background: transparent;
    border: 1.5px dashed var(--border-strong);
    border-radius: var(--radius);
    cursor: pointer;
    transition:
      color 0.15s,
      border-color 0.15s,
      background 0.15s;
  }
  .add-source:hover {
    color: var(--accent-text);
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  textarea {
    font-size: var(--fs-sm);
  }

  /* Etiquetas */
  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    height: auto;
    min-height: 36px;
    padding: 4px 8px;
    cursor: text;
  }
  .chips:focus-within {
    border-color: var(--accent);
    box-shadow: var(--focus);
  }
  .chips.invalid {
    border-color: var(--bad);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 0 3px 0 9px;
    font-size: var(--fs-sm);
    font-weight: 600;
    line-height: 24px;
    border-radius: 999px;
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .chip-x {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    color: inherit;
    background: none;
    border: none;
    border-radius: 999px;
    cursor: pointer;
  }
  .chip-x:hover {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }
  .chip-input {
    flex: 1;
    min-width: 120px;
    height: 26px;
    padding: 0 2px;
    font: inherit;
    color: var(--text-1);
    background: none;
    border: none;
    outline: none;
  }

  /* Horario */
  .sched {
    display: flex;
    flex-direction: column;
    gap: 14px;
    margin: 0;
    padding: 14px 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface-2);
  }
  .switch-row {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    cursor: pointer;
  }
  .switch-row > span {
    display: flex;
    flex-direction: column;
    gap: 1px;
    font-size: var(--fs-sm);
  }
  .switch-row .faint {
    font-size: var(--fs-sm);
  }
  .switch-row .switch {
    margin-top: 1px;
  }
  .sched-body {
    display: flex;
    flex-direction: column;
    gap: 14px;
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
    transition:
      background 0.15s,
      color 0.15s,
      border-color 0.15s;
  }
  .day.on {
    color: var(--accent-contrast);
    background: var(--accent);
    border-color: var(--accent);
  }
  .presets {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .preset {
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
  .preset.on {
    color: var(--accent-text);
    background: var(--accent-soft);
    border-color: transparent;
  }
  .times {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
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
  .every {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 14px;
    font-size: var(--fs-sm);
  }
  .every label {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .num {
    width: 68px;
    height: 32px;
  }
  .preview {
    display: flex;
    gap: 8px;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .preview :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--accent);
  }
  .preview > span {
    display: flex;
    flex-direction: column;
  }
</style>
