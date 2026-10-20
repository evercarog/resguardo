<script lang="ts">
  // Inicio: con más de un cliente, «Todos los clientes»; con uno, ese; sin ninguno, la lista.
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { app, cargarClientes } from "$lib/estado.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import ErrorCarga from "$lib/componentes/ErrorCarga.svelte";

  let error = $state("");
  async function entrar() {
    try {
      await cargarClientes();
    } catch (e) {
      error = (e as Error).message;
      return;
    }
    error = "";
    const n = app.clientes.length;
    await goto(n > 1 ? "/todos" : n === 1 ? `/c/${app.clientes[0].id}` : "/clientes", { replaceState: true });
  }
  onMount(entrar);
</script>

<div class="page">
  {#if error}<ErrorCarga que="tus clientes" {error} reintentar={entrar} />{:else}<Cargando />{/if}
</div>
