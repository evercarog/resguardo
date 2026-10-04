<script lang="ts">
  // Página de error de la consola (una dirección que no existe, sobre todo).
  import { page } from "$app/state";
  import PantallaError from "$lib/componentes/PantallaError.svelte";

  const estado = $derived(page.status);
</script>

<svelte:head><title>{estado === 404 ? "No existe" : "Error"} · Resguardo Server</title></svelte:head>

<main>
  {#if estado === 404}
    <PantallaError pantallaCompleta ilustracion="no-encontrado" titulo="Esta página no existe" texto="Puede que el enlace esté mal copiado o que lo que buscabas ya no esté.">
      <a class="btn btn-primary" href="/">Ir al inicio</a>
    </PantallaError>
  {:else if estado === 403}
    <PantallaError pantallaCompleta ilustracion="sin-permiso" titulo="No tienes permiso para ver esto" texto="Si lo necesitas, pide a una persona propietaria del cliente que cambie tu papel.">
      <a class="btn btn-primary" href="/">Ir al inicio</a>
    </PantallaError>
  {:else}
    <PantallaError pantallaCompleta ilustracion="sin-conexion" titulo="Algo no ha ido bien" texto={page.error?.message || "Vuelve a intentarlo en un momento."}>
      <button class="btn btn-primary" onclick={() => location.reload()}>Volver a intentarlo</button>
      <a class="btn" href="/">Ir al inicio</a>
    </PantallaError>
  {/if}
</main>
