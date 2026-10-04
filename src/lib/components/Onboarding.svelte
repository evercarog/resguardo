<script lang="ts">
  // Primeros pasos: «Bienvenido a Resguardo» y tres pasos guiados. El progreso
  // sale de lo que ya hay (destinos, copias, copias automáticas), así que se
  // conserva solo entre sesiones; ocultar la tarjeta se recuerda aparte.
  import { fade } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { CalendarClock, Check, FolderSync, HardDrive, KeyRound, Lock, Plus, ShieldCheck, X } from "@lucide/svelte";
  import type { Repo } from "$lib/api";
  import { agent } from "$lib/agent.svelte";
  import { setUi, ui } from "$lib/ui.svelte";
  import Logo from "./Logo.svelte";

  interface Props {
    repos: Repo[];
    /** Pantalla completa (sin destinos) o tarjeta en Estado. */
    variant: "page" | "card";
    onadd: () => void;
    onnewcopy: () => void;
    /** Abrir el destino donde se programan las copias automáticas. */
    onopen: (repo: Repo) => void;
  }
  let { repos, variant, onadd, onnewcopy, onopen }: Props = $props();

  const hasDest = $derived(repos.length > 0);
  const withCopies = $derived(repos.filter((r) => r.plans?.length));
  const hasCopy = $derived(withCopies.length > 0);
  /** El agente solo existe en Windows; sin él, el tercer paso no aplica. */
  const agentSupported = $derived(agent.info?.supported ?? true);
  const hasAuto = $derived(!!agent.info?.repos.some((a) => repos.some((r) => r.id === a.id)));

  const steps = $derived([
    { id: "dest", icon: HardDrive, title: "Añade un repositorio", text: "La caja cifrada donde se guardan las copias, en un disco externo, un servidor o la nube.", done: hasDest },
    { id: "copy", icon: FolderSync, title: "Crea tu primera copia", text: "Qué carpetas proteger. Cada copia guarda versiones en un repositorio.", done: hasCopy },
    ...(agentSupported
      ? [{ id: "auto", icon: CalendarClock, title: "Programa las copias automáticas", text: "Que se hagan solas con su horario, aunque la app esté cerrada.", done: hasAuto }]
      : []),
  ]);
  const doneCount = $derived(steps.filter((s) => s.done).length);
  const next = $derived(steps.find((s) => !s.done) ?? null);
  const finished = $derived(!next);

  function act(id: string) {
    if (id === "dest") onadd();
    else if (id === "copy") onnewcopy();
    else if (withCopies[0]) onopen(withCopies[0]);
  }
  const ACTION: Record<string, string> = { dest: "Añadir repositorio", copy: "Crear la copia", auto: "Programar" };
</script>

