<script lang="ts">
  // Inicio: el último cliente abierto, el único que hay o la lista.
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { app, cargarClientes } from "$lib/estado.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";

  onMount(async () => {
    await cargarClientes();
    let ultimo: string | null = null;
    try {
      ultimo = localStorage.getItem("resguardo.cliente");
    } catch {
      /* sin almacenamiento */
    }
    const destino = app.clientes.find((c) => c.id === ultimo) ?? (app.clientes.length === 1 ? app.clientes[0] : null);
    await goto(destino ? `/c/${destino.id}` : "/clientes", { replaceState: true });
  });
</script>

<div class="page"><Cargando /></div>
