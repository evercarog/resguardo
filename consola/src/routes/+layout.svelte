<script lang="ts">
  import "@fontsource-variable/inter";
  import "$ui/estilos.css";
  import "../app.css";
  import { onMount, type Snippet } from "svelte";
  import { goto } from "$app/navigation";
  import { dev } from "$app/environment";
  import { page } from "$app/state";
  import Toaster from "$lib/componentes/Toaster.svelte";
  import BarraProgreso from "$lib/componentes/BarraProgreso.svelte";
  import Logo from "$ui/componentes/Logo.svelte";
  import PantallaError from "$lib/componentes/PantallaError.svelte";
  import * as api from "$lib/api";
  import { app } from "$lib/estado.svelte";
  import { iniciarApariencia } from "$lib/apariencia.svelte";
  import { tooltipsDeIconos } from "$lib/tooltip";

  let { children }: { children: Snippet } = $props();
  let error = $state("");

  // Rutas que no necesitan sesión.
  const PUBLICAS = ["/entrar", "/primer-arranque", "/invitacion"];

  // Los botones de solo icono enseñan su nombre en un tooltip (lib/tooltip.ts).
  onMount(() => tooltipsDeIconos());

  // PWA: el service worker solo guarda el armazón de la consola (nunca la API)
  // y enseña «Sin conexión» si no hay red (src/service-worker.ts). Si el
  // navegador no lo admite o no se fía del certificado, la consola va igual.
  onMount(() => {
    if (!dev && "serviceWorker" in navigator) navigator.serviceWorker.register("/service-worker.js").catch(() => {});
  });

  onMount(async () => {
    iniciarApariencia();
    try {
      app.servidor = await api.servidor();
    } catch (e) {
      error = (e as Error).message;
      return;
    }
    const ruta = page.url.pathname;
    if (!app.servidor.inicializado) {
      if (ruta !== "/primer-arranque") await goto("/primer-arranque", { replaceState: true });
      app.listo = true;
      return;
    }
    if (!PUBLICAS.some((p) => ruta.startsWith(p))) {
      try {
        app.cuenta = await api.cuenta({ sinRedirigir: true });
      } catch {
        await goto(`/entrar?volver=${encodeURIComponent(ruta + page.url.search)}`, { replaceState: true });
      }
    }
    app.listo = true;
  });
</script>

{#if error}
  <main>
    <PantallaError pantallaCompleta ilustracion="sin-conexion" titulo="No hay conexión con Resguardo Server" texto={error}>
      <button class="btn btn-primary" onclick={() => location.reload()}>Volver a intentarlo</button>
    </PantallaError>
  </main>
{:else if app.listo}
  {@render children()}
{:else}
  <!-- Mientras se pregunta al servidor quién eres: la marca, quieta, y la barra de arriba. -->
  <div class="arranque" role="status" aria-label="Conectando con Resguardo Server…"><Logo size={36} /><span>Conectando…</span></div>
{/if}

<BarraProgreso />
<Toaster />

<style>
  .arranque {
    display: grid;
    place-content: center;
    justify-items: center;
    gap: var(--sp-3);
    min-height: 100dvh;
    font-size: var(--fs-sm);
    color: var(--text-3);
    animation: aparecer 0.4s 0.3s both;
  }
  @keyframes aparecer {
    from {
      opacity: 0;
    }
  }
</style>
