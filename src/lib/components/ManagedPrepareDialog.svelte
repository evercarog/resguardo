<script lang="ts">
  // «Preparar un equipo»: cómo llevar Resguardo Agente a un equipo y
  // emparejarlo. Con el asistente (memoria USB o carpeta compartida) o, para
  // muchos equipos, sin ventanas desde la línea de órdenes.
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { Check, CircleAlert, Copy, Download, LoaderCircle, MonitorDown, Terminal, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { AgentInstallerInfo } from "$lib/api";
  import { toast } from "$lib/toast.svelte";
  import Modal from "./Modal.svelte";

  interface Props {
    onclose: () => void;
    /** Abrir «Emparejar un equipo» (para tener el código). */
    onpair: () => void;
  }
  let { onclose, onpair }: Props = $props();

  let info = $state<AgentInstallerInfo | null>(null);
  let mode = $state<"guiado" | "it">("guiado");
  let busy = $state(false);
  let saved = $state<{ path: string; sha256: string } | null>(null);
  let error = $state("");
  let copied = $state(false);

  onMount(async () => {
    try {
      info = await api.managedAgentInstaller();
    } catch (e) {
      error = String(e);
    }
  });

  const file = $derived(info?.file_name ?? "Resguardo-Agente-setup.exe");
  const silent = $derived(`${file} /S /CODE=CÓDIGO /TRAY=1`);

  async function save() {
    error = "";
    const folder = await open({ directory: true, title: "Dónde guardar el instalador del agente (p. ej. una memoria USB)" });
    if (typeof folder !== "string") return;
    busy = true;
    try {
      saved = await api.managedSaveAgentInstaller(folder);
      toast("Instalador del agente guardado.");
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      setTimeout(() => (copied = false), 1600);
    } catch {
      /* sin portapapeles */
    }
  }
</script>

<Modal {onclose} labelledby="prep-title" width={600}>
  <header class="dlg-head">
    <div class="dlg-title">
      <span class="ticon"><MonitorDown size={18} /></span>
      <div>
        <h2 id="prep-title">Preparar un equipo</h2>
        <p>Instala Resguardo Agente y emparéjalo con esta consola.</p>
      </div>
    </div>
    <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={onclose}><X size={17} /></button>
  </header>

  <div class="body">
    <div class="segmented" role="group" aria-label="Cómo instalarlo">
      <button class:on={mode === "guiado"} aria-pressed={mode === "guiado"} onclick={() => (mode = "guiado")}>Con el asistente</button>
      <button class:on={mode === "it"} aria-pressed={mode === "it"} onclick={() => (mode = "it")}>Sin ventanas (muchos equipos)</button>
    </div>

    {#if info && !info.available}
      <div class="notice notice-warn" role="status">
        <CircleAlert size={16} />
        <p>Esta versión de Resguardo no incluye el instalador del agente. Actualiza Resguardo a una versión que lo incluya.</p>
      </div>
    {/if}

    <ol class="steps">
      <li>
        <strong>Lleva el instalador a ese equipo</strong>
        <span class="faint">En una memoria USB o en una carpeta compartida de la red.</span>
        {#if info?.available}
          <div class="row">
            <button class="btn btn-sm" onclick={save} disabled={busy}>
              {#if busy}<span class="spin"><LoaderCircle size={14} /></span>{:else}<Download size={14} />{/if} Guardar el instalador del agente…
            </button>
            <span class="faint mono small">{file}</span>
          </div>
          {#if saved}
            <p class="saved"><Check size={14} /> Guardado en <span class="mono">{saved.path}</span></p>
            <p class="faint small hash">SHA-256: <span class="mono">{saved.sha256}</span></p>
          {/if}
        {/if}
      </li>
      {#if mode === "guiado"}
        <li>
          <strong>Ábrelo en ese equipo y escribe el código</strong>
          <span class="faint">Pide permiso de administrador. Antes, pulsa aquí <button class="link" onclick={onpair}>Emparejar un equipo</button> para tener el código (vale 15 minutos).</span>
        </li>
        <li>
          <strong>Comprueba el número</strong>
          <span class="faint">El instalador y esta consola muestran el mismo número de 6 cifras: si coincide, pulsa «Coincide: confirmar». Después, asígnale qué copiar.</span>
        </li>
      {:else}
        <li>
          <strong>Instálalo sin ventanas con el código</strong>
          <span class="faint">Como administrador (por ejemplo, desde tu herramienta de despliegue). Cada equipo necesita su propio código: pulsa <button class="link" onclick={onpair}>Emparejar un equipo</button> una vez por equipo.</span>
          <div class="cmd">
            <Terminal size={14} />
            <code class="mono">{silent}</code>
            <button class="icon-btn" title="Copiar la orden" aria-label="Copiar la orden" onclick={() => copy(silent)}>
              {#if copied}<Check size={15} />{:else}<Copy size={15} />{/if}
            </button>
          </div>
          <span class="faint small">/TRAY=0 para no mostrar el icono en la bandeja. Termina con código 2 si el emparejamiento falla.</span>
        </li>
        <li>
          <strong>Confirma en esta consola</strong>
          <span class="faint">Sin pantalla en el equipo, comprueba que el nombre que aparece aquí es el del equipo que instalaste. El número de comprobación queda en «emparejamiento.txt», en la carpeta del agente (solo administradores).</span>
        </li>
      {/if}
    </ol>

    {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}
  </div>

  <footer>
    <button class="btn btn-ghost" onclick={onclose}>Cerrar</button>
    <button class="btn btn-primary" onclick={onpair}>Emparejar un equipo</button>
  </footer>
</Modal>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .segmented {
    align-self: flex-start;
  }
  .steps {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    margin: 0;
    padding-left: 22px;
  }
  .steps li {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .steps li > span {
    font-size: var(--fs-sm);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3);
    margin-top: 6px;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .saved {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 6px 0 0;
    font-size: var(--fs-sm);
    color: var(--ok);
  }
  .saved .mono {
    color: var(--text-1);
    word-break: break-all;
  }
  .hash {
    margin: 0;
    word-break: break-all;
  }
  .cmd {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
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
    font-size: var(--fs-sm);
  }
  .link {
    padding: 0;
    font: inherit;
    color: var(--accent-text);
    background: none;
    border: none;
    cursor: pointer;
    text-decoration: underline;
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
