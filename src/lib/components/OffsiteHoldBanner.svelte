<script lang="ts">
  import { slide } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { CircleAlert, FileSearch, Pause, Play, ShieldAlert, ShieldCheck } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Repo } from "$lib/api";
  import { agent } from "$lib/agent.svelte";
  import { formatBytes, formatNumber, formatTime } from "$lib/format";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { pauseOf } from "$lib/pause.svelte";
  import { toast } from "$lib/toast.svelte";
  import HelpLink from "./HelpLink.svelte";
  import PauseDialog from "./PauseDialog.svelte";

  // Aviso de «cambio inusual»: una copia de este destino cambió mucho más de lo
  // normal (¿ransomware?) y el agente frenó la subida a la nube. Se muestra en
  // el destino y en Estado.
  interface Props {
    repo: Repo;
    /** Abrir las diferencias de esa versión con la anterior (si se puede desde aquí). */
    onviewchanges?: (snapshotId: string) => void;
    /** Abrir el destino (en Estado, en lugar de ver los cambios aquí). */
    onopen?: () => void;
  }
  let { repo, onviewchanges, onopen }: Props = $props();

  const info = $derived(agent.info);
  const hold = $derived(info?.offsite_holds?.[repo.id] ?? null);
  const paused = $derived(pauseOf(repo.id));
  let pausing = $state(false);
  let error = $state("");

  const text = $derived.by(() => {
    if (!hold) return "";
    const when = hold.backup_finished ? ` de las ${formatTime(hold.backup_finished)}` : "";
    const plan = hold.plan_name ? ` «${hold.plan_name}»` : "";
    const normal =
      hold.typical_bytes != null && hold.typical_files != null
        ? `lo normal: ~${formatBytes(hold.typical_bytes)} y ~${formatNumber(hold.typical_files)}`
        : "aún no hay copias suficientes para saber lo normal";
    return `la copia${plan}${when} añadió ${formatBytes(hold.data_added)} y ${formatNumber(hold.files)} archivos (${normal}).`;
  });

  async function resume() {
    error = "";
    if (!info?.elevated) {
      try {
        await api.relaunchAsAdmin();
      } catch (e) {
        error = String(e);
      }
      return;
    }
    const done = await withPassword({
      title: "Reanudar la subida a la nube",
      message: `Confirmas que el cambio en «${repo.name}» es normal (por ejemplo, una carpeta grande nueva). El agente volverá a subir las versiones según su horario.`,
      repoName: repo.name,
      confirmLabel: "Es normal, reanudar",
      action: async (password) => {
        agent.info = await api.agentOffsiteResume(repo.id, password);
      },
    });
    if (done) toast(`Subida a la nube de «${repo.name}» reanudada`);
  }
</script>

{#if hold}
  <div class="notice hold" role="alert" transition:slide={{ duration: dur(180) }}>
    <ShieldAlert size={18} />
    <div class="body">
      <p>
        <strong>Cambio inusual en «{repo.name}»:</strong>
        {text} La subida a la nube está frenada por precaución.
        <HelpLink topic="mant-cambio-inusual" label="qué hacer ante un cambio inusual" />
      </p>
      <div class="actions">
        {#if onviewchanges}
          <button class="btn btn-sm" onclick={() => onviewchanges(hold.snapshot_id)}><FileSearch size={14} /> Ver qué cambió</button>
        {:else if onopen}
          <button class="btn btn-sm" onclick={onopen}><FileSearch size={14} /> Abrir el repositorio</button>
        {/if}
        <button class="btn btn-sm" onclick={resume}>
          {#if info?.elevated}<Play size={13} /> Es normal, reanudar la subida{:else}<ShieldCheck size={14} /> Es normal, reanudar (requiere administrador){/if}
        </button>
        {#if !paused && info?.repos.some((r) => r.id === repo.id)}
          <button class="btn btn-sm btn-ghost" onclick={() => (pausing = true)}><Pause size={13} /> Pausar también las copias automáticas</button>
        {/if}
      </div>
      {#if error}<p class="err"><CircleAlert size={14} /> {error}</p>{/if}
    </div>
  </div>
{/if}

{#if pausing}<PauseDialog {repo} onclose={() => (pausing = false)} />{/if}

<style>
  .hold {
    background: var(--bad-soft);
    color: var(--bad);
    border: 1px solid color-mix(in srgb, var(--bad) 35%, transparent);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  .body p {
    margin: 0;
    color: var(--text-1);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .err {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--bad) !important;
    font-size: var(--fs-sm);
  }
</style>
