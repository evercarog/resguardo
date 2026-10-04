<script lang="ts">
  import { Check, CircleAlert, Copy, Eye, EyeOff, FolderPlus, Info, LoaderCircle, Package, RefreshCw, TriangleAlert, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Repo } from "$lib/api";
  import { generatePassword, siblingLocation } from "$lib/placeRepos";
  import { toast } from "$lib/toast.svelte";
  import Modal from "./Modal.svelte";
  import InfoTip from "./InfoTip.svelte";

  // Dentro de un destino: «Usar este» (un repositorio que ya existe, con su
  // contraseña) o «Crear uno nuevo aquí» (ruta sugerida y contraseña
  // generada). Las credenciales del destino (claves de la nube, usuario del
  // rest-server) se toman de `template`, otro repositorio del mismo destino.
  interface Props {
    mode: "use" | "new";
    placeName: string;
    template: Repo;
    /** «Usar este»: su ubicación y su ruta dentro del destino. */
    location?: string;
    path?: string;
    /** Nombre del equipo, para sugerir la ruta del nuevo. */
    deviceName?: string;
    onclose: () => void;
    oncreated: (repo: Repo, generatedPassword: boolean) => void;
  }
  let { mode, placeName, template, location: given = "", path = "", deviceName = "", onclose, oncreated }: Props = $props();

  // svelte-ignore state_referenced_locally
  let name = $state(mode === "use" ? (path && path !== "." ? path.split(/[\\/]/).pop()! : placeName) : deviceName || "Este equipo");
  // svelte-ignore state_referenced_locally
  let location = $state(mode === "use" ? given : siblingLocation(template, deviceName || "este-equipo"));
  let locationTouched = $state(false);
  // svelte-ignore state_referenced_locally
  let password = $state(mode === "new" ? generatePassword() : "");
  // svelte-ignore state_referenced_locally
  let show = $state(mode === "new");
  let saved = $state(false);
  let busy = $state(false);
  let error = $state("");
  let copied = $state(false);

  // Al cambiar el nombre, la ruta sugerida lo sigue (hasta que se edita a mano).
  $effect(() => {
    if (mode === "new" && !locationTouched) location = siblingLocation(template, name || "repositorio");
  });

  async function copyPassword() {
    try {
      await navigator.clipboard.writeText(password);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      /* sin portapapeles */
    }
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (busy) return;
    error = "";
    if (!name.trim() || !location.trim() || !password) {
      error = "Completa el nombre, la ubicación y la contraseña.";
      return;
    }
    if (mode === "new" && !saved) {
      error = "Guarda antes la contraseña: sin ella nadie podrá abrir este repositorio.";
      return;
    }
    busy = true;
    try {
      const repo = await api.addRepo({
        name: name.trim(),
        location: location.trim(),
        password,
        create: mode === "new",
        cacert: template.cacert ?? null,
        cloudFrom: template.cloud_key_id ? template.id : null,
        restFrom: template.rest_username ? template.id : null,
      });
      toast(mode === "new" ? `Repositorio «${repo.name}» creado en «${placeName}»` : `Repositorio «${repo.name}» añadido`);
      oncreated(repo, mode === "new");
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }
</script>

<Modal {onclose} labelledby="pr-title" width={560} dismissible={false}>
  <header class="dlg-head">
    <div class="dlg-title">
      <span class="ticon">{#if mode === "new"}<FolderPlus size={19} />{:else}<Package size={19} />{/if}</span>
      <div>
        <h2 id="pr-title">{mode === "new" ? "Crear un repositorio nuevo" : "Usar este repositorio"} <InfoTip id="repositorio" /></h2>
        <p class="faint">En el destino «{placeName}»</p>
      </div>
    </div>
    <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={onclose}><X size={17} /></button>
  </header>

  <form class="body" onsubmit={submit}>
    <label class="field">
      <span class="field-label">Nombre en Resguardo</span>
      <input class="input" bind:value={name} maxlength="80" required />
    </label>

    <label class="field">
      <span class="field-label">Ubicación</span>
      <input class="input mono" bind:value={location} oninput={() => (locationTouched = true)} spellcheck="false" readonly={mode === "use"} required />
      {#if mode === "new"}<span class="field-hint">Junto a los demás repositorios del destino. Puedes cambiarla.</span>{/if}
    </label>

    <label class="field">
      <span class="field-label">{mode === "new" ? "Contraseña del repositorio nuevo" : "Contraseña de este repositorio"}</span>
      <span class="pw">
        <input class="input mono" type={show ? "text" : "password"} bind:value={password} autocomplete="new-password" spellcheck="false" readonly={mode === "new"} required />
        <button type="button" class="icon-btn" title={show ? "Ocultar" : "Mostrar"} aria-label={show ? "Ocultar la contraseña" : "Mostrar la contraseña"} onclick={() => (show = !show)}>
          {#if show}<EyeOff size={15} />{:else}<Eye size={15} />{/if}
        </button>
        {#if mode === "new"}
          <button type="button" class="icon-btn" title="Copiar" aria-label="Copiar la contraseña" onclick={copyPassword}>
            {#if copied}<Check size={15} />{:else}<Copy size={15} />{/if}
          </button>
          <button type="button" class="icon-btn" title="Generar otra" aria-label="Generar otra contraseña" onclick={() => ((password = generatePassword()), (saved = false))}>
            <RefreshCw size={15} />
          </button>
        {/if}
      </span>
    </label>

    {#if mode === "new"}
      <div class="notice notice-info">
        <Info size={16} />
        <p>
          Resguardo ha generado una contraseña fuerte. Se guarda cifrada en este equipo y en el kit de recuperación, que se abre al terminar. Ningún
          otro equipo la conoce: este repositorio es solo de este equipo.
        </p>
      </div>
      <label class="check"><input type="checkbox" bind:checked={saved} /> La he copiado o la guardaré en el kit de recuperación</label>
    {:else}
      <div class="notice notice-warn">
        <TriangleAlert size={16} />
        <p>
          Si otro equipo también copia en este repositorio, los dos escribís en el mismo sitio y quien tenga la contraseña ve todas las versiones.
          Lo normal es usarlo para <strong>restaurar o verificar</strong> desde aquí; para copiar, mejor crea uno nuevo.
        </p>
      </div>
    {/if}

    {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}

    <footer>
      <button type="button" class="btn btn-ghost" onclick={onclose} disabled={busy}>Cancelar</button>
      <button class="btn btn-primary" disabled={busy}>
        {#if busy}<span class="spin" style="display:grid"><LoaderCircle size={14} /></span>{/if}
        {mode === "new" ? "Crear repositorio" : "Comprobar y añadir"}
      </button>
    </footer>
  </form>
</Modal>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .pw {
    display: flex;
    align-items: center;
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
