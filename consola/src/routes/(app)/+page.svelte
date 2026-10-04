<script lang="ts">
  // Inicio: con más de un cliente, «Todos los clientes»; con uno, ese; sin ninguno, la lista.
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { app, cargarClientes } from "$lib/estado.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";

  onMount(async () => {
    await cargarClientes();
    const n = app.clientes.length;
    await goto(n > 1 ? "/todos" : n === 1 ? `/c/${app.clientes[0].id}` : "/clientes", { replaceState: true });
  });
</script>

<div class="page"><Cargando /></div>
