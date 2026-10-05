<script lang="ts">
  // Marco de las pantallas con sesión: barra lateral (cajón en móvil) y contenido.
  import { onMount, type Snippet } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { Menu, Search } from "@lucide/svelte";
  import { afterNavigate } from "$app/navigation";
  import { page } from "$app/state";
  import Logo from "$ui/componentes/Logo.svelte";
  import { dur } from "$ui/movimiento";
  import BarraLateral from "$lib/componentes/BarraLateral.svelte";
  import SinConexion from "$lib/componentes/SinConexion.svelte";
  import MarcaCliente from "$lib/componentes/MarcaCliente.svelte";
  import Paleta from "$lib/componentes/Paleta.svelte";
  import AyudaAtajos from "$lib/componentes/AyudaAtajos.svelte";
  import { alPulsar, atajos } from "$lib/atajos.svelte";
  import CopiasEnMarcha from "$lib/componentes/CopiasEnMarcha.svelte";
  import AccionesGlobales from "$lib/componentes/AccionesGlobales.svelte";
  import { actual, app, cargarClientes } from "$lib/estado.svelte";
  import { todasEnMarcha } from "$lib/progreso.svelte";
  import { ponerIcono } from "$lib/favicon";
  import { barra } from "$lib/barra.svelte";

  let { children }: { children: Snippet } = $props();
  let cajon = $state(false);
  let principal = $state<HTMLElement>();

  onMount(() => {
    void cargarClientes();
  });
  // El icono de la pestaña: punto rojo con avisos sin revisar, azul con algo en marcha.
  $effect(() => {
    const avisos = actual.id ? actual.avisosAbiertos > 0 : app.clientes.some((x) => x.avisos);
    ponerIcono(avisos ? "aviso" : actual.id && todasEnMarcha().length ? "marcha" : "normal");
  });
  onMount(() => () => ponerIcono("normal"));
  // Al cambiar de pantalla, el foco va al contenido (lectores de pantalla y teclado).
  afterNavigate(({ type }) => {
    if (type !== "enter") principal?.focus({ preventScroll: true });
  });

  // «Saltar al contenido»: el foco va al <main> (sin cambiar la URL).
  function saltar(e: MouseEvent) {
    e.preventDefault();
    principal?.focus();
    principal?.scrollIntoView();
  }

  // Cajón del móvil: foco al primer enlace al abrir y de vuelta al botón al cerrar.
  let botonMenu = $state<HTMLButtonElement>();
  function atrapar(nodo: HTMLElement) {
    const previo = document.activeElement as HTMLElement | null;
    queueMicrotask(() => (nodo.querySelector<HTMLElement>("a[href], button") ?? nodo).focus());
    return {
      destroy: () => {
        // Si al cerrar se navegó, el foco ya está en el contenido: no se le quita.
        const a = document.activeElement;
        if (a && a !== document.body && !nodo.contains(a)) return;
        (botonMenu && document.contains(botonMenu) ? botonMenu : previo)?.focus();
      },
    };
  }
  function trampa(e: KeyboardEvent) {
    if (e.key !== "Tab") return;
    const nodo = e.currentTarget as HTMLElement;
    const xs = Array.from(nodo.querySelectorAll<HTMLElement>('a[href], button:not([disabled]), [tabindex]:not([tabindex="-1"])')).filter((x) => x.offsetParent !== null);
    if (!xs.length) return;
    const [primero, ultimo] = [xs[0], xs[xs.length - 1]];
    if (e.shiftKey && document.activeElement === primero) (e.preventDefault(), ultimo.focus());
    else if (!e.shiftKey && document.activeElement === ultimo) (e.preventDefault(), primero.focus());
  }
</script>

