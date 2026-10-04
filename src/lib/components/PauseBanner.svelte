<script lang="ts">
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { CircleAlert, CirclePause, Play, ShieldCheck } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Repo } from "$lib/api";
  import { agent } from "$lib/agent.svelte";
  import { forgetIntent, pendingEditor, rememberIntent } from "$lib/intent.svelte";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { pauseOf, untilWords } from "$lib/pause.svelte";
  import { toast } from "$lib/toast.svelte";

  // Aviso de «Copias automáticas en pausa» en un destino y en cada una de sus
  // copias, con «Reanudar ahora» (contraseña del destino y administrador).
  let { repo, change }: { repo: Repo; /** Además de reanudar, cambiar la duración. */ change?: () => void } = $props();

  // "Ahora" avanza para que el aviso desaparezca solo al terminar la pausa.
  let now = $state(Date.now());
  onMount(() => {
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });

  const info = $derived(agent.info);
  const pause = $derived(pauseOf(repo.id, now));
  let error = $state("");

  async function resume() {
    error = "";
    if (!info?.elevated) {
      rememberIntent(repo.id, "resume");
      try {
        await api.relaunchAsAdmin();
      } catch (e) {
        forgetIntent();
        error = String(e);
      }
      return;
    }
    const done = await withPassword({
      title: "Reanudar copias automáticas",
      message: `Las copias automáticas de «${repo.name}» vuelven a hacerse según su horario. Si durante la pausa se saltó alguna hora, se hará una sola copia en los próximos minutos.`,
      repoName: repo.name,
      confirmLabel: "Reanudar",
      action: async (password) => {
        agent.info = await api.agentResume(repo.id, password);
      },
    });
    if (done) toast(`Copias automáticas de «${repo.name}» reanudadas`);
  }

  // Al volver de reabrir como administrador para reanudar.
  $effect(() => {
    if (pendingEditor.resume === repo.id && info?.supported) {
      pendingEditor.resume = null;
      if (info.elevated && pause) resume();
    }
  });
</script>

{#if pause}
  <div class="notice paused" role="status" transition:slide={{ duration: dur(180) }}>
    <CirclePause size={16} />
    <p>
      <strong>Copias automáticas en pausa {untilWords(pause, new Date(now))}.</strong>
      Mientras tanto no se hacen copias programadas, verificaciones ni copias externas de «{repo.name}».
      <span class="actions">
        <button class="notice-action" onclick={resume}>
          {#if info?.elevated}<Play size={12} fill="currentColor" /> Reanudar ahora{:else}<ShieldCheck size={12} /> Reanudar ahora (requiere administrador){/if}
        </button>
        {#if change && info?.elevated}<button class="notice-action" onclick={change}>Cambiar la duración</button>{/if}
      </span>
    </p>
  </div>
  {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}
{/if}

<style>
  /* Neutro: una pausa es una decisión, no un aviso ni un error. */
  .paused {
    background: var(--surface-3);
    color: var(--text-2);
    border: 1px solid var(--border);
  }
  .actions {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 4px 12px;
  }
</style>
