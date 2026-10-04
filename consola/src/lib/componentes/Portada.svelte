<script lang="ts">
  // Marco de las pantallas sin sesión (entrar, primer arranque, invitación).
  import type { Snippet } from "svelte";
  import Logo from "$ui/componentes/Logo.svelte";
  import { app } from "$lib/estado.svelte";
  let { titulo, sub, children, pie }: { titulo: string; sub?: string; children: Snippet; pie?: Snippet } = $props();
</script>

<main class="portada fondo-portada">
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
    padding: var(--sp-16, 64px) var(--sp-4) var(--sp-6);
  }
  .marca {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: var(--sp-2);
    font-size: 17px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  /* La tarjeta flota sobre el fondo: sombra amplia en claro; en oscuro, borde algo más marcado y un filo de luz arriba. */
  .caja {
    width: min(440px, 100%);
    padding: var(--sp-8);
    border-radius: var(--radius-xl);
    box-shadow:
      inset 0 1px 0 rgb(255 255 255 / 0.04),
      0 1px 2px rgb(0 0 0 / 0.04),
      0 24px 48px -20px rgb(0 0 0 / 0.22);
    animation: rise var(--dur-slow) var(--ease-out) both;
  }
  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-theme="light"])) .caja {
      border-color: var(--border-strong);
    }
  }
  :global(:root[data-theme="dark"]) .caja,
  :global(:root[data-theme="black"]) .caja {
    border-color: var(--border-strong);
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
