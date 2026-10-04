<script lang="ts">
  // Pantalla de error con su ilustración (sin conexión, no existe, sin
  // permiso): qué pasa en una frase y qué hacer, con una o dos acciones.
  import type { Snippet } from "svelte";
  import Ilustracion, { type NombreIlustracion } from "$ui/componentes/Ilustracion.svelte";

  let {
    ilustracion,
    titulo,
    texto,
    nivel = 1,
    pantallaCompleta = false,
    children,
  }: { ilustracion: NombreIlustracion; titulo: string; texto?: string; nivel?: 1 | 2; pantallaCompleta?: boolean; children?: Snippet } = $props();
</script>

<div class="pantalla-error" class:completa={pantallaCompleta} role={nivel === 2 ? "alert" : undefined}>
  <Ilustracion nombre={ilustracion} ancho={176} />
  {#if nivel === 1}<h1>{titulo}</h1>{:else}<h2>{titulo}</h2>{/if}
  {#if texto}<p>{texto}</p>{/if}
  {#if children}<div class="acciones">{@render children()}</div>{/if}
</div>

<style>
  .pantalla-error {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-10) var(--sp-6);
    text-align: center;
  }
  .completa {
    justify-content: center;
    min-height: 100dvh;
  }
  h1,
  h2 {
    margin: var(--sp-2) 0 0;
    font-size: var(--fs-title);
    line-height: var(--lh-title);
    font-weight: 650;
    letter-spacing: -0.018em;
  }
  p {
    max-width: 440px;
    margin: 0;
    color: var(--text-2);
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--sp-2);
    margin-top: var(--sp-2);
  }
</style>