{#snippet list()}
  <ol class="steps">
    {#each steps as s, i (s.id)}
      {@const current = next?.id === s.id}
      <li class:done={s.done} class:current>
        <span class="mark" aria-hidden="true">{#if s.done}<Check size={14} strokeWidth={3} />{:else}{i + 1}{/if}</span>
        <span class="text">
          <strong>{s.title}{#if s.done}<span class="sr-only"> (hecho)</span>{/if}</strong>
          <span>{s.text}</span>
        </span>
        {#if current}
          <button class="btn btn-sm btn-primary" onclick={() => act(s.id)}>
            {#if s.id === "dest"}<Plus size={14} />{:else if s.id === "copy"}<Plus size={14} />{:else}<CalendarClock size={14} />{/if}
            {ACTION[s.id]}
          </button>
        {:else if !s.done}
          <span class="later">Después</span>
        {/if}
      </li>
    {/each}
  </ol>
{/snippet}

{#if variant === "page"}
  <div class="page-welcome" in:fade={{ duration: dur(200) }}>
    <Logo size={64} />
    <h1>Bienvenido a Resguardo</h1>
    <p class="lead">Copias cifradas de tus archivos, en un disco, un servidor o la nube. En tres pasos lo tienes listo.</p>
    <div class="card welcome-card">
      <div class="progress-row">
        <span class="faint">Primeros pasos</span>
        <span class="num faint">{doneCount} de {steps.length}</span>
      </div>
      <div class="progress" role="progressbar" aria-label="Primeros pasos" aria-valuenow={doneCount} aria-valuemin={0} aria-valuemax={steps.length}>
        <div style:width="{(doneCount / steps.length) * 100}%"></div>
      </div>
      {@render list()}
    </div>
    <ul class="points">
      <li><ShieldCheck size={16} /><span><strong>Todo cifrado.</strong> restic cifra tus archivos antes de guardarlos.</span></li>
      <li><KeyRound size={16} /><span><strong>Contraseñas a salvo.</strong> En el almacén de credenciales del sistema.</span></li>
      <li><Lock size={16} /><span><strong>Sin telemetría.</strong> Tus datos van solo a donde tú decidas.</span></li>
    </ul>
  </div>
{:else if !finished && !ui.onboardingHidden}
  <section class="card onboard" aria-labelledby="onboard-title" in:fade={{ duration: dur(160) }}>
    <div class="onboard-head">
      <div>
        <h2 id="onboard-title">Primeros pasos</h2>
        <p class="faint">{doneCount} de {steps.length} hechos. Te queda poco para tenerlo todo protegido.</p>
      </div>
      <button class="icon-btn" title="Ocultar los primeros pasos" aria-label="Ocultar los primeros pasos" onclick={() => setUi("onboardingHidden", true)}><X size={16} /></button>
    </div>
    <div class="progress" role="progressbar" aria-label="Primeros pasos" aria-valuenow={doneCount} aria-valuemin={0} aria-valuemax={steps.length}>
      <div style:width="{(doneCount / steps.length) * 100}%"></div>
    </div>
    {@render list()}
  </section>
{/if}

<style>
  .page-welcome {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-3);
    max-width: 600px;
    margin: 6vh auto 0;
    text-align: center;
  }
  h1 {
    margin-top: var(--sp-3);
    font-size: var(--fs-display);
    line-height: var(--lh-display);
    font-weight: 650;
    letter-spacing: -0.022em;
  }
  .lead {
    margin: 0 0 var(--sp-3);
    max-width: 460px;
    font-size: var(--fs-h2);
    line-height: 1.6;
    color: var(--text-2);
    text-wrap: balance;
  }
  .welcome-card {
    width: 100%;
    padding: var(--sp-5);
    text-align: left;
  }
  .progress-row {
    display: flex;
    justify-content: space-between;
    margin-bottom: var(--sp-2);
    font-size: var(--fs-xs);
  }
  .steps {
    list-style: none;
    margin: var(--sp-4) 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
  .steps li {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: 12px 0;
    border-top: 1px solid var(--border);
  }
  .steps li:first-child {
    border-top: none;
  }
  .mark {
    display: grid;
    place-items: center;
    flex: none;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-3);
    box-shadow: inset 0 0 0 1px var(--border-strong);
  }
  li.current .mark {
    color: var(--accent-contrast);
    background: var(--accent);
    box-shadow: none;
  }
  li.done .mark {
    color: var(--ok);
    background: var(--ok-soft);
    box-shadow: none;
  }
  .text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .text strong {
    font-size: var(--fs-body);
    font-weight: 500;
    color: var(--text-1);
  }
  li.done .text strong {
    color: var(--text-2);
    text-decoration: line-through;
    text-decoration-color: var(--border-strong);
  }
  .later {
    flex: none;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .points {
    list-style: none;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-4);
    width: 100%;
    margin: var(--sp-6) 0 0;
    padding: 0;
    text-align: left;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .points li {
    display: flex;
    gap: var(--sp-2);
  }
  .points :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--text-2);
  }
  .points strong {
    color: var(--text-1);
    font-weight: 500;
  }
  .onboard {
    padding: var(--sp-5);
  }
  .onboard-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--sp-3);
    margin-bottom: var(--sp-3);
  }
  .onboard h2 {
    font-size: var(--fs-h2);
    line-height: var(--lh-h2);
  }
  .onboard-head p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  @media (max-width: 760px) {
    .points {
      grid-template-columns: 1fr;
    }
  }
</style>
