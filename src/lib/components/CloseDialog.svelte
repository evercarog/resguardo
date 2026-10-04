<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { CalendarClock, CircleAlert, LoaderCircle, Square, TriangleAlert } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { RunningJob } from "$lib/api";
  import Modal from "./Modal.svelte";

  // Al pulsar la X con copias o restauraciones en curso, el backend no cierra
  // la ventana y avisa aquí para que se elija qué hacer.
  let jobs = $state<RunningJob[] | null>(null);
  let busy = $state(false);
  let error = $state("");

  const handOff = $derived(jobs?.filter((j) => j.can_hand_off) ?? []);
  const stayLocal = $derived(jobs?.filter((j) => !j.can_hand_off) ?? []);

  onMount(() => {
    const off = listen("close-requested", async () => {
      error = "";
      jobs = await api.runningJobs();
    });
    return () => void off.then((f) => f());
  });

  async function close(mode: "handoff" | "background" | "stop") {
    busy = true;
    error = "";
    try {
      await api.closeApp(mode);
    } catch (e) {
      error = String(e);
      busy = false;
    }
  }

  const label = (j: RunningJob) => (j.kind === "restore" ? `restauración de «${j.name}»` : `copia de «${j.name}»`);
</script>

{#if jobs}
  <Modal onclose={() => !busy && (jobs = null)} labelledby="close-title" width={480}>
    <div class="body">
      <span class="ic"><TriangleAlert size={22} /></span>
      <h2 id="close-title">Hay {jobs.length === 1 ? "una operación" : "operaciones"} en curso</h2>

      {#if handOff.length}
        <p class="muted">
          El agente puede terminar {handOff.length === 1 ? "la" : "las"}
          {handOff.map(label).join(", ")} aunque cierres Resguardo o la sesión de Windows. La retomará en unos minutos y aprovechará
          lo que ya se subió.
        </p>
      {/if}
      {#if stayLocal.length}
        <p class="muted">
          {stayLocal.length === 1 ? "La" : "Las"}
          {stayLocal.map(label).join(", ")}
          no {stayLocal.length === 1 ? "puede pasar" : "pueden pasar"} al agente: seguirá{stayLocal.length === 1 ? "" : "n"}
          en segundo plano y Resguardo se cerrará al terminar (si cierras la sesión de Windows, se detendrá{stayLocal.length === 1 ? "" : "n"}).
        </p>
      {/if}

      {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}

      <div class="actions">
        <button class="btn btn-primary" onclick={() => close(handOff.length ? "handoff" : "background")} disabled={busy}>
          {#if busy}<span class="spin" style="display:grid"><LoaderCircle size={15} /></span>{:else}<CalendarClock size={15} />{/if}
          {handOff.length ? "Pasar al agente y cerrar" : "Terminar en segundo plano y cerrar"}
        </button>
        <button class="btn" onclick={() => close("stop")} disabled={busy}><Square size={12} fill="currentColor" /> Detener todo y cerrar</button>
        <button class="btn btn-ghost" onclick={() => (jobs = null)} disabled={busy}>Seguir con la app abierta</button>
      </div>
    </div>
  </Modal>
{/if}

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .ic {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: var(--radius-lg);
    color: var(--warn);
    background: var(--warn-soft);
  }
  h2 {
    font-size: 17px;
    font-weight: 650;
  }
  p {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.55;
  }
  .actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 6px;
  }
  .actions .btn {
    height: 38px;
  }
</style>
