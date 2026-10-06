<script lang="ts">
  // v1.47: «Mover a otro sitio…» en marcha en este repositorio (el que se
  // mueve o el nuevo), lo empezara esta consola u otra del equipo. El equipo lo
  // cuenta a todas sus consolas con el progreso de siempre; aquí solo se
  // enseña. Los pasos (y cancelarlos o seguir) los lleva el navegador que lo
  // empezó: desde otra consola no se puede tocar. `onseguir`: «Ver los pasos»
  // (solo si este navegador lleva el movimiento).
  import { ArrowRightLeft } from "@lucide/svelte";
  import { tareasDe } from "$lib/progreso.svelte";
  import { textoMover } from "$lib/textoProgreso";
  import { moviendoDe } from "$lib/mover";
  import ProgresoCopia from "./ProgresoCopia.svelte";

  let { equipo, repo, onseguir }: { equipo: string; repo: string; onseguir?: () => void } = $props();

  const tarea = $derived(moviendoDe(tareasDe(equipo), repo));
  const titulo = $derived(tarea ? textoMover(tarea) : null);
  const esElNuevo = $derived(!!tarea && tarea.repo === repo && tarea.origen !== repo);
</script>

{#if tarea && titulo}
  <div class="notice notice-info moviendose" role="status">
    <ArrowRightLeft size={16} />
    <div class="cuerpo">
      <p><strong>{titulo}</strong></p>
      <p class="pequeno">
        {#if esElNuevo}
          Este es el repositorio nuevo{tarea.nombre_origen ? ` de «${tarea.nombre_origen}»` : ""}: está recibiendo su historial.
        {:else}
          {tarea.nombre ? `Al terminar, las copias guardarán en «${tarea.nombre}». ` : ""}Este repositorio no se toca hasta que se deje de usar.
        {/if}
        {#if tarea.otra_consola}
          Los pasos los lleva {tarea.consola ? `la consola «${tarea.consola}»` : "la otra consola"}: desde aquí solo se ve cómo va.
        {:else if !onseguir}
          Lo lleva otro navegador de esta consola.
        {/if}
      </p>
      <!-- La barra, con lo que hace en corto (el título ya dice que se mueve y desde dónde). -->
      <ProgresoCopia tarea={{ ...tarea, etapa: tarea.paso === "ultimo" ? "Trayendo lo copiado mientras tanto" : "Trayendo el historial" }} {equipo} grafica={false} />
      {#if onseguir && !tarea.otra_consola}<button type="button" class="btn btn-sm" onclick={onseguir}>Ver los pasos</button>{/if}
    </div>
  </div>
{/if}

<style>
  .moviendose {
    align-items: flex-start;
  }
  .cuerpo {
    display: grid;
    gap: var(--sp-2);
    min-width: 0;
    flex: 1;
  }
  .cuerpo p {
    margin: 0;
  }
  .pequeno {
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .cuerpo .btn {
    justify-self: start;
  }
</style>
