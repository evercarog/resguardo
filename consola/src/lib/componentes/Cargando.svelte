<script lang="ts">
  // Atajo de siempre: filas de esqueleto (ver Esqueleto.svelte para otras formas).
  import Esqueleto from "./Esqueleto.svelte";
  let { filas = 4, forma = "filas" }: { filas?: number; forma?: "filas" | "tarjetas" | "tabla" | "cifras" | "ficha" | "lineas" } = $props();
</script>

<Esqueleto {forma} n={filas} />
