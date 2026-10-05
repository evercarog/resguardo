<script lang="ts">
  // El mapa de la protección de todos los clientes: el de siempre
  // (MapaProteccion, lib/mapa.ts) con una columna más, los clientes, que se
  // pliegan y despliegan. Con muchos clientes, los que van bien empiezan
  // plegados. Filtro por cliente y por estado, y buscar por nombre (clientes,
  // equipos, repositorios, destinos). Lo que cada persona pliega se recuerda
  // en este navegador.
  import { ChevronsDownUp, ChevronsUpDown, Search } from "@lucide/svelte";
  import MapaProteccion from "../mapa/MapaProteccion.svelte";
  import { construirMapaGlobal, plegadosPorDefecto, type FiltroEstado, type PanelCliente } from "$lib/global";
  import { enMarchaEn, tareasTodos } from "$lib/todos.svelte";
  import { guardar as guardarTexto, leerTexto } from "$lib/recordar";

  let { clientes, ahora }: { clientes: PanelCliente[]; ahora: number } = $props();

  const CLAVE = "todos.mapa";
  /** Lo que la persona eligió (id → plegado); lo demás, lo de por defecto. */
  let elegidos = $state<Record<string, boolean>>({});
  let cliente = $state("");
  let estado = $state<FiltroEstado>("todos");
  let buscar = $state("");
  $effect(() => {
    let x: { p?: Record<string, boolean>; c?: string; e?: FiltroEstado } | null = null;
    try {
      x = JSON.parse(leerTexto(CLAVE) || "null");
    } catch {
      /* lo de siempre */
    }
    if (x?.p) elegidos = x.p;
    if (x?.c) cliente = x.c;
    if (x?.e) estado = x.e;
  });
  const guardar = () => guardarTexto(CLAVE, JSON.stringify({ p: elegidos, c: cliente, e: estado }));

  // Lo de por defecto se decide con la primera carga (no cambia solo mientras se mira).
  let porDefecto = $state<Set<string> | null>(null);
  $effect(() => {
    if (!porDefecto && clientes.length) porDefecto = plegadosPorDefecto(clientes, enMarchaEn, ahora);
  });
  const plegados = $derived(new Set(clientes.map((c) => c.id).filter((id) => elegidos[id] ?? porDefecto?.has(id) ?? false)));
  function alPlegar(id: string) {
    elegidos = { ...elegidos, [id]: !plegados.has(id) };
    guardar();
  }
  function todosA(plegar: boolean) {
    elegidos = Object.fromEntries(clientes.map((c) => [c.id, plegar]));
    guardar();
  }

  function enVivo(e: string, r: string, tipo: "copia" | "copia_externa"): string | null {
    const t = tareasTodos(e, { repo: r, tipos: [tipo] })[0];
    if (!t) return null;
    const p = t.porcentaje != null ? Math.floor(Math.max(0, Math.min(1, t.porcentaje)) * 100) : null;
    return `${tipo === "copia" ? "Copiando" : "Subiendo"}${p != null ? ` ${p} %` : "…"}`;
  }
  const clienteValido = $derived(clientes.some((c) => c.id === cliente) ? cliente : "");
  const mapa = $derived(construirMapaGlobal(clientes, { ahora, plegados, cliente: clienteValido, estado, buscar, enVivo }));
  const ESTADOS: { id: FiltroEstado; texto: string }[] = [
    { id: "todos", texto: "Todos" },
    { id: "problemas", texto: "Con problemas" },
    { id: "al_dia", texto: "Al día" },
  ];
  const hayPlegados = $derived(mapa.clientes.some((x) => x.plegado));
  const vacio = $derived(
    buscar.trim() ? `Nada se llama «${buscar.trim()}» en ${clienteValido ? "este cliente" : "tus clientes"}.` : estado === "problemas" ? "Ningún cliente tiene problemas ahora." : estado === "al_dia" ? "Ningún cliente está del todo al día." : "Todavía no hay copias que dibujar.",
  );
</script>

{#snippet herramientas()}
  <label class="buscar">
    <Search size={14} aria-hidden="true" />
    <span class="sr-only">Buscar en el mapa</span>
    <input type="search" placeholder="Buscar equipo, repositorio…" bind:value={buscar} />
  </label>
  <label class="sel">
    <span class="sr-only">Cliente</span>
    <select bind:value={cliente} onchange={guardar}>
      <option value="">Todos los clientes</option>
      {#each clientes as c (c.id)}<option value={c.id}>{c.nombre}</option>{/each}
    </select>
    <ChevronsUpDown size={14} aria-hidden="true" />
  </label>
  <div class="segmentos" role="group" aria-label="Estado">
    {#each ESTADOS as e (e.id)}
      <button type="button" aria-pressed={estado === e.id} onclick={() => ((estado = e.id), guardar())}>{e.texto}</button>
    {/each}
  </div>
  {#if mapa.clientes.length > 1}
    <button type="button" class="btn btn-sm btn-ghost" onclick={() => todosA(!hayPlegados)}>
      {#if hayPlegados}<ChevronsUpDown size={14} />Desplegar todos{:else}<ChevronsDownUp size={14} />Plegar todos{/if}
    </button>
  {/if}
{/snippet}

<MapaProteccion equipos={[]} informes={{}} cliente="todos" {ahora} titulo="Mapa de la protección" dado={mapa} {herramientas} {alPlegar} listaDesde={820} {vacio} />
{#if mapa.fuera && mapa.nodos.length}
  <p class="faint fuera">{mapa.fuera === 1 ? "1 cliente no sale" : `${mapa.fuera} clientes no salen`} con este filtro.</p>
{/if}

<style>
  .buscar {
    position: relative;
    display: inline-flex;
    align-items: center;
  }
  .buscar :global(svg) {
    position: absolute;
    left: 9px;
    color: var(--text-3);
    pointer-events: none;
  }
  .buscar input {
    width: 210px;
    min-height: 30px;
    padding: 0 10px 0 30px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-1);
    background: var(--surface);
    border: 1px solid var(--border-input);
    border-radius: var(--radius);
  }
  .sel {
    position: relative;
    display: inline-flex;
    align-items: center;
  }
  .sel select {
    min-height: 30px;
    max-width: 200px;
    padding: 0 28px 0 10px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-1);
    background: var(--surface);
    border: 1px solid var(--border-input);
    border-radius: var(--radius);
    appearance: none;
    cursor: pointer;
  }
  .sel :global(svg) {
    position: absolute;
    right: 8px;
    color: var(--text-3);
    pointer-events: none;
  }
  .segmentos {
    display: inline-flex;
    padding: 2px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .segmentos button {
    min-height: 26px;
    padding: 0 10px;
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 500;
    color: var(--text-2);
    background: none;
    border: 0;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .segmentos button[aria-pressed="true"] {
    color: var(--text-1);
    background: var(--surface);
    box-shadow: var(--shadow-sm), 0 0 0 1px var(--border);
  }
  .fuera {
    margin: calc(-1 * var(--sp-3)) 0 0;
    font-size: var(--fs-xs);
  }
  @media (max-width: 640px) {
    .buscar,
    .buscar input {
      width: 100%;
    }
    .buscar {
      flex: 1 1 100%;
    }
  }
</style>
