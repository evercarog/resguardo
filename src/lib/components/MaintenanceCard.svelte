<script lang="ts">
  import { onMount, untrack } from "svelte";
  import Advanced from "./Advanced.svelte";
  import { slide } from "svelte/transition";
  import {
    CircleAlert,
    ArchiveRestore,
    CloudUpload,
    Info,
    LoaderCircle,
    Play,
    ShieldCheck,
    ShieldAlert,
    TriangleAlert,
    Wrench,
  } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { AgentRun, Repo, Schedule } from "$lib/api";
  import { agent, nextRun, offsiteSources, offsiteTargetName, refreshAgent, scheduleLabel } from "$lib/agent.svelte";
  import { pendingEditor } from "$lib/intent.svelte";
  import { pauseOf } from "$lib/pause.svelte";
  import { formatDate } from "$lib/format";
  import { dur } from "$lib/motion";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { toast } from "$lib/toast.svelte";
  import { repoKind } from "$lib/repoKind";
  import HelpLink from "./HelpLink.svelte";
  import RunResult from "./RunResult.svelte";
  import Modal from "./Modal.svelte";
  import CollapseToggle from "./CollapseToggle.svelte";
  import { ui, setUi } from "$lib/ui.svelte";
  import TaskProgress from "./TaskProgress.svelte";
  import RetentionPanel from "./RetentionPanel.svelte";
  import { policySummary, samePolicy } from "$lib/retention";
  import RelTime from "./RelTime.svelte";

  // Mantenimiento programado del agente: verificación (restic check) y copia
  // externa (restic copy a otro repositorio, p. ej. S3). La retención del
  // repositorio principal la hace el servidor si es append-only.
  // `repos`: todos los destinos, para poder subir la copia externa a otro de ellos.
  // `onchange`: un destino se guardó (p. ej. la retención del destino de la copia externa).
  let { repo, repos = [], onchange }: { repo: Repo; repos?: Repo[]; onchange?: (repo: Repo) => void } = $props();

  const info = $derived(agent.info);
  const current = $derived(info?.repos.find((r) => r.id === repo.id) ?? null);
  const tasks = $derived(info?.tasks ?? null);
  /** Copias automáticas del destino en pausa: tampoco se verifica ni se sube (salvo con «Ahora»). */
  const paused = $derived(info ? pauseOf(repo.id) : null);
  const verifyRun = $derived<AgentRun | null>(tasks?.runs[`verify:${repo.id}`] ?? null);
  const offsiteRun = $derived<AgentRun | null>(tasks?.runs[`offsite:${repo.id}`] ?? null);
  const cloudVerifyRun = $derived<AgentRun | null>(tasks?.runs[`verify_offsite:${repo.id}`] ?? null);
  const restoreRun = $derived<AgentRun | null>(tasks?.runs[`restore_test:${repo.id}`] ?? null);
  /** Destinos que suben su copia externa a este: aquí no hacen falta copias propias. */
  const sources = $derived(offsiteSources(repo.id));
  let section: HTMLElement | undefined = $state();
  // «Gestionar desde «X»» (desde el destino de la copia externa): se muestra esta tarjeta.
  $effect(() => {
    if (pendingEditor.maintenance === repo.id && section) {
      pendingEditor.maintenance = null;
      setUi("maintCollapsed", false);
      section.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  });
  // Abrir directamente el editor de una tarea (esquema del destino, «Mejorar la protección»).
  $effect(() => {
    const want = pendingEditor.maintEditor;
    if (!want || want.repoId !== repo.id || !section || !info) return;
    pendingEditor.maintEditor = null;
    setUi("maintCollapsed", false);
    untrack(() => {
      if (want.which === "verify") startVerify();
      else if (want.which === "restore_test") startRestore();
      else startOffsite();
    });
    section.scrollIntoView({ behavior: "smooth", block: "start" });
  });
  /** Tarea en curso de este repositorio (si da señales de vida). */
  const running = $derived.by(() => {
    const r = tasks?.running;
    if (!r || r.repo_id !== repo.id) return null;
    return Date.now() - new Date(r.updated ?? r.started).getTime() < 15 * 60_000 ? r : null;
  });

  // «Ejecutar ahora» pedido: se consulta a menudo hasta que el agente lo tome.
  let pending = $state<"verify" | "offsite" | "verify_offsite" | "restore_test" | null>(null);
  onMount(() => {
    const t = setInterval(() => {
      if (running || pending) refreshAgent();
    }, 3_000);
    return () => clearInterval(t);
  });
  $effect(() => {
    if (pending && running?.kind === pending) pending = null;
  });

  const WEEKDAYS = ["Lunes", "Martes", "Miércoles", "Jueves", "Viernes", "Sábado", "Domingo"];

  /** Horario editable (sin «solo vigilar»). «Después de cada copia» solo en la copia externa. */
  function scheduleDraft(s: Schedule | undefined, fallback: Schedule) {
    const base = s && s.kind !== "monitor" && s.kind !== "plans" ? s : fallback;
    return {
      kind: base.kind as "hours" | "daily" | "weekly" | "after_backup",
      every: base.kind === "hours" ? base.every : 24,
      time: base.kind === "daily" || base.kind === "weekly" ? base.time : "03:00",
      weekday: base.kind === "weekly" ? base.weekday : 6,
      minMinutes: base.kind === "after_backup" ? base.min_minutes : 30,
    };
  }
  function toSchedule(d: ReturnType<typeof scheduleDraft>): Schedule {
    if (d.kind === "after_backup") return { kind: "after_backup", min_minutes: Math.min(1440, Math.max(0, Math.floor(d.minMinutes) || 0)) };
    if (d.kind === "hours") return { kind: "hours", every: Math.max(1, Math.floor(d.every) || 1) };
    if (d.kind === "daily") return { kind: "daily", time: d.time };
    return { kind: "weekly", weekday: d.weekday, time: d.time };
  }

  // ---------- Verificación ----------
  let editVerify = $state(false);
  let vSched = $state(scheduleDraft(undefined, { kind: "weekly", weekday: 6, time: "03:00" }));
  let vPercent = $state(5);
  /** Qué datos se leen: solo la estructura, una parte al azar o rotativa (todo cada N verificaciones). */
  let vMode = $state<"structure" | "random" | "rotate">("rotate");
  let vParts = $state(4);
  let vError = $state("");

  /** En la nube, leer datos cuesta tráfico de descarga: por defecto, solo la estructura. */
  const isCloud = $derived(/^(s3|b2|azure|gs|swift|rclone):/i.test(repo.location.trim()));

  /** «Opciones avanzadas» de la verificación: qué se lee y si no es lo recomendado. */
  const verifyHint = $derived(
    vMode === "rotate" ? `Rotativa: todos los datos cada ${vParts} verificaciones` : vMode === "random" ? `Una parte al azar: ${vPercent} %` : "Solo la estructura",
  );
  const verifyCustom = $derived(vMode !== (isCloud ? "structure" : "rotate") || (vMode === "rotate" && vParts !== 4));

  function startVerify() {
    const v = current?.verify;
    vSched = scheduleDraft(v?.schedule, { kind: "weekly", weekday: 6, time: "03:00" });
    vPercent = v?.subset_percent || 5;
    vParts = v?.rotate_parts || 4;
    vMode = v ? (v.rotate_parts ? "rotate" : v.subset_percent ? "random" : "structure") : isCloud ? "structure" : "rotate";
    vError = "";
    editVerify = true;
  }

  /** «semanal + 4 partes = todos los datos cada 4 semanas, ~25 % cada vez». */
  function coverageHint(s: Schedule, parts: number) {
    const n = Math.max(2, Math.min(52, Math.round(parts) || 4));
    const pct = Math.round(100 / n);
    const every =
      s.kind === "weekly"
        ? ["semanal", `${n} semanas`]
        : s.kind === "daily"
          ? ["diaria", `${n} días`]
          : s.kind === "hours"
            ? [`cada ${s.every} h`, s.every * n >= 48 ? `${Math.round((s.every * n) / 24)} días` : `${s.every * n} horas`]
            : null;
    return every ? `${every[0]} + ${n} partes = todos los datos cada ${every[1]}, ~${pct} % cada vez` : `~${pct} % cada vez`;
  }

  /** Avance de una rotativa (misma regla que el agente: si cambió el número de partes, empieza de 1). */
  function rotationOf(parts: number | undefined, key: string) {
    if (!parts) return null;
    const r = tasks?.rotation?.[key];
    const next = r && r.parts === parts && r.next_part >= 1 && r.next_part <= parts ? r.next_part : 1;
    return { parts, next, lastFull: r?.last_full_at ?? null };
  }
  const rotation = $derived(rotationOf(current?.verify?.rotate_parts, repo.id));
  const cloudRotation = $derived(rotationOf(current?.offsite?.verify?.rotate_parts, `offsite:${repo.id}`));

  /** «Verificar también la copia en la nube» (editor de la copia externa). Por defecto: semanal y solo la estructura. */
  let cv = $state({
    on: false,
    sched: scheduleDraft(undefined, { kind: "weekly", weekday: 6, time: "05:00" }),
    mode: "structure" as "structure" | "random" | "rotate",
    percent: 5,
    parts: 4,
  });

  /** Datos que lee una verificación, en palabras. */
  function readingLabel(v: { subset_percent: number; rotate_parts?: number }) {
    return v.rotate_parts ? `rotativa: todo cada ${v.rotate_parts} verificaciones` : v.subset_percent ? `lee el ${v.subset_percent} % al azar` : "solo la estructura";
  }

  // ---------- Prueba de restauración ----------
  let editRestore = $state(false);
  /** Por defecto, cada sábado a las 04:00 (la verificación propone el domingo a las 03:00). */
  let rSched = $state(scheduleDraft(undefined, { kind: "weekly", weekday: 5, time: "04:00" }));
  let rFiles = $state(20);
  let rMaxMb = $state(200);

  function startRestore() {
    const t = current?.restore_test;
    rSched = scheduleDraft(t?.schedule, { kind: "weekly", weekday: 5, time: "04:00" });
    rFiles = t?.files ?? 20;
    rMaxMb = t?.max_mb ?? 200;
    editRestore = true;
  }

  async function saveRestore(off = false) {
    const test = off
      ? null
      : {
          schedule: toSchedule(rSched),
          files: Math.min(500, Math.max(1, Math.round(rFiles) || 20)),
          max_mb: Math.min(10240, Math.max(1, Math.round(rMaxMb) || 200)),
        };
    const done = await withPassword({
      title: off ? "Quitar la prueba de restauración" : "Prueba de restauración",
      message: off
        ? `Se dejará de probar la restauración de «${repo.name}».`
        : `El agente restaurará ${test!.files} archivos al azar de «${repo.name}» (hasta ${test!.max_mb} MB) ${scheduleLabel(test!.schedule)}, comprobará que salen enteros y los borrará. Se restauran en una carpeta solo para el sistema.`,
      repoName: repo.name,
      confirmLabel: off ? "Quitar" : "Programar",
      danger: off,
      action: async (password) => {
        agent.info = await api.agentSetRestoreTest(repo.id, test, password);
      },
    });
    if (done) {
      editRestore = false;
      toast(off ? `Prueba de restauración de «${repo.name}» quitada` : `Prueba de restauración de «${repo.name}»: ${scheduleLabel(test!.schedule)}`);
    }
  }

  async function saveVerify(off = false) {
    vError = "";
    const verify = off
      ? null
      : {
          schedule: toSchedule(vSched),
          subset_percent: vMode === "random" ? Math.min(100, Math.max(1, Math.round(vPercent) || 5)) : 0,
          rotate_parts: vMode === "rotate" ? Math.min(52, Math.max(2, Math.round(vParts) || 4)) : 0,
        };
    const done = await withPassword({
      title: off ? "Quitar la verificación" : "Programar la verificación",
      message: off
        ? `Se dejará de verificar «${repo.name}» automáticamente.`
        : `El agente comprobará «${repo.name}» ${scheduleLabel(verify!.schedule)}${
            verify!.rotate_parts
              ? `, leyendo cada vez una parte distinta de los datos (todos los datos cada ${verify!.rotate_parts} verificaciones)`
              : verify!.subset_percent
                ? `, leyendo el ${verify!.subset_percent} % de los datos cada vez`
                : ""
          }. Mientras tanto, sus copias esperan.`,
      repoName: repo.name,
      confirmLabel: off ? "Quitar" : "Programar",
      danger: off,
      action: async (password) => {
        agent.info = await api.agentSetVerify(repo.id, verify, password);
      },
    });
    if (done) {
      editVerify = false;
      toast(off ? `Verificación de «${repo.name}» quitada` : `Verificación de «${repo.name}»: ${scheduleLabel(verify!.schedule)}`);
    }
  }

  // ---------- Copia externa ----------
  const PROVIDERS = [
    { id: "b2", label: "Backblaze B2", region: "us-west-004", endpoint: (r: string) => `s3.${r}.backblazeb2.com` },
    { id: "wasabi", label: "Wasabi", region: "us-east-1", endpoint: (r: string) => `s3.${r}.wasabisys.com` },
    { id: "r2", label: "Cloudflare R2", region: "auto", endpoint: (a: string) => `${a}.r2.cloudflarestorage.com` },
    { id: "aws", label: "Amazon S3", region: "us-east-1", endpoint: (r: string) => `s3.${r}.amazonaws.com` },
    { id: "s3", label: "Otro compatible con S3", region: "", endpoint: (e: string) => e },
    { id: "otro", label: "Otra ubicación de restic", region: "", endpoint: () => "" },
    { id: "destino", label: "Otro repositorio de Resguardo", region: "", endpoint: () => "" },
  ] as const;
  type ProviderId = (typeof PROVIDERS)[number]["id"];

  let editOffsite = $state(false);
  let provider = $state<ProviderId>("b2");
  let region = $state("us-west-004");
  let account = $state(""); // R2: id de cuenta · S3 genérico: servidor
  let bucket = $state("");
  let folder = $state("");
  let rawLocation = $state("");
  /** Otro destino de la app (proveedor "destino"). */
  let target = $state("");
  const otherRepos = $derived(repos.filter((r) => r.id !== repo.id));
  const targetRepo = $derived(otherRepos.find((r) => r.id === target) ?? null);
  /** El destino elegido solo recibe esta copia externa (sin copias propias ni
   *  otros orígenes): es seguro aplicar allí la retención (igual que el agente). */
  const targetExclusive = $derived(
    !!targetRepo &&
      targetRepo.plans.length === 0 &&
      !(info?.repos ?? []).some(
        (r) => r.id !== repo.id && (r.id === targetRepo.id || r.offsite?.provider === `destino:${targetRepo.id}`),
      ),
  );
  let keyId = $state("");
  let keySecret = $state("");
  let samePassword = $state(true);
  let destPassword = $state("");
  // Recomendado: subir en cuanto hay una versión nueva.
  let oSched = $state(scheduleDraft(undefined, { kind: "after_backup", min_minutes: 30 }));
  // Freno ante cambios inusuales (activado por defecto; al volver a guardar una copia externa sin él, se añade).
  let guardOn = $state(true);
  let guardFactor = $state(20);
  let guardMinGb = $state(2);
  let guardMinFiles = $state(5000);
  let applyRetention = $state(true);
  /** Límite de subida en Mbit/s (0 o vacío: sin límite). */
  let limitMbit = $state(0);
  // 1 Mbit/s = 1e6 bit/s = 1e6 / 8 / 1024 KiB/s.
  const limitKib = $derived(limitMbit > 0 ? Math.max(1, Math.round((limitMbit * 1e6) / 8 / 1024)) : null);
  let oError = $state("");

  /** KiB/s → Mbit/s con un decimal (p. ej. 0,5). */
  const kibToMbit = (kib: number) => Math.round((kib * 1024 * 8) / 1e5) / 10;

  const slug = (s: string) =>
    s
      .normalize("NFD")
      .replace(/[\u0300-\u036f]/g, "")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "") || "repo";

  const prov = $derived(PROVIDERS.find((p) => p.id === provider)!);
  const location = $derived.by(() => {
    if (provider === "destino") return targetRepo?.location ?? "";
    if (provider === "otro") return rawLocation.trim();
    const host = provider === "r2" || provider === "s3" ? prov.endpoint(account.trim()) : prov.endpoint(region.trim());
    const path = [bucket.trim(), folder.trim()].filter(Boolean).join("/");
    return host && bucket.trim() ? `s3:https://${host.replace(/^https?:\/\//, "")}/${path}` : "";
  });
  const effectiveRegion = $derived(provider === "otro" || provider === "destino" ? null : provider === "r2" ? "auto" : region.trim() || null);
  const hasCreds = $derived(keyId.trim() !== "" && keySecret.trim() !== "");
  const editingExisting = $derived(!!current?.offsite && current.offsite.location === location);
  const canSave = $derived(
    provider === "destino"
      ? !!targetRepo
      : location !== "" && (hasCreds || editingExisting || provider === "otro") && (samePassword || destPassword.length >= 8),
  );

  /** «Opciones avanzadas» de la copia externa: límite de subida, freno y verificación de la nube. */
  const offsiteHint = $derived(
    [
      limitMbit > 0 ? `Subida limitada a ${limitMbit} Mbit/s` : "Sin límite de subida",
      guardOn ? `frena a partir de ${guardFactor} veces lo normal` : null,
      cv.on ? (cv.mode === "rotate" ? `nube: rotativa cada ${cv.parts}` : cv.mode === "random" ? `nube: ${cv.percent} % al azar` : "nube: solo la estructura") : null,
    ]
      .filter(Boolean)
      .join(" · "),
  );
  const offsiteCustom = $derived(
    limitMbit > 0 || (guardOn && (guardFactor !== 20 || guardMinGb !== 2 || guardMinFiles !== 5000)) || (cv.on && cv.mode !== "structure"),
  );

  function startOffsite() {
    const o = current?.offsite;
    oError = "";
    keyId = keySecret = destPassword = "";
    samePassword = true;
    applyRetention = o ? !!o.retention : true;
    limitMbit = o?.limit_upload_kib ? kibToMbit(o.limit_upload_kib) : 0;
    oSched = scheduleDraft(o?.schedule, { kind: "after_backup", min_minutes: 30 });
    const g = o?.guard;
    guardOn = true;
    guardFactor = g?.factor ?? 20;
    guardMinGb = g ? Math.round((g.min_bytes / 2 ** 30) * 10) / 10 : 2;
    guardMinFiles = g?.min_files ?? 5000;
    const ov = o?.verify;
    cv = {
      on: !!ov,
      sched: scheduleDraft(ov?.schedule, { kind: "weekly", weekday: 6, time: "05:00" }),
      mode: ov ? (ov.rotate_parts ? "rotate" : ov.subset_percent ? "random" : "structure") : "structure",
      percent: ov?.subset_percent || 5,
      parts: ov?.rotate_parts || 4,
    };
    target = "";
    if (o?.provider.startsWith("destino:")) {
      provider = "destino";
      target = o.provider.slice("destino:".length);
    } else if (o) {
      provider = (PROVIDERS.some((p) => p.id === o.provider) ? o.provider : "otro") as ProviderId;
      // Se reconstruyen los campos desde la ubicación guardada.
      const m = o.location.match(/^s3:https?:\/\/([^/]+)\/([^/]+)\/?(.*)$/);
      if (m && provider !== "otro") {
        const host = m[1];
        bucket = m[2];
        folder = m[3];
        if (provider === "r2") account = host.replace(/\.r2\.cloudflarestorage\.com$/, "");
        else if (provider === "s3") account = host;
        else region = o.region ?? host.split(".")[1] ?? region;
      } else {
        provider = "otro";
        rawLocation = o.location;
      }
    } else {
      provider = "b2";
      region = PROVIDERS[0].region;
      account = bucket = "";
      folder = slug(repo.name);
      rawLocation = "";
    }
    editOffsite = true;
  }

  function pickProvider(id: ProviderId) {
    provider = id;
    if (id === "destino" && !target && otherRepos.length === 1) target = otherRepos[0].id;
    const p = PROVIDERS.find((x) => x.id === id)!;
    if (p.region) region = p.region;
  }

  async function saveOffsite(off = false) {
    oError = "";
    const schedule = toSchedule(oSched);
    const toTarget = provider === "destino";
    const creds = !toTarget && (hasCreds || !samePassword) ? { key_id: keyId.trim() || null, key_secret: keySecret || null, password: samePassword ? null : destPassword } : null;
    let prepared: string | null = null;
    // Hacia otro destino de la app: su contraseña también, porque el agente
    // recibe sus credenciales para subir allí.
    let passwordTarget: string | null = null;
    if (!off && toTarget) {
      const tgt = targetRepo;
      if (!tgt) return;
      passwordTarget = await withPassword({
        title: "Contraseña del repositorio de la copia",
        message: `Para subir allí, el agente recibirá la contraseña y las credenciales de «${tgt.name}». Escribe su contraseña para confirmarlo.`,
        repoName: tgt.name,
        confirmLabel: "Continuar",
        action: (pw) => api.checkPassword(tgt.id, pw),
      });
      if (!passwordTarget) return;
    }
    const done = await withPassword({
      title: off ? "Quitar la copia externa" : "Copia externa",
      message: off
        ? `El agente dejará de subir «${repo.name}» y olvidará las credenciales del repositorio. Lo que ya se subió no se borra.`
        : toTarget
          ? `El agente copiará las versiones de «${repo.name}» a «${targetRepo?.name}» ${scheduleLabel(schedule)}, con la contraseña y las credenciales de ese repositorio.`
          : `Resguardo comprobará el repositorio (y lo creará si no existe) y el agente subirá las versiones de «${repo.name}» ${scheduleLabel(schedule)}. Las credenciales se guardan cifradas para el agente.`,
      repoName: repo.name,
      confirmLabel: off ? "Quitar" : "Guardar",
      danger: off,
      action: async (password) => {
        if (!off && creds && (hasCreds || provider === "otro")) {
          prepared = await api.offsitePrepare(repo.id, location, effectiveRegion, creds, password);
        }
        agent.info = await api.agentSetOffsite(
          repo.id,
          off
            ? null
            : toTarget
              ? // Allí se aplica la retención propia de ese repositorio (la guardada en él), no la de este.
                { target, location: "", provider: "", region: null, schedule, apply_retention: targetExclusive && applyRetention && !!targetRepo?.retention, limit_upload_kib: limitKib, guard: guardValue(), verify: cloudVerifyValue() }
              : { location, provider, region: effectiveRegion, schedule, apply_retention: applyRetention && !!repo.retention, limit_upload_kib: limitKib, guard: guardValue(), verify: cloudVerifyValue() },
          off ? null : creds,
          password,
          off ? null : passwordTarget,
        );
      },
    });
    if (done) {
      editOffsite = false;
      if (off) toast(`Copia externa de «${repo.name}» quitada`);
      else
        toast(
          prepared === "created"
            ? "Repositorio creado. La primera subida puede tardar: se hará en segundo plano."
            : `Copia externa de «${repo.name}»${toTarget ? ` a «${targetRepo?.name}»` : ""}: ${scheduleLabel(schedule)}`,
        );
    }
  }

  // ---------- Retención de la copia externa ----------

  /** Destino de la app al que sube la copia externa configurada (si es uno de ellos). */
  const offsiteTarget = $derived.by(() => {
    const prov = current?.offsite?.provider ?? "";
    return prov.startsWith("destino:") ? (repos.find((r) => r.id === prov.slice("destino:".length)) ?? null) : null;
  });
  /** Política que debería usar el agente allí: la del destino de la app, o la de este destino (ubicación escrita). */
  const expectedOffsitePolicy = $derived(current?.offsite?.provider.startsWith("destino:") ? (offsiteTarget?.retention ?? null) : (repo.retention ?? null));
  /** La política cambió desde que se configuró la copia externa: el agente sigue usando la anterior. */
  const offsiteRetentionStale = $derived(
    !!current?.offsite?.retention && (!expectedOffsitePolicy || !samePolicy(current.offsite.retention, expectedOffsitePolicy)),
  );
  /** Destino cuya retención se edita en un diálogo. */
  let editingTarget = $state<Repo | null>(null);

  /** Hay algo que no debe quedar oculto: una tarea en curso o un formulario abierto. */
  const busy = $derived(!!running || !!pending || editVerify || editRestore || editOffsite);
  const open = $derived(!ui.maintCollapsed || busy);
  /** Resumen de una línea para la tarjeta plegada. */
  const collapsedSummary = $derived(
    current
      ? [
          `Verificación: ${current.verify ? "activada" : "desactivada"}`,
          `Prueba de restauración: ${current.restore_test ? "activada" : "desactivada"}`,
          `Copia externa: ${current.offsite ? "activada" : "desactivada"}`,
        ].join(" · ")
      : "Sin configurar",
  );

  /** Vuelve a guardar la copia externa tal como está, con la retención actual. */
  async function reapplyOffsite() {
    if (!info?.elevated) return elevate();
    startOffsite();
    editOffsite = false;
    await saveOffsite();
  }

  /** Verificación de la copia en la nube desde el formulario (null: no). */
  function cloudVerifyValue() {
    if (!cv.on) return null;
    return {
      schedule: toSchedule(cv.sched),
      subset_percent: cv.mode === "random" ? Math.min(100, Math.max(1, Math.round(cv.percent) || 5)) : 0,
      rotate_parts: cv.mode === "rotate" ? Math.min(52, Math.max(2, Math.round(cv.parts) || 4)) : 0,
    };
  }

  /** Umbrales del freno desde el formulario (null: sin freno). */
  function guardValue() {
    if (!guardOn) return null;
    return {
      factor: Math.min(1000, Math.max(2, Math.round(guardFactor) || 20)),
      min_bytes: Math.max(2 ** 20, Math.round((Number(guardMinGb) || 2) * 2 ** 30)),
      min_files: Math.max(1, Math.round(guardMinFiles) || 5000),
    };
  }

  /** Subida frenada por un cambio inusual en este destino. */
  const hold = $derived(info?.offsite_holds?.[repo.id] ?? null);

  async function runNow(kind: "verify" | "offsite" | "verify_offsite" | "restore_test") {
    // Con la subida frenada, subir a mano requiere confirmarlo (con la contraseña).
    if (kind === "offsite" && hold) {
      const done = await withPassword({
        title: "Subir pese al cambio inusual",
        message: `La subida de «${repo.name}» está frenada por un cambio inusual. Si subes ahora, esa versión llegará a la nube (sin aplicar la retención allí). Hazlo solo si ya comprobaste que el cambio es normal.`,
        repoName: repo.name,
        confirmLabel: "Subir de todos modos",
        danger: true,
        action: (password) => api.checkPassword(repo.id, password),
      });
      if (!done) return;
    }
    try {
      await api.agentTaskNow(repo.id, kind);
      pending = kind;
      toast(kind === "verify" ? "El agente empezará a verificar en unos minutos." : "El agente empezará a subir en unos minutos.", "info");
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function elevate() {
    try {
      await api.relaunchAsAdmin();
    } catch (e) {
      toast(String(e), "error");
    }
  }

  const next = (s: Schedule | undefined, run: AgentRun | null, enabledAt: string | undefined) => {
    if (!s || !enabledAt) return null;
    return nextRun(s, new Date(run?.started ?? enabledAt));
  };
  const nextLabel = (d: Date | null) => (paused ? "en pausa" : !d ? "—" : d.getTime() <= Date.now() ? "en los próximos minutos" : formatDate(d.toISOString()));
  /** Próxima subida: con «después de cada copia» no hay hora fija. */
  const offsiteNextLabel = $derived.by(() => {
    const o = current?.offsite;
    if (!o) return "—";
    if (running?.kind === "offsite") return "en curso";
    if (hold) return "frenada (cambio inusual)";
    if (o.schedule.kind === "after_backup") return paused ? "en pausa" : "tras la próxima copia con cambios";
    return nextLabel(next(o.schedule, offsiteRun, o.enabled_at));
  });
  function PROVIDER_LABEL(id: string) {
    if (id.startsWith("destino:")) {
      const r = repos.find((x) => x.id === id.slice("destino:".length));
      return r ? `a «${r.name}»` : "a otro repositorio (ya no está en Resguardo)";
    }
    return PROVIDERS.find((p) => p.id === id)?.label ?? "Otra ubicación";
  }
</script>

{#snippet scheduleEditor(d: ReturnType<typeof scheduleDraft>, afterBackup = false)}
  <div class="segmented" role="group" aria-label="Frecuencia">
    {#each [...(afterBackup ? [["after_backup", "Tras cada copia"]] : []), ["hours", "Cada N horas"], ["daily", "Diaria"], ["weekly", "Semanal"]] as [k, label]}
      <button class:on={d.kind === k} aria-pressed={d.kind === k} onclick={() => (d.kind = k as typeof d.kind)}>{label}</button>
    {/each}
  </div>
  {#if d.kind === "after_backup"}
    <label class="row">
      Después de cada copia con cambios (como mucho cada <input class="input num" type="number" min="0" max="1440" bind:value={d.minMinutes} /> min)
    </label>
    <p class="faint icon-note"><Info size={14} /> <span>Sube a la nube en cuanto hay una versión nueva. Si no hubo cambios, no sube nada. (Recomendado.)</span></p>
  {:else if d.kind === "hours"}
    <label class="row">Cada <input class="input num" type="number" min="1" max="744" bind:value={d.every} /> horas</label>
  {:else if d.kind === "daily"}
    <label class="row">Todos los días a las <input class="input time" type="time" bind:value={d.time} /></label>
  {:else}
    <label class="row">
      Cada
      <select class="input" bind:value={d.weekday} aria-label="Día de la semana">{#each WEEKDAYS as w, i}<option value={i}>{w.toLowerCase()}</option>{/each}</select>
      a las <input class="input time" type="time" bind:value={d.time} />
    </label>
  {/if}
{/snippet}

{#snippet lastRun(run: AgentRun | null)}
  {#if run}
    <RunResult {run} alwaysMessage />
  {:else}
    <strong class="faint">todavía ninguna</strong>
  {/if}
{/snippet}

{#if info?.supported}
  <section class="card maint" bind:this={section}>
    <header>
      <div class="title">
        <span class="card-icon" class:on={!!(current?.verify || current?.offsite)}><Wrench size={18} /></span>
        <div>
          <h2 class="section-title">Mantenimiento</h2>
          <p class="faint">{open ? "Verificar que las copias están sanas y guardar otra fuera de este sitio." : collapsedSummary}</p>
        </div>
      </div>
      <CollapseToggle {open} label="Mantenimiento" controls="maint-body-{repo.id}" locked={busy} ontoggle={() => setUi("maintCollapsed", open)} />
    </header>

    {#if open}
    <div class="body" id="maint-body-{repo.id}" transition:slide={{ duration: dur(180) }}>

    {#if !current && sources.length}
      <p class="faint note">
        Este repositorio recibe la copia externa de {sources.map((s) => `«${s.name}»`).join(" y ")}; no necesita copias propias. Su verificación se programa en
        «{sources[0].name}» → Copia externa.
      </p>
    {:else if !current}
      <p class="faint note">
        Activa primero <strong>Copias automáticas</strong> en este repositorio (o «Solo vigilar», si las copias las hace otro programa): el agente usa esa
        misma configuración para verificar y subir.
      </p>
    {:else}
      <!-- Verificación -->
      <div class="task">
        <div class="task-head">
          <div>
            <h3><ShieldCheck size={15} /> Verificación <HelpLink topic="mant-verificacion" label="la verificación" /></h3>
            <p class="faint">
              {#if current.verify}
                <strong class="on-text">Activada</strong> · {scheduleLabel(current.verify.schedule)}{current.verify.rotate_parts
                  ? ` · rotativa: todos los datos cada ${current.verify.rotate_parts} verificaciones`
                  : current.verify.subset_percent
                    ? ` · lee el ${current.verify.subset_percent} % de los datos al azar`
                    : " · solo la estructura"}
              {:else}
                Desactivada: nadie comprueba que este repositorio esté sano.
              {/if}
            </p>
          </div>
          {#if !editVerify}
            <div class="task-actions">
              {#if current.verify}
                <button class="btn btn-ghost btn-sm" onclick={() => runNow("verify")} disabled={!!running || pending === "verify"} title="Verificar ahora">
                  <Play size={12} fill="currentColor" /> Ahora
                </button>
              {/if}
              {#if info.elevated}
                <button class="btn btn-sm" onclick={startVerify}>{current.verify ? "Cambiar" : "Programar"}</button>
              {:else}
                <button class="btn btn-sm" onclick={elevate} title="Resguardo se reabrirá como administrador (Windows lo pedirá) y volverás aquí">
                  <ShieldCheck size={14} />
                  {current.verify ? "Cambiar" : "Programar"} (requiere administrador)
                </button>
              {/if}
            </div>
          {/if}
        </div>

        {#if running?.kind === "verify"}
          <TaskProgress task={running} />
        {:else if pending === "verify"}
          <div class="live"><span class="spin"><LoaderCircle size={14} /></span><span>Esperando al agente… (empieza en menos de 5 minutos)</span></div>
        {/if}

        {#if current.verify && !editVerify}
          <div class="fact-boxes">
            <div><span class="faint">Próxima</span><strong>{nextLabel(next(current.verify.schedule, verifyRun, current.verify.enabled_at))}</strong></div>
            <div><span class="faint">Última</span>{@render lastRun(verifyRun)}</div>
          </div>
          {#if rotation}
            <p class="faint rot-line">
              Rotativa: próxima parte {rotation.next} de {rotation.parts} ·
              {#if rotation.lastFull}último ciclo completo <RelTime iso={rotation.lastFull} />{:else}todavía sin ciclo completo{/if}
            </p>
          {/if}
        {/if}

        {#if editVerify}
          <div class="editor" transition:slide={{ duration: dur(180) }}>
            {@render scheduleEditor(vSched)}
            <Advanced id="verificacion" hint={verifyHint} custom={verifyCustom}>
              <span class="sub-label">Datos que se leen</span>
              <div class="radios">
                <label class="radio"><input type="radio" name="vmode-{repo.id}" value="structure" bind:group={vMode} /> Solo la estructura</label>
                <label class="radio">
                  <input type="radio" name="vmode-{repo.id}" value="random" bind:group={vMode} /> Una parte al azar:
                  <input class="input num" type="number" min="1" max="100" bind:value={vPercent} disabled={vMode !== "random"} /> %
                </label>
                <label class="radio">
                  <input type="radio" name="vmode-{repo.id}" value="rotate" bind:group={vMode} /> Rotativa: todos los datos cada
                  <input class="input num" type="number" min="2" max="52" bind:value={vParts} disabled={vMode !== "rotate"} /> verificaciones (recomendada)
                </label>
              </div>
              {#if vMode === "rotate"}
                <p class="faint icon-note"><Info size={14} /> <span>{coverageHint(toSchedule(vSched), vParts)}.</span></p>
              {/if}
              {#if isCloud && vMode !== "structure"}
                <p class="faint icon-note warn"><TriangleAlert size={14} /> <span>Leer datos de la nube cuesta tráfico de descarga (según el proveedor).</span></p>
              {/if}
              <p class="faint icon-note">
                <Info size={14} />
                <span>
                  Siempre se comprueba la estructura (índices, copias, carpetas). Leer además los datos detecta archivos dañados en el disco
                  del servidor. Con «Rotativa», cada vez se lee una parte distinta y al completar el ciclo se ha leído todo; «al azar» no lo
                  garantiza. Mientras se verifica, las copias de este repositorio esperan.
                </span>
              </p>
            </Advanced>
            {#if vError}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{vError}</p></div>{/if}
            <footer>
              {#if current.verify}<button class="btn btn-ghost danger-text" onclick={() => saveVerify(true)}>Quitar</button>{/if}
              <span class="spacer"></span>
              <button class="btn btn-ghost" onclick={() => (editVerify = false)}>Cancelar</button>
              <button class="btn btn-primary" onclick={() => saveVerify()}>Guardar</button>
            </footer>
          </div>
        {/if}
      </div>

      <!-- Prueba de restauración -->
      <div class="task">
        <div class="task-head">
          <div>
            <h3><ArchiveRestore size={15} /> Prueba de restauración <HelpLink topic="mant-prueba-restauracion" label="la prueba de restauración" /></h3>
            <p class="faint">
              {#if current.restore_test}
                <strong class="on-text">Activada</strong> · {scheduleLabel(current.restore_test.schedule)} · {current.restore_test.files} archivos, hasta
                {current.restore_test.max_mb} MB
              {:else}
                Desactivada: nadie comprueba que las copias se puedan recuperar de verdad.
              {/if}
            </p>
          </div>
          {#if !editRestore}
            <div class="task-actions">
              {#if current.restore_test}
                <button class="btn btn-ghost btn-sm" onclick={() => runNow("restore_test")} disabled={!!running || pending === "restore_test"} title="Probar ahora">
                  <Play size={12} fill="currentColor" /> Ahora
                </button>
              {/if}
              {#if info.elevated}
                <button class="btn btn-sm" onclick={startRestore}>{current.restore_test ? "Cambiar" : "Programar"}</button>
              {:else}
                <button class="btn btn-sm" onclick={elevate} title="Resguardo se reabrirá como administrador (Windows lo pedirá) y volverás aquí">
                  <ShieldCheck size={14} />
                  {current.restore_test ? "Cambiar" : "Programar"} (requiere administrador)
                </button>
              {/if}
            </div>
          {/if}
        </div>

        {#if running?.kind === "restore_test"}
          <TaskProgress task={running} />
        {:else if pending === "restore_test"}
          <div class="live"><span class="spin"><LoaderCircle size={14} /></span><span>Esperando al agente… (empieza en menos de 5 minutos)</span></div>
        {/if}

        {#if current.restore_test && !editRestore}
          <div class="fact-boxes">
            <div><span class="faint">Próxima</span><strong>{nextLabel(next(current.restore_test.schedule, restoreRun, current.restore_test.enabled_at))}</strong></div>
            <div><span class="faint">Última</span>{@render lastRun(restoreRun)}</div>
          </div>
        {/if}

        {#if editRestore}
          <div class="editor" transition:slide={{ duration: dur(180) }}>
            {@render scheduleEditor(rSched)}
            <Advanced id="prueba" hint="{rFiles} archivos al azar, hasta {rMaxMb} MB" custom={rFiles !== 20 || rMaxMb !== 200}>
              <label class="row">
                Restaurar <input class="input num" type="number" min="1" max="500" bind:value={rFiles} /> archivos al azar, hasta
                <input class="input num" type="number" min="1" max="10240" bind:value={rMaxMb} /> MB en total
              </label>
            </Advanced>
            <p class="faint icon-note">
              <Info size={14} />
              <span>
                El agente elige una versión (la última o una de los últimos 30 días) y archivos de distintos tamaños, los restaura en una carpeta
                solo para el sistema, comprueba que salen enteros y los borra. Así sabes que las copias se pueden recuperar, no solo que existen.
              </span>
            </p>
            <footer>
              {#if current.restore_test}<button class="btn btn-ghost danger-text" onclick={() => saveRestore(true)}>Quitar</button>{/if}
              <span class="spacer"></span>
              <button class="btn btn-ghost" onclick={() => (editRestore = false)}>Cancelar</button>
              <button class="btn btn-primary" onclick={() => saveRestore()}>Guardar</button>
            </footer>
          </div>
        {/if}
      </div>

      <!-- Copia externa -->
      <div class="task">
        <div class="task-head">
          <div>
            <h3><CloudUpload size={15} /> Copia externa <HelpLink topic="mant-copia-externa" label="la copia externa" /></h3>
            <p class="faint">
              {#if current.offsite}
                <strong class="on-text">Activada</strong> · {PROVIDER_LABEL(current.offsite.provider)} · {scheduleLabel(current.offsite.schedule)}{current
                  .offsite.retention
                  ? " · con retención"
                  : ""}{current.offsite.limit_upload_kib ? ` · máx. ${kibToMbit(current.offsite.limit_upload_kib).toLocaleString("es")} Mbit/s` : ""}
              {:else}
                Desactivada: todas las copias están en un solo lugar.
              {/if}
            </p>
          </div>
          {#if !editOffsite}
            <div class="task-actions">
              {#if current.offsite}
                <button class="btn btn-ghost btn-sm" onclick={() => runNow("offsite")} disabled={!!running || pending === "offsite"} title="Subir ahora">
                  <Play size={12} fill="currentColor" /> Ahora
                </button>
              {/if}
              {#if info.elevated}
                <button class="btn btn-sm" onclick={startOffsite}>{current.offsite ? "Cambiar" : "Configurar"}</button>
              {:else}
                <button class="btn btn-sm" onclick={elevate} title="Resguardo se reabrirá como administrador (Windows lo pedirá) y volverás aquí">
                  <ShieldCheck size={14} />
                  {current.offsite ? "Cambiar" : "Configurar"} (requiere administrador)
                </button>
              {/if}
            </div>
          {/if}
        </div>

        {#if running?.kind === "offsite"}
          <TaskProgress task={running} target={offsiteTargetName(repo.id, repos)} />
        {:else if pending === "offsite"}
          <div class="live"><span class="spin"><LoaderCircle size={14} /></span><span>Esperando al agente… (empieza en menos de 5 minutos)</span></div>
        {/if}

        {#if current.offsite && !editOffsite}
          <div class="fact-boxes">
            <div><span class="faint">Próxima subida</span><strong class:held-text={!!hold}>{offsiteNextLabel}</strong></div>
            <div><span class="faint">Última</span>{@render lastRun(offsiteRun)}</div>
          </div>
          <p class="faint dest mono selectable" title={current.offsite.location}>{current.offsite.location}</p>
          <div class="ret-line">
            <span>
              <strong>Retención allí:</strong>
              {current.offsite.retention ? policySummary(current.offsite.retention) : "ninguna: se guardan todas las versiones."}
            </span>
            {#if offsiteTarget}
              <button class="link" onclick={() => (editingTarget = offsiteTarget)}>Editar la retención de «{offsiteTarget.name}»</button>
            {/if}
          </div>
          {#if current.offsite.retention}
            <p class="faint icon-note">
              <Info size={14} /> <span>El agente la aplica después de cada subida. Con <em>Object Lock</em>, lo borrado solo libera espacio cuando vence el bloqueo.</span>
            </p>
          {/if}
          {#if current.offsite.verify}
            <div class="cloud-verify">
              <div class="ret-line">
                <span>
                  <strong>Verificación de la nube:</strong>
                  {#if cloudVerifyRun}
                    <RelTime iso={cloudVerifyRun.finished} />, {cloudVerifyRun.result === "ok" ? "sin errores" : cloudVerifyRun.message}
                  {:else}
                    todavía ninguna
                  {/if}
                  <span class="faint">· {scheduleLabel(current.offsite.verify.schedule)} · {readingLabel(current.offsite.verify)}</span>
                </span>
                <button
                  class="btn btn-ghost btn-sm"
                  onclick={() => runNow("verify_offsite")}
                  disabled={!!running || pending === "verify_offsite"}
                  title="Verificar ahora la copia en la nube"
                >
                  <Play size={12} fill="currentColor" /> Verificar ahora
                </button>
              </div>
              {#if cloudRotation}
                <p class="faint rot-line">
                  Rotativa: próxima parte {cloudRotation.next} de {cloudRotation.parts} ·
                  {#if cloudRotation.lastFull}último ciclo completo <RelTime iso={cloudRotation.lastFull} />{:else}todavía sin ciclo completo{/if}
                </p>
              {/if}
              {#if running?.kind === "verify_offsite"}
                <TaskProgress task={running} target={offsiteTargetName(repo.id, repos)} />
              {:else if pending === "verify_offsite"}
                <div class="live"><span class="spin"><LoaderCircle size={14} /></span><span>Esperando al agente… (empieza en menos de 5 minutos)</span></div>
              {/if}
            </div>
          {/if}
          {#if offsiteRetentionStale}
            <div class="notice notice-warn">
              <TriangleAlert size={16} />
              <p>
                {offsiteTarget ? `La retención de «${offsiteTarget.name}» cambió` : "La retención de este repositorio cambió"}: el agente sigue usando la anterior.
                <button class="notice-action" onclick={reapplyOffsite}>{info.elevated ? "Aplicar cambios" : "Abrir como administrador para aplicarlos"}</button>
              </p>
            </div>
          {/if}
        {/if}

        {#if editOffsite}
          <div class="editor" transition:slide={{ duration: dur(180) }}>
            <label class="field">
              <span>Proveedor</span>
              <select class="input" value={provider} onchange={(e) => pickProvider(e.currentTarget.value as ProviderId)}>
                {#each PROVIDERS as p}<option value={p.id}>{p.label}</option>{/each}
              </select>
            </label>

            {#if provider === "destino"}
              {#if otherRepos.length}
                <label class="field">
                  <span>Repositorio</span>
                  <select class="input" bind:value={target}>
                    <option value="" disabled>Elige un repositorio…</option>
                    {#each otherRepos as r (r.id)}<option value={r.id}>{r.name} · {repoKind(r.location).label}</option>{/each}
                  </select>
                </label>
                <p class="faint icon-note">
                  <Info size={14} />
                  Se usan la ubicación, la contraseña y las credenciales que ya tiene guardadas ese repositorio. Allí verás las versiones de «{repo.name}»
                  junto a las suyas.
                </p>
              {:else}
                <div class="notice notice-info">
                  <Info size={16} />
                  <p>No tienes otro repositorio. Añade uno con el <strong>+</strong> de «Destinos» y vuelve aquí.</p>
                </div>
              {/if}
            {:else if provider === "otro"}
              <label class="field">
                <span>Ubicación de restic de la copia externa</span>
                <input class="input mono" bind:value={rawLocation} placeholder="b2:bucket:carpeta  ·  rest:https://servidor/ruta  ·  E:\Copias\repo" />
              </label>
            {:else}
              <div class="grid2">
                {#if provider === "r2"}
                  <label class="field"><span>ID de cuenta de Cloudflare</span><input class="input mono" bind:value={account} placeholder="0123abcd…" /></label>
                {:else if provider === "s3"}
                  <label class="field"><span>Servidor (endpoint)</span><input class="input mono" bind:value={account} placeholder="s3.ejemplo.com" /></label>
                {:else}
                  <label class="field"><span>Región</span><input class="input mono" bind:value={region} placeholder={prov.region} /></label>
                {/if}
                {#if provider === "s3"}
                  <label class="field"><span>Región (si la pide)</span><input class="input mono" bind:value={region} placeholder="opcional" /></label>
                {/if}
                <label class="field"><span>Bucket</span><input class="input mono" bind:value={bucket} placeholder="mis-copias" /></label>
                <label class="field"><span>Carpeta dentro del bucket</span><input class="input mono" bind:value={folder} placeholder={slug(repo.name)} /></label>
              </div>
            {/if}

            {#if provider !== "destino"}
            <div class="grid2">
              <label class="field">
                <span>{provider === "otro" ? "ID de clave (si hace falta)" : "ID de la clave (Access key)"}</span>
                <input class="input mono" bind:value={keyId} autocomplete="off" placeholder={editingExisting ? "guardada · déjalo vacío para conservarla" : ""} />
              </label>
              <label class="field">
                <span>Clave secreta (Secret key)</span>
                <input class="input mono" type="password" bind:value={keySecret} autocomplete="off" placeholder={editingExisting ? "guardada" : ""} />
              </label>
            </div>

            <label class="check"><input type="checkbox" bind:checked={samePassword} /> Cifrar la copia externa con la misma contraseña de este repositorio</label>
            {#if !samePassword}
              <label class="field">
                <span>Contraseña de la copia externa (mínimo 8 caracteres; guárdala en un lugar seguro)</span>
                <input class="input" type="password" bind:value={destPassword} autocomplete="new-password" />
              </label>
            {/if}
            {/if}

            <span class="sub-label">Cuándo subir</span>
            {@render scheduleEditor(oSched, true)}

            <label class="check">
              <input type="checkbox" bind:checked={guardOn} />
              Frenar la subida si una copia cambia mucho más de lo normal
              <HelpLink topic="mant-cambio-inusual" label="el freno ante cambios inusuales" />
            </label>

            <label class="check">
              <input type="checkbox" bind:checked={cv.on} />
              Verificar también la copia en «{provider === "destino" ? (targetRepo?.name ?? "el repositorio") : prov.label}»
            </label>
            {#if cv.on}
              <div class="cloud-editor">
                {@render scheduleEditor(cv.sched)}
                <p class="faint icon-note">
                  <Info size={14} />
                  <span>
                    Comprueba con <code>restic check</code> que lo subido está sano, con las mismas credenciales de la subida y nunca a la vez que ella.
                    Leer datos de la nube cuesta tráfico de descarga (según el proveedor).
                  </span>
                </p>
              </div>
            {/if}

            <Advanced id="copia-externa" hint={offsiteHint} custom={offsiteCustom}>
              <label class="row">
                Velocidad máxima de subida
                <input class="input num" type="number" min="0" max="10000" step="0.1" bind:value={limitMbit} aria-describedby="limit-hint" />
                Mbit/s
              </label>
              <p class="faint icon-note" id="limit-hint">
                <Info size={14} />
                Para no saturar el internet de la oficina mientras se sube. 0 = sin límite. Como referencia, la mitad de tu velocidad de
                subida es un buen punto de partida. <HelpLink topic="mant-limite" label="el límite de subida" />
              </p>
              {#if guardOn}
                <span class="sub-label">Cuándo se frena la subida</span>
                <div class="row wrap">
                  Más de <input class="input num" type="number" min="2" max="1000" bind:value={guardFactor} /> veces lo normal y, como mínimo,
                  <input class="input num" type="number" min="0.1" step="0.5" bind:value={guardMinGb} /> GB o
                  <input class="input num wide" type="number" min="1" step="100" bind:value={guardMinFiles} /> archivos nuevos o cambiados.
                </div>
              {/if}
              {#if cv.on}
                <span class="sub-label">Datos que se leen al verificar la copia en la nube</span>
                <div class="radios">
                  <label class="radio"><input type="radio" name="cvmode-{repo.id}" value="structure" bind:group={cv.mode} /> Solo la estructura</label>
                  <label class="radio">
                    <input type="radio" name="cvmode-{repo.id}" value="random" bind:group={cv.mode} /> Una parte al azar:
                    <input class="input num" type="number" min="1" max="100" bind:value={cv.percent} disabled={cv.mode !== "random"} /> %
                  </label>
                  <label class="radio">
                    <input type="radio" name="cvmode-{repo.id}" value="rotate" bind:group={cv.mode} /> Rotativa: todo cada
                    <input class="input num" type="number" min="2" max="52" bind:value={cv.parts} disabled={cv.mode !== "rotate"} /> verificaciones
                  </label>
                </div>
              {/if}
            </Advanced>

            {#if provider === "destino"}
              {#if targetRepo && targetExclusive}
                <label class="check" class:disabled={!targetRepo.retention}>
                  <input type="checkbox" bind:checked={applyRetention} disabled={!targetRepo.retention} />
                  {#if targetRepo.retention}
                    Aplicar en «{targetRepo.name}» su propia retención (solo recibe esta copia externa)
                  {:else}
                    Aplicar retención allí: «{targetRepo.name}» aún no tiene una política
                  {/if}
                </label>
                <div class="ret-line">
                  <span>{targetRepo.retention ? policySummary(targetRepo.retention) : "Sin política guardada."}</span>
                  <button class="link" onclick={() => (editingTarget = targetRepo)}>Editar la retención de «{targetRepo.name}»</button>
                </div>
                {#if !targetRepo.retention || !applyRetention}
                  <p class="faint icon-note warn"><TriangleAlert size={14} /> Sin retención, allí se guardan todas las versiones para siempre y su tamaño solo crece.</p>
                {:else}
                  <p class="faint icon-note">
                    <Info size={14} />
                    <span>El agente la aplica después de cada subida y solo sube las versiones que esa política conservaría. Con
                      <em>Object Lock</em>, lo borrado solo libera espacio cuando vence el bloqueo.</span>
                  </p>
                {/if}
              {:else if targetRepo}
                <p class="faint icon-note">
                  <Info size={14} />
                  La retención de «{targetRepo.name}» la gestiona ese destino: tiene copias propias o recibe de otro origen, así que
                  aplicarla desde aquí borraría también esas versiones.
                </p>
              {/if}
            {:else}
            <label class="check" class:disabled={!repo.retention}>
              <input type="checkbox" bind:checked={applyRetention} disabled={!repo.retention} />
              {#if repo.retention}
                Aplicar allí la retención de este repositorio (se suben solo las versiones que se conservarían)
              {:else}
                Aplicar retención allí: define primero una política en la pestaña <strong>Retención</strong>
              {/if}
            </label>
            {#if !repo.retention || !applyRetention}
              <p class="faint icon-note warn"><TriangleAlert size={14} /> Sin retención, allí se guardan todas las versiones para siempre y su tamaño solo crece.</p>
            {/if}
            {/if}

            {#if provider !== "destino"}
            <div class="notice notice-info">
              <ShieldAlert size={16} />
              <p>
                <strong>Protege el repositorio contra borrados.</strong> <HelpLink topic="mant-proteger" label="cómo proteger el repositorio" /> Activa el versionado del bucket (u <em>Object Lock</em>) y usa una clave
                que no pueda borrar versiones antiguas. Así, aunque alguien tome este equipo, lo que ya se subió no se puede eliminar.
              </p>
            </div>
            {/if}

            {#if location}<p class="faint dest mono" title={location}>Repositorio: {location}</p>{/if}
            {#if oError}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{oError}</p></div>{/if}
            <footer>
              {#if current.offsite}<button class="btn btn-ghost danger-text" onclick={() => saveOffsite(true)}>Quitar</button>{/if}
              <span class="spacer"></span>
              <button class="btn btn-ghost" onclick={() => (editOffsite = false)}>Cancelar</button>
              <button
                class="btn btn-primary"
                onclick={() => saveOffsite()}
                disabled={!canSave}
                title={canSave ? undefined : provider === "destino" ? "Elige el repositorio al que subir la copia" : "Completa la ubicación, las claves y la contraseña"}
              >{provider === "destino" ? "Guardar" : "Probar y guardar"}</button>
            </footer>
          </div>
        {/if}
      </div>
    {/if}
    </div>
    {/if}
  </section>
{/if}

{#if editingTarget}
  <Modal onclose={() => (editingTarget = null)} labelledby="target-ret-title" width={760} dismissible={false}>
    <div class="target-ret">
      <p class="faint" id="target-ret-title">Retención de «{editingTarget.name}» · se guarda con la contraseña de ese repositorio</p>
      <RetentionPanel
        repo={editingTarget}
        embedded
        onchange={(r) => {
          onchange?.(r);
          editingTarget = r;
        }}
        onsaved={() => (editingTarget = null)}
      />
      <footer><button class="btn btn-ghost" onclick={() => (editingTarget = null)}>Cerrar</button></footer>
    </div>
  </Modal>
{/if}

<style>
  .maint {
    padding: 20px 22px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .maint > header {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .title {
    display: flex;
    flex: 1;
    min-width: 0;
    align-items: center;
    gap: 12px;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  header p,
  .task-head p {
    margin: 1px 0 0;
    font-size: var(--fs-sm);
  }
  .note {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .task {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-top: 14px;
    border-top: 1px solid var(--border);
  }
  .task-head {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px 12px;
  }
  .task-head > div:first-child {
    flex: 1 1 240px;
    min-width: 0;
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-body);
    font-weight: 650;
  }
  .task-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .on-text {
    color: var(--accent-text);
  }
  .live {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 12px;
    border-radius: var(--radius);
    background: var(--accent-soft);
    color: var(--accent-text);
    font-size: var(--fs-sm);
    font-weight: 550;
  }
  .live .spin {
    display: grid;
  }
  .dest {
    margin: 0;
    font-size: var(--fs-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .editor {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
    flex-wrap: wrap;
  }
  .row .input {
    width: auto;
    height: 32px;
  }
  .num {
    width: 76px !important;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: var(--fs-sm);
    font-weight: 550;
    color: var(--text-2);
    min-width: 0;
  }
  .grid2 {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 10px;
  }
  .check {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: var(--fs-sm);
    line-height: 1.45;
  }
  .check.disabled {
    color: var(--text-3);
  }
  .check input {
    margin-top: 3px;
  }
  .sub-label {
    font-size: var(--fs-sm);
    font-weight: 550;
    color: var(--text-2);
  }
  .cloud-verify {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .cloud-verify .ret-line {
    align-items: center;
    justify-content: space-between;
  }
  .cloud-editor {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-left: 26px;
  }
  .rot-line {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .radio {
    flex-wrap: wrap;
  }
  .radio .input {
    width: 70px;
    height: 30px;
  }
  .held-text {
    color: var(--warn);
  }
  .row.wrap {
    flex-wrap: wrap;
    margin-top: 8px;
    line-height: 2.2;
  }
  .num.wide {
    width: 90px;
  }
  .ret-line {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 10px;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .ret-line .link {
    font-size: var(--fs-sm);
  }
  .target-ret {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .target-ret > p {
    margin: 0;
    font-size: var(--fs-sm);
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
  }
</style>
