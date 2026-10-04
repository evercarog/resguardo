<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { fly, scale } from "svelte/transition";
  import { ArrowRight, Check, CircleCheck, PartyPopper, SkipForward, Sparkles, X } from "@lucide/svelte";
  import type { ProtectionItem, Repo } from "$lib/api";
  import { refreshAgent } from "$lib/agent.svelte";
  import { dur } from "$lib/motion";
  import { protections, protectionTone, refreshProtection } from "$lib/protection.svelte";

  // «Mejorar la protección»: un panel que se queda a la vista mientras
  // arreglas, una por una, las cosas que faltan en la salud de la protección
  // de un destino. Cada paso abre el editor que ya existe en la página; el
  // anillo se actualiza solo y, al llegar a «7 de 7», lo celebra (sin
  // animación si el sistema pide reducir el movimiento).
  interface Props {
    repo: Repo;
    /** Abrir lo que arregla una comprobación (el editor de la página). */
    onfix: (id: ProtectionItem["id"]) => void;
    onclose: () => void;
  }
  let { repo, onfix, onclose }: Props = $props();

  /** Orden de los pasos: que las copias se hagan y, después, poder recuperarlas pase lo que pase. */
  const ORDER: ProtectionItem["id"][] = ["copias", "kit", "restauracion", "verificacion", "externa", "retencion", "borrado"];

  const WHY: Record<ProtectionItem["id"], string> = {
    copias: "Que las copias se hagan solas a su hora, aunque nadie se acuerde y la app esté cerrada.",
    kit: "Una hoja con lo necesario para abrir tus copias desde otro equipo si pierdes este. Guárdala en papel o fuera del equipo.",
    restauracion: "Cada cierto tiempo se restauran unos archivos al azar y se comprueba que salen enteros: así sabes que las copias se pueden recuperar.",
    verificacion: "Comprueba que lo guardado no se ha dañado en el disco o en el servidor.",
    externa: "Otra copia fuera de este sitio (la nube u otro disco), por si hay un robo, un incendio o un ransomware.",
    retencion: "Decide cuántas versiones antiguas se guardan para que el repositorio no crezca sin fin.",
    borrado: "Que nadie, ni un ransomware con acceso a este equipo, pueda borrar las versiones guardadas.",
  };
  const DO: Record<ProtectionItem["id"], string> = {
    copias: "Programar las copias",
    kit: "Preparar el kit",
    restauracion: "Programar la prueba",
    verificacion: "Programar la verificación",
    externa: "Configurar la copia externa",
    retencion: "Definir la retención",
    borrado: "Ver cómo protegerlo",
  };

  const protection = $derived(protections[repo.id] ?? null);
  const items = $derived(protection?.items ?? []);
  const stateOf = (id: ProtectionItem["id"]) => items.find((i) => i.id === id)?.state ?? "unknown";

  /** Los pasos son lo que faltaba al abrir el asistente (lo que se arregla queda marcado). */
  let steps = $state<ProtectionItem["id"][]>([]);
  $effect(() => {
    if (steps.length || !protection) return;
    untrack(() => {
      const pending = new Set(protection.items.filter((i) => i.state !== "ok").map((i) => i.id));
      steps = ORDER.filter((id) => pending.has(id));
    });
  });
  /** Pasos saltados (se puede volver a ellos). */
  let skipped = $state<ProtectionItem["id"][]>([]);
  let chosen = $state<ProtectionItem["id"] | null>(null);
  const current = $derived.by(() => {
    if (chosen && stateOf(chosen) !== "ok") return chosen;
    return steps.find((id) => stateOf(id) !== "ok" && !skipped.includes(id)) ?? steps.find((id) => stateOf(id) !== "ok") ?? null;
  });
  const currentItem = $derived(items.find((i) => i.id === current) ?? null);
  const done = $derived(steps.filter((id) => stateOf(id) === "ok").length);
  const complete = $derived(!!protection && protection.score === protection.total);

  // Mientras está abierto, el estado se refresca a menudo (lo que se configura lo hace el agente).
  onMount(() => {
    const t = setInterval(() => {
      void refreshAgent();
      void refreshProtection(repo);
    }, 5_000);
    return () => clearInterval(t);
  });

  function skip() {
    if (!current) return;
    skipped = [...skipped.filter((x) => x !== current), current];
    chosen = null;
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onclose();
    }
  }

  // Anillo: circunferencia de r = 26.
  const C = 2 * Math.PI * 26;
  const ratio = $derived(protection ? protection.score / Math.max(1, protection.total) : 0);
  const tone = $derived(protectionTone(protection));
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<aside class="guide" aria-label="Mejorar la protección de «{repo.name}»" {onkeydown} in:fly={{ y: 16, duration: dur(200) }}>
  <header>
    <svg class="ring tone-{tone}" width="44" height="44" viewBox="0 0 64 64" aria-hidden="true">
      <circle cx="32" cy="32" r="26" class="track" />
      {#if ratio > 0}<circle cx="32" cy="32" r="26" class="arc" stroke-dasharray="{C * ratio} {C}" transform="rotate(-90 32 32)" />{/if}
    </svg>
    <div class="htext">
      <h2><Sparkles size={15} /> Mejorar la protección</h2>
      <p class="faint" aria-live="polite">
        {#if protection}<strong>{protection.score} de {protection.total}</strong> en «{repo.name}»{#if steps.length && !complete} · paso {Math.min(done + 1, steps.length)} de {steps.length}{/if}{:else}Comprobando…{/if}
      </p>
    </div>
    <button class="icon-btn" title="Cerrar (Escape)" aria-label="Cerrar el asistente" onclick={onclose}><X size={16} /></button>
  </header>

  {#if complete}
    <div class="celebrate" in:scale={{ start: 0.92, duration: dur(320) }} role="status">
      <span class="big"><PartyPopper size={26} /></span>
      <strong>¡{protection?.total} de {protection?.total}!</strong>
      <p>Las copias de «{repo.name}» están protegidas por todos los frentes. Resguardo te avisará si algo deja de estarlo.</p>
      <button class="btn btn-primary" onclick={onclose}><Check size={14} /> Hecho</button>
    </div>
  {:else if currentItem && current}
    <div class="step">
      <span class="label">{currentItem.label}</span>
      <p class="detail">{currentItem.detail}</p>
      <p class="why">{WHY[current]}</p>
      <div class="actions">
        <button class="btn btn-ghost btn-sm" onclick={skip} disabled={steps.filter((id) => stateOf(id) !== "ok").length < 2}><SkipForward size={13} /> Saltar</button>
        <button class="btn btn-primary btn-sm" onclick={() => onfix(current)}>{DO[current]} <ArrowRight size={13} /></button>
      </div>
    </div>
  {:else if protection && !steps.length}
    <p class="faint pad">No falta nada que se pueda arreglar desde aquí.</p>
  {/if}

  {#if steps.length && !complete}
    <ol class="steps">
      {#each steps as id (id)}
        {@const st = stateOf(id)}
        <li>
          <button class:on={id === current} class:ok={st === "ok"} onclick={() => (chosen = id)} disabled={st === "ok"} aria-current={id === current ? "step" : undefined}>
            <span class="mark">{#if st === "ok"}<CircleCheck size={14} />{:else}<span class="num">{steps.indexOf(id) + 1}</span>{/if}</span>
            {items.find((i) => i.id === id)?.label ?? id}
          </button>
        </li>
      {/each}
    </ol>
  {/if}
</aside>

<style>
  .guide {
    position: fixed;
    right: 20px;
    bottom: 20px;
    z-index: 8;
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: min(380px, calc(100vw - 32px));
    max-height: calc(100vh - 40px);
    overflow: auto;
    padding: 14px 16px 16px;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: calc(var(--radius) + 4px);
    box-shadow: var(--shadow-lg);
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .htext {
    flex: 1;
    min-width: 0;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-base, 15px);
    font-weight: 650;
  }
  .htext p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .ring {
    flex: none;
  }
  .ring circle {
    fill: none;
    stroke-width: 7;
  }
  .ring .track {
    stroke: var(--surface-3);
  }
  .ring .arc {
    stroke-linecap: round;
    transition: stroke-dasharray 0.5s ease;
  }
  .tone-ok .arc {
    stroke: var(--ok);
  }
  .tone-warn .arc {
    stroke: var(--warn);
  }
  .tone-bad .arc {
    stroke: var(--bad);
  }
  .tone-muted .arc {
    stroke: var(--neutral);
  }
  .step {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px;
    border-radius: var(--radius);
    background: var(--surface-2);
  }
  .label {
    font-weight: 650;
  }
  .detail,
  .why {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .detail {
    color: var(--text-1);
  }
  .why {
    color: var(--text-2);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
  .steps {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .steps button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 8px;
    font: inherit;
    font-size: var(--fs-sm);
    text-align: left;
    color: var(--text-2);
    background: none;
    border: none;
    border-radius: 6px;
    cursor: pointer;
  }
  .steps button:hover:not(:disabled),
  .steps button.on {
    color: var(--text-1);
    background: var(--surface-2);
  }
  .steps button.on {
    font-weight: 600;
  }
  .steps button.ok {
    color: var(--ok);
    text-decoration: line-through;
    text-decoration-color: color-mix(in srgb, var(--ok) 50%, transparent);
    cursor: default;
  }
  .mark {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    flex: none;
  }
  .num {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    font-size: 11px;
    font-weight: 650;
    border-radius: 50%;
    border: 1.5px solid currentColor;
  }
  .celebrate {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 12px 8px 4px;
    text-align: center;
  }
  .celebrate .big {
    display: grid;
    place-items: center;
    width: 52px;
    height: 52px;
    border-radius: 50%;
    color: var(--ok);
    background: var(--ok-soft);
  }
  .celebrate strong {
    font-size: 18px;
  }
  .celebrate p {
    margin: 0 0 6px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .pad {
    margin: 0;
    font-size: var(--fs-sm);
  }
</style>
