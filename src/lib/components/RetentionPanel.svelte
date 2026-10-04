<script lang="ts">
  import { onMount } from "svelte";
  import { fade, slide } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { Archive, Check, CircleAlert, Copy, Info, LoaderCircle, Lock, Plus, Server, Trash2, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Policy, Repo, RetentionPreview } from "$lib/api";
  import { formatDate, formatDay, formatNumber, formatTime } from "$lib/format";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { health, refresh } from "$lib/status.svelte";
  import { toast } from "$lib/toast.svelte";
  import {
    COUNT_KEYS,
    EMPTY_POLICY,
    UNLIMITED,
    isEmptyPolicy,
    normalizePolicy,
    policyArgs,
    policyShellArgs,
    policySummary,
    reasonLabel,
    restRepoPath,
    samePolicy,
    serverCommands,
    type CountKey,
  } from "$lib/retention";
  import HelpLink from "./HelpLink.svelte";
  import Advanced from "./Advanced.svelte";

  interface Props {
    repo: Repo;
    onchange: (repo: Repo) => void;
    /** Dentro de un diálogo (p. ej. la retención del destino de una copia externa): sin tarjeta propia. */
    embedded?: boolean;
    /** Se llama al guardar (después de `onchange`). */
    onsaved?: () => void;
  }
  let { repo, onchange, embedded = false, onsaved }: Props = $props();

  // ---------- Formulario ----------

  type Unit = "d" | "m" | "y";
  type Row = { n: number; unit: Unit | "always" };
  type Period = "all" | "hourly" | "daily" | "weekly" | "monthly" | "yearly";
  const PERIODS: { key: Period; label: string; always?: boolean }[] = [
    { key: "all", label: "Todas las de los últimos" },
    { key: "hourly", label: "Una por hora durante" },
    { key: "daily", label: "Una por día durante" },
    { key: "weekly", label: "Una por semana durante" },
    { key: "monthly", label: "Una por mes durante", always: true },
    { key: "yearly", label: "Una por año durante", always: true },
  ];
  const withinKey = (p: Period) => (p === "all" ? "keep_within" : (`keep_within_${p}` as const));

  const COUNTS: { key: CountKey; label: string }[] = [
    { key: "keep_last", label: "Últimas" },
    { key: "keep_hourly", label: "Por hora" },
    { key: "keep_daily", label: "Diarias" },
    { key: "keep_weekly", label: "Semanales" },
    { key: "keep_monthly", label: "Mensuales" },
    { key: "keep_yearly", label: "Anuales" },
  ];

  const PRESETS_WITHIN: { label: string; hint: string; policy: Partial<Policy> }[] = [
    {
      label: "Frecuente",
      hint: "horarias 15 días · diarias 1 año · mensuales siempre",
      policy: { keep_within_hourly: "15d", keep_within_daily: "1y", keep_monthly: UNLIMITED },
    },
    { label: "Nube ligera", hint: "diarias 60 días · mensuales siempre", policy: { keep_within_daily: "60d", keep_monthly: UNLIMITED } },
    {
      label: "Equilibrada",
      hint: "diarias 30 días · semanales 6 meses · mensuales 2 años",
      policy: { keep_within_daily: "30d", keep_within_weekly: "6m", keep_within_monthly: "2y" },
    },
    { label: "Solo recientes", hint: "todas las de los últimos 90 días", policy: { keep_within: "90d" } },
  ];
  const PRESETS_COUNT: { label: string; hint: string; policy: Partial<Policy> }[] = [
    { label: "Equilibrada", hint: "7 diarias, 4 semanales, 12 mensuales, 3 anuales", policy: { keep_daily: 7, keep_weekly: 4, keep_monthly: 12, keep_yearly: 3 } },
    { label: "Ligera", hint: "7 diarias y 4 semanales", policy: { keep_daily: 7, keep_weekly: 4 } },
    { label: "Amplia", hint: "30 diarias, 8 semanales, 24 mensuales, 10 anuales", policy: { keep_daily: 30, keep_weekly: 8, keep_monthly: 24, keep_yearly: 10 } },
  ];

  function parseDuration(d: string | null | undefined): Row {
    const m = /^(\d+)([dmy])$/.exec(d ?? "");
    return m ? { n: Number(m[1]), unit: m[2] as Unit } : { n: 0, unit: "d" };
  }

  type GroupMode = "default" | "all" | "tags" | "custom";
  type FilterMode = "all" | "tags" | "host" | "paths";

  // Estado del formulario, cargado desde una política.
  let mode = $state<"within" | "count">("within");
  let rows = $state<Record<Period, Row>>({} as Record<Period, Row>);
  let counts = $state<Record<CountKey, { n: number; unlimited: boolean }>>({} as Record<CountKey, { n: number; unlimited: boolean }>);
  let countWithin = $state<Row>({ n: 0, unit: "d" });
  let groupMode = $state<GroupMode>("default");
  let groupKeys = $state<{ host: boolean; paths: boolean; tags: boolean }>({ host: true, paths: true, tags: false });
  let filterMode = $state<FilterMode>("all");
  let filterTags = $state<string[]>([]);
  let filterHost = $state("");
  let filterPaths = $state<string[]>([]);
  let keepTags = $state<string[]>([]);

  function load(p: Partial<Policy>) {
    const n = normalizePolicy(p);
    // «Por cantidad» si usa cantidades que «Por plazos» no puede expresar (en plazos solo caben
    // «una por mes / año siempre»).
    const periodRules = !!(n.keep_within_hourly || n.keep_within_daily || n.keep_within_weekly || n.keep_within_monthly || n.keep_within_yearly);
    const plainCounts = COUNT_KEYS.some((k) => n[k] !== 0 && !((k === "keep_monthly" || k === "keep_yearly") && n[k] === UNLIMITED));
    mode = plainCounts && !periodRules ? "count" : "within";
    const r = {} as Record<Period, Row>;
    for (const p of PERIODS) {
      const row = parseDuration(n[withinKey(p.key)]);
      if (p.always && n[`keep_${p.key}` as CountKey] === UNLIMITED) row.unit = "always";
      if (row.unit === "always") row.n = row.n || 1;
      r[p.key] = row;
    }
    rows = r;
    const c = {} as Record<CountKey, { n: number; unlimited: boolean }>;
    for (const k of COUNT_KEYS) c[k] = { n: n[k] > 0 ? n[k] : 0, unlimited: n[k] === UNLIMITED };
    counts = c;
    countWithin = parseDuration(n.keep_within);
    groupMode = !n.group_by ? "default" : n.group_by.length === 0 ? "all" : n.group_by.join() === "tags" ? "tags" : "custom";
    groupKeys = { host: !!n.group_by?.includes("host"), paths: !!n.group_by?.includes("paths"), tags: !!n.group_by?.includes("tags") };
    if (groupMode !== "custom") groupKeys = { host: true, paths: true, tags: false };
    filterTags = [...(n.filter_tags ?? [])];
    filterHost = n.filter_host ?? "";
    filterPaths = [...(n.filter_paths ?? [])];
    filterMode = filterTags.length ? "tags" : filterHost ? "host" : filterPaths.length ? "paths" : "all";
    keepTags = [...(n.keep_tags ?? [])];
  }
  // Sin política guardada se propone «Frecuente».
  // svelte-ignore state_referenced_locally
  load(repo.retention ?? PRESETS_WITHIN[0].policy);

  /** La política que describe el formulario. */
  const draft = $derived.by<Policy>(() => {
    const p: Policy = { ...EMPTY_POLICY };
    if (mode === "within") {
      for (const per of PERIODS) {
        const row = rows[per.key];
        if (!row) continue;
        if (row.unit === "always") {
          if (per.key === "monthly" || per.key === "yearly") p[`keep_${per.key}`] = UNLIMITED;
        } else if (row.n > 0) {
          p[withinKey(per.key)] = `${Math.floor(row.n)}${row.unit}`;
        }
      }
    } else {
      for (const k of COUNT_KEYS) p[k] = counts[k]?.unlimited ? UNLIMITED : Math.max(0, Math.floor(Number(counts[k]?.n) || 0));
      if (countWithin.n > 0 && countWithin.unit !== "always") p.keep_within = `${Math.floor(countWithin.n)}${countWithin.unit}`;
    }
    p.group_by =
      groupMode === "default"
        ? null
        : groupMode === "all"
          ? []
          : groupMode === "tags"
            ? ["tags"]
            : (["host", "paths", "tags"] as const).filter((k) => groupKeys[k]);
    p.filter_tags = filterMode === "tags" ? [...filterTags] : [];
    p.filter_host = filterMode === "host" ? filterHost.trim() || null : null;
    p.filter_paths = filterMode === "paths" ? [...filterPaths] : [];
    p.keep_tags = [...keepTags];
    return normalizePolicy(p);
  });

  const empty = $derived(isEmptyPolicy(draft));
  const dirty = $derived(repo.retention ? !samePolicy(draft, repo.retention) : !empty);
  const summary = $derived(policySummary(draft));

  function applyPreset(p: Partial<Policy>) {
    // Un ajuste rápido cambia las reglas; agrupar, filtrar y proteger se conservan.
    load({ ...p, group_by: draft.group_by, filter_tags: draft.filter_tags, filter_host: draft.filter_host, filter_paths: draft.filter_paths, keep_tags: draft.keep_tags });
  }

  // ---------- Sugerencias: etiquetas, equipos y carpetas que ya existen ----------

  onMount(() => {
    if (!health[repo.id]) refresh(repo.id);
  });
  const snaps = $derived(health[repo.id]?.snapshots ?? []);
  const knownTags = $derived(
    [...new Set([...snaps.flatMap((s) => s.tags ?? []), ...(repo.plans ?? []).flatMap((p) => p.tags)])].sort((a, b) => a.localeCompare(b)),
  );
  /** Etiquetas de las copias (planes) de este destino: se ofrecen como atajo. */
  const planTags = $derived([...new Set((repo.plans ?? []).flatMap((p) => p.tags))]);
  const knownHosts = $derived([...new Set(snaps.map((s) => s.hostname))].sort());
  const knownPaths = $derived([...new Set(snaps.flatMap((s) => s.paths ?? []))].sort());

  // Entradas de las listas (etiquetas y carpetas).
  let tagInput = $state("");
  let pathInput = $state("");
  let keepInput = $state("");
  let listError = $state("");
  const tagProblem = (t: string) =>
    /[\s,'"`$\\]/.test(t) || t.startsWith("-") ? `«${t}»: las etiquetas no llevan espacios, comas ni comillas.` : "";
  const valueProblem = (v: string) => (/['"`$\n\r]/.test(v) || v.startsWith("-") ? `«${v}»: sin comillas, $ ni saltos de línea.` : "");

  function addTo(list: string[], value: string, check: (v: string) => string): string[] {
    const v = value.trim();
    if (!v) return list;
    const problem = check(v);
    if (problem) {
      listError = problem;
      return list;
    }
    listError = "";
    return list.includes(v) ? list : [...list, v];
  }
  function onListKey(e: KeyboardEvent, add: () => void) {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      add();
    }
  }

  // ---------- Vista previa ----------

  let preview = $state<RetentionPreview | null>(null);
  let loading = $state(false);
  let error = $state("");
  let show = $state<"all" | "keep" | "remove">("all");

  const kept = $derived(preview?.items.filter((i) => i.keep) ?? []);
  const removed = $derived(preview?.items.filter((i) => !i.keep) ?? []);
  const outside = $derived(kept.filter((i) => i.reasons.includes("outside filter")).length);
  const shown = $derived(show === "keep" ? kept : show === "remove" ? removed : (preview?.items ?? []));

  /** Línea de tiempo: posición de cada snapshot entre el más antiguo y el más reciente. */
  const timeline = $derived.by(() => {
    const items = preview?.items ?? [];
    if (items.length < 2) return [];
    const times = items.map((i) => new Date(i.snapshot.time).getTime());
    const min = Math.min(...times);
    const span = Math.max(1, Math.max(...times) - min);
    return items.map((i, n) => ({ keep: i.keep, x: ((times[n] - min) / span) * 100, id: i.snapshot.id, time: i.snapshot.time }));
  });
  const oldest = $derived(preview?.items.at(-1)?.snapshot.time);
  const newest = $derived(preview?.items[0]?.snapshot.time);

  // Vista previa automática (con una pequeña espera mientras se editan los números).
  let request = 0;
  $effect(() => {
    const snapshot = JSON.stringify(draft);
    if (empty) {
      preview = null;
      error = "";
      return;
    }
    const mine = ++request;
    loading = true;
    const t = setTimeout(async () => {
      try {
        const result = await api.retentionPreview(repo.id, JSON.parse(snapshot));
        if (mine === request) {
          preview = result;
          error = "";
        }
      } catch (e) {
        if (mine === request) error = String(e);
      } finally {
        if (mine === request) loading = false;
      }
    }, 450);
    return () => clearTimeout(t);
  });

  // ---------- Guardar y aplicar ----------

  async function save() {
    const done = await withPassword({
      title: "Guardar la política de retención",
      message: `Cambiar la configuración de «${repo.name}» requiere su contraseña. Guardar no borra ninguna versión.`,
      repoName: repo.name,
      confirmLabel: "Guardar",
      action: async (password) => onchange(await api.setRetention(repo.id, empty ? null : draft, password)),
    });
    if (done) {
      toast(`Política de retención de «${repo.name}» guardada`);
      onsaved?.();
    }
  }

  const isRest = $derived(restRepoPath(repo.location) !== null);
  /** Para una terminal de este equipo (Windows): valores vacíos o con espacios entre comillas dobles. */
  const localArgs = $derived(policyArgs(draft).map((a) => (/^[A-Za-z0-9_./:,@%+=\\-]+$/.test(a) ? a : `"${a}"`)).join(" "));
  const command = $derived(isRest ? `restic forget --prune ${policyShellArgs(draft)}` : `restic forget --prune ${localArgs}`);
  const server = $derived(serverCommands(draft, repo.name, repo.location));

  let copied = $state<"" | "cmd" | "server">("");
  async function copy(text: string, which: "cmd" | "server") {
    try {
      await navigator.clipboard.writeText(text);
      copied = which;
      setTimeout(() => (copied = ""), 1400);
    } catch {
      /* sin portapapeles */
    }
  }
</script>

{#snippet chips(list: string[], remove: (v: string) => void)}
  {#each list as v (v)}
    <span class="chip">{v}<button class="chip-x" aria-label="Quitar {v}" onclick={() => remove(v)}><X size={11} /></button></span>
  {/each}
{/snippet}

<section class="panel" class:card={!embedded} class:embedded in:fade={{ duration: dur(150) }}>
  <header>
    <div>
      <h2>
        Política de retención
        {#if !repo.retention}
          <span class="badge badge-sm">Propuesta</span>
        {:else if dirty}
          <span class="badge badge-sm tone-warn">Cambios sin guardar</span>
        {:else}
          <span class="badge badge-sm tone-success"><Check size={11} strokeWidth={3} /> Guardada en Resguardo</span>
        {/if}
        <HelpLink topic="retencion-plazos" label="la retención por plazos" />
      </h2>
      <p class="faint">
        Qué versiones de «{repo.name}» conservar al hacer limpieza (<code>restic forget</code>). Guardarla no borra nada; aquí ves
        lo que pasaría al aplicarla.
      </p>
    </div>
    {#if dirty}
      <button class="btn btn-primary btn-sm" onclick={save} transition:fade={{ duration: dur(120) }}><Lock size={13} /> Guardar política</button>
    {/if}
  </header>

  <div class="segmented inline" role="group" aria-label="Tipo de política">
    <button class:on={mode === "within"} aria-pressed={mode === "within"} onclick={() => (mode = "within")}>Por plazos (recomendado)</button>
    <button class:on={mode === "count"} aria-pressed={mode === "count"} onclick={() => (mode = "count")}>Por cantidad</button>
  </div>

  <div class="presets">
    {#each mode === "within" ? PRESETS_WITHIN : PRESETS_COUNT as p}
      <button class="preset" onclick={() => applyPreset(p.policy)} title={p.hint}><strong>{p.label}</strong> <span class="faint">· {p.hint}</span></button>
    {/each}
    <button class="preset" onclick={() => applyPreset({})}>Vaciar</button>
  </div>

  {#if mode === "within"}
    <div class="rows">
      {#each PERIODS as per (per.key)}
        {@const row = rows[per.key]}
        <div class="rrow" class:active={row.unit === "always" || row.n > 0}>
          <span class="rlabel">{per.label}</span>
          <input
            class="input num"
            type="number"
            min="0"
            max="999"
            aria-label="{per.label} (cantidad)"
            bind:value={row.n}
            disabled={row.unit === "always"}
            style:visibility={row.unit === "always" ? "hidden" : null}
          />
          <select class="input" aria-label="{per.label} (unidad)" bind:value={row.unit}>
            <option value="d">{row.n === 1 ? "día" : "días"}</option>
            <option value="m">{row.n === 1 ? "mes" : "meses"}</option>
            <option value="y">{row.n === 1 ? "año" : "años"}</option>
            {#if per.always}<option value="always">siempre</option>{/if}
          </select>
        </div>
      {/each}
      <p class="faint small">0 = no se usa esa regla. Cada regla conserva la última versión de cada hora, día, semana… del periodo indicado, contado desde la versión más reciente.</p>
    </div>
  {:else}
    <div class="fields">
      {#each COUNTS as f (f.key)}
        {@const c = counts[f.key]}
        <label class="num-field" class:active={c.unlimited || c.n > 0}>
          <span>{f.label}</span>
          <input class="input" type="number" min="0" max="9999" bind:value={c.n} disabled={c.unlimited} />
          <span class="unl"><input type="checkbox" bind:checked={c.unlimited} /> todas</span>
        </label>
      {/each}
      <label class="num-field within" class:active={countWithin.n > 0}>
        <span>Todo lo de los últimos</span>
        <span class="within-row">
          <input class="input" type="number" min="0" max="999" bind:value={countWithin.n} />
          <select class="input" bind:value={countWithin.unit}>
            <option value="d">días</option>
            <option value="m">meses</option>
            <option value="y">años</option>
          </select>
        </span>
      </label>
    </div>
  {/if}

  <Advanced
    id="retencion"
    hint={groupMode !== "default" || filterMode !== "all" || keepTags.length > 0 ? "A qué versiones se aplica y cómo se agrupan (personalizado)" : "A todas las versiones, agrupadas por equipo y carpetas"}
    custom={groupMode !== "default" || filterMode !== "all" || keepTags.length > 0}
  >
    <div class="more-body">
      <div class="opt">
        <span class="opt-label">Aplicar a</span>
        <div class="radios">
          {#each [["all", "Todas las versiones"], ["tags", "Solo las que tengan alguna de estas etiquetas…"], ["host", "Solo de este equipo…"], ["paths", "Solo de estas carpetas…"]] as [v, label]}
            <label class="radio"><input type="radio" name="filter-{repo.id}" value={v} bind:group={filterMode} /> {label}</label>
          {/each}
        </div>
        {#if filterMode === "tags"}
          <div class="list-input">
            {@render chips(filterTags, (v) => (filterTags = filterTags.filter((x) => x !== v)))}
            <input
              class="input"
              list="ret-tags-{repo.id}"
              placeholder="etiqueta"
              bind:value={tagInput}
              onkeydown={(e) => onListKey(e, () => ((filterTags = addTo(filterTags, tagInput, tagProblem)), (tagInput = "")))}
            />
            <button class="btn btn-sm" onclick={() => ((filterTags = addTo(filterTags, tagInput, tagProblem)), (tagInput = ""))}><Plus size={13} /> Añadir</button>
          </div>
          {#if planTags.length}
            <div class="picks">
              <span class="faint">Etiquetas de las copias:</span>
              {#each planTags as t (t)}
                <button class="pick" disabled={filterTags.includes(t)} onclick={() => (filterTags = addTo(filterTags, t, tagProblem))}>{t}</button>
              {/each}
            </div>
          {/if}
        {:else if filterMode === "host"}
          <input class="input host-input" list="ret-hosts-{repo.id}" placeholder="Nombre del equipo" bind:value={filterHost} />
        {:else if filterMode === "paths"}
          <div class="list-input">
            {@render chips(filterPaths, (v) => (filterPaths = filterPaths.filter((x) => x !== v)))}
            <input
              class="input mono"
              list="ret-paths-{repo.id}"
              placeholder="Carpeta tal como aparece en las versiones"
              bind:value={pathInput}
              onkeydown={(e) => e.key === "Enter" && (e.preventDefault(), (filterPaths = addTo(filterPaths, pathInput, valueProblem)), (pathInput = ""))}
            />
            <button class="btn btn-sm" onclick={() => ((filterPaths = addTo(filterPaths, pathInput, valueProblem)), (pathInput = ""))}><Plus size={13} /> Añadir</button>
          </div>
        {/if}
        {#if filterMode !== "all"}<p class="faint small">Las demás versiones no se tocan nunca.</p>{/if}
      </div>

      <div class="opt">
        <span class="opt-label">Nunca borrar las versiones con la etiqueta…</span>
        <div class="list-input">
          {@render chips(keepTags, (v) => (keepTags = keepTags.filter((x) => x !== v)))}
          <input
            class="input"
            list="ret-tags-{repo.id}"
            placeholder="p. ej. conservar"
            bind:value={keepInput}
            onkeydown={(e) => onListKey(e, () => ((keepTags = addTo(keepTags, keepInput, tagProblem)), (keepInput = "")))}
          />
          <button class="btn btn-sm" onclick={() => ((keepTags = addTo(keepTags, keepInput, tagProblem)), (keepInput = ""))}><Plus size={13} /> Añadir</button>
        </div>
      </div>

      <div class="opt">
        <span class="opt-label">Agrupar</span>
        <select class="input group-select" bind:value={groupMode} aria-label="Agrupación">
          <option value="default">Por equipo y carpetas (predeterminado de restic)</option>
          <option value="all">Todas juntas, un solo grupo</option>
          <option value="tags">Por etiquetas</option>
          <option value="custom">Personalizado…</option>
        </select>
        {#if groupMode === "custom"}
          <div class="radios inline">
            <label class="radio"><input type="checkbox" bind:checked={groupKeys.host} /> equipo</label>
            <label class="radio"><input type="checkbox" bind:checked={groupKeys.paths} /> carpetas</label>
            <label class="radio"><input type="checkbox" bind:checked={groupKeys.tags} /> etiquetas</label>
          </div>
        {/if}
        <p class="faint small">
          Por defecto restic aplica la política por separado a cada equipo y carpeta de origen. Elige «Todas juntas» si el repositorio recibe versiones de
          varios orígenes (p. ej. un script antiguo) y quieres una sola política para todo.
        </p>
      </div>
      {#if listError}<p class="err small" role="alert">{listError}</p>{/if}
    </div>
  </Advanced>

  <datalist id="ret-tags-{repo.id}">{#each knownTags as t (t)}<option value={t}></option>{/each}</datalist>
  <datalist id="ret-hosts-{repo.id}">{#each knownHosts as h (h)}<option value={h}></option>{/each}</datalist>
  <datalist id="ret-paths-{repo.id}">{#each knownPaths as p (p)}<option value={p}></option>{/each}</datalist>

  <p class="policy-summary" aria-live="polite"><Info size={14} /> <span>{summary}</span></p>

  {#if empty}
    <div class="notice notice-info"><Info size={16} /><p>Elige un ajuste rápido o indica algún plazo o cantidad para ver qué versiones se conservarían.</p></div>
  {:else if error}
    <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>
  {:else if preview}
    <div class="summary" class:stale={loading}>
      <button class="tile keep" class:on={show === "keep"} onclick={() => (show = show === "keep" ? "all" : "keep")}>
        <Archive size={18} />
        <span><strong>{formatNumber(kept.length)}</strong> se conservan{#if outside}<span class="faint">&nbsp;({formatNumber(outside)} fuera del filtro)</span>{/if}</span>
      </button>
      <button class="tile remove" class:on={show === "remove"} onclick={() => (show = show === "remove" ? "all" : "remove")}>
        <Trash2 size={18} />
        <span><strong>{formatNumber(removed.length)}</strong> se eliminarían</span>
      </button>
      {#if loading}<span class="spin loader"><LoaderCircle size={16} /></span>{/if}
    </div>

    {#if timeline.length}
      <div class="timeline" aria-hidden="true">
        <div class="track">
          {#each timeline as t (t.id)}
            <span class="tick" class:keep={t.keep} style:left="{t.x}%" title={formatDate(t.time)}></span>
          {/each}
        </div>
        <div class="ends faint"><span>{oldest ? formatDate(oldest) : ""}</span><span>{newest ? formatDate(newest) : ""}</span></div>
      </div>
    {/if}

    {#if preview.groups > 1}
      <p class="faint groups">
        <Info size={13} /> restic aplica la política por separado a cada grupo ({preview.groups} grupos). Para una sola política para todo, elige
        «Todas juntas, un solo grupo» en «Agrupar».
      </p>
    {/if}

    <div class="items">
      {#each shown as item, i (item.snapshot.id)}
        {@const day = formatDay(item.snapshot.time)}
        {#if i === 0 || formatDay(shown[i - 1].snapshot.time) !== day}
          <div class="day">{day}</div>
        {/if}
        <div class="item" class:removed={!item.keep}>
          <span class="time">{formatTime(item.snapshot.time)}</span>
          <span class="id mono">{item.snapshot.short_id}</span>
          <span class="reasons">
            {#if item.keep}
              {#each [...new Set(item.reasons.map(reasonLabel))] as r}<span class="reason" class:neutral={r === "Fuera del filtro"}>{r}</span>{/each}
            {:else}
              <span class="gone">Se eliminaría</span>
            {/if}
          </span>
          <span class="host faint">{item.snapshot.hostname}</span>
        </div>
      {/each}
    </div>
  {:else}
    <div class="loading faint"><span class="spin"><LoaderCircle size={16} /></span> Calculando con restic…</div>
  {/if}

  {#if !empty && !embedded}
    <div class="apply" transition:slide={{ duration: dur(150) }}>
      {#if isRest}
        <p>
          <strong><Server size={14} /> Aplicar esta política en el servidor</strong> — un servidor REST en modo solo-añadir
          (<code>--append-only</code>) no deja borrar desde aquí. Ejecuta esto en la máquina (o el contenedor) de rest-server, con la ruta real del
          repositorio en lugar de <code>/ruta/de/rest-server</code>:
        </p>
        <div class="cmd block">
          <pre class="selectable">{server}</pre>
          <button class="icon-btn" onclick={() => copy(server, "server")} title="Copiar los comandos" aria-label="Copiar los comandos">
            {#if copied === "server"}<Check size={14} />{:else}<Copy size={14} />{/if}
          </button>
        </div>
        <p class="faint tip">Revisa el paso 1 (<code>--dry-run</code>) antes del 2: lo que se olvida y se libera con <code>prune</code> no se puede recuperar.</p>
      {:else}
        <p>
          <strong>Para aplicarla</strong> ejecuta este comando en una terminal con acceso a este repositorio (Resguardo no aplica la retención de un repositorio
          por sí mismo; sí la de la copia externa):
        </p>
        <div class="cmd">
          <code class="selectable">{command}</code>
          <button class="icon-btn" onclick={() => copy(command, "cmd")} title="Copiar comando" aria-label="Copiar el comando">
            {#if copied === "cmd"}<Check size={14} />{:else}<Copy size={14} />{/if}
          </button>
        </div>
        <p class="faint tip">Prueba primero con <code>--dry-run</code> en lugar de <code>--prune</code>.</p>
      {/if}
    </div>
  {/if}
</section>

<style>
  .panel {
    padding: 20px 22px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 16px;
  }
  h2 {
    font-size: var(--fs-h2);
    font-weight: 650;
  }
  header p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  header p {
    max-width: 620px;
    line-height: 1.5;
  }
  .presets {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .preset {
    height: 28px;
    padding: 0 12px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
    cursor: pointer;
  }
  .preset:hover {
    color: var(--accent-text);
    border-color: var(--accent);
  }
  .fields {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(92px, 1fr));
    gap: 8px;
  }
  .num-field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 9px 10px 10px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    transition:
      border-color 0.15s,
      background 0.15s;
  }
  .num-field > span:first-child {
    font-size: var(--fs-xs);
    font-weight: 650;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .num-field.active {
    border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
    background: var(--accent-soft);
  }
  .num-field.active > span:first-child {
    color: var(--accent-text);
  }
  .num-field .input {
    height: 32px;
    padding: 0 8px;
    font-size: var(--fs-h2);
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }
  .within-row {
    display: flex;
    gap: 6px;
  }
  .within-row input {
    width: 64px;
    flex: none;
  }
  .within-row select {
    font-size: var(--fs-sm);
    font-weight: 500;
  }
  .within {
    grid-column: span 2;
  }

  .summary {
    display: flex;
    align-items: center;
    gap: 10px;
    transition: opacity 0.15s;
  }
  .summary.stale {
    opacity: 0.6;
  }
  .tile {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 16px;
    font: inherit;
    text-align: left;
    border: 1.5px solid transparent;
    border-radius: var(--radius);
    cursor: pointer;
  }
  .tile strong {
    font-family: var(--font-display);
    font-size: 20px;
    margin-right: 4px;
  }
  .tile.keep {
    color: var(--ok);
    background: var(--ok-soft);
  }
  .tile.remove {
    color: var(--bad);
    background: var(--bad-soft);
  }
  .tile.on {
    border-color: currentColor;
  }
  .tile span {
    color: var(--text-1);
  }
  .loader {
    display: grid;
    color: var(--text-3);
  }

  .timeline {
    padding: 4px 6px 0;
  }
  .track {
    position: relative;
    height: 22px;
    border-radius: 999px;
    background: var(--surface-2);
    border: 1px solid var(--border);
  }
  .tick {
    position: absolute;
    top: 50%;
    width: 6px;
    height: 6px;
    translate: -50% -50%;
    border-radius: 50%;
    background: var(--bad);
    opacity: 0.45;
  }
  .tick.keep {
    width: 10px;
    height: 10px;
    background: var(--ok);
    opacity: 1;
    box-shadow: 0 0 0 2px var(--surface);
    z-index: 1;
  }
  .ends {
    display: flex;
    justify-content: space-between;
    margin-top: 4px;
    font-size: var(--fs-xs);
  }
  .groups {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: -6px 0 0;
    font-size: var(--fs-sm);
  }

  .items {
    max-height: 360px;
    overflow: auto;
    margin: 0 -6px;
    padding: 0 6px;
  }
  .day {
    position: sticky;
    top: 0;
    padding: 10px 0 4px;
    font-size: var(--fs-xs);
    font-weight: 650;
    color: var(--text-2);
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }
  .item {
    display: grid;
    grid-template-columns: 52px 92px minmax(0, 1fr) 150px;
    align-items: center;
    gap: 10px;
    padding: 7px 0;
    border-bottom: 1px solid var(--border);
  }
  .time {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .id {
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .reasons {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .reason {
    padding: 0 8px;
    font-size: var(--fs-xs);
    font-weight: 600;
    line-height: 20px;
    border-radius: 999px;
    color: var(--ok);
    background: var(--ok-soft);
  }
  .gone {
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--bad);
  }
  .item.removed .time,
  .item.removed .id {
    color: var(--text-3);
    text-decoration: line-through;
  }
  .host {
    font-size: var(--fs-sm);
    text-align: right;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .loading {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 20px 0;
  }
  .loading .spin {
    display: grid;
  }

  .apply {
    padding: 14px 16px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-size: var(--fs-sm);
  }
  .apply p {
    margin: 0;
    line-height: 1.55;
  }
  .cmd {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 10px 0 8px;
    padding: 8px 8px 8px 12px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }
  .cmd code {
    flex: 1;
    font-size: var(--fs-sm);
    overflow-x: auto;
    white-space: nowrap;
  }
  .tip {
    font-size: var(--fs-sm);
  }
  .embedded {
    padding: 0;
  }
  .preset strong {
    font-weight: 600;
  }
  /* Por plazos: una fila por regla. */
  .rows {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .rrow {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 76px 110px;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    font-size: var(--fs-sm);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    transition:
      border-color 0.15s,
      background 0.15s;
  }
  .rrow.active {
    border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
    background: var(--accent-soft);
  }
  .rrow .input {
    height: 32px;
  }
  .rrow .num {
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }
  .small {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .unl {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .more-body {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .opt {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .opt-label {
    font-size: var(--fs-sm);
    font-weight: 650;
    color: var(--text-2);
  }
  .list-input {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .list-input .input {
    flex: 1 1 180px;
    height: 32px;
  }
  .host-input,
  .group-select {
    max-width: 380px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 0 4px 0 9px;
    font-size: var(--fs-sm);
    font-weight: 600;
    line-height: 24px;
    border-radius: 999px;
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .chip-x {
    display: grid;
    padding: 2px;
    color: inherit;
    background: none;
    border: 0;
    border-radius: 50%;
    cursor: pointer;
  }
  .picks {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .pick {
    padding: 0 9px;
    font: inherit;
    font-size: var(--fs-xs);
    line-height: 22px;
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
    cursor: pointer;
  }
  .pick:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .policy-summary {
    display: flex;
    gap: 8px;
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.5;
    font-weight: 550;
  }
  .policy-summary :global(svg) {
    flex: none;
    margin-top: 3px;
    color: var(--accent);
  }
  .reason.neutral {
    color: var(--text-2);
    background: var(--surface-3);
  }
  .apply strong :global(svg) {
    vertical-align: -2px;
  }
  .cmd.block {
    align-items: flex-start;
  }
  .cmd pre {
    flex: 1;
    margin: 0;
    font-family: var(--mono);
    font-size: var(--fs-xs);
    line-height: 1.55;
    overflow-x: auto;
    white-space: pre;
  }
  @media (max-width: 700px) {
    .rrow {
      grid-template-columns: minmax(0, 1fr) 64px 96px;
    }
  }
</style>
