<script lang="ts">
  // Todo lo de un cliente: se carga al entrar y se refresca cada 15 s mientras
  // la pestaña está visible (sin recargar la pantalla).
  import { onMount, untrack, type Snippet } from "svelte";
  import { page } from "$app/state";
  import { actual, cargarCliente } from "$lib/estado.svelte";
  import { vigilarProgreso } from "$lib/progreso.svelte";
  import PantallaError from "$lib/componentes/PantallaError.svelte";

  let { children }: { children: Snippet } = $props();
  const id = $derived(page.params.c ?? "");

  $effect(() => {
    if (!id) return;
    void cargarCliente(id);
    try {
      localStorage.setItem("resguardo.cliente", id);
    } catch {
      /* sin almacenamiento */
    }
  });

  // Lo que está en marcha en sus equipos (progreso de las copias), casi en vivo.
  $effect(() => {
    const c = id;
    return c ? untrack(() => vigilarProgreso(c)) : undefined;
  });

  onMount(() => {
    const t = setInterval(() => {
      if (document.visibilityState === "visible" && actual.id) void cargarCliente(actual.id, { silencioso: true });
    }, 15_000);
    return () => clearInterval(t);
  });
</script>

{#if actual.error && !actual.cliente}
  <div class="page">
    {#if actual.errorCodigo === "prohibido"}
      <PantallaError ilustracion="sin-permiso" titulo="Este cliente no es para tu cuenta" texto={actual.error}><a class="btn" href="/clientes">Ver mis clientes</a></PantallaError>
    {:else if actual.errorCodigo === "no_existe"}
      <PantallaError ilustracion="no-encontrado" titulo="No encontramos este cliente" texto="Puede que lo hayan quitado o que ya no seas parte de él."><a class="btn" href="/clientes">Ver mis clientes</a></PantallaError>
    {:else if actual.errorCodigo === "red"}
      <PantallaError ilustracion="sin-conexion" titulo="Sin conexión con el servidor" texto={actual.error}><button class="btn btn-primary" onclick={() => cargarCliente(id)}>Reintentar</button></PantallaError>
    {:else}
      <div class="notice notice-danger"><p>{actual.error}</p></div>
    {/if}
  </div>
{:else}
  {@render children()}
{/if}
