<script lang="ts">
  import { ArrowRightLeft, Check, Info, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Plan, Repo } from "$lib/api";
  import { agent, refreshAgent } from "$lib/agent.svelte";
  import { MAX_PLANS } from "$lib/plans";
  import { repoKind } from "$lib/repoKind";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { toast } from "$lib/toast.svelte";
  import Modal from "./Modal.svelte";

  // «Cambiar destino»: mueve una copia (plan) a otro destino. Pide la
  // contraseña de los dos destinos, porque cambia la configuración de ambos.
  interface Props {
    repo: Repo;
    plan: Plan;
    repos: Repo[];
    onclose: () => void;
    /** Destinos ya guardados ([origen, destino]) y la copia en su nuevo sitio. */
    onmoved: (updated: Repo[], to: Repo, plan: Plan) => void;
    /** Añadir un destino (si no hay otro al que moverla). */
    onadddestination?: () => void;
  }
  let { repo, plan, repos, onclose, onmoved, onadddestination }: Props = $props();

  const others = $derived(repos.filter((r) => r.id !== repo.id));
  let target = $state<string | null>(null);
  let busy = $state(false);

  const chosen = $derived(others.find((r) => r.id === target) ?? null);

  async function move() {
    const to = chosen;
    if (!to) return;
    // Datos de la copia antes de moverla (después puede dejar de existir aquí).
    const { id: planId, name: planName, schedule: planSchedule } = plan;
    busy = true;
    // Primero la contraseña del destino actual (solo se comprueba)…
    const pwFrom = await withPassword({
      title: "Cambiar repositorio (1 de 2)",
      message: `Para sacar «${planName}» de «${repo.name}», confirma con la contraseña de ese repositorio.`,
      repoName: repo.name,
      confirmLabel: "Continuar",
      action: (password) => api.checkPassword(repo.id, password),
    });
    if (!pwFrom) {
      busy = false;
      return;
    }
    // …y después la del destino nuevo, que es cuando se mueve de verdad.
    let result: Repo[] | null = null;
    const pwTo = await withPassword({
      title: "Cambiar repositorio (2 de 2)",
      message: `Para guardar «${planName}» en «${to.name}», confirma con la contraseña de ese repositorio.`,
      repoName: to.name,
      confirmLabel: "Mover copia",
      action: async (password) => {
        result = await api.moveJob(repo.id, planId, to.id, pwFrom, password);
      },
    });
    busy = false;
    if (!pwTo || !result) return;
    const updated = result as Repo[];
    const dest = updated.find((r) => r.id === to.id) ?? updated[1];
    // El backend siempre la añade al final (puede que con otro id o nombre si ya existían).
    const moved = dest.plans[dest.plans.length - 1];
    toast(`«${moved.name}» se guarda ahora en «${dest.name}»`);
    // Como administrador el backend ya actualizó el agente; si no, queda pendiente.
    const info = agent.info;
    const affected = !!planSchedule || !!info?.repos.some((r) => r.id === repo.id && r.schedule.kind === "plans");
    if (info?.supported && affected) {
      if (info.elevated) refreshAgent();
      else
        toast(
          "Las copias automáticas aún no reflejan el cambio: aplícalo en «Copias automáticas» de cada repositorio como administrador.",
          "info",
          9000,
        );
    }
    onmoved(updated, dest, moved);
  }
</script>

<Modal {onclose} labelledby="move-title" width={560}>
  <header class="dlg-head">
    <div class="dlg-title">
      <span class="ticon"><ArrowRightLeft size={19} /></span>
      <div>
        <h2 id="move-title">Cambiar repositorio</h2>
        <p class="faint">«{plan.name}» se guarda en «{repo.name}»</p>
      </div>
    </div>
    <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={onclose}><X size={17} /></button>
  </header>

  {#if others.length === 0}
    <div class="notice notice-info">
      <Info size={16} />
      <p>No tienes otro repositorio al que moverla. Añade uno (otro disco, un servidor o la nube) y vuelve aquí.</p>
    </div>
    <footer>
      <button class="btn btn-ghost" onclick={onclose}>Cancelar</button>
      {#if onadddestination}
        <button class="btn btn-primary" onclick={() => (onclose(), onadddestination())}>Añadir repositorio</button>
      {/if}
    </footer>
  {:else}
    <p class="lead">¿Dónde quieres guardar esta copia a partir de ahora?</p>
    <div class="dests" role="radiogroup" aria-label="Repositorio nuevo">
      {#each others as r (r.id)}
        {@const k = repoKind(r.location)}
        {@const full = r.plans.length >= MAX_PLANS}
        <button
          class="dest"
          class:on={target === r.id}
          role="radio"
          aria-checked={target === r.id}
          disabled={full}
          title={full ? `Este repositorio ya tiene el máximo de ${MAX_PLANS} copias` : r.name}
          onclick={() => (target = r.id)}
        >
          <span class="dicon"><k.icon size={17} /></span>
          <span class="dtext">
            <strong>{r.name}</strong>
            <span class="faint">{k.label} · {full ? "lleno" : `${r.plans.length} ${r.plans.length === 1 ? "copia" : "copias"}`}</span>
          </span>
          {#if target === r.id}<span class="check"><Check size={15} /></span>{/if}
        </button>
      {/each}
    </div>

    <div class="notice notice-info">
      <Info size={16} />
      <p>
        La primera copia en {chosen ? `«${chosen.name}»` : "el repositorio nuevo"} será completa, así que puede tardar más de lo normal.
        Las versiones anteriores se quedan en «{repo.name}» y puedes seguir restaurándolas desde ese repositorio.
      </p>
    </div>
    {#if plan.schedule}
      <p class="faint small">
        Tiene horario: si Resguardo está abierto como administrador, las copias automáticas se actualizan solas. Si no, verás un aviso para aplicarlas.
      </p>
    {/if}

    <footer>
      <button class="btn btn-ghost" onclick={onclose}>Cancelar</button>
      <button class="btn btn-primary" onclick={move} disabled={!chosen || busy} title={chosen ? `Mover a «${chosen.name}»` : "Elige antes un repositorio"}>
        <ArrowRightLeft size={14} /> Mover copia
      </button>
    </footer>
  {/if}
</Modal>

<style>
  .lead {
    margin: 0 0 10px;
    font-size: var(--fs-sm);
    font-weight: 550;
  }
  .dests {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 14px;
  }
  .dest {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    font: inherit;
    text-align: left;
    color: var(--text-1);
    background: var(--surface);
    border: 1.5px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    transition:
      border-color 0.15s,
      background 0.15s;
  }
  .dest:hover:not(:disabled) {
    border-color: var(--border-strong);
  }
  .dest.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .dest:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }
  .dicon {
    display: grid;
    place-items: center;
    flex: none;
    width: 34px;
    height: 34px;
    border-radius: var(--radius);
    color: var(--text-2);
    background: var(--surface-3);
  }
  .dest.on .dicon {
    color: var(--accent-contrast);
    background: var(--accent);
  }
  .dtext {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .dtext strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dtext .faint {
    font-size: var(--fs-xs);
  }
  .check {
    display: grid;
    color: var(--accent-text);
  }
  .small {
    margin: 10px 0 0;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }
</style>
