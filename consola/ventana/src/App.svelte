<script lang="ts">
  // La ventana del agente (docs/agente-ventana.md): lo que pasa ahora (con las
  // ondas en vivo), las copias, el historial y, con la clave, los ajustes (y en
  // modo sin consola, todo lo del equipo). Lo pesado se carga al abrirlo.
  import Logo from "$ui/componentes/Logo.svelte";
  import { Activity, CalendarClock, History, Settings2 } from "@lucide/svelte";
  import { vivo } from "./puente.svelte";
  import { tono } from "./estado";
  import Ahora from "./Ahora.svelte";
  import Copias from "./Copias.svelte";
  import Historial from "./Historial.svelte";

  type Pestana = "ahora" | "copias" | "historial" | "ajustes";
  let pestana = $state<Pestana>("ahora");
  const b = $derived(vivo.datos?.bandeja ?? null);
  const t = $derived(tono(b, vivo.datos?.ahora));
  // Sin consola y sin clave: lo primero es «Usar sin consola» (en Ajustes).
  const sinConfigurar = $derived(!!b && !b.vinculado);
  const cargarAjustes = () => import("./ajustes/Ajustes.svelte");
  const PESTANAS: { id: Pestana; texto: string; icono: typeof Activity }[] = [
    { id: "ahora", texto: "Ahora", icono: Activity },
    { id: "copias", texto: "Copias", icono: CalendarClock },
    { id: "historial", texto: "Historial", icono: History },
    { id: "ajustes", texto: "Ajustes", icono: Settings2 },
  ];
</script>

<div class="marco">
  <header class="cab">
    <Logo size={30} />
    <div class="cab-texto">
      <p class="nombre">Resguardo</p>
      <p class="estado v-cortar" data-tono={t}>
        <span class="punto" aria-hidden="true"></span>{b?.text ?? "Leyendo el estado del servicio…"}
      </p>
    </div>
  </header>

  <nav class="segmented pestanas" aria-label="Secciones">
    {#each PESTANAS as p (p.id)}
      {@const Icono = p.icono}
      <button class:on={pestana === p.id} aria-current={pestana === p.id ? "page" : undefined} onclick={() => (pestana = p.id)}>
        <Icono size={15} aria-hidden="true" />{p.texto}
      </button>
    {/each}
  </nav>

  <main class="cuerpo">
    {#if pestana === "ahora"}
      <Ahora irAjustes={() => (pestana = "ajustes")} {sinConfigurar} />
    {:else if pestana === "copias"}
      <Copias />
    {:else if pestana === "historial"}
      <Historial />
    {:else}
      {#await cargarAjustes()}
        <p class="v-sub">Cargando…</p>
      {:then m}
        <m.default />
      {:catch e}
        <p class="v-error">{e.message}</p>
      {/await}
    {/if}
  </main>
</div>

<style>
  .marco {
    display: grid;
    grid-template-rows: auto auto 1fr;
    height: 100%;
    min-height: 0;
  }
  .cab {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-4) var(--sp-4) var(--sp-3);
  }
  .cab-texto {
    min-width: 0;
  }
  .nombre {
    margin: 0;
    font-weight: 650;
    font-size: var(--fs-h2);
    letter-spacing: -0.01em;
  }
  .estado {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .punto {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--neutral);
  }
  .estado[data-tono="ok"] .punto {
    background: var(--ok);
    box-shadow: 0 0 8px var(--ok);
  }
  .estado[data-tono="info"] .punto {
    background: var(--info);
    box-shadow: 0 0 8px var(--info);
    animation: latido 1.6s ease-in-out infinite;
  }
  .estado[data-tono="warn"] .punto {
    background: var(--warn);
  }
  .estado[data-tono="bad"] .punto {
    background: var(--bad);
  }
  @keyframes latido {
    50% {
      opacity: 0.35;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .estado[data-tono="info"] .punto {
      animation: none;
    }
  }
  .pestanas {
    margin: 0 var(--sp-4);
  }
  .cuerpo {
    min-height: 0;
    overflow-y: auto;
    padding: var(--sp-4);
    scrollbar-gutter: stable;
  }
</style>
