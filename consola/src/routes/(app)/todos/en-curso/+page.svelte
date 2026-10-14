<script lang="ts">
  // «Todos los clientes» → En curso (plan 0.8.0, D4, 14xx; 13f, 14p): lo que está en
  // marcha ahora en todos los clientes de la cuenta (copias, espejos, restauraciones,
  // verificaciones…), casi en vivo, con su barra, sus cifras y sus ondas. Lo da el
  // panel (GET /api/panel/progreso, cada 3 s mientras hay algo).
  import { Activity } from "@lucide/svelte";
  import { plural } from "$lib/formato";
  import { todos } from "$lib/todos.svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import EnMarchaTodos from "$lib/componentes/todos/EnMarchaTodos.svelte";
  import FiltroCliente from "$lib/componentes/todos/FiltroCliente.svelte";

  let cliente = $state("");
  let buscar = $state("");
  const NOMBRES: Record<string, [string, string]> = {
    copia: ["copia", "copias"],
    restauracion: ["restauración", "restauraciones"],
    verificacion: ["verificación", "verificaciones"],
    espejo: ["espejo", "espejos"],
  };
  const tareas = $derived(todos.progreso.filter((x) => !cliente || x.cliente === cliente).flatMap((x) => x.tareas));
  const resumen = $derived(
    Object.entries(
      tareas.reduce<Record<string, number>>((m, t) => {
        m[t.tipo] = (m[t.tipo] ?? 0) + 1;
        return m;
      }, {}),
    )
      .map(([t, n]) => (NOMBRES[t] ? plural(n, NOMBRES[t][0], NOMBRES[t][1]) : `${n} · ${t}`))
      .join(" · "),
  );
  const clientes = $derived(
    todos.clientes.filter((c) => (!cliente || c.id === cliente) && (!buscar.trim() || `${c.nombre} ${c.equipos.map((e) => e.nombre).join(" ")}`.toLowerCase().includes(buscar.trim().toLowerCase()))),
  );
</script>

<svelte:head><title>En curso · Todos los clientes · Resguardo Server</title></svelte:head>

<div class="page ancha">
  <CabeceraPagina titulo="En curso" icono={Activity} migas={[{ texto: "Todos los clientes", href: "/todos" }, { texto: "En curso" }]} resumen={todos.cargado ? (tareas.length ? `${resumen} en marcha` : "Nada en marcha ahora") : undefined} />
  {#if !todos.cargado}
    <Esqueleto forma="filas" n={3} etiqueta="Cargando lo que está en marcha…" />
  {:else}
    <FiltroCliente seccion="en-curso" bind:cliente bind:buscar placeholder="Buscar cliente o equipo" />
    {#if tareas.length}
      <EnMarchaTodos {clientes} />
    {:else}
      <div class="card"><Vacio icono={Activity} ilustracion="todo-en-orden" titulo="Nada en marcha ahora" texto="Cuando empiece una copia, un espejo, una restauración o una verificación en cualquier cliente, sale aquí con su progreso." /></div>
    {/if}
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
</style>
