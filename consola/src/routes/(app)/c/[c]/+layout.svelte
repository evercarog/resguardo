<script lang="ts">
  // Todo lo de un cliente: se carga al entrar y se mantiene al día sin recargar
  // la pantalla. Con el canal en vivo (v1.39, $lib/vivo.svelte), en cuanto el
  // servidor dice que algo cambió (y de respaldo cada minuto); sin él, cada
  // 15 s mientras la pestaña está visible.
  import { untrack, type Snippet } from "svelte";
  import { page } from "$app/state";
  import { actual, cargarCliente } from "$lib/estado.svelte";
  import { recargarInformes } from "$lib/informes.svelte";
  import { vigilarProgreso } from "$lib/progreso.svelte";
  import { conectarVivo, seguirCambios, type Cambio } from "$lib/vivo.svelte";
  import PantallaError from "$lib/componentes/PantallaError.svelte";

  let { children }: { children: Snippet } = $props();
  const id = $derived(page.params.c ?? "");

  $effect(() => {
    if (!id) return;
    // Sin seguir lo que lee cargarCliente (actual.cliente…), que también escribe.
    untrack(() => void cargarCliente(id));
    try {
      localStorage.setItem("resguardo.cliente", id);
    } catch {
      /* sin almacenamiento */
    }
  });

  // El canal en vivo del cliente (antes que el progreso: este se apunta a sus cambios).
  $effect(() => {
    const c = id;
    return c ? untrack(() => conectarVivo(c)) : undefined;
  });

  // Lo que está en marcha en sus equipos (progreso de las copias), casi en vivo.
  $effect(() => {
    const c = id;
    return c ? untrack(() => vigilarProgreso(c)) : undefined;
  });

  /** ¿Cambia el resumen («Estado», equipos, avisos abiertos, órdenes con espera)? El progreso, solo al terminar. */
  const tocaResumen = (c: Cambio) => (c.t === "progreso" ? c.estado === "termina" : c.t !== "historial");
  /** ¿Cambian las cifras de los informes (espacio, versiones, últimas copias)? */
  const tocaInformes = (c: Cambio) => c.t === "informe" || c.t === "config" || (c.t === "progreso" && c.estado === "termina");

  $effect(() => {
    const c = id;
    if (!c) return;
    return untrack(() => {
      const dejarResumen = seguirCambios(() => actual.id === c && cargarCliente(c, { silencioso: true }), { ms: 15_000, toca: tocaResumen });
      const dejarInformes = seguirCambios(() => actual.id === c && recargarInformes(c), { ms: 15_000, toca: tocaInformes });
      return () => {
        dejarResumen();
        dejarInformes();
      };
    });
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
