<script lang="ts">
  // 15nn: al crear o recibir un cliente, si ya hay uno con ese nombre (sin
  // distinguir mayúsculas ni tildes): «¿querías añadirle equipos?», con el
  // camino a ese cliente. Se puede seguir igualmente: es solo un aviso.
  import { TriangleAlert } from "@lucide/svelte";
  import { clienteRepetido, type ClienteConNombre } from "$lib/nombresClientes";

  let {
    nombre,
    clientes,
    alUsar,
    usables,
  }: {
    nombre: string;
    clientes: readonly ClienteConNombre[];
    /** Si se da, el aviso ofrece usar ese cliente en vez de crear otro (p. ej. para su código de conexión). */
    alUsar?: (c: ClienteConNombre) => void;
    /** Con `alUsar`: los ids que se pueden usar (sin el campo, todos). */
    usables?: readonly string[];
  } = $props();

  const igual = $derived(clienteRepetido(nombre, clientes));
  const miembro = $derived(!!igual && igual.soy_miembro !== false);
  const usable = $derived(!!igual && !!alUsar && (!usables || usables.includes(igual.id)));
</script>

{#if igual}
  <div class="notice notice-warn repetido" role="status">
    <TriangleAlert size={16} />
    <div class="cuerpo">
      {#if igual.archivado}
        <p><strong>Ya hay un cliente «{igual.nombre}», archivado.</strong> ¿Querías volver a usarlo? Recupéralo en <a href="/servidor/clientes">Clientes del servidor</a>.</p>
      {:else}
        <p><strong>Ya hay un cliente «{igual.nombre}».</strong> ¿Querías añadirle equipos?</p>
        {#if usable && alUsar}
          <button type="button" class="btn btn-sm" onclick={() => alUsar(igual)}>Usar «{igual.nombre}»</button>
        {:else if alUsar && miembro}
          <p class="nota">No eres su propietario: su código de conexión te lo da quien lo es.</p>
        {:else if miembro}
          <p class="enlaces">
            <a href="/c/{igual.id}/emparejar">Añadir un equipo</a>
            <span aria-hidden="true">·</span>
            <a href="/c/{igual.id}/ajustes/servidor">Traer sus equipos de otra consola</a>
            <span class="nota">(«Dar un código de conexión»)</span>
          </p>
        {:else}
          <p class="nota">No eres miembro: pide a su propietario que te invite o que te dé un código de conexión.</p>
        {/if}
      {/if}
      <p class="nota">Si de verdad es otro, sigue: puede haber dos con el mismo nombre.</p>
    </div>
  </div>
{/if}

<style>
  .repetido {
    align-items: flex-start;
  }
  .cuerpo {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    min-width: 0;
  }
  .cuerpo p {
    margin: 0;
  }
  .enlaces {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 8px;
  }
  .nota {
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
</style>
