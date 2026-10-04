<script lang="ts">
  // «Equipos gestionados» (fase 5, docs/agente-gestionado.md): esta consola
  // empareja equipos con Resguardo Agente, les asigna qué copiar y cuándo, y
  // guarda sus copias en el Servidor de copias. Todo lo que les manda va
  // firmado por ella; desde aquí también se puede restaurar lo suyo.
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { dur } from "$lib/motion";
  import {
    ArchiveRestore,
    Eraser,
    MonitorDown,
    CalendarClock,
    CircleAlert,
    CirclePause,
    EyeOff,
    FolderSync,
    LoaderCircle,
    Monitor,
    MonitorCheck,
    Play,
    Plus,
    Server,
    ShieldCheck,
    Unlink,
  } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { ManagedEndpoint, ManagedRetention, Plan, Repo, ServerStatus } from "$lib/api";
  import { formatBytes, formatDate } from "$lib/format";
  import { planContentsLabel, planScheduleSentence } from "$lib/plans";
  import { relaunchToSettings } from "$lib/settingsNav.svelte";
  import { toast } from "$lib/toast.svelte";
  import HelpLink from "./HelpLink.svelte";
  import ManagedPairDialog from "./ManagedPairDialog.svelte";
  import ManagedPlanDialog from "./ManagedPlanDialog.svelte";
  import ManagedPrepareDialog from "./ManagedPrepareDialog.svelte";
  import Modal from "./Modal.svelte";
  import RelTime from "./RelTime.svelte";

  interface Props {
    /** Abre un repositorio (para restaurar lo de un equipo). */
    onopenrepo: (repo: Repo) => void;
    /** Ajustes → Este equipo (para activar el Servidor de copias). */
    onsettings: () => void;
  }
  let { onopenrepo, onsettings }: Props = $props();

  let srv = $state<ServerStatus | null>(null);
  let endpoints = $state<ManagedEndpoint[] | null>(null);
  let error = $state("");
  let pairing = $state(false);
  let preparing = $state(false);
  let editing = $state<{ endpoint: ManagedEndpoint; plan: Plan | null } | null>(null);
  let unpairing = $state<ManagedEndpoint | null>(null);
  let busy = $state<string>("");

  onMount(load);
  async function load() {
    error = "";
    try {
      srv = await api.serverStatus();
      endpoints = await api.managedList();
    } catch (e) {
      error = String(e);
    }
  }

  const active = $derived((endpoints ?? []).filter((e) => !e.stopped));
  const withoutPlans = $derived(active.filter((e) => !e.plans.length).length);

  function replace(e: ManagedEndpoint) {
    endpoints = (endpoints ?? []).some((x) => x.device_id === e.device_id) ? endpoints!.map((x) => (x.device_id === e.device_id ? e : x)) : [...(endpoints ?? []), e];
  }

  async function savePlans(e: ManagedEndpoint, plans: Plan[]) {
    const updated = await api.managedSetConfig(e.device_id, plans, e.tray, e.tray_toasts);
    replace(updated);
    editing = null;
    toast(`Enviado a «${e.name}»: lo aplicará en unos minutos.`);
  }

  async function setTray(e: ManagedEndpoint, tray: boolean, toasts: boolean) {
    busy = `tray:${e.device_id}`;
    try {
      replace(await api.managedSetConfig(e.device_id, $state.snapshot(e.plans) as Plan[], tray, toasts));
    } catch (err) {
      toast(String(err), "error");
    } finally {
      busy = "";
    }
  }

  async function backupNow(e: ManagedEndpoint, p: Plan) {
    busy = `run:${e.device_id}#${p.id}`;
    try {
      await api.managedBackupNow(e.device_id, p.id);
      toast(`Pedido a «${e.name}»: copiará «${p.name}» en unos minutos.`);
    } catch (err) {
      toast(String(err), "error");
    } finally {
      busy = "";
    }
  }

  const RETENTION: { id: ManagedRetention; label: string; hint: string }[] = [
    { id: "frecuente", label: "Frecuente", hint: "horarias 15 días · diarias 1 año · mensuales siempre" },
    { id: "equilibrada", label: "Equilibrada", hint: "diarias 30 días · semanales 6 meses · mensuales 2 años" },
    { id: "ligera", label: "Ligera", hint: "diarias 60 días · mensuales siempre" },
    { id: "todo", label: "Guardar todo", hint: "no se borra ninguna versión" },
  ];

  async function setRetention(e: ManagedEndpoint, retention: ManagedRetention) {
    busy = `ret:${e.device_id}`;
    try {
      replace(await api.managedSetRetention(e.device_id, retention));
    } catch (err) {
      toast(String(err), "error");
    } finally {
      busy = "";
    }
  }

  async function pruneNow(e: ManagedEndpoint) {
    busy = `prune:${e.device_id}`;
    try {
      await api.managedPruneNow(e.device_id);
      toast(`Retención aplicada a las copias de «${e.name}».`);
      await load();
    } catch (err) {
      toast(String(err), "error", 8000);
    } finally {
      busy = "";
    }
  }

  /** Hace más de 2 días de la última versión (con copias asignadas con horario). */
  const late = (e: ManagedEndpoint) =>
    !e.stopped && e.plans.some((p) => p.schedule) && (!e.last_snapshot || Date.now() - Date.parse(e.last_snapshot) > 2 * 86_400_000) && Date.now() - Date.parse(e.paired_at) > 86_400_000;

  async function restore(e: ManagedEndpoint) {
    busy = `open:${e.device_id}`;
    try {
      onopenrepo(await api.managedOpenRepo(e.device_id));
    } catch (err) {
      toast(String(err), "error");
    } finally {
      busy = "";
    }
  }

  async function unpair() {
    const e = unpairing;
    if (!e) return;
    busy = `unpair:${e.device_id}`;
    try {
      replace(await api.managedUnpair(e.device_id));
      toast(`«${e.name}» ya no se gestiona desde aquí.`);
      unpairing = null;
    } catch (err) {
      toast(String(err), "error");
    } finally {
      busy = "";
    }
  }
