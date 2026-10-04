<script lang="ts">
  import type { Snippet } from "svelte";
  import { slide } from "svelte/transition";
  import { ChevronRight, SlidersHorizontal } from "@lucide/svelte";
  import { dur } from "$lib/motion";
  import { setUi, ui } from "$lib/ui.svelte";

  // «Opciones avanzadas»: lo que casi nadie necesita cambiar, plegado y con
  // los valores recomendados ya puestos. Se recuerda si cada usuario lo
  // desplegó (por `id`, en este equipo). Plegado, `hint` dice qué hay dentro
  // y cómo está; con `custom`, que algo no está en su valor recomendado.
  interface Props {
    /** Clave para recordarlo («plan», «retencion», «copia-externa»…). */
    id: string;
    label?: string;
    /** Qué hay dentro y cómo está ahora («Etiquetas: diaria · solo si hay cambios»). */
    hint?: string;
    /** Algo no está en su valor recomendado: se avisa y, si nunca se tocó, se despliega. */
    custom?: boolean;
    children: Snippet;
  }
  let { id, label = "Opciones avanzadas", hint = "", custom = false, children }: Props = $props();

  const open = $derived(ui.advanced[id] ?? custom);
  const region = `adv-${Math.random().toString(36).slice(2, 10)}`;

  function toggle() {
    setUi("advanced", { ...ui.advanced, [id]: !open });
  }
</script>

<div class="adv" class:open>
  <button type="button" class="adv-toggle" aria-expanded={open} aria-controls={region} onclick={toggle}>
    <span class="chev"><ChevronRight size={14} /></span>
    <SlidersHorizontal size={14} />
    <span class="adv-label">{label}</span>
    {#if custom}<span class="badge badge-sm tone-neutral" title="Algo no está en su valor recomendado">personalizadas</span>{/if}
    {#if !open && hint}<span class="adv-hint">{hint}</span>{/if}
  </button>
  {#if open}
    <div class="adv-body" id={region} transition:slide={{ duration: dur(160) }}>
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .adv {
    border-top: 1px dashed var(--border-strong);
    padding-top: 10px;
  }
  .adv-toggle {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    width: 100%;
    padding: 2px 0;
    font: inherit;
    font-size: var(--fs-sm);
    text-align: left;
    color: var(--text-2);
    background: none;
    border: none;
    cursor: pointer;
  }
  .adv-toggle:hover {
    color: var(--text-1);
  }
  .adv-toggle:focus-visible {
    outline: none;
    box-shadow: var(--focus);
    border-radius: 4px;
  }
  .adv-label {
    font-weight: 550;
  }
  .adv-hint {
    flex: 1 1 200px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-3);
  }
  .adv-hint::before {
    content: "· ";
  }
  .chev {
    display: grid;
    transition: transform 0.15s;
  }
  .open .chev {
    transform: rotate(90deg);
  }
  .adv-body {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding-top: 12px;
  }
</style>
