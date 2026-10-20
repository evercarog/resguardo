<script lang="ts">
  // 15hh: «Parar» (y, en las copias, «Pausar») lo que está en marcha en un equipo, desde su
  // progreso en vivo. Con la clave de administración (decisión del responsable: parar o pausar
  // deja datos sin proteger) y sin espera. Lo ya copiado se aprovecha la próxima vez; una pausa
  // sigue sola a su hora y no cuenta como fallida. Queda en el historial de todas las consolas,
  // con quién la paró. Solo si el agente lo admite (manda `parar` en el progreso).
  import { mount, unmount } from "svelte";
  import { CirclePause, CircleStop } from "@lucide/svelte";
  import { actual, puede } from "$lib/estado.svelte";
  import type { TareaEnMarcha } from "$lib/tipos";
  import PararDialogo from "./PararDialogo.svelte";

  let { tarea, equipo }: { tarea: TareaEnMarcha; equipo: string } = $props();

  const eq = $derived(actual.equipos.find((e) => e.id === equipo) ?? null);
  const visible = $derived(!!tarea.parar && tarea.fase !== "en_cola" && !!eq && !!actual.cliente && puede.ordenar(actual.cliente.rol));

  /** El diálogo va montado aparte: lo que se para se va de su progreso enseguida (y este botón con ello). */
  function abrir(modo: "parar" | "pausar") {
    if (!eq || !actual.cliente) return;
    const sitio = document.createElement("div");
    document.body.appendChild(sitio);
    const d = mount(PararDialogo, {
      target: sitio,
      props: {
        cliente: actual.cliente,
        equipo: eq,
        tarea: $state.snapshot(tarea) as TareaEnMarcha,
        modo,
        onclose: () => {
          void unmount(d);
          sitio.remove();
        },
      },
    });
  }
</script>

{#if visible}
  <div class="parar">
    <button type="button" class="btn btn-sm btn-ghost" onclick={() => abrir("parar")}><CircleStop size={14} aria-hidden="true" />Parar…</button>
    {#if tarea.pausable}
      <button type="button" class="btn btn-sm btn-ghost" onclick={() => abrir("pausar")}><CirclePause size={14} aria-hidden="true" />Pausar…</button>
    {/if}
  </div>
{/if}

<style>
  .parar {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1);
  }
</style>
