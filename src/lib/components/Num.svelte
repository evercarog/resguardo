<script lang="ts">
  // Cifra que cambia con una transición suave (sin movimiento si el sistema
  // pide reducirlo). Siempre con cifras tabulares.
  import { Tween } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { dur } from "$lib/motion";

  let { value, format = (n: number) => Math.round(n).toLocaleString("es") }: { value: number; format?: (n: number) => string } = $props();

  const shown = new Tween(0, { duration: dur(420), easing: cubicOut });
  $effect(() => {
    shown.target = value;
  });
</script>

<span class="num">{format(shown.current)}</span>
