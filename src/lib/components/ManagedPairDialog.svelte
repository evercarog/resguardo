<script lang="ts">
  // Emparejar un equipo gestionado (fase 5, docs/agente-gestionado.md): la
  // consola abre un código, el equipo se une con `resguardo-agente.exe
  // --pair <código>` y los dos muestran un código de comprobación (SAS) que
  // hay que ver igual en ambos antes de confirmar.
  import { onDestroy, onMount } from "svelte";
  import { Check, CircleAlert, Copy, LoaderCircle, Monitor, ShieldCheck, Terminal, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { ManagedEndpoint, ManagedPairingPoll, ManagedPairingStart } from "$lib/api";
  import Modal from "./Modal.svelte";

  interface Props {
    onclose: () => void;
    onpaired: (e: ManagedEndpoint) => void;
  }
  let { onclose, onpaired }: Props = $props();

  let start = $state<ManagedPairingStart | null>(null);
  let poll = $state<ManagedPairingPoll | null>(null);
  let error = $state("");
  let busy = $state(false);
  let copied = $state<"" | "code" | "cmd">("");
  let now = $state(Date.now());
  let timer: ReturnType<typeof setInterval> | undefined;
  let clock: ReturnType<typeof setInterval> | undefined;

  const command = $derived(start ? `resguardo-agente.exe --pair ${start.code}` : "");
  const expiresAt = $derived(Date.parse(poll?.expires_at ?? start?.expires_at ?? ""));
  const left = $derived(Number.isFinite(expiresAt) ? Math.max(0, Math.round((expiresAt - now) / 1000)) : null);
  const expired = $derived(poll?.status === "expired" || poll?.status === "cancelled" || left === 0);
  const joined = $derived(poll?.status === "joined" && !!poll.sas);
  const leftLabel = $derived(left == null ? "" : `${Math.floor(left / 60)}:${String(left % 60).padStart(2, "0")}`);

  async function open() {
    error = "";
    poll = null;
    start = null;
    busy = true;
    try {
      start = await api.managedPairStart();
      schedule();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function schedule() {
    clearInterval(timer);
    timer = setInterval(async () => {
      if (!start || joined || expired) return;
      try {
        poll = await api.managedPairPoll(start.pairing_id);
      } catch (e) {
        error = String(e);
      }
    }, 3000);
  }

  onMount(() => {
    open();
    clock = setInterval(() => (now = Date.now()), 1000);
  });
  onDestroy(() => {
    clearInterval(timer);
    clearInterval(clock);
  });

  async function copy(text: string, what: "code" | "cmd") {
    try {
      await navigator.clipboard.writeText(text);
      copied = what;
      setTimeout(() => (copied = ""), 1600);
    } catch {
      /* sin portapapeles: el texto se puede seleccionar */
    }
  }

  async function confirm() {
    if (!start) return;
    busy = true;
    error = "";
    try {
      const e = await api.managedPairConfirm(start.pairing_id);
      onpaired(e);
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  /** No coincide: la web borra el equipo sin confirmar. */
  async function mismatch() {
    if (!start) return;
    busy = true;
    try {
      await api.managedPairCancel(start.pairing_id);
    } catch {
      /* caduca solo en 15 minutos */
    }
    busy = false;
    poll = { status: "cancelled", expires_at: null, endpoint_name: poll?.endpoint_name ?? null, sas: null };
  }

  async function close() {
    // Cerrar sin confirmar: que no quede un equipo a medias.
    if (start && !expired && poll?.status !== "confirmed") {
      try {
        await api.managedPairCancel(start.pairing_id);
      } catch {
        /* caduca solo */
      }
    }
    onclose();
  }
</script>

<Modal onclose={close} labelledby="pair-title" width={540} dismissible={false}>
  <header class="dlg-head">
    <div class="dlg-title">
      <span class="ticon"><Monitor size={18} /></span>
      <div>
        <h2 id="pair-title">Emparejar un equipo</h2>
        <p>Lo gestionarás desde aquí y copiará en este Servidor de copias.</p>
      </div>
    </div>
    <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={close}><X size={17} /></button>
  </header>

  <div class="body">
    {#if busy && !start}
      <p class="faint wait"><span class="spin"><LoaderCircle size={15} /></span> Preparando el código…</p>
    {:else if expired}
      <div class="notice notice-warn" role="status">
        <CircleAlert size={16} />
        <p>
          {#if poll?.status === "cancelled"}Emparejamiento cancelado: ese equipo no queda unido.{:else}El código ha caducado.{/if}
          Puedes empezar de nuevo con un código nuevo.
        </p>
      </div>
    {:else if joined && poll}
      <p>
        <strong>«{poll.endpoint_name ?? "Un equipo"}»</strong> se ha unido. Comprueba que en ese equipo aparece <strong>el mismo código</strong>:
      </p>
      <div class="sas num" aria-label="Código de comprobación">{poll.sas}</div>
      <p class="faint small">
        Si no coincide, alguien podría estar haciéndose pasar por él: pulsa «No coincide». Quedan {leftLabel} para confirmar.
      </p>
    {:else if start}
      <ol class="steps">
        <li>
          <span>En el equipo que quieres gestionar, abre una consola <strong>como administrador</strong> en la carpeta de Resguardo Agente y escribe:</span>
          <div class="cmd">
            <Terminal size={14} />
            <code class="mono">{command}</code>
            <button class="icon-btn" title="Copiar la orden" aria-label="Copiar la orden" onclick={() => copy(command, "cmd")}>
              {#if copied === "cmd"}<Check size={15} />{:else}<Copy size={15} />{/if}
            </button>
          </div>
        </li>
        <li>
          <span>O escribe solo el código cuando lo pida:</span>
          <div class="code-row">
            <span class="code mono">{start.code}</span>
            <button class="btn btn-sm" onclick={() => copy(start!.code, "code")}>
              {#if copied === "code"}<Check size={14} /> Copiado{:else}<Copy size={14} /> Copiar{/if}
            </button>
          </div>
        </li>
      </ol>
      <p class="faint wait" aria-live="polite">
        <span class="spin"><LoaderCircle size={15} /></span> Esperando a que el equipo se una… <span class="num">{leftLabel}</span>
      </p>
    {/if}

    {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}

    <p class="faint note">
      <ShieldCheck size={14} />
      <span>El código vale una sola vez y caduca. Todo lo que mandes al equipo va firmado por esta consola y cifrado solo para él: ni la web puede leerlo ni cambiarlo.</span>
    </p>
  </div>

  <footer>
    {#if expired}
      <button class="btn btn-ghost" onclick={onclose}>Cerrar</button>
      <button class="btn btn-primary" onclick={open} disabled={busy}>Nuevo código</button>
    {:else if joined}
      <button class="btn btn-ghost" onclick={mismatch} disabled={busy}>No coincide</button>
      <button class="btn btn-primary" onclick={confirm} disabled={busy}>
        {#if busy}<span class="spin" style="display:grid"><LoaderCircle size={15} /></span> Preparando su copia…{:else}<Check size={15} /> Coincide: confirmar{/if}
      </button>
    {:else}
      <button class="btn btn-ghost" onclick={close}>Cancelar</button>
    {/if}
  </footer>
</Modal>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .body p {
    margin: 0;
  }
  .small {
    font-size: var(--fs-sm);
  }
  .wait {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
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
  .steps {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    margin: 0;
    padding-left: 20px;
    font-size: var(--fs-sm);
  }
  .steps li > span {
    display: block;
    margin-bottom: 8px;
  }
  .cmd {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 6px 6px 12px;
    border-radius: var(--radius);
    background: var(--surface-2);
    border: 1px solid var(--border);
    color: var(--text-2);
  }
  .cmd code {
    flex: 1;
    min-width: 0;
    overflow-x: auto;
    white-space: nowrap;
    color: var(--text-1);
    user-select: all;
  }
  .code-row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }
  .code {
    font-size: 22px;
    font-weight: 600;
    letter-spacing: 0.08em;
    user-select: all;
  }
  .sas {
    align-self: center;
    padding: 10px 22px;
    font-size: 34px;
    font-weight: 650;
    letter-spacing: 0.12em;
    border-radius: var(--radius-lg);
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .note {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    font-size: var(--fs-xs);
  }
  .note :global(svg) {
    flex: none;
    margin-top: 2px;
  }
</style>
