<script lang="ts">
  // Opciones de un destino del espejo (docs/espejo.md), para un almacén con
  // `admite: "espejo_flexible"`: cuándo (el mismo editor que las copias) y
  // «después de cada copia nueva».
  import EditorHorario from "./EditorHorario.svelte";
  import type { Horario } from "$lib/tipos";

  let {
    id,
    horario = $bindable(),
    trasCopia = $bindable(),
  }: {
    id: string;
    horario: Horario;
    trasCopia: boolean;
  } = $props();
</script>

<fieldset class="grupo">
  <legend class="field-label">Cuándo</legend>
  <EditorHorario id="{id}-horario" bind:horario admiteReglas para="verificacion" />
  <label class="switch-row">
    <input type="checkbox" bind:checked={trasCopia} />
    <span>También después de cada copia nueva<span class="faint">Cuando llega una versión nueva a este equipo, el espejo empieza unos minutos después (si llegan varias seguidas, espera a la última).</span></span>
  </label>
</fieldset>

<style>
  .grupo {
    border: 0;
    padding: 0;
    margin: 0;
    display: grid;
    gap: 0.6rem;
    min-width: 0;
  }
  .grupo legend {
    padding: 0;
    margin-bottom: 0.4rem;
  }
</style>
