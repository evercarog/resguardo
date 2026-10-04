<script lang="ts">
  // Ajustes → Este equipo → Resguardo Web: vincular, enviar ahora y
  // desvincular. Lo usa el agente del sistema: pide administrador.
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { CircleAlert, CircleCheck, Globe, Link2, LoaderCircle, RefreshCw, ShieldCheck, Unlink, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { WebInfo } from "$lib/api";
  import { toast } from "$lib/toast.svelte";
  import Modal from "./Modal.svelte";
  import RelTime from "./RelTime.svelte";
  import SettingRow from "./SettingRow.svelte";
  import Advanced from "./Advanced.svelte";
  import { web } from "$lib/web.svelte";
  import { relaunchToSettings } from "$lib/settingsNav.svelte";

  const info = $derived(web.info);
  let error = $state("");
  let busy = $state(false);

  // Diálogo de vinculación
  let open = $state(false);
  let code = $state("");
  let name = $state("");
  let url = $state("");
  let key = $state("");
  let formError = $state("");
  let confirmUnpair = $state(false);

  onMount(async () => {
    try {
      web.info = await api.webInfo();
    } catch (e) {
      error = String(e);
    }
  });

  function startPair() {
    if (!info) return;
    code = "";
    name = info.default_name;
    url = info.link?.url ?? info.default_url;
    key = info.link?.key ?? info.default_key;
    formError = "";
    open = true;
  }

  /** Formato ABCD-EFGH mientras se escribe. */
  function onCode(e: Event) {
    const raw = (e.currentTarget as HTMLInputElement).value.toUpperCase().replace(/[^A-Z0-9]/g, "").slice(0, 8);
    code = raw.length > 4 ? `${raw.slice(0, 4)}-${raw.slice(4)}` : raw;
  }

  async function pair(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    formError = "";
    try {
      web.info = await api.webPair(url, key, code, name);
      open = false;
      toast(`Equipo vinculado con la web como «${web.info.link?.device_name ?? name}»`);
    } catch (err) {
      formError = String(err);
    } finally {
      busy = false;
    }
  }

  async function run(action: () => Promise<WebInfo>) {
    busy = true;
    error = "";
    try {
      web.info = await action();
      return true;
    } catch (e) {
      error = String(e);
      return false;
    } finally {
      busy = false;
    }
  }

  async function unpair() {
    confirmUnpair = false;
    if (await run(api.webUnpair)) toast("Equipo desconectado de la web", "info");
  }

  const elevate = () => relaunchToSettings("equipo").catch((e) => (error = String(e)));
  const link = $derived(info?.link ?? null);
</script>

<SettingRow
  label="Resguardo Web"
  description="Ve el estado de las copias de este equipo desde el navegador o el móvil. Se envían fechas, tamaños y resultados; nunca contraseñas, rutas ni nombres de archivos."
  needs={["admin"]}
>
  {#if link && !link.revoked}
    <button
      class="btn btn-ghost btn-sm"
      onclick={() => run(api.webReportNow)}
      disabled={busy || !info?.elevated}
      title={info?.elevated ? "Enviar el estado ahora" : "Abre Resguardo como administrador para enviar ahora"}
    >
      <span class:spin={busy} style="display:grid"><RefreshCw size={14} /></span> Enviar ahora
    </button>
    {#if info?.elevated}
      <button class="btn btn-sm" onclick={() => (confirmUnpair = true)} disabled={busy}><Unlink size={14} /> Desvincular</button>
    {:else}
      <button class="btn btn-sm" onclick={elevate}><ShieldCheck size={14} /> Abrir como administrador</button>
    {/if}
  {:else if info}
    <button class="btn btn-sm" onclick={startPair}><Link2 size={14} /> {link ? "Volver a vincular" : "Vincular"}</button>
  {/if}
  {#snippet status()}
    {#if !info}
      Consultando…
    {:else if link?.revoked}
      <span class="warn-text">La web desvinculó este equipo.</span>
    {:else if link}
      <span class="linked"><Globe size={13} /> Vinculado como <strong>{link.device_name}</strong></span>
      {#if info.state.last_report}· último informe <RelTime iso={info.state.last_report} />{:else}· esperando el primer informe{/if}
    {:else}
      No vinculado.
    {/if}
  {/snippet}
  {#snippet below()}
    {#if info?.state.last_error && link && !link.revoked}
      <div class="notice notice-warn" transition:slide={{ duration: dur(150) }}><CircleAlert size={16} /><p>Último envío fallido: {info.state.last_error}</p></div>
    {/if}
    {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}
  {/snippet}
</SettingRow>

{#if confirmUnpair}
  <Modal onclose={() => (confirmUnpair = false)} labelledby="unpair-title" width={420}>
    <h2 id="unpair-title">¿Desconectar de la web?</h2>
    <p class="muted">Este equipo dejará de enviar su estado a Resguardo Web. Las copias no se tocan.</p>
    <footer>
      <button class="btn btn-ghost" onclick={() => (confirmUnpair = false)}>Cancelar</button>
      <button class="btn btn-danger" onclick={unpair}><Unlink size={15} /> Desconectar</button>
    </footer>
  </Modal>
{/if}

{#if open && info}
  <Modal onclose={() => !busy && (open = false)} labelledby="pair-title" width={460} dismissible={false}>
    <header class="dlg-head">
      <div class="dlg-title">
        <span class="card-icon on"><Globe size={18} /></span>
        <h2 id="pair-title">Conectar con la web</h2>
      </div>
      <button class="icon-btn" onclick={() => (open = false)} disabled={busy} title="Cerrar" aria-label="Cerrar"><X size={17} /></button>
    </header>

    {#if !info.elevated}
      <p class="muted">
        Para vincular el equipo hace falta abrir Resguardo como administrador: el vínculo lo usa el agente del sistema, que envía el
        estado aunque la app esté cerrada.
      </p>
      <footer>
        <button class="btn btn-ghost" onclick={() => (open = false)}>Cancelar</button>
        <button class="btn btn-primary" onclick={elevate}><ShieldCheck size={15} /> Abrir como administrador</button>
      </footer>
    {:else}
      <form onsubmit={pair}>
        <p class="muted small">En tu web, ve a <strong>Vincular</strong>, genera un código y escríbelo aquí. Dura 15 minutos.</p>
        <input
          class="input code mono"
          value={code}
          oninput={onCode}
          placeholder="ABCD-EFGH"
          autocomplete="off"
          spellcheck="false"
          aria-label="Código de vinculación"
          required
        />
        <label class="field">
          <span class="field-label">Nombre de este equipo en la web</span>
          <input class="input" bind:value={name} maxlength="120" required />
        </label>

        <Advanced id="resguardo-web" hint="Servidor de Resguardo Web (el predeterminado sirve para casi todos)" custom={!url || !key || (!!info && (url !== info.default_url || key !== info.default_key))}>
          <label class="field">
            <span class="field-label">Dirección de Supabase</span>
            <input class="input mono" bind:value={url} placeholder="https://tu-proyecto.supabase.co" spellcheck="false" required />
          </label>
          <label class="field">
            <span class="field-label">Clave publicable</span>
            <input class="input mono" bind:value={key} placeholder="sb_publishable_…" spellcheck="false" required />
          </label>
        </Advanced>

        <p class="faint small privacy">
          <ShieldCheck size={13} /> Se envía el estado de los repositorios con copias automáticas o vigilados: fechas, duraciones, tamaños y resultado.
          Nunca contraseñas, rutas ni nombres de archivos.
        </p>

        {#if formError}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{formError}</p></div>{/if}
        <footer>
          <button type="button" class="btn btn-ghost" onclick={() => (open = false)} disabled={busy}>Cancelar</button>
          <button class="btn btn-primary" disabled={busy || code.replace("-", "").length !== 8}>
            {#if busy}<span class="spin" style="display:grid"><LoaderCircle size={15} /></span>{:else}<CircleCheck size={15} />{/if}
            Vincular
          </button>
        </footer>
      </form>
    {/if}
  </Modal>
{/if}

<style>
  .linked {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--ok);
  }
  .warn-text {
    color: var(--warn);
  }
  .linked strong {
    color: var(--text-1);
    font-weight: 600;
  }
  .dlg-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 14px;
  }
  .dlg-title {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  h2 {
    font-size: 17px;
    font-weight: 650;
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .small {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .code {
    height: 56px;
    font-size: 28px;
    font-weight: 650;
    letter-spacing: 0.12em;
    text-align: center;
  }
  .privacy {
    display: flex;
    gap: 6px;
  }
  .privacy :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--ok);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
</style>
