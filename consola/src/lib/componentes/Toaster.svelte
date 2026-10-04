<script lang="ts">
  import { fly } from "svelte/transition";
  import { CircleAlert, CircleCheck, Info, TriangleAlert, X } from "@lucide/svelte";
  import { dur } from "$ui/movimiento";
  import { cerrarToast, toasts } from "$lib/avisos.svelte";

  const ICONO = { ok: CircleCheck, bad: CircleAlert, info: Info, warn: TriangleAlert };
</script>

<div class="toaster" aria-live="polite" aria-atomic="false">
  {#each toasts as t (t.id)}
    {@const Icono = ICONO[t.tono]}
    <div class="toast card tone-{t.tono}" role={t.tono === "bad" ? "alert" : "status"} transition:fly={{ y: 12, duration: dur(180) }}>
      <span class="ic"><Icono size={16} /></span>
      <p>{t.texto}</p>
      {#if t.accion?.href}
        <a class="btn btn-sm btn-ghost" href={t.accion.href} onclick={() => cerrarToast(t.id)}>{t.accion.texto}</a>
      {:else if t.accion}
        <button
          class="btn btn-sm btn-ghost"
          onclick={() => {
            t.accion?.hacer?.();
            cerrarToast(t.id);
          }}>{t.accion.texto}</button
        >
      {/if}
      <button class="icon-btn" aria-label="Cerrar aviso" onclick={() => cerrarToast(t.id)}><X size={14} /></button>
    </div>
  {/each}
</div>

<style>
  .toaster {
    position: fixed;
    right: var(--sp-4);
    bottom: var(--sp-4);
    z-index: 50;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    width: min(380px, calc(100vw - 32px));
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 10px 10px 14px;
    box-shadow: var(--shadow-md);
    pointer-events: auto;
  }
  .toast > :global(.btn) {
    margin: -3px 0;
  }
  .ic {
    display: grid;
    margin-top: 2px;
    color: var(--tone);
  }
  p {
    flex: 1;
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 20px;
  }
</style>
