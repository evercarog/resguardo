<script lang="ts">
  // 15hh: el diálogo de «Parar» o «Pausar» (ver PararTarea.svelte). Va aparte, montado en la
  // página, porque lo que se para desaparece de su progreso en cuanto para: el diálogo tiene que
  // seguir abierto para enseñar la respuesta del equipo.
  import { hastaDePausa, OPCIONES_PAUSA, type OpcionPausa } from "$lib/parar";
  import type { Cliente, Equipo, TareaEnMarcha } from "$lib/tipos";
  import OrdenDialog from "./OrdenDialog.svelte";

  let { cliente, equipo, tarea, modo, onclose }: { cliente: Cliente; equipo: Equipo; tarea: TareaEnMarcha; modo: "parar" | "pausar"; onclose: () => void } = $props();

  let opcion = $state<OpcionPausa>("noche");
  const nombre = $derived(tarea.nombre ? `«${tarea.nombre}»` : "esto");
  const cuerpo = $derived(modo === "pausar" ? { clave: tarea.parar, pausa: hastaDePausa(opcion, new Date()) } : { clave: tarea.parar });
</script>

<OrdenDialog
  {cliente}
  {equipo}
  tipo="parar"
  {cuerpo}
  titulo={modo === "pausar" ? `Pausar ${nombre}` : `Parar ${nombre}`}
  descripcion={modo === "pausar"
    ? `${equipo.nombre} la para ahora y sigue sola a la hora que elijas, aprovechando lo ya copiado. Mientras, sale «En pausa» y no cuenta como fallida.`
    : `${equipo.nombre} la para en unos segundos. Lo copiado hasta ahora se aprovecha la próxima vez. Queda en el historial de todas las consolas.`}
  accion={modo === "pausar" ? "Pausar" : "Parar"}
  campos={modo === "pausar" ? camposPausa : undefined}
  {onclose}
/>

{#snippet camposPausa()}
  <fieldset class="opciones">
    <legend class="field-label">Seguir sola</legend>
    {#each OPCIONES_PAUSA as o (o.id)}
      <label class="opcion"><input type="radio" name="pausa" value={o.id} bind:group={opcion} /> {o.texto}</label>
    {/each}
  </fieldset>
{/snippet}

<style>
  .opciones {
    display: grid;
    gap: var(--sp-1);
    margin: 0;
    padding: 0;
    border: 0;
  }
  .opcion {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--fs-sm);
  }
</style>
