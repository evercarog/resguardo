<script lang="ts">
  // Pantalla de bloqueo: Resguardo solo se usa tras pasar Windows Hello.
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { CircleAlert, Fingerprint, LoaderCircle, Lock } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { dur } from "$lib/motion";
  import { markUnlocked } from "$lib/lock.svelte";
  import { toast } from "$lib/toast.svelte";
  import Logo from "./Logo.svelte";

  /** Al abrir la app se pide enseguida; tras un rato sin usarla, se espera a que pulses. */
  let { autoStart = false }: { autoStart?: boolean } = $props();

  let busy = $state(false);
  let message = $state("");

  async function unlock() {
    if (busy) return;
    busy = true;
    message = "";
    try {
      const r = await api.appLockUnlock();
      if (r.result === "verified") markUnlocked();
      else if (r.result === "unavailable") {
        markUnlocked();
        if (r.message) toast(r.message, "info", 9000);
      } else if (r.result === "canceled") message = "Cancelado. Pulsa el botón para intentarlo de nuevo.";
      else message = "Demasiados intentos. Espera un momento y vuelve a intentarlo.";
    } catch (e) {
      message = String(e);
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    if (autoStart) void unlock();
  });
</script>

<main class="lock" in:fade={{ duration: dur(160) }}>
  <div class="box">
    <div class="brand"><Logo size={44} /></div>
    <h1><Lock size={18} /> Resguardo está bloqueado</h1>
    <p class="muted">Para ver o restaurar las copias, confirma que eres tú con Windows Hello (rostro, huella o PIN).</p>
    <button class="btn btn-primary big" onclick={unlock} disabled={busy}>
      {#if busy}<span class="spin" style="display:grid"><LoaderCircle size={16} /></span> Esperando a Windows…{:else}<Fingerprint size={16} /> Desbloquear{/if}
    </button>
    {#if message}
      <p class="msg" role="alert"><CircleAlert size={14} /> {message}</p>
    {/if}
    <p class="faint small">Las copias automáticas siguen funcionando aunque la app esté bloqueada.</p>
  </div>
</main>

<style>
  .lock {
    display: grid;
    place-items: center;
    min-height: 100vh;
    padding: 24px;
    background: var(--bg);
  }
  .box {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    max-width: 420px;
    text-align: center;
  }
  .brand {
    margin-bottom: 4px;
  }
  h1 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: 20px;
  }
  .box > p {
    margin: 0;
    line-height: 1.5;
  }
  .big {
    height: 40px;
    padding: 0 22px;
    font-size: var(--fs-body);
    margin-top: 6px;
  }
  .msg {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--bad);
    font-size: var(--fs-sm);
  }
  .small {
    font-size: var(--fs-sm);
    margin-top: 8px !important;
  }
</style>
