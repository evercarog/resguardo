<script lang="ts">
  // 0.8.1 (H5, H2): «Informes» con sus pestañas (lib/secciones.ts).
  import type { Snippet } from "svelte";
  import SeccionPestanas from "$lib/componentes/SeccionPestanas.svelte";

  let { children }: { children: Snippet } = $props();
</script>

<SeccionPestanas seccion="informes">{@render children()}</SeccionPestanas>