</script>

<div class="page" in:fade={{ duration: dur(160) }}>
  <header class="page-top">
    <div>
      <h1>Equipos gestionados</h1>
      <p>Equipos que administras desde aquí: tú decides qué copian y cuándo, y sus copias se guardan en este Servidor de copias.</p>
    </div>
    {#if srv?.enabled && endpoints?.length}
      <div class="top-actions">
        <button class="btn" onclick={() => (preparing = true)}><MonitorDown size={15} /> Preparar un equipo</button>
        <button class="btn btn-primary" onclick={() => (pairing = true)}><Plus size={15} /> Emparejar un equipo</button>
      </div>
    {/if}
  </header>

  {#if error}
    <div class="notice notice-danger" role="alert">
      <CircleAlert size={16} />
      <p>{error}</p>
      {#if error.includes("administrador")}
        <button class="btn btn-sm" onclick={() => relaunchToSettings("equipo")}>Abrir como administrador</button>
      {/if}
    </div>
  {:else if !srv || !endpoints}
    <p class="faint">Consultando…</p>
  {:else if !srv.enabled}
    <section class="card empty-state">
      <Server size={28} />
      <p><strong>Primero, el Servidor de copias</strong><span>Los equipos gestionados guardan sus copias en este equipo. Actívalo en Ajustes → Este equipo y vuelve aquí.</span></p>
      <button class="btn btn-primary" onclick={onsettings}>Ir a Ajustes</button>
    </section>
  {:else if !endpoints.length}
    <section class="card empty-state">
      <MonitorCheck size={28} />
      <p>
        <strong>Aún no gestionas ningún equipo</strong>
        <span>Instala Resguardo Agente en el equipo (sin ventana, solo un servicio y un icono en la bandeja), empareja con un código y asígnale sus copias desde aquí.</span>
      </p>
      <div class="top-actions">
        <button class="btn" onclick={() => (preparing = true)}><MonitorDown size={15} /> Preparar un equipo</button>
        <button class="btn btn-primary" onclick={() => (pairing = true)}><Plus size={15} /> Emparejar un equipo</button>
      </div>
    </section>
  {:else}
    {#if withoutPlans}
      <div class="notice notice-info" role="status">
        <CalendarClock size={16} />
        <p>{withoutPlans === 1 ? "Un equipo aún no tiene" : `${withoutPlans} equipos aún no tienen`} copias asignadas: no copia nada hasta que le añadas una.</p>
      </div>
    {/if}

    <div class="list">
      {#each endpoints as e (e.device_id)}
        <section class="card endpoint" class:stopped={e.stopped}>
          <div class="head">
            <span class="icon"><Monitor size={18} /></span>
            <div class="name">
              <strong>{e.name}</strong>
              <span class="faint">Emparejado <RelTime iso={e.paired_at} /> · en el servidor como <span class="mono">{e.server_user}</span></span>
            </div>
            {#if e.stopped}
              <span class="badge tone-neutral"><CirclePause size={12} /> Ya no se gestiona</span>
            {:else if !e.plans.length}
              <span class="badge tone-warn">Sin copias</span>
            {:else if late(e)}
              <span class="badge tone-warn"><CircleAlert size={12} /> Sin copias recientes</span>
            {:else}
              <span class="badge tone-ok"><ShieldCheck size={12} /> Gestionado</span>
            {/if}
          </div>

          <dl class="facts">
            <div>
              <dt>Última versión</dt>
              <dd>{#if e.last_snapshot}<span title={formatDate(e.last_snapshot)}><RelTime iso={e.last_snapshot} /></span>{:else}<span class="faint">todavía ninguna</span>{/if}</dd>
            </div>
            <div>
              <dt>Versiones</dt>
              <dd class="num">{e.snapshots}</dd>
            </div>
            <div>
              <dt>En el servidor</dt>
              <dd class="num">{formatBytes(e.bytes)}</dd>
            </div>
          </dl>

          {#if !e.stopped}
            <ul class="plans">
              {#each e.plans as p (p.id)}
                <li>
                  <FolderSync size={15} />
                  <button class="link pname" onclick={() => (editing = { endpoint: e, plan: p })} title="Cambiar esta copia">{p.name}</button>
                  <span class="faint pwhen">{planContentsLabel(p)} · {p.schedule ? planScheduleSentence(p.schedule) : "solo a mano"}</span>
                  <button class="btn btn-sm" onclick={() => backupNow(e, p)} disabled={busy === `run:${e.device_id}#${p.id}`} title="Pedir al equipo que copie ya">
                    {#if busy === `run:${e.device_id}#${p.id}`}<span class="spin"><LoaderCircle size={13} /></span>{:else}<Play size={13} />{/if} Copiar ahora
                  </button>
                </li>
              {/each}
              <li>
                <button class="btn btn-ghost btn-sm" onclick={() => (editing = { endpoint: e, plan: null })}><Plus size={13} /> Añadir una copia</button>
              </li>
            </ul>

            <div class="tray">
              <label class="switch-row">
                <input type="checkbox" class="switch" checked={e.tray} disabled={busy === `tray:${e.device_id}`} onchange={(ev) => setTray(e, ev.currentTarget.checked, e.tray_toasts)} />
                <span>Icono en la bandeja <span class="faint">Su usuario ve un estado tranquilo («Tus archivos están protegidos») y quién gestiona el equipo.</span></span>
              </label>
              <label class="switch-row">
                <input type="checkbox" class="switch" checked={e.tray_toasts} disabled={!e.tray || busy === `tray:${e.device_id}`} onchange={(ev) => setTray(e, e.tray, ev.currentTarget.checked)} />
                <span>Avisar de cada copia <span class="faint">«Copia en curso» y «Copia terminada». Desactivado, no molesta.</span></span>
              </label>
            </div>
          {/if}

          <div class="retention">
            <label class="ret-label" for="ret-{e.device_id}"><Eraser size={14} /> Versiones que se guardan</label>
            <select id="ret-{e.device_id}" class="input ret-select" value={e.retention} disabled={busy === `ret:${e.device_id}`} onchange={(ev) => setRetention(e, ev.currentTarget.value as ManagedRetention)}>
              {#each RETENTION as r (r.id)}<option value={r.id}>{r.label} · {r.hint}</option>{/each}
            </select>
            {#if e.retention !== "todo"}
              <button class="btn btn-ghost btn-sm" onclick={() => pruneNow(e)} disabled={!!busy} title="Esta consola borra en el servidor las versiones que ya no se guardan (cada semana lo hace sola)">
                {#if busy === `prune:${e.device_id}`}<span class="spin"><LoaderCircle size={13} /></span> Aplicando…{:else}Aplicar ahora{/if}
              </button>
            {/if}
            <span class="faint ret-note">
              {#if e.last_prune_error}<span class="bad-text">La última vez falló: {e.last_prune_error}</span>
              {:else if e.last_prune}Aplicada <RelTime iso={e.last_prune} />. Se aplica cada semana en este servidor.
              {:else if e.retention !== "todo"}Se aplica cada semana en este servidor (el equipo no puede borrar nada).{/if}
            </span>
          </div>

          <footer class="actions">
            <button class="btn btn-sm" onclick={() => restore(e)} disabled={busy === `open:${e.device_id}`} title="Abre sus copias aquí para ver versiones y restaurar">
              {#if busy === `open:${e.device_id}`}<span class="spin"><LoaderCircle size={13} /></span>{:else}<ArchiveRestore size={14} />{/if} Restaurar sus archivos…
            </button>
            {#if !e.stopped}
              <button class="btn btn-ghost btn-sm danger-text" onclick={() => (unpairing = e)}><Unlink size={14} /> Dejar de gestionar</button>
            {/if}
          </footer>
        </section>
      {/each}
    </div>
  {/if}

  <p class="faint note">
    <EyeOff size={14} />
    <span>
      Lo que ves de cada equipo: sus copias y si van bien. Sus archivos solo se abren aquí si pulsas «Restaurar». Cada orden va firmada por esta consola y cifrada solo para ese
      equipo: ni Resguardo Web puede leerla ni falsificarla. <HelpLink topic="equipos-gestionados" label="los equipos gestionados" />
    </span>
  </p>
</div>

{#if pairing}
  <ManagedPairDialog
    onclose={() => (pairing = false)}
    onpaired={(e) => {
      pairing = false;
      replace(e);
      toast(`«${e.name}» emparejado. Ahora asígnale qué copiar.`);
      editing = { endpoint: e, plan: null };
    }}
  />
{/if}

{#if preparing}
  <ManagedPrepareDialog
    onclose={() => (preparing = false)}
    onpair={() => {
      preparing = false;
      pairing = true;
    }}
  />
{/if}

{#if editing}
  <ManagedPlanDialog endpoint={editing.endpoint} plan={editing.plan} onclose={() => (editing = null)} onsave={(plans) => savePlans(editing!.endpoint, plans)} />
{/if}

{#if unpairing}
  <Modal onclose={() => (unpairing = null)} labelledby="unpair-title" width={480}>
    <header class="dlg-head">
      <div class="dlg-title">
        <span class="ticon danger"><Unlink size={18} /></span>
        <div>
          <h2 id="unpair-title">¿Dejar de gestionar «{unpairing.name}»?</h2>
          <p>Ese equipo dejará de copiar.</p>
        </div>
      </div>
    </header>
    <p class="muted">
      Sus copias ya hechas se quedan en este servidor y las puedes seguir restaurando. Para volver a gestionarlo hará falta un administrador en ese equipo.
    </p>
    <footer>
      <button class="btn btn-ghost" onclick={() => (unpairing = null)}>Cancelar</button>
      <button class="btn btn-danger" onclick={unpair} disabled={!!busy}>Dejar de gestionar</button>
    </footer>
  </Modal>
{/if}

<style>
  .top-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .facts {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-3);
    margin: 0;
    padding: var(--sp-3) var(--sp-4);
    border-radius: var(--radius-lg);
    background: var(--surface-2);
  }
  .facts dt {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .facts dd {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
    font-weight: 500;
  }
  .retention {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px var(--sp-3);
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .ret-label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
    font-weight: 500;
  }
  .ret-select {
    width: auto;
    max-width: 100%;
    height: 32px;
    font-size: var(--fs-sm);
  }
  .ret-note {
    flex-basis: 100%;
    font-size: var(--fs-xs);
  }
  .bad-text {
    color: var(--bad);
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .endpoint {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-5);
  }
  .endpoint.stopped {
    opacity: 0.8;
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }
  .icon {
    display: grid;
    place-items: center;
    flex: none;
    width: 36px;
    height: 36px;
    border-radius: var(--radius);
    color: var(--text-2);
    background: var(--surface-2);
  }
  .name {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .name strong {
    font-weight: 600;
  }
  .name > span {
    font-size: var(--fs-xs);
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
    min-height: 38px;
    color: var(--text-2);
  }
  .pname {
    font-size: var(--fs-sm);
    font-weight: 500;
    color: var(--text-1);
  }
  .link {
    padding: 0;
    font: inherit;
    background: none;
    border: none;
    cursor: pointer;
  }
  .link:hover {
    text-decoration: underline;
  }
  .pwhen {
    flex: 1;
    min-width: 0;
    font-size: var(--fs-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tray {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: var(--sp-3) var(--sp-5);
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .tray .switch-row > span {
    display: flex;
    flex-direction: column;
  }
  .tray .faint {
    font-size: var(--fs-xs);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .danger-text {
    color: var(--bad);
  }
  .note {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-xs);
  }
  .note :global(svg) {
    flex: none;
    margin-top: 2px;
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
