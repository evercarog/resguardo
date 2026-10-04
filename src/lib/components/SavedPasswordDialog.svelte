<script lang="ts">
  // Cambiar la contraseña GUARDADA de un repositorio (no la del repositorio):
  // para cuando su dueño la cambió en otro sitio (p. ej. con restic key passwd).
  // Antes de guardarla se comprueba que abre el repositorio.
  import { CircleAlert, Eye, EyeOff, KeyRound, LoaderCircle, ShieldCheck, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { relaunchToSettings } from "$lib/settingsNav.svelte";
  import { toast } from "$lib/toast.svelte";
  import Modal from "./Modal.svelte";

  interface Props {
    repoId: string;
    repoName: string;
    onclose: () => void;
    onsaved: () => void;
  }
  let { repoId, repoName, onclose, onsaved }: Props = $props();

  let password = $state("");
  let show = $state(false);
  let busy = $state(false);
  let error = $state("");
  let needsAdmin = $state(false);

  async function save(e: SubmitEvent) {
    e.preventDefault();
    if (!password) return;
    busy = true;
    error = "";
    try {
      const r = await api.updateSavedPassword(repoId, password);
      if (r === "agente-admin") {
        needsAdmin = true;
        toast(`Contraseña guardada actualizada en «${repoName}» para la app.`);
      } else {
        toast(r === "agente" ? `Contraseña guardada actualizada en «${repoName}» (también para las copias automáticas).` : `Contraseña guardada actualizada en «${repoName}».`);
        onsaved();
      }
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }
</script>

<Modal {onclose} labelledby="savedpw-title" width={480} dismissible={false}>
  <header class="dlg-head">
    <div class="dlg-title">
      <span class="ticon"><KeyRound size={18} /></span>
      <div>
        <h2 id="savedpw-title">Actualizar la contraseña guardada</h2>
        <p>«{repoName}»</p>
      </div>
    </div>
    <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={onclose}><X size={17} /></button>
  </header>

  {#if needsAdmin}
    <div class="body">
      <div class="notice notice-warn" role="status">
        <CircleAlert size={16} />
        <p>La app ya usa la nueva. Las <strong>copias automáticas</strong> de este repositorio aún tienen la antigua: abre Resguardo como administrador y vuelve a actualizarla aquí.</p>
      </div>
    </div>
    <footer>
      <button class="btn btn-ghost" onclick={onsaved}>Más tarde</button>
      <button class="btn btn-primary" onclick={() => relaunchToSettings("general")}><ShieldCheck size={15} /> Abrir como administrador</button>
    </footer>
  {:else}
    <form onsubmit={save}>
      <div class="body">
        <p class="muted">
          Si cambiaste la contraseña de este repositorio en otro sitio (otro equipo o <code>restic key passwd</code>), escribe aquí la nueva. Resguardo comprueba que lo abre antes de guardarla. No cambia la contraseña del repositorio.
        </p>
        <label class="field">
          <span class="field-label">Contraseña nueva del repositorio</span>
          <span class="pw">
            <!-- svelte-ignore a11y_autofocus -->
            <input class="input" type={show ? "text" : "password"} autocomplete="off" spellcheck="false" bind:value={password} autofocus />
            <button type="button" class="icon-btn" title={show ? "Ocultar" : "Mostrar"} aria-label={show ? "Ocultar la contraseña" : "Mostrar la contraseña"} onclick={() => (show = !show)}>
              {#if show}<EyeOff size={15} />{:else}<Eye size={15} />{/if}
            </button>
          </span>
        </label>
        {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}
      </div>
      <footer>
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
        <button class="btn btn-primary" disabled={busy || !password}>
          {#if busy}<span class="spin"><LoaderCircle size={15} /></span> Comprobando…{:else}Comprobar y guardar{/if}
        </button>
      </footer>
    </form>
  {/if}
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
  .pw {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .pw .input {
    flex: 1;
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
