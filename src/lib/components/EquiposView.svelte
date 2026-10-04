<script lang="ts">
  // «Todos mis equipos»: los equipos de la cuenta de Resguardo Web (el
  // servidor, oficinas…), agrupados por cliente, con el mismo diseño que Estado.
  // De los otros equipos solo se ve el estado y se puede pedir «Copiar ahora»
  // de una copia: nada se puede borrar, restaurar ni cambiar a distancia.
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { dur } from "$lib/motion";
  import {
    CircleAlert,
    CircleCheck,
    CirclePause,
    Clock,
    Cloud,
    CloudOff,
    KeyRound,
    LoaderCircle,
    LogOut,
    Monitor,
    Play,
    RefreshCw,
    ShieldAlert,
    ShieldCheck,
    TriangleAlert,
    Users,
    WifiOff,
  } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { AccountOverview, AccountStatus, RemoteCommand, RemoteDevice, RemoteRepo } from "$lib/api";
  import { formatBytes, formatDate, formatRelative } from "$lib/format";
  import { toast } from "$lib/toast.svelte";
  import HelpLink from "./HelpLink.svelte";
  import RelTime from "./RelTime.svelte";

  let status = $state<AccountStatus | null>(null);
  let data = $state<AccountOverview | null>(null);
  let error = $state("");
  let loading = $state(false);

  // Inicio de sesión
  let email = $state("");
  let password = $state("");
  let code = $state("");
  let busy = $state(false);
  let formError = $state("");

  onMount(async () => {
    try {
      status = await api.accountStatus();
      if (status.signed_in) await load();
    } catch (e) {
      error = String(e);
    }
  });

  async function load() {
    loading = true;
    error = "";
    try {
      data = await api.accountOverview();
    } catch (e) {
      error = String(e);
      if (/inicia sesión|caducó/i.test(error)) status = await api.accountStatus();
    } finally {
      loading = false;
    }
  }

  // Se actualiza solo cada minuto mientras la vista está abierta.
  $effect(() => {
    if (!status?.signed_in) return;
    const t = setInterval(() => document.visibilityState === "visible" && load(), 60_000);
    return () => clearInterval(t);
  });

  async function signIn(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    formError = "";
    try {
      status = await api.accountSignIn(email, password);
    } catch (err) {
      formError = String(err);
    } finally {
      password = ""; // nunca se guarda
      busy = false;
    }
  }

  async function verify(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    formError = "";
    try {
      status = await api.accountVerify(code);
      code = "";
      await load();
    } catch (err) {
      formError = String(err);
    } finally {
      busy = false;
    }
  }

  async function signOut() {
    status = await api.accountSignOut();
    data = null;
  }

  // ---------- Datos ----------

  const HOUR = 3_600_000;
  type Tone = "ok" | "warn" | "bad" | "paused" | "neutral";

  function repoState(r: RemoteRepo, now: number): { tone: Tone; label: string } {
    if (r.paused) return { tone: "paused", label: "En pausa" };
    if (r.offsite_hold) return { tone: "bad", label: "Subida frenada" };
    if (!r.last_snapshot_at) return { tone: "neutral", label: "Sin versiones" };
    const since = (now - new Date(r.last_snapshot_at).getTime()) / HOUR;
    const expected = r.expected_hours ?? 24;
    if (since > expected * 2 + 1) return { tone: "bad", label: "Atrasado" };
    if (since > expected * 1.25 + 1) return { tone: "warn", label: "Con retraso" };
    if (r.last_run?.result === "error") return { tone: "bad", label: "Falló la última" };
    return { tone: "ok", label: "Al día" };
  }
  const TONE_ICON = { ok: CircleCheck, warn: Clock, bad: TriangleAlert, paused: CirclePause, neutral: CircleAlert };

  /** Sin informe en más de una hora: el equipo está apagado o sin conexión. */
  const offline = (d: RemoteDevice, now: number) => !d.last_seen_at || now - new Date(d.last_seen_at).getTime() > HOUR;

  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });

  const groups = $derived.by(() => {
    if (!data) return [];
    const byClient = new Map<string, { name: string; devices: { device: RemoteDevice; repos: RemoteRepo[] }[] }>();
    const clientName = (id: string | null) => data!.clients.find((c) => c.id === id)?.name ?? "Sin cliente";
    for (const d of data.devices) {
      const key = d.client_id ?? "";
      if (!byClient.has(key)) byClient.set(key, { name: clientName(d.client_id), devices: [] });
      byClient.get(key)!.devices.push({ device: d, repos: data.repos.filter((r) => r.device_id === d.id) });
    }
    return [...byClient.values()].sort((a, b) => a.name.localeCompare(b.name));
  });

  const counts = $derived.by(() => {
    const repos = data?.repos ?? [];
    const states = repos.map((r) => repoState(r, now));
    return {
      devices: data?.devices.length ?? 0,
      offline: (data?.devices ?? []).filter((d) => offline(d, now)).length,
      bad: states.filter((s) => s.tone === "bad").length,
      warn: states.filter((s) => s.tone === "warn").length,
      bytes: repos.reduce((n, r) => n + (r.last_total_bytes ?? 0), 0),
    };
  });
  const attention = $derived(counts.bad + counts.warn + counts.offline);
  const heroTone = $derived(counts.bad || counts.offline ? "bad" : counts.warn ? "warn" : "ok");

  // ---------- «Copiar ahora» a distancia ----------

  /** Peticiones de esta sesión, por «equipo/destino/plan». */
  let asked = $state<Record<string, RemoteCommand | { status: "sending" }>>({});
  const planKey = (d: string, r: string, p: string) => `${d}/${r}/${p}`;

  function lastCommand(d: string, r: string, p: string) {
    const mine = asked[planKey(d, r, p)];
    if (mine) return mine;
    return data?.commands.find((c) => c.device_id === d && c.repo_id === r && c.plan_id === p && ["pending", "claimed"].includes(c.status)) ?? null;
  }

  async function copyNow(device: RemoteDevice, repo: RemoteRepo, planId: string) {
    const key = planKey(device.id, repo.repo_id, planId);
    asked[key] = { status: "sending" };
    try {
      const id = await api.accountRequestBackup(device.id, repo.repo_id, planId);
      asked[key] = { id, device_id: device.id, repo_id: repo.repo_id, plan_id: planId, requested_at: new Date().toISOString(), status: "pending" };
      poll(key, id);
    } catch (e) {
      delete asked[key];
      toast(String(e), "error", 8000);
    }
  }

  /** Sigue la petición hasta que termina (cada 10 s, como mucho una hora). */
  function poll(key: string, id: string) {
    const started = Date.now();
    const t = setInterval(async () => {
      try {
        const c = await api.accountCommand(id);
        if (c) asked[key] = c;
        if (!c || ["done", "failed", "expired", "rejected"].includes(c.status) || Date.now() - started > HOUR) {
          clearInterval(t);
          if (c?.status === "done") load();
        }
      } catch {
        clearInterval(t);
      }
    }, 10_000);
  }

  function commandText(c: RemoteCommand | { status: "sending" }) {
    switch (c.status) {
      case "sending":
        return "Pidiendo…";
      case "pending":
        return "Pedida… empieza en menos de 5 minutos";
      case "claimed":
        return "En marcha";
      case "done":
        return "Hecha";
      case "expired":
        return "Caducó: el equipo no la recogió a tiempo";
      default:
        return `${c.status === "rejected" ? "Rechazada" : "Falló"}: ${"message" in c && c.message ? c.message : "sin detalle"}`;
    }
  }
