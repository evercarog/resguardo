<script lang="ts">
  import { onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { Check, CircleAlert, Copy, CopyPlus, Info, LoaderCircle, Square, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { CloneProgress, Repo } from "$lib/api";
  import { generatePassword, siblingLocation } from "$lib/placeRepos";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { repoKind } from "$lib/repoKind";
  import { toast } from "$lib/toast.svelte";
  import Modal from "./Modal.svelte";

  // «Clonar a otro destino»: un repositorio nuevo con todas las versiones de
  // este (restic copy), en el destino que elijas. Tiene su propia contraseña;
  // el de origen no cambia.
  interface Props {
    repo: Repo;
    repos: Repo[];
    onclose: () => void;
    oncreated: (repo: Repo) => void;
  }
  let { repo, repos, onclose, oncreated }: Props = $props();

  /** Destinos posibles: cada uno con un repositorio suyo para sus credenciales. */
  const places = $derived.by(() => {
    const out: { id: string; name: string; template: Repo }[] = [];
    for (const r of repos) {
      const id = r.place_id ?? `repo:${r.id}`;
      if (!out.some((p) => p.id === id)) out.push({ id, name: r.place_name ?? repoKind(r.location).label, template: r });
    }
    return out;
  });
  // svelte-ignore state_referenced_locally
  let placeId = $state(places.find((p) => p.id !== repo.place_id)?.id ?? places[0]?.id ?? "");
  const place = $derived(places.find((p) => p.id === placeId) ?? null);
  // svelte-ignore state_referenced_locally
  let name = $state(`${repo.name} (clon)`);
  let location = $state("");
  let touched = $state(false);
  $effect(() => {
    if (place && !touched) location = siblingLocation(place.template, name || "clon");
  });
  let password = $state(generatePassword());
  let saved = $state(false);
  let copied = $state(false);
  let running = $state(false);
  let progress = $state<CloneProgress | null>(null);
  let error = $state("");

  let unlisten: (() => void) | null = null;
  onDestroy(() => unlisten?.());

  async function start(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    if (!place || !name.trim() || !location.trim()) return;
    if (!saved) {
      error = "Guarda antes la contraseña del repositorio nuevo.";
      return;
    }
    const source = await withPassword({
      title: `Clonar «${repo.name}»`,
      message: `Se copiarán todas las versiones de «${repo.name}» a un repositorio nuevo en «${place.name}». Confirma con la contraseña de «${repo.name}».`,
      repoName: repo.name,
      confirmLabel: "Clonar",
      action: (pw) => api.checkPassword(repo.id, pw),
    });
    if (!source) return;
    running = true;
    progress = null;
    unlisten = await listen<CloneProgress>(api.CLONE_PROGRESS_EVENT, ({ payload }) => {
      if (payload.from === repo.id) progress = payload;
    });
    try {
      const created = await api.cloneRepo(repo.id, place.template.id, location.trim(), name.trim(), password, source);
      toast(`«${repo.name}» clonado en «${place.name}» como «${created.name}»`);
      oncreated(created);
    } catch (err) {
      error = String(err);
    } finally {
      running = false;
      unlisten?.();
      unlisten = null;
    }
  }

  async function copyPassword() {
    try {
      await navigator.clipboard.writeText(password);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      /* sin portapapeles */
    }
  }

  const percent = $derived(progress && progress.total ? Math.min(100, (progress.done / progress.total) * 100) : 0);
</script>

<Modal onclose={() => !running && onclose()} labelledby="clone-title" width={580} dismissible={false}>
  <header class="dlg-head">
    <div class="dlg-title">
      <span class="ticon"><CopyPlus size={19} /></span>
      <div>
        <h2 id="clone-title">Clonar a otro destino</h2>
        <p class="faint">Todas las versiones de «{repo.name}», en un repositorio nuevo con su propia contraseña</p>
      </div>
    </div>
    <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={onclose} disabled={running}><X size={17} /></button>
  </header>

  <form class="body" onsubmit={start}>
    <label class="field">
      <span class="field-label">Destino</span>
      <select class="input" bind:value={placeId} onchange={() => (touched = false)} disabled={running}>
        {#each places as p (p.id)}<option value={p.id}>{p.name}{p.id === repo.place_id ? " (el mismo)" : ""}</option>{/each}
      </select>
    </label>
    <label class="field">
      <span class="field-label">Nombre del repositorio nuevo</span>
      <input class="input" bind:value={name} maxlength="80" required disabled={running} />
    </label>
    <label class="field">
      <span class="field-label">Ubicación</span>
      <input class="input mono" bind:value={location} oninput={() => (touched = true)} spellcheck="false" required disabled={running} />
    </label>
    <label class="field">
      <span class="field-label">Contraseña del repositorio nuevo</span>
      <span class="pw">
        <input class="input mono" value={password} readonly />
        <button type="button" class="icon-btn" title="Copiar" aria-label="Copiar la contraseña" onclick={copyPassword}>
          {#if copied}<Check size={15} />{:else}<Copy size={15} />{/if}
        </button>
      </span>
    </label>
    <label class="check"><input type="checkbox" bind:checked={saved} disabled={running} /> La he copiado o la guardaré en el kit de recuperación</label>

    <p class="faint icon-note">
      <Info size={14} />
      <span>
        El nuevo se crea con los mismos parámetros de troceado que «{repo.name}», así que entre los dos se aprovecha la deduplicación. Puede tardar
        según el tamaño y la conexión. Si los dos están en nubes con claves distintas, clona primero a un disco o a un servidor.
      </span>
    </p>

    {#if running}
      <div class="progress" role="status" aria-live="polite">
        <span class="stage"><span class="spin" style="display:grid"><LoaderCircle size={14} /></span> {progress?.stage ?? "Preparando…"}</span>
        {#if progress?.total}<span class="faint">{progress.done} de {progress.total} versiones</span>{/if}
        <div class="track"><div class="fill" style:width="{percent}%"></div></div>
      </div>
    {/if}
    {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}

    <footer>
      {#if running}
        <button type="button" class="btn" onclick={() => api.cancelClone(repo.id)}><Square size={11} fill="currentColor" /> Detener</button>
      {:else}
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
        <button class="btn btn-primary" disabled={!place}>Clonar</button>
      {/if}
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
  .progress {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .stage {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 550;
  }
  .track {
    height: 6px;
    border-radius: 999px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.3s;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
