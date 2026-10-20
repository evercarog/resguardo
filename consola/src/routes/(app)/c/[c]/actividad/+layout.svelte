<script lang="ts">
  // Plan 0.8.1, B1 (15bb): la sección con sus pestañas (lib/secciones.ts).
  import type { Snippet } from "svelte";
  import SeccionPestanas from "$lib/componentes/SeccionPestanas.svelte";

  let { children }: { children: Snippet } = $props();
</script>

<SeccionPestanas seccion="actividad">{@render children()}</SeccionPestanas>
