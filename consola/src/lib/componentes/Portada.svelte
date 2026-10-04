<script lang="ts">
  // Marco de las pantallas sin sesión (entrar, primer arranque, invitación).
  import type { Snippet } from "svelte";
  import Logo from "$ui/componentes/Logo.svelte";
  import { app } from "$lib/estado.svelte";
  let { titulo, sub, children, pie }: { titulo: string; sub?: string; children: Snippet; pie?: Snippet } = $props();
</script>

<main class="portada">
  <div class="marca"><Logo size={36} /><span>Resguardo Server</span></div>
  <section class="card caja">
    <h1>{titulo}</h1>
    {#if sub}<p class="sub">{sub}</p>{/if}
    {@render children()}
  </section>
  {#if pie}<div class="pie">{@render pie()}</div>{/if}
  {#if app.servidor}<p class="version faint">{app.servidor.nombre} {app.servidor.version}</p>{/if}
</main>

<style>
  .portada {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-5);
    min-height: 100dvh;
    padding: var(--sp-12) var(--sp-4) var(--sp-6);
    background: var(--bg-subtle);
  }
  .marca {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 17px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  .caja {
    width: min(440px, 100%);
    padding: var(--sp-8);
    border-radius: var(--radius-xl);
  }
  h1 {
    font-size: var(--fs-title);
    line-height: var(--lh-title);
    font-weight: 650;
    letter-spacing: -0.018em;
  }
  .sub {
    margin: 6px 0 var(--sp-6);
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .pie {
    font-size: var(--fs-sm);
    color: var(--text-2);
    text-align: center;
  }
  .version {
    margin-top: auto;
    font-size: var(--fs-xs);
  }
  @media (max-width: 640px) {
    .portada {
      padding-top: var(--sp-8);
    }
    .caja {
      padding: var(--sp-6) var(--sp-5);
    }
  }
</style>
