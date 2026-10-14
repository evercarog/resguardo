<script lang="ts">
  // Plan 0.8.0, D4: el filtro de cliente de las secciones de «Todos los clientes»
  // (y la búsqueda). Se recuerda en el navegador por sección.
  import { Search } from "@lucide/svelte";
  import { todos } from "$lib/todos.svelte";
  import { guardar, leerTexto } from "$lib/recordar";

  let { seccion, cliente = $bindable(""), buscar = $bindable(""), placeholder = "Buscar" }: { seccion: string; cliente?: string; buscar?: string; placeholder?: string } = $props();

  // Lo recordado, una vez (sin seguir lo que se cambia aquí).
  let leido = false;
  $effect.pre(() => {
    if (leido || !todos.cargado) return;
    leido = true;
    const v = leerTexto(`todos.${seccion}.cliente`);
    if (v && todos.clientes.some((c) => c.id === v)) cliente = v;
  });
  function poner(v: string) {
    cliente = v;
    guardar(`todos.${seccion}.cliente`, v);
  }
</script>

<div class="barra">
  <label class="buscar">
    <Search size={15} />
    <input class="input" type="search" {placeholder} bind:value={buscar} aria-label={placeholder} />
  </label>
  {#if todos.clientes.length > 1}
    <select class="input cliente" value={cliente} aria-label="Filtrar por cliente" onchange={(e) => poner(e.currentTarget.value)}>
      <option value="">Todos los clientes</option>
      {#each todos.clientes as c (c.id)}<option value={c.id}>{c.nombre}</option>{/each}
    </select>
  {/if}
</div>

<style>
  .barra {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3);
  }
  .buscar {
    position: relative;
    flex: 1;
    min-width: 200px;
  }
  .buscar :global(svg) {
    position: absolute;
    top: 10px;
    left: 11px;
    color: var(--text-3);
  }
  .buscar .input {
    padding-left: 34px;
  }
  .cliente {
    width: auto;
    max-width: 100%;
  }
</style>