{#if app.cuenta}
  <a class="saltar" href="#contenido" onclick={saltar}>Saltar al contenido</a>
  <div class="marco" class:plegada={barra.plegada}>
    <!-- La barra lateral ya es un <nav> con nombre: sin <aside> alrededor (un punto de referencia de más). -->
    <div class="lateral"><BarraLateral plegable /></div>

    <header class="movil">
      <button class="icon-btn" bind:this={botonMenu} aria-label="Abrir el menú" aria-expanded={cajon} aria-controls={cajon ? "cajon" : undefined} onclick={() => (cajon = true)}><Menu size={20} /></button>
      {#if page.url.pathname === "/todos"}<a class="marca" href="/todos"><Logo size={22} /><span>Todos los clientes</span></a>{:else}<a class="marca" href="/">{#if actual.cliente}<MarcaCliente nombre={actual.cliente.nombre} marca={actual.cliente.marca} tam={24} />{:else}<Logo size={22} />{/if}<span>{actual.cliente?.nombre ?? "Resguardo"}</span></a>{/if}
      <button class="icon-btn buscar-movil" aria-label="Buscar o ir a…" onclick={() => (atajos.paleta = true)}><Search size={18} /></button>
      <span class="en-marcha-movil"><CopiasEnMarcha compacto /></span>
    </header>

    {#if cajon}
      <div class="velo" role="presentation" onclick={() => (cajon = false)} transition:fade={{ duration: dur(150) }}></div>
      <!-- En móvil, el menú es un diálogo: el foco entra, se queda dentro y vuelve al botón al cerrar. -->
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div id="cajon" class="cajon" role="dialog" aria-modal="true" aria-label="Menú" tabindex="-1" use:atrapar onkeydown={trampa} transition:fly={{ x: -280, duration: dur(200) }}>
        <BarraLateral alNavegar={() => (cajon = false)} />
      </div>
    {/if}

    <main id="contenido" class="contenido" bind:this={principal} tabindex="-1">
      <SinConexion />
      {@render children()}
    </main>
  </div>
  <Paleta />
  <AccionesGlobales />
  <AyudaAtajos />
{/if}

<svelte:window
  onkeydown={(e) => {
    if (cajon && e.key === "Escape") cajon = false;
    else if (app.cuenta) alPulsar(e);
  }}
/>

<style>
  .marco {
    display: grid;
    grid-template-columns: var(--sidebar-w) minmax(0, 1fr);
    min-height: 100dvh;
    transition: grid-template-columns var(--dur) var(--ease);
  }
  /* Plegada a iconos (lib/barra.svelte.ts). */
  .marco.plegada {
    grid-template-columns: var(--sidebar-plegada) minmax(0, 1fr);
  }
  .lateral {
    position: sticky;
    top: 0;
    height: 100dvh;
  }
  .contenido {
    min-width: 0;
    outline: none;
  }
  .movil {
    display: none;
  }
  .velo {
    position: fixed;
    inset: 0;
    z-index: 30;
    background: var(--overlay);
  }
  .cajon {
    position: fixed;
    top: 0;
    bottom: 0;
    left: 0;
    z-index: 31;
    width: min(280px, 86vw);
    box-shadow: var(--shadow-lg);
  }
  .cajon:focus {
    outline: none;
  }
  @media (max-width: 1100px) {
    .marco:not(.plegada) {
      grid-template-columns: 232px minmax(0, 1fr);
    }
  }
  @media (max-width: 860px) {
    .marco {
      display: block;
    }
    .lateral {
      display: none;
    }
    .movil {
      position: sticky;
      top: 0;
      z-index: 5;
      display: flex;
      align-items: center;
      gap: 8px;
      height: 52px;
      padding: 0 var(--sp-3);
      background: color-mix(in srgb, var(--bg) 88%, transparent);
      backdrop-filter: blur(8px);
      border-bottom: 1px solid var(--border);
    }
    .marca {
      display: flex;
      align-items: center;
      gap: 8px;
      min-width: 0;
      font-weight: 600;
      color: var(--text-1);
      text-decoration: none;
    }
    .en-marcha-movil {
      margin-left: auto;
      flex: none;
    }
    .marca span {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .buscar-movil {
      margin-left: auto;
      width: 36px;
      height: 36px;
    }
  }
</style>
