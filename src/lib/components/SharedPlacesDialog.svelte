<script lang="ts">
  import { onMount } from "svelte";
  import { Check, CircleAlert, Clock, Copy, Inbox, LoaderCircle, LogIn, RefreshCw, Share2, ShieldCheck, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { ReceivedShare, Repo, SharesAvailable } from "$lib/api";
  import { formatDate, formatRelative } from "$lib/format";
  import { generatePassword, slug } from "$lib/placeRepos";
  import { openSettings } from "$lib/settingsNav.svelte";
  import { toast } from "$lib/toast.svelte";
  import Modal from "./Modal.svelte";
  import HelpLink from "./HelpLink.svelte";

  // «Usar un destino compartido»: lo que comparten tus otros equipos (se pide
  // y llega cifrado en unos minutos) y lo ya recibido, donde se crea o conecta
  // un repositorio propio (docs/compartir.md).
  interface Props {
    onclose: () => void;
    oncreated: (repo: Repo, generated: boolean) => void;
    /** Ir a «Todos mis equipos» (para iniciar sesión). */
    onequipos: () => void;
  }
  let { onclose, oncreated, onequipos }: Props = $props();

  let available = $state<SharesAvailable | null>(null);
  let availableError = $state("");
  let received = $state<ReceivedShare[] | null>(null);
  let receivedError = $state("");
  let elevated = $state(true);
  let loading = $state(false);
  let deviceName = $state("este-equipo");

  async function load() {
    loading = true;
    await Promise.all([
      api.sharesAvailable().then(
        (a) => ((available = a), (availableError = "")),
        (e) => (availableError = String(e)),
      ),
      api.sharesReceived().then(
        (r) => ((received = r), (receivedError = ""), (elevated = true)),
        (e) => {
          receivedError = String(e);
          elevated = !/administrador/i.test(String(e));
        },
      ),
    ]);
    loading = false;
  }

  onMount(() => {
    void load();
    api.webInfo().then(
      (w) => (deviceName = w.default_name || deviceName),
      () => {},
    );
    // Lo pedido llega en unos minutos: se vuelve a mirar mientras está abierto.
    const t = setInterval(() => void load(), 20_000);
    return () => clearInterval(t);
  });

  const notSignedIn = $derived(/sesi[oó]n|inicia/i.test(availableError));
  const requestOf = (id: string) => available?.requests.find((r) => r.share_id === id && r.status !== "expired" && r.status !== "cancelled") ?? null;
  const isMine = (deviceId: string) => !!available?.this_device_id && deviceId === available.this_device_id;

  let asking = $state<string | null>(null);
  async function ask(id: string) {
    asking = id;
    try {
      await api.shareRequest(id);
      toast("Pedido: llegará cifrado en unos minutos", "info");
      await load();
    } catch (e) {
      toast(String(e), "error", 7000);
    } finally {
      asking = null;
    }
  }

  async function cancel(id: string) {
    try {
      await api.shareCancel(id);
      toast("Petición cancelada", "info");
      await load();
    } catch (e) {
      toast(String(e), "error");
    }
  }

  // Crear o conectar un repositorio en un destino recibido.
  let form = $state<{ share: ReceivedShare; create: boolean; name: string; location: string; password: string; saved: boolean } | null>(null);
  let busy = $state(false);
  let formError = $state("");
  let copied = $state(false);
  function start(share: ReceivedShare, create: boolean) {
    formError = "";
    const base = share.base.replace(/\/+$/, "");
    form = {
      share,
      create,
      name: create ? deviceName : share.meta.name,
      location: create ? `${base}/${slug(deviceName)}${share.meta.kind === "rest" ? "/" : ""}` : `${base}/`,
      password: create ? generatePassword() : "",
      saved: !create,
    };
  }
  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!form || busy) return;
    if (form.create && !form.saved) {
      formError = "Guarda antes la contraseña: sin ella nadie podrá abrir este repositorio.";
      return;
    }
    busy = true;
    formError = "";
    try {
      const repo = await api.receivedShareAdd(form.share.id, form.name.trim(), form.location.trim(), form.password, form.create);
      toast(`Repositorio «${repo.name}» ${form.create ? "creado" : "añadido"} en «${form.share.meta.name}»`);
      oncreated(repo, form.create);
    } catch (err) {
      formError = String(err);
    } finally {
      busy = false;
    }
  }
