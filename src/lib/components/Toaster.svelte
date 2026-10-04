<script lang="ts">
  import { fly, fade } from "svelte/transition";
  import { flip } from "svelte/animate";
  import { CircleAlert, CircleCheck, Info } from "@lucide/svelte";
  import { dismissToast, toasts } from "$lib/toast.svelte";
  import { dur } from "$lib/motion";

  const ICON = { success: CircleCheck, info: Info, error: CircleAlert };
</script>

<div class="toaster" role="status" aria-live="polite">
  {#each toasts as t (t.id)}
    {@const Icon = ICON[t.kind]}
    <button
      class="toast toast-{t.kind}"
      title="Cerrar aviso"
      onclick={() => dismissToast(t.id)}
      in:fly={{ y: 12, duration: dur(200) }}
      out:fade={{ duration: dur(150) }}
      animate:flip={{ duration: dur(200) }}
    >
      <span class="ic"><Icon size={16} /></span>
      <span class="msg">{t.message}</span>
    </button>
  {/each}
</div>

<style>
  .toaster {
    position: fixed;
    right: 20px;
    bottom: 20px;
    z-index: 20;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 8px;
    max-width: min(420px, calc(100vw - 40px));
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 11px 14px;
    font: inherit;
    font-size: var(--fs-sm);
    line-height: 1.45;
    text-align: left;
    color: var(--text-1);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-md);
    cursor: pointer;
    pointer-events: auto;
  }
  .ic {
    display: grid;
    flex: none;
    margin-top: 1px;
  }
  .toast-success .ic {
    color: var(--ok);
  }
  .toast-info .ic {
    color: var(--accent);
  }
  .toast-error {
    border-color: color-mix(in srgb, var(--bad) 45%, var(--border-strong));
  }
  .toast-error .ic {
    color: var(--bad);
  }
</style>
