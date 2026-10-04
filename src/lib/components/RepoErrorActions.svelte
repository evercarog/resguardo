<script lang="ts">
  // Lo que se puede hacer ante dos errores con arreglo:
  // - un bloqueo antiguo (restic dice que lo dejó una operación que ya no
  //   está en marcha): «Desbloquear» (restic unlock, solo los antiguos);
  // - la contraseña guardada ya no abre el repositorio (se cambió en otro
  //   sitio): «Actualizar la contraseña guardada…».
  import { KeyRound, LoaderCircle, LockOpen } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { toast } from "$lib/toast.svelte";
  import SavedPasswordDialog from "./SavedPasswordDialog.svelte";

  interface Props {
    repoId: string;
    repoName: string;
    message: string | null | undefined;
    /** Tras arreglarlo (para volver a intentar). */
    onfixed?: () => void;
  }
  let { repoId, repoName, message, onfixed }: Props = $props();

  const stale = $derived(!!message?.startsWith(api.STALE_LOCK));
  const wrongPassword = $derived(!!message && /^Contraseña incorrecta\.?$/.test(message.trim()));
  let busy = $state(false);
  let changing = $state(false);

  async function unlock() {
    busy = true;
    try {
      await api.unlockRepo(repoId);
      toast(`«${repoName}» desbloqueado: ya se puede volver a copiar.`);
      onfixed?.();
    } catch (e) {
      toast(String(e), "error", 8000);
    } finally {
      busy = false;
    }
  }
</script>

{#if stale}
  <button class="btn btn-sm" onclick={unlock} disabled={busy} title="Quita solo los bloqueos de operaciones que ya no están en marcha (restic unlock)">
    {#if busy}<span class="spin"><LoaderCircle size={13} /></span>{:else}<LockOpen size={13} />{/if} Desbloquear
  </button>
{:else if wrongPassword}
  <button class="btn btn-sm" onclick={() => (changing = true)} title="Si cambiaste la contraseña del repositorio en otro sitio, guarda aquí la nueva">
    <KeyRound size={13} /> Actualizar la contraseña guardada…
  </button>
{/if}

{#if changing}
  <SavedPasswordDialog
    {repoId}
    {repoName}
    onclose={() => (changing = false)}
    onsaved={() => {
      changing = false;
      onfixed?.();
    }}
  />
{/if}

<style>
  .btn {
    align-self: flex-start;
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
