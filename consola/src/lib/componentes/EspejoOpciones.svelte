<script lang="ts">
  // Opciones de un destino del espejo (docs/espejo.md), para un almacén con
  // `admite: "espejo_flexible"`: cuándo (el mismo editor que las copias),
  // «después de cada copia nueva» y qué repositorios van a él.
  import EditorHorario from "./EditorHorario.svelte";
  import type { Horario } from "$lib/tipos";

  let {
    id,
    horario = $bindable(),
    trasCopia = $bindable(),
    todos = $bindable(),
    elegidos = $bindable(),
    repositorios,
    nombre = (r: string) => r,
    verificarPct = $bindable(),
    nube = false,
  }: {
    id: string;
    horario: Horario;
    trasCopia: boolean;
    /** Todos los repositorios del almacén (también los que lleguen después). */
    todos: boolean;
    /** Con `todos` apagado: los que van a este destino. */
    elegidos: string[];
    /** Los repositorios que guarda el almacén (`<usuario>/<repo>`). */
    repositorios: string[];
    /** Cómo se llama un repositorio en la consola («Contabilidad, de RECEPCION»). */
    nombre?: (r: string) => string;
    /** §3d: % de lo que hay en el destino que se comprueba cada día. */
    verificarPct: number;
    /** Es una nube (comprobar es descargar). */
    nube?: boolean;
  } = $props();
  const PORCENTAJES = [0, 1, 2, 5, 10, 25, 50, 100];

  // Los elegidos que ya no están en el almacén también se ven (para poder quitarlos).
  const lista = $derived([...new Set([...repositorios, ...elegidos])].sort());
  function alternar(r: string, si: boolean) {
    elegidos = si ? [...new Set([...elegidos, r])] : elegidos.filter((x) => x !== r);
  }
</script>

<fieldset class="grupo">
  <legend class="field-label">Cuándo</legend>
  <EditorHorario id="{id}-horario" bind:horario admiteReglas para="verificacion" />
  <label class="switch-row">
    <input type="checkbox" bind:checked={trasCopia} />
    <span>También después de cada copia nueva<span class="faint">Cuando llega una versión nueva a este equipo, el espejo empieza unos minutos después (si llegan varias seguidas, espera a la última).</span></span>
  </label>
</fieldset>

<fieldset class="grupo">
  <legend class="field-label">Qué repositorios</legend>
  <div class="segmented" role="group" aria-label="Qué repositorios van a este destino">
    <button type="button" class:on={todos} aria-pressed={todos} onclick={() => (todos = true)}>Todos</button>
    <button type="button" class:on={!todos} aria-pressed={!todos} onclick={() => (todos = false)}>Solo algunos</button>
  </div>
  {#if todos}
    <p class="faint">Todo lo que guarda este equipo, también los repositorios que lleguen después.</p>
  {:else if !lista.length}
    <p class="faint">Este equipo aún no guarda ningún repositorio.</p>
  {:else}
    <ul class="repos" aria-label="Repositorios del almacén">
      {#each lista as r (r)}
        <li>
          <label class="check">
            <input type="checkbox" checked={elegidos.includes(r)} onchange={(e) => alternar(r, e.currentTarget.checked)} />
            <span>{nombre(r)}{#if nombre(r) !== r}<span class="faint mono"> {r}</span>{/if}{#if !repositorios.includes(r)}<span class="faint"> · ya no está en el almacén</span>{/if}</span>
          </label>
        </li>
      {/each}
    </ul>
    {#if !elegidos.length}<p class="error-campo">Elige al menos uno (o todos).</p>{:else}<p class="faint">Los repositorios que lleguen después no entran solos: la consola te preguntará.</p>{/if}
  {/if}
</fieldset>

<fieldset class="grupo">
  <legend class="field-label">Comprobar el espejo</legend>
  <div class="field">
    <label class="field-label" for="{id}-verif">Cada día, comprobar</label>
    <select id="{id}-verif" class="input" bind:value={verificarPct}>
      {#each PORCENTAJES as p (p)}<option value={p}>{p === 0 ? "Nada" : p === 100 ? "Todo lo que hay en él" : `El ${p} % de lo que hay en él`}</option>{/each}
    </select>
    <span class="field-hint">
      {#if nube}Para comprobarlo hay que descargarlo: en una nube cuenta como bajada.{:else}Se lee y se compara con su huella; lo que esté mal se vuelve a copiar del almacén.{/if}
      Antes de copiar, cada archivo del almacén se comprueba siempre: uno dañado no se copia y se avisa.
    </span>
  </div>
</fieldset>

<style>
  .grupo {
    border: 0;
    padding: 0;
    margin: 0;
    display: grid;
    gap: 0.6rem;
    min-width: 0;
  }
  .grupo legend {
    padding: 0;
    margin-bottom: 0.4rem;
  }
  .repos {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 0.35rem;
    max-height: 14rem;
    overflow: auto;
  }
  .check {
    display: flex;
    gap: 0.5rem;
    align-items: flex-start;
    overflow-wrap: anywhere;
  }
  .check input {
    margin-top: 0.2rem;
    flex: none;
  }
</style>