</script>

<div class="page" in:fade={{ duration: dur(160) }}>
  {#if !status}
    <p class="faint">Consultando…</p>
  {:else if !status.signed_in}
    <!-- Iniciar sesión: correo y contraseña, y después el código del autenticador. -->
    <div class="signin">
      <header class="page-top">
        <div>
          <h1>Todos mis equipos</h1>
          <p>Mira el estado de tus otros equipos (el servidor, otros portátiles…) y pídeles «Copiar ahora».</p>
        </div>
      </header>
      <section class="card form-card">
        <div class="dlg-title">
          <span class="ticon"><Users size={18} /></span>
          <div>
            <h2>Entra con tu cuenta de Resguardo Web</h2>
            <p>La misma con la que entras en la web. Hace falta la verificación en dos pasos.</p>
          </div>
        </div>
        {#if status.needs_code}
          <form onsubmit={verify}>
            <label class="field">
              <span class="field-label">Código del autenticador</span>
              <input class="input code" inputmode="numeric" autocomplete="one-time-code" maxlength="7" placeholder="123 456" bind:value={code} />
              <span class="field-hint">El de 6 cifras que muestra tu aplicación de autenticación para {status.email}.</span>
            </label>
            {#if formError}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{formError}</p></div>{/if}
            <footer>
              <button type="button" class="btn btn-ghost" onclick={signOut}>Cancelar</button>
              <button class="btn btn-primary" disabled={busy || code.replace(/\D/g, "").length !== 6}>
                {#if busy}<span class="spin" style="display:grid"><LoaderCircle size={15} /></span>{/if} Verificar
              </button>
            </footer>
          </form>
        {:else}
          <form onsubmit={signIn}>
            <label class="field">
              <span class="field-label">Correo</span>
              <input class="input" type="email" autocomplete="username" bind:value={email} />
            </label>
            <label class="field">
              <span class="field-label">Contraseña</span>
              <input class="input" type="password" autocomplete="current-password" bind:value={password} />
              <span class="field-hint">No se guarda: Resguardo solo guarda la sesión, cifrada para tu usuario de Windows.</span>
            </label>
            {#if formError}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{formError}</p></div>{/if}
            <footer>
              <button class="btn btn-primary" disabled={busy || !email || !password}>
                {#if busy}<span class="spin" style="display:grid"><LoaderCircle size={15} /></span>{/if} Continuar
              </button>
            </footer>
          </form>
        {/if}
      </section>
      <p class="faint note">
        <ShieldCheck size={14} />
        <span>
          Para pedir una copia, ese equipo tiene que tener activadas las <strong>Copias a distancia</strong> (en Resguardo, en ese equipo). A distancia
          solo se pueden pedir copias: nada se puede borrar, restaurar ni cambiar.
          <HelpLink topic="copias-a-distancia" label="las copias a distancia" />
        </span>
      </p>
    </div>
  {:else}
    <section class="hero tone-{heroTone}" aria-labelledby="eq-title">
      <div class="hero-top">
        <span class="hero-mark" aria-hidden="true">
          {#if heroTone === "ok"}<ShieldCheck size={22} />{:else if heroTone === "warn"}<TriangleAlert size={22} />{:else}<CircleAlert size={22} />{/if}
        </span>
        <div class="hero-text">
          <h1 id="eq-title">
            {#if !data}Todos mis equipos{:else if attention}{attention} {attention === 1 ? "cosa necesita" : "cosas necesitan"} atención{:else}Todos tus equipos al día{/if}
          </h1>
          <p>
            {counts.devices} {counts.devices === 1 ? "equipo" : "equipos"}
            {#if counts.bytes}· {formatBytes(counts.bytes)} protegidos{/if}
            {#if counts.offline}· <span class="bad-text">{counts.offline} sin conexión</span>{/if}
            · {status.email}
          </p>
        </div>
        <span class="hero-actions">
          <button class="btn btn-ghost btn-sm" onclick={load} disabled={loading}><span class:spin={loading} style="display:grid"><RefreshCw size={14} /></span> Actualizar</button>
          <button class="btn btn-ghost btn-sm" onclick={signOut} title="Cerrar la sesión de la cuenta en este equipo"><LogOut size={14} /> Cerrar sesión</button>
        </span>
      </div>
      <p class="hero-note faint">
        <KeyRound size={13} /> Solo lectura y «Copiar ahora»: a distancia no se puede borrar, restaurar ni cambiar nada. Cada equipo decide si acepta copias a
        distancia.
      </p>
    </section>

    {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}

    {#if data && !data.devices.length}
      <div class="empty-state">
        <Monitor size={28} strokeWidth={1.6} />
        <p>Todavía no hay equipos en tu cuenta. Vincula cada equipo desde Resguardo, en «Resguardo Web».</p>
      </div>
    {/if}

    {#each groups as g (g.name)}
      <section class="client">
        <div class="section-head"><h2>{g.name} <span class="count">· {g.devices.length}</span></h2></div>
        {#each g.devices as { device, repos } (device.id)}
          {@const isThis = device.name === status.this_device}
          <article class="card device">
            <header class="device-head">
              <span class="repo-icon"><Monitor size={16} /></span>
              <span class="device-name">
                <strong>{device.name}{#if isThis}<span class="faint">{" · este equipo"}</span>{/if}</strong>
                <span class="faint">
                  {#if offline(device, now)}<WifiOff size={12} /> Sin conexión{#if device.last_seen_at}{" desde "}<RelTime iso={device.last_seen_at} />{/if}{:else}Visto <RelTime
                      iso={device.last_seen_at!}
                    />{/if}
                  {#if device.app_version} · v{device.app_version}{/if}
                </span>
              </span>
              {#if device.remote_backup_enabled}
                <span class="badge badge-sm tone-info" title="Este equipo acepta «Copiar ahora» a distancia">Copias a distancia</span>
              {:else}
                <span class="badge badge-sm tone-neutral" title="Actívalo en Resguardo, en ese equipo, para poder pedirle copias">Sin copias a distancia</span>
              {/if}
            </header>
            {#if !repos.length}
              <p class="faint none">Este equipo aún no informó de ningún repositorio.</p>
            {/if}
            <div class="repos">
              {#each repos as r (r.repo_id)}
                {@const st = repoState(r, now)}
                {@const Icon = TONE_ICON[st.tone]}
                <div class="repo">
                  <div class="repo-head">
                    <strong>{r.name}</strong>
                    <span class="faint kind">{r.host ?? r.kind}</span>
                    <span class="badge tone-{st.tone}"><Icon size={12} /> {st.label}</span>
                  </div>
                  <dl class="facts">
                    <div>
                      <dt>Última versión</dt>
                      <dd>{#if r.last_snapshot_at}<span title={formatDate(r.last_snapshot_at)}>{formatRelative(r.last_snapshot_at)}</span>{:else}<span class="faint">—</span>{/if}</dd>
                    </div>
                    <div>
                      <dt>Nube</dt>
                      <dd>
                        {#if r.offsite_hold}<span class="bad-text"><ShieldAlert size={12} /> frenada</span>
                        {:else if r.offsite_run?.finished}
                          <span class={r.offsite_run.result === "error" ? "bad-text" : r.offsite_run.result === "warning" ? "warn-text" : ""}>
                            <Cloud size={12} /> {r.offsite_run.result === "error" ? "falló" : formatRelative(r.offsite_run.finished)}
                          </span>
                        {:else}<span class="faint"><CloudOff size={12} /> ninguna</span>{/if}
                      </dd>
                    </div>
                    <div>
                      <dt>Protección</dt>
                      <dd>{#if r.protection}<span class="num">{r.protection.score} de {r.protection.total}</span>{:else}<span class="faint">—</span>{/if}</dd>
                    </div>
                  </dl>
                  {#if r.plans?.length}
                    <ul class="plans">
                      {#each r.plans as p (p.id)}
                        {@const c = lastCommand(device.id, r.repo_id, p.id)}
                        {@const running = c && ["sending", "pending", "claimed"].includes(c.status)}
                        <li>
                          <span class="pname">{p.name}</span>
                          <span class="pwhen faint">
                            {#if c}<span class="cmd s-{c.status}">{commandText(c)}</span>
                            {:else if p.last_run?.finished}{p.last_run.result === "error" ? "falló" : "última"} <RelTime iso={p.last_run.finished} />{:else}nunca{/if}
                          </span>
                          {#if !isThis}
                            <button
                              class="btn btn-sm"
                              onclick={() => copyNow(device, r, p.id)}
                              disabled={!!running || !device.remote_backup_enabled}
                              title={!device.remote_backup_enabled
                                ? "Ese equipo no acepta copias a distancia: actívalo en Resguardo, en ese equipo"
                                : running
                                  ? "Ya hay una copia pedida"
                                  : `Pedir a ${device.name} que copie «${p.name}» ahora`}
                            >
                              <Play size={12} fill="currentColor" /> Copiar ahora
                            </button>
                          {/if}
                        </li>
                      {/each}
                    </ul>
                  {/if}
                </div>
              {/each}
            </div>
          </article>
        {/each}
      </section>
    {/each}
  {/if}
</div>

<style>
  .signin {
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
    max-width: 520px;
  }
  .form-card {
    padding: var(--sp-6);
  }
  .form-card form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    margin-top: var(--sp-5);
  }
  .form-card footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--sp-2);
  }
  .code {
    max-width: 180px;
    font-size: 18px;
    letter-spacing: 0.15em;
    font-variant-numeric: tabular-nums;
  }
  .note {
    display: flex;
    gap: var(--sp-2);
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .note :global(svg) {
    flex: none;
    margin-top: 2px;
  }
  .hero {
    --tone: var(--ok);
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-6);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-xl);
  }
  .hero.tone-warn {
    --tone: var(--warn);
  }
  .hero.tone-bad {
    --tone: var(--bad);
  }
  .hero-top {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
  }
  .hero-mark {
    display: grid;
    place-items: center;
    flex: none;
    width: 48px;
    height: 48px;
    border-radius: 999px;
    color: var(--tone);
    background: color-mix(in srgb, var(--tone) var(--soft), transparent);
  }
  .hero-text {
    flex: 1;
    min-width: 0;
  }
  .hero-text h1 {
    font-size: var(--fs-display);
    line-height: var(--lh-display);
    font-weight: 650;
    letter-spacing: -0.022em;
  }
  .hero-text p {
    margin: 4px 0 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .hero-actions {
    display: flex;
    gap: 4px;
    align-self: flex-start;
  }
  .hero-note {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
    font-size: var(--fs-xs);
  }
  .client {
    display: flex;
    flex-direction: column;
  }
  .device {
    padding: var(--sp-5);
    margin-bottom: var(--sp-4);
  }
  .device-head {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }
  .repo-icon {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: var(--radius);
    color: var(--text-2);
    background: var(--surface-2);
  }
  .device-name {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .device-name strong {
    font-weight: 600;
  }
  .device-name > span {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: var(--fs-xs);
  }
  .none {
    margin: var(--sp-3) 0 0;
    font-size: var(--fs-sm);
  }
  .repos {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: var(--sp-3);
    margin-top: var(--sp-4);
  }
  .repo {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-4);
    border-radius: var(--radius-lg);
    background: var(--surface-2);
  }
  .repo-head {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }
  .repo-head strong {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kind {
    flex: 1;
    font-size: var(--fs-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .facts {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-3);
    margin: 0;
  }
  .facts dt {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .facts dd {
    display: flex;
    align-items: center;
    gap: 4px;
    margin: 2px 0 0;
    font-size: var(--fs-sm);
    font-weight: 500;
    white-space: nowrap;
  }
  .facts dd :global(svg) {
    vertical-align: -2px;
  }
  .bad-text {
    color: var(--bad);
  }
  .warn-text {
    color: var(--warn);
  }
  .plans {
    list-style: none;
    margin: 0;
    padding: var(--sp-2) 0 0;
    border-top: 1px solid var(--border);
  }
  .plans li {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-height: 34px;
  }
  .pname {
    font-size: var(--fs-sm);
    font-weight: 500;
  }
  .pwhen {
    flex: 1;
    min-width: 0;
    font-size: var(--fs-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cmd.s-pending,
  .cmd.s-sending,
  .cmd.s-claimed {
    color: var(--info);
  }
  .cmd.s-done {
    color: var(--ok);
  }
  .cmd.s-failed,
  .cmd.s-rejected,
  .cmd.s-expired {
    color: var(--bad);
  }
</style>
