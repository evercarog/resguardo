<script lang="ts">
  import { CircleAlert, CircleCheck, TriangleAlert } from "@lucide/svelte";
  import type { AgentRun } from "$lib/api";
  import { formatBytes, formatDate, formatRelative } from "$lib/format";
  import { agent } from "$lib/agent.svelte";
  import FailedFilesDialog from "./FailedFilesDialog.svelte";
  import RepoErrorActions from "./RepoErrorActions.svelte";

  // Resultado de una ejecución del agente (copia, verificación o subida):
  // icono, cuándo terminó, datos añadidos y, si hizo falta, el mensaje.
  interface Props {
    run: AgentRun;
    /** Mostrar siempre el mensaje (en verificaciones y subidas explica qué se hizo). */
    alwaysMessage?: boolean;
    /** Copia automática de este plan: si falló algo, «Ver qué archivos» (solo administradores). */
    planRef?: { repoId: string; planId: string; name: string };
    /** Su repositorio: si falló por un bloqueo antiguo o la contraseña guardada, cómo arreglarlo. */
    repo?: { id: string; name: string };
  }
  let { run, alwaysMessage = false, planRef, repo }: Props = $props();
  const canSeeFiles = $derived(!!planRef && !!agent.info?.elevated && run.result !== "ok");
  let showFiles = $state(false);
</script>

<span class="result r-{run.result}">
  <strong title={formatDate(run.finished)}>
    <span class="icon">
      {#if run.result === "ok"}<CircleCheck size={14} />{:else if run.result === "warning"}<TriangleAlert size={14} />{:else}<CircleAlert size={14} />{/if}
    </span>
    {formatRelative(run.finished)}
  </strong>
  {#if run.unchanged}
    <span class="faint added" title="No había cambios: no hizo falta guardar una versión nueva">sin cambios</span>
  {:else if run.data_added != null}<span class="faint added">+{formatBytes(run.data_added)}</span>{/if}
</span>
{#if run.message && (alwaysMessage || run.result !== "ok")}<span class="msg">{run.message}</span>{/if}
{#if repo && run.result === "error"}<RepoErrorActions repoId={repo.id} repoName={repo.name} message={run.message} />{/if}
{#if canSeeFiles}
  <button class="link files" onclick={() => (showFiles = true)} title="Las rutas completas, del registro detallado del agente (solo administradores)">Ver qué archivos</button>
{/if}
{#if showFiles && planRef}
  <FailedFilesDialog repoId={planRef.repoId} planId={planRef.planId} name={planRef.name} onclose={() => (showFiles = false)} />
{/if}

<style>
  .result {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    column-gap: 6px;
  }
  strong {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--ok);
  }
  .r-warning strong {
    color: var(--warn);
  }
  .r-error strong {
    color: var(--bad);
  }
  .icon {
    display: grid;
    flex: none;
  }
  .added {
    font-size: var(--fs-sm);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .files {
    font-size: var(--fs-xs);
    justify-self: start;
  }
  .msg {
    font-size: var(--fs-xs);
    color: var(--text-2);
    overflow-wrap: anywhere;
  }
</style>
