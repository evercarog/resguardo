<script lang="ts">
  import { fade } from "svelte/transition";
  import { dur } from "$lib/motion";
  import { CircleAlert, CircleCheck, CircleHelp, CircleX, RefreshCw, ShieldCheck, Sparkles } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { ProtectionItem, Repo } from "$lib/api";
  import { agent } from "$lib/agent.svelte";
  import { lastSnapshotTime, protections, protectionTone, refreshProtection } from "$lib/protection.svelte";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { toast } from "$lib/toast.svelte";
  import HelpLink from "./HelpLink.svelte";
  import InfoTip from "./InfoTip.svelte";

  // «Salud de la protección»: lo que protege de verdad las copias de un destino,
  // de un vistazo (anillo «5 de 7») y con un botón para arreglar cada cosa.
  // `compact`: solo el anillo y una línea (tarjetas de Estado).
  interface Props {
    repo: Repo;
    compact?: boolean;
    onchange?: (repo: Repo) => void;
    /** Llevar a donde se arregla una comprobación (en el destino). */
    onfix?: (id: ProtectionItem["id"]) => void;
    /** Abrir el asistente «Mejorar la protección». */
    onimprove?: () => void;
  }
  let { repo, compact = false, onchange, onfix, onimprove }: Props = $props();

  const protection = $derived(protections[repo.id] ?? null);
  let probing = $state(false);

  /** Versión más reciente que conoce la interfaz (para saber si la copia externa va atrasada). */
  const lastSnapshot = $derived(lastSnapshotTime(repo.id));

  // Se recalcula cuando cambia lo que la afecta: el destino, el agente o sus versiones.
  $effect(() => {
    void [agent.info, JSON.stringify(repo), lastSnapshot];
    void refreshProtection(repo);
  });

  // Servidor REST: se comprueba si es de solo añadir (como mucho una vez al día).
  $effect(() => {
    if (compact || !repo.location.startsWith("rest:")) return;
    const fresh = repo.append_only && Date.now() - new Date(repo.append_only.checked_at).getTime() < 24 * 3_600_000;
    if (!fresh) probe(false);
  });

  async function probe(force: boolean) {
    probing = true;
    try {
      onchange?.(await api.probeAppendOnly(repo.id, force));
    } catch {
      /* sin comprobar */
    } finally {
      probing = false;
    }
  }

  async function toggleObjectLock() {
    const on = !repo.object_lock;
    await withPassword({
      title: on ? "Bloqueo de objetos activado" : "Quitar el bloqueo de objetos",
      message: on
        ? `Confirmas que el bucket de «${repo.name}» tiene activado el bloqueo de objetos (Object Lock): las versiones no se pueden borrar hasta que vence el bloqueo.`
        : `Se dejará de contar «${repo.name}» como protegido contra borrado.`,
      repoName: repo.name,
      confirmLabel: on ? "Confirmar" : "Quitar",
      danger: !on,
      action: async (password) => onchange?.(await api.setObjectLock(repo.id, on, password)),
    });
  }

  const ratio = $derived(protection ? protection.score / Math.max(1, protection.total) : 0);
  // Como la web (docs/diseno.md): «bad» si algo falla, «warn» si falta algo, «ok» si todo está bien.
  const tone = $derived(protectionTone(protection));
  const issues = $derived(protection?.items.filter((i) => i.state !== "ok") ?? []);
  const cloud = $derived(/^(s3|b2|azure|gs):/i.test(repo.location));

  /** Botón de cada comprobación (qué hacer para arreglarla). */
  function action(i: ProtectionItem): { label: string; run: () => void } | null {
    if (i.state === "ok" && !(i.id === "borrado" && cloud && repo.object_lock)) return null;
    switch (i.id) {
      case "copias":
        return { label: i.state === "bad" && i.detail.startsWith("Sin") ? "Programar" : "Revisar", run: () => onfix?.("copias") };
      case "borrado":
        if (repo.location.startsWith("rest:")) return { label: probing ? "Comprobando…" : "Comprobar de nuevo", run: () => probe(true) };
        if (cloud) return { label: repo.object_lock ? "Quitar «tiene bloqueo»" : "Tiene bloqueo de objetos", run: toggleObjectLock };
        return { label: "Copia externa", run: () => onfix?.("externa") };
      case "externa":
        return { label: i.detail.startsWith("Todas") ? "Configurar" : "Revisar", run: () => onfix?.("externa") };
      case "kit":
        return { label: "Preparar el kit", run: () => onfix?.("kit") };
      case "retencion":
        return { label: "Definir", run: () => onfix?.("retencion") };
      default:
        return { label: i.detail.startsWith("Sin") ? "Programar" : "Revisar", run: () => onfix?.(i.id) };
    }
  }

  // Anillo: circunferencia de r = 26.
  const C = 2 * Math.PI * 26;
