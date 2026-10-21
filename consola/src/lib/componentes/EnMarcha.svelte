<script lang="ts">
  // Lo que está en marcha en un equipo, donde se ponga: en la fila de una copia
  // (`copia`), en la tarjeta de un repositorio (`repo`) o, en las listas de
  // equipos, como chip (`compacto`). No pinta nada si no hay nada en marcha.
  // `alTerminar`: cuando termina lo que se estaba enseñando. `marco`: en una tarjeta.
  // `sinMover`: sin los pasos de «Mover a otro sitio…» (los cuenta MoviendoseAviso).
  // 0.8.2 (C3): lo que estaba en marcha cuando el equipo dejó de responder sale como
  // «Interrumpida: PC-02 se desconectó a las 12:40» (lib/interrumpidas.ts). `sinCortadas`: no.
  import { CircleAlert, LoaderCircle } from "@lucide/svelte";
  import { actual, reloj } from "$lib/estado.svelte";
  import { enMarcha } from "$lib/progreso.svelte";
  import { ultimos } from "$lib/informes.svelte";
  import { cortadasVigentes, textoCortada } from "$lib/interrumpidas";
  import { NOMBRE_TAREA } from "$lib/textoProgreso";
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
    sinCortadas = false,
  }: { equipo: string; copia?: string; repo?: string; tipos?: TipoTarea[]; compacto?: boolean; marco?: boolean; alTerminar?: (t: TareaEnMarcha) => void; sinMover?: boolean; sinCortadas?: boolean } = $props();

  const tareas = $derived(tareasDe(equipo, { copia, repo, tipos }).filter((t) => !sinMover || !t.mover));
  const cortadas = $derived(
    tareas.length || sinCortadas
      ? []
      : cortadasVigentes(enMarcha.cortadas, actual.equipos, ultimos.porEquipo, equipo).filter(
          (c) => (copia === undefined || (c.tarea.tipo === "copia" && c.tarea.copia === copia)) && (repo === undefined || c.tarea.repo === repo) && (!tipos || tipos.includes(c.tarea.tipo)) && (!sinMover || !c.tarea.mover),
        ),
  );
  const queEra = (t: TareaEnMarcha) => (t.nombre ? `«${t.nombre}»` : NOMBRE_TAREA[t.tipo].toLowerCase());
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

{#if !tareas.length && cortadas.length}
  {#if compacto}
    <span class="badge badge-sm tone-warn" use:tip={cortadas.map((c) => `${queEra(c.tarea)}: ${textoCortada(c, reloj.ahora)}`).join(" · ")}><CircleAlert size={12} aria-hidden="true" />Interrumpida</span>
  {:else}
    <div class="en-marcha" class:card={marco} class:p={marco}>
      {#each cortadas as c (`${c.tarea.tipo}|${c.tarea.repo}|${c.tarea.copia ?? ""}`)}
        <p class="cortada" role="status"><CircleAlert size={15} aria-hidden="true" /><span><strong>{textoCortada(c, reloj.ahora)}.</strong> <span class="faint">{copia === undefined ? `${c.tarea.nombre ? `«${c.tarea.nombre}» estaba en marcha` : `Estaba ${NOMBRE_TAREA[c.tarea.tipo].toLowerCase()}`}. ` : ""}{c.tarea.tipo === "copia" ? "Lo ya subido se aprovecha al repetirla. " : ""}{c.equipo.conectado ? "El equipo dirá qué pasó en su próximo informe." : "Cuando vuelva, el equipo dirá qué pasó."}</span></span></p>
      {/each}
    </div>
  {/if}
{:else if tareas.length}
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
  .cortada {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    margin: 0;
    color: var(--text-1);
    font-size: var(--fs-sm);
    overflow-wrap: anywhere;
  }
  .cortada :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--warn);
  }
  .en-marcha {
    display: grid;
    width: 100%;
    box-sizing: border-box;
    gap: var(--sp-3);
    min-width: 0;
  }
</style>