</script>

<Modal {onclose} labelledby="sh-title" width={640} dismissible={false}>
  <header class="dlg-head">
    <div class="dlg-title">
      <span class="ticon"><Share2 size={19} /></span>
      <div>
        <h2 id="sh-title">Usar un destino compartido <HelpLink topic="compartir-destino" label="compartir un destino" /></h2>
        <p class="faint">Una cuenta de la nube o un servidor que compartes desde otro equipo tuyo</p>
      </div>
    </div>
    <button class="icon-btn" title="Actualizar" aria-label="Actualizar" onclick={load} disabled={loading}>
      {#if loading}<span class="spin" style="display:grid"><LoaderCircle size={16} /></span>{:else}<RefreshCw size={16} />{/if}
    </button>
    <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={onclose}><X size={17} /></button>
  </header>

  {#if form}
    <form class="body" onsubmit={submit}>
      <p class="lead">
        {form.create ? "Crear un repositorio nuevo" : "Usar un repositorio que ya existe"} en <strong>«{form.share.meta.name}»</strong> (recibido de
        «{form.share.from_device}»).
      </p>
      <label class="field"><span class="field-label">Nombre en Resguardo</span><input class="input" bind:value={form.name} maxlength="80" required /></label>
      <label class="field"><span class="field-label">Ubicación</span><input class="input mono" bind:value={form.location} spellcheck="false" required /></label>
      <label class="field">
        <span class="field-label">{form.create ? "Contraseña del repositorio nuevo" : "Contraseña de ese repositorio"}</span>
        <span class="pw">
          <input class="input mono" type={form.create ? "text" : "password"} bind:value={form.password} readonly={form.create} required />
          {#if form.create}
            <button
              type="button"
              class="icon-btn"
              title="Copiar"
              aria-label="Copiar la contraseña"
              onclick={async () => {
                await navigator.clipboard.writeText(form?.password ?? "").catch(() => {});
                copied = true;
                setTimeout(() => (copied = false), 1500);
              }}>{#if copied}<Check size={15} />{:else}<Copy size={15} />{/if}</button
            >
          {/if}
        </span>
      </label>
      {#if form.create}
        <label class="check"><input type="checkbox" bind:checked={form.saved} /> La he copiado o la guardaré en el kit de recuperación</label>
        <p class="faint small">La contraseña es solo de este equipo: el equipo que compartió el destino no la conoce ni puede abrir este repositorio.</p>
      {:else}
        <p class="faint small">Si otro equipo copia en ese repositorio, quien tenga la contraseña ve las versiones de los dos. Úsalo sobre todo para restaurar o verificar.</p>
      {/if}
      {#if formError}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{formError}</p></div>{/if}
      <footer>
        <button type="button" class="btn btn-ghost" onclick={() => (form = null)} disabled={busy}>Atrás</button>
        <button class="btn btn-primary" disabled={busy}>
          {#if busy}<span class="spin" style="display:grid"><LoaderCircle size={14} /></span>{/if}
          {form.create ? "Crear repositorio" : "Comprobar y añadir"}
        </button>
      </footer>
    </form>
  {:else}
    <div class="body">
      <h3 class="sub"><Inbox size={15} /> Recibidos en este equipo</h3>
      {#if !elevated}
        <div class="notice notice-info">
          <ShieldCheck size={16} />
          <p>
            Lo recibido lo guarda el agente: para verlo y usarlo, abre Resguardo como administrador.
            <button class="link notice-action" onclick={() => api.relaunchAsAdmin().catch((e) => toast(String(e), "error"))}>Abrir como administrador</button>
          </p>
        </div>
      {:else if receivedError}
        <div class="notice notice-danger"><CircleAlert size={16} /><p>{receivedError}</p></div>
      {:else if received && !received.length}
        <p class="faint small">Todavía nada. Pide abajo un destino y aparecerá aquí en unos minutos.</p>
      {:else if received}
        <ul class="list">
          {#each received as r (r.id)}
            <li>
              <span class="txt">
                <strong>{r.meta.name}</strong>
                <span class="faint small">de «{r.from_device}» · {r.meta.host ?? r.meta.kind} · recibido <span title={formatDate(r.received_at)}>{formatRelative(r.received_at)}</span></span>
              </span>
              <span class="acts">
                <button class="btn btn-ghost btn-sm" onclick={() => start(r, false)}>Usar uno existente</button>
                <button class="btn btn-primary btn-sm" onclick={() => start(r, true)}>Crear repositorio aquí</button>
              </span>
            </li>
          {/each}
        </ul>
      {/if}

      <h3 class="sub"><Share2 size={15} /> Compartidos por tus otros equipos</h3>
      {#if notSignedIn}
        <div class="notice notice-info">
          <LogIn size={16} />
          <p>
            Para verlos, inicia sesión en «Todos mis equipos» (con tu código de verificación).
            <button class="link notice-action" onclick={onequipos}>Iniciar sesión</button>
          </p>
        </div>
      {:else if availableError}
        <div class="notice notice-danger"><CircleAlert size={16} /><p>{availableError}</p></div>
      {:else if available && !available.this_device_id}
        <div class="notice notice-info">
          <CircleAlert size={16} />
          <p>
            Este equipo no está vinculado con Resguardo Web, así que no puede recibir nada cifrado.
            <button class="link notice-action" onclick={() => (onclose(), openSettings("equipo"))}>Vincularlo</button>
          </p>
        </div>
      {:else if available && !available.shares.length}
        <p class="faint small">Ninguno de tus equipos comparte un destino. Actívalo en la página de un destino de la nube o de un servidor, en el equipo que lo tiene.</p>
      {:else if available}
        <ul class="list">
          {#each available.shares as s (s.id)}
            {@const req = requestOf(s.id)}
            <li>
              <span class="txt">
                <strong>{s.name}</strong>
                <span class="faint small">de «{s.device_name}» · {s.host ?? s.kind}{s.base && s.base !== "/" ? ` · ${s.base}` : ""} · desde {formatRelative(s.created_at)}</span>
                {#if req?.status === "rejected"}<span class="err small">No se entregó: {req.reason ?? "sin motivo"}</span>{/if}
              </span>
              <span class="acts">
                {#if isMine(s.device_id)}
                  <span class="badge badge-sm tone-neutral">Es de este equipo</span>
                {:else if req?.status === "pending"}
                  <span class="badge badge-sm tone-info" title="Se entrega pasados unos 5 minutos: hasta entonces puedes cancelarlo"><Clock size={11} /> Pedido · llega en unos minutos</span>
                  <button class="btn btn-ghost btn-sm" onclick={() => cancel(req.id)}>Cancelar</button>
                {:else if req?.status === "delivered" || req?.status === "received"}
                  <span class="badge badge-sm tone-ok"><Check size={11} /> Recibido</span>
                {:else}
                  <button class="btn btn-sm" onclick={() => ask(s.id)} disabled={asking === s.id}>Pedir</button>
                {/if}
              </span>
            </li>
          {/each}
        </ul>
        <p class="faint small">
          Al pedirlo, recibes un aviso y tienes 5 minutos para cancelarlo; después, el equipo que lo comparte lo cifra solo para este y lo entrega en su siguiente ciclo. Los dos equipos muestran un aviso.
        </p>
      {/if}
    </div>
  {/if}
</Modal>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .sub {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 6px 0 0;
    font-size: var(--fs-sm);
    font-weight: 650;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 12px;
  }
  .list li + li {
    border-top: 1px solid var(--border);
  }
  .txt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .acts {
    display: flex;
    gap: 6px;
    flex: none;
  }
  .small {
    font-size: var(--fs-sm);
  }
  .err {
    color: var(--bad);
  }
  .lead {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .pw {
    display: flex;
    gap: 4px;
  }
  .pw .input {
    flex: 1;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