</script>

{#snippet ring(size: number)}
  <svg class="ring tone-{tone}" width={size} height={size} viewBox="0 0 64 64" aria-hidden="true">
    <circle cx="32" cy="32" r="26" class="track" />
    {#if ratio > 0}<circle cx="32" cy="32" r="26" class="arc" stroke-dasharray="{C * ratio} {C}" transform="rotate(-90 32 32)" />{/if}
  </svg>
{/snippet}

{#if compact}
  {#if protection}
    <span class="compact tone-{tone}" title={issues.map((i) => `${i.label}: ${i.detail}`).join("\n")} in:fade={{ duration: dur(150) }}>
      {@render ring(18)}
      <span>Protección <strong>{protection.score} de {protection.total}</strong>{issues.length ? ` · ${issues.length} por revisar` : " · todo en orden"}</span>
    </span>
  {/if}
{:else}
  <section class="card health tone-{tone}" in:fade={{ duration: dur(160) }}>
    <div class="summary">
      <div class="ring-wrap">
        {@render ring(74)}
        <span class="score">{#if protection}<strong>{protection.score}</strong><span>de {protection.total}</span>{:else}…{/if}</span>
      </div>
      <div class="sum-text">
        <h2 class="section-title"><ShieldCheck size={16} /> Salud de la protección <HelpLink topic="salud-proteccion" label="la salud de la protección" /></h2>
        <p class="faint">
          {#if !protection}
            Comprobando…
          {:else if !issues.length}
            Todo en orden: las copias de «{repo.name}» están protegidas por todos los frentes.
          {:else}
            {issues.length === 1 ? "Falta una cosa" : `Faltan ${issues.length} cosas`} para que las copias de «{repo.name}» estén protegidas del todo.
          {/if}
        </p>
      </div>
      {#if onimprove && issues.length}
        <button class="btn btn-sm improve" onclick={onimprove}><Sparkles size={14} /> Mejorar la protección</button>
      {/if}
    </div>

    {#if protection}
      <ul class="items">
        {#each protection.items as i (i.id)}
          {@const act = action(i)}
          <li class="item st-{i.state}">
            <span class="ic">
              {#if i.state === "ok"}<CircleCheck size={17} />{:else if i.state === "warn"}<CircleAlert size={17} />{:else if i.state === "bad"}<CircleX size={17} />{:else}<CircleHelp size={17} />{/if}
            </span>
            <span class="txt">
              <strong>{i.label} <InfoTip id="prot-{i.id}" /></strong>
              <span class="faint">{i.detail}</span>
            </span>
            {#if act}
              <button class="btn btn-ghost btn-sm" onclick={act.run} disabled={i.id === "borrado" && probing}>
                {#if i.id === "borrado" && repo.location.startsWith("rest:")}<RefreshCw size={12} />{/if}
                {act.label}
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </section>
{/if}

<style>
  .health {
    padding: 18px 22px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .summary {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .ring-wrap {
    position: relative;
    display: grid;
    place-items: center;
    flex: none;
  }
  .score {
    position: absolute;
    display: flex;
    flex-direction: column;
    align-items: center;
    line-height: 1;
  }
  .score strong {
    font-family: var(--font-display);
    font-size: 22px;
  }
  .score span {
    font-size: var(--fs-overline);
    color: var(--text-3);
  }
  .sum-text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    flex: 1;
  }
  .improve {
    flex: none;
    align-self: center;
  }
  .sum-text h2 {
    display: flex;
    align-items: center;
    gap: 7px;
  }
  .sum-text p {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .ring .track {
    fill: none;
    stroke: var(--surface-3);
    stroke-width: 7;
  }
  .ring .arc {
    fill: none;
    stroke: var(--tone);
    stroke-width: 7;
    stroke-linecap: round;
    transition: stroke-dasharray 0.6s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .items {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 6px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 10px 9px 12px;
    border-radius: var(--radius);
    background: var(--surface-2);
    border: 1px solid transparent;
    min-width: 0;
  }
  .item .ic {
    display: grid;
    flex: none;
  }
  .st-ok .ic {
    color: var(--ok);
  }
  .st-warn .ic {
    color: var(--warn);
  }
  .st-bad .ic {
    color: var(--bad);
  }
  .st-unknown .ic {
    color: var(--text-3);
  }
  .st-bad {
    background: var(--bad-soft);
  }
  .txt {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    font-size: var(--fs-sm);
    line-height: 1.4;
  }
  .txt .faint {
    font-size: var(--fs-xs);
  }
  .item .btn {
    flex: none;
    white-space: nowrap;
  }
  .compact {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .compact > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .compact .ring .track,
  .compact .ring .arc {
    stroke-width: 9;
  }
</style>
