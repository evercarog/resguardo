<script lang="ts">
  // Lo que está en marcha en un equipo, donde se ponga: en la fila de una copia
  // (`copia`), en la tarjeta de un repositorio (`repo`) o, en las listas de
  // equipos, como chip (`compacto`). No pinta nada si no hay nada en marcha.
  // `alTerminar`: cuando termina lo que se estaba enseñando. `marco`: en una tarjeta.
  // `sinMover`: sin los pasos de «Mover a otro sitio…» (los cuenta MoviendoseAviso).
  import { LoaderCircle } from "@lucide/svelte";
  import { tip } from "$lib/tooltip";
  import { alTerminarTarea, pctConjunto, tareasDe } from "$lib/progreso.svelte";
  import { textoFase } from "$lib/textoProgreso";
  import type { TareaEnMarcha, TipoTarea } from "$lib/tipos";
  import ProgresoCopia from "./ProgresoCopia.svelte";

  let {
    equipo,
    copia,
    repo,
    tipos,
    compacto = false,
    marco = false,
    alTerminar,
    sinMover = false,
  }: { equipo: string; copia?: string; repo?: string; tipos?: TipoTarea[]; compacto?: boolean; marco?: boolean; alTerminar?: (t: TareaEnMarcha) => void; sinMover?: boolean } = $props();

  const tareas = $derived(tareasDe(equipo, { copia, repo, tipos }).filter((t) => !sinMover || !t.mover));
  /** Varias a la vez en el mismo equipo (chip): «2 en marcha · 35 %», con el detalle en el título. */
  const conjunto = $derived(tareas.length > 1 ? pctConjunto(tareas.map((tarea) => ({ equipo, tarea }))) : null);

  $effect(() => {
    if (!alTerminar) return;
    const f = alTerminar;
    return alTerminarTarea((e, t) => {
      if (e === equipo && (copia === undefined || t.copia === copia) && (repo === undefined || t.repo === repo) && (!tipos || tipos.includes(t.tipo))) f(t);
    });
  });
</script>

{#if tareas.length}
  {#if compacto}
    {#if tareas.length > 1}
      <span class="badge badge-sm tone-info" use:tip={tareas.map((t) => `${t.nombre ? `«${t.nombre}»` : textoFase(t)}`).join(" · ")}>
        <LoaderCircle size={12} class="spin" aria-hidden="true" />{tareas.length} en marcha{conjunto != null ? ` · ${conjunto} %` : "…"}
      </span>
    {:else}
      <ProgresoCopia tarea={tareas[0]} {equipo} compacto />
    {/if}
  {:else}
    <div class="en-marcha" class:card={marco} class:p={marco}>
      {#each tareas as t (`${t.tipo}|${t.repo}|${t.copia ?? ""}`)}<ProgresoCopia tarea={t} {equipo} />{/each}
    </div>
  {/if}
{/if}

<style>
  .en-marcha {
    display: grid;
    width: 100%;
    box-sizing: border-box;
    gap: var(--sp-3);
    min-width: 0;
  }
</style>
