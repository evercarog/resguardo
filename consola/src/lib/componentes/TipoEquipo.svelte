<script lang="ts">
  // Plan 0.8.1, C3 (15c): corregir a mano el tipo de un equipo (físico, máquina
  // virtual y de qué tipo, o contenedor). Lo guarda el propio equipo
  // (`tipo_equipo`, inofensiva, solo administradores) y lo ven igual todas sus
  // consolas, como el nombre. «Lo que detecta el agente» quita la corrección.
  import { Cpu, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { pedirAlEquipo } from "$lib/pedirAlEquipo";
  import { avisar } from "$lib/avisos.svelte";
  import { cargarCliente } from "$lib/estado.svelte";
  import { cuerpoTipo, NOMBRE_CLASE, PLATAFORMAS, textoTipo, tipoEquipo, type ClaseEquipo } from "$lib/tipoEquipo";
  import type { Cliente, Equipo } from "$lib/tipos";

  let { cliente, equipo, onclose }: { cliente: Cliente; equipo: Equipo; onclose: () => void } = $props();

  const actual = $derived(tipoEquipo(equipo));
  let clase = $state<ClaseEquipo | "">((() => (tipoEquipo(equipo).origen === "a_mano" ? tipoEquipo(equipo).clase : ""))());
  let plataforma = $state((() => (tipoEquipo(equipo).origen === "a_mano" ? (tipoEquipo(equipo).plataforma ?? "") : ""))());
  let ocupado = $state(false);
  let error = $state("");
  const opciones = $derived(clase === "virtual" || clase === "contenedor" ? PLATAFORMAS[clase] : []);

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    ocupado = true;
    error = "";
    try {
      const r = await pedirAlEquipo(cliente, equipo, "tipo_equipo", cuerpoTipo(clase, plataforma));
      avisar(r.hecha ? (clase ? `${equipo.nombre}: ${NOMBRE_CLASE[clase].toLowerCase()}, en todas sus consolas.` : `${equipo.nombre}: el tipo que detecta el agente.`) : r.texto);
      void cargarCliente(cliente.id, { silencioso: true });
      onclose();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }
</script>

<Modal labelledby="t-tipo-equipo" {onclose} width={480}>
  <div class="dlg-title">
    <span class="ticon"><Cpu size={18} /></span>
    <div>
      <h2 id="t-tipo-equipo">Tipo de equipo</h2>
      <p>El agente lo detecta solo ({textoTipo(actual).toLowerCase()}{actual.origen === "a_mano" ? ", corregido a mano" : ""}). Corrígelo si no acierta: lo guarda {equipo.nombre} y lo verán igual todas sus consolas.</p>
    </div>
  </div>
  <form class="form" onsubmit={guardar}>
    <fieldset class="field">
      <legend class="field-label">Es</legend>
      <div class="opciones">
        <label><input type="radio" name="clase" value="" bind:group={clase} disabled={ocupado} />Lo que detecta el agente</label>
        <label><input type="radio" name="clase" value="fisico" bind:group={clase} disabled={ocupado} />Un equipo físico</label>
        <label><input type="radio" name="clase" value="virtual" bind:group={clase} disabled={ocupado} />Una máquina virtual</label>
        <label><input type="radio" name="clase" value="contenedor" bind:group={clase} disabled={ocupado} />Un contenedor</label>
      </div>
    </fieldset>
    {#if opciones.length}
      <div class="field">
        <label class="field-label" for="te-plat">{clase === "virtual" ? "De qué tipo" : "Cuál"}</label>
        <select id="te-plat" class="input" bind:value={plataforma} disabled={ocupado}>
          <option value="">No lo sé</option>
          {#each opciones as o (o.id)}<option value={o.id}>{o.nombre}</option>{/each}
        </select>
      </div>
    {/if}
    {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
    <footer>
      <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
      <button type="submit" class="btn btn-primary" disabled={ocupado}>{ocupado ? "Guardando…" : "Guardar"}</button>
    </footer>
  </form>
</Modal>

<style>
  fieldset {
    margin: 0;
    padding: 0;
    border: 0;
  }
  .opciones {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 6px;
  }
  .opciones label {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 24px;
    font-size: var(--fs-body);
  }
</style>
