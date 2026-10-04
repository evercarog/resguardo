<script lang="ts">
  import { CircleAlert, Eye, EyeOff, KeyRound, LoaderCircle, LockKeyhole, TriangleAlert } from "@lucide/svelte";
  import { passwordPrompt } from "$lib/passwordPrompt.svelte";
  import Modal from "./Modal.svelte";

  let password = $state("");
  let show = $state(false);
  let busy = $state(false);
  let error = $state("");
  let shake = $state(false);

  const req = $derived(passwordPrompt.current);

  function finish(result: string | null) {
    const current = passwordPrompt.current;
    passwordPrompt.current = null;
    password = "";
    show = false;
    error = "";
    current?.resolve(result);
  }

  function close() {
    if (!busy) finish(null);
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!req || busy || !password) return;
    busy = true;
    error = "";
    try {
      await req.action(password);
      finish(password);
    } catch (e) {
      error = String(e);
      shake = false;
      requestAnimationFrame(() => (shake = true));
    } finally {
      busy = false;
    }
  }

  function autofocus(node: HTMLInputElement) {
    node.focus();
  }
</script>

{#if req}
  <Modal onclose={close} labelledby="pw-title" width={440} dismissible={false}>
    <form onsubmit={submit} class:shake onanimationend={() => (shake = false)}>
      <div class="icon" class:danger={req.danger}>
        {#if req.danger}<TriangleAlert size={22} />{:else}<LockKeyhole size={22} />{/if}
      </div>
      <h2 id="pw-title">{req.title}</h2>
      <p class="muted">{req.message}</p>

      <label class="field">
        <span class="field-label">Contraseña del repositorio «{req.repoName}»</span>
        <span class="password">
          <KeyRound size={15} />
          <input
            class="input"
            class:bad={!!error}
            type={show ? "text" : "password"}
            bind:value={password}
            autocomplete="off"
            use:autofocus
            oninput={() => (error = "")}
          />
          <button type="button" class="icon-btn" onclick={() => (show = !show)} title={show ? "Ocultar la contraseña" : "Mostrar la contraseña"} aria-label={show ? "Ocultar la contraseña" : "Mostrar la contraseña"} aria-pressed={show}>
            {#if show}<EyeOff size={15} />{:else}<Eye size={15} />{/if}
          </button>
        </span>
      </label>

      {#if error}
        <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>
      {/if}

      <footer>
        <button type="button" class="btn btn-ghost" onclick={close} disabled={busy}>Cancelar</button>
        <button class="btn {req.danger ? 'btn-danger' : 'btn-primary'}" type="submit" disabled={busy || !password}>
          {#if busy}<span class="spin" style="display:grid"><LoaderCircle size={15} /></span> Comprobando…{:else}{req.confirmLabel}{/if}
        </button>
      </footer>
    </form>
  </Modal>
{/if}

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .icon {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: var(--radius-lg);
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .icon.danger {
    color: var(--bad);
    background: var(--bad-soft);
  }
  h2 {
    font-size: 17px;
    font-weight: 650;
    margin-top: 2px;
  }
  p {
    margin: -6px 0 2px;
    font-size: var(--fs-sm);
    line-height: 1.55;
  }
  .password {
    position: relative;
    display: flex;
    align-items: center;
  }
  .password > :global(svg) {
    position: absolute;
    left: 11px;
    color: var(--text-3);
    pointer-events: none;
  }
  .password .input {
    padding-left: 33px;
    padding-right: 38px;
  }
  .password .icon-btn {
    position: absolute;
    right: 4px;
  }
  .bad {
    border-color: var(--bad);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 2px;
  }
  .shake {
    animation: shake 0.35s ease-in-out;
  }
  @keyframes shake {
    20%,
    60% {
      transform: translateX(-6px);
    }
    40%,
    80% {
      transform: translateX(6px);
    }
  }
</style>
