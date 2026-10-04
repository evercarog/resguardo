<script lang="ts">
  // Chip de estado: siempre icono y texto, nunca solo color (docs/diseno.md).
  import { CircleAlert, CircleCheck, CircleDashed, CirclePause, Info, LoaderCircle, TriangleAlert } from "@lucide/svelte";
  import type { Tono } from "$lib/salud";

  let { tono, texto, pequeno = false, girando = false }: { tono: Tono; texto: string; pequeno?: boolean; girando?: boolean } = $props();
  const ICONO = { ok: CircleCheck, warn: TriangleAlert, bad: CircleAlert, info: Info, paused: CirclePause, neutral: CircleDashed };
  const Icono = $derived(girando ? LoaderCircle : ICONO[tono]);
</script>

<span class="badge tone-{tono}" class:badge-sm={pequeno}><Icono size={12} class={girando ? "spin" : ""} />{texto}</span>
