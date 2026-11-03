<script lang="ts">
  // 16h: en la ficha de un equipo que no está en otra consola del cliente: «Este equipo no
  // llega a esa consola» (sin internet, red aislada) con un motivo opcional, que lo saca
  // del aviso «N equipos no están en todas tus consolas»; y, si ya está marcado, lo dice
  // («solo en esta consola, a propósito») con «Deshacer». Es de esta consola.
  import { marcarNoLlega } from "$lib/noLlega.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";

  let {
    cliente,
    equipo,
    consola,
    marcado = null,
    puede,
  }: {
    cliente: string;
    equipo: { id: string; nombre: string };
    consola: { identidad: string; nombre: string };
    /** La marca, si ya está: su motivo (o null sin motivo). `undefined`: sin marcar. */
    marcado?: { motivo: string | null } | null;
    puede: boolean;
  } = $props();

  let abierto = $state(false);
  let motivo = $state("");
  let ocupado = $state(false);

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    ocupado = true;
    try {
      await marcarNoLlega(cliente, equipo.id, consola.identidad, motivo);
      avisar(`Hecho: ${equipo.nombre} ya no cuenta como pendiente en «${consola.nombre}».`);
      abierto = false;
      motivo = "";
    } catch (err) {
      fallo(err);
    } finally {
      ocupado = false;
    }
  }
  async function deshacer() {
    ocupado = true;
    try {
      await marcarNoLlega(cliente, equipo.id, consola.identidad, null, true);
      avisar(`${equipo.nombre} vuelve a contar como pendiente en «${consola.nombre}».`);
    } catch (err) {
      fallo(err);
    } finally {
      ocupado = false;
    }
  }
</script>

{#if marcado}
  <p class="nota marcado" data-testid="no-llega-marcado">
    Solo en esta consola, a propósito: no llega a «{consola.nombre}»{marcado.motivo ? ` (${marcado.motivo})` : ""}.
    {#if puede}<button type="button" class="btn btn-ghost btn-sm" disabled={ocupado} onclick={deshacer}>Deshacer</button>{/if}
  </p>
{:else if puede}
  {#if abierto}
    <form class="form-nl" onsubmit={guardar}>
      <label class="nota" for="nl-{equipo.id}-{consola.identidad}">¿Por qué? (opcional)</label>
      <input id="nl-{equipo.id}-{consola.identidad}" class="input input-sm" maxlength="200" bind:value={motivo} placeholder="Sin internet, red aislada…" disabled={ocupado} />
      <button class="btn btn-sm" disabled={ocupado}>Guardar</button>
      <button type="button" class="btn btn-ghost btn-sm" disabled={ocupado} onclick={() => (abierto = false)}>Cancelar</button>
    </form>
  {:else}
    <button type="button" class="btn btn-ghost btn-sm" onclick={() => (abierto = true)} data-testid="no-llega">Este equipo no llega a esa consola</button>
  {/if}
{/if}

<style>
  .nota {
    font-size: var(--fs-xs);
    color: var(--text-2);
    margin: 0;
  }
  .marcado {
    margin-bottom: var(--sp-3);
  }
  .form-nl {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .form-nl .input {
    flex: 1;
    min-width: 12rem;
  }
</style>
