<script lang="ts" module>
  import type { FiltroVersiones } from "$lib/tipos";
  export interface PedidoRepoNuevo {
    /** Traer las versiones de otro repositorio de este equipo (y cuáles). */
    origen?: { repo: string; nombre: string; filtro: FiltroVersiones | null };
    /** En una nube de este equipo (Avanzado): su nombre. */
    nube?: string;
  }
</script>

<script lang="ts">
  // «Dónde» de una copia (plan 0.7.26, bloque 3): los repositorios del equipo
  // agrupados por destino, con el icono de su tipo; «+ Nuevo repositorio»
  // (vacío o trayendo las versiones de otro, todas o algunas) y, en
  // «Avanzado», las nubes del propio equipo con su aviso.
  import { untrack } from "svelte";
  import { Cloud, Database, History, Plus, TriangleAlert } from "@lucide/svelte";
  import { tip } from "$lib/tooltip";
  import TipoDestino from "$lib/componentes/TipoDestino.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import { errorFiltro, filtroParaOrden, pasosDelRepo, usoNubeDirecta } from "$lib/cadenas";
  import { tipoDePaso } from "$lib/regla321";
  import { lista } from "$lib/formato";
  import type { TipoCorto } from "$lib/tipoDestino";
  import type { CopiaConfig, DestinoCatalogo, Equipo } from "$lib/tipos";

  let {
    id,
    repos,
    equipo,
    equipos,
    catalogo,
    copias,
    copia,
    value = $bindable(),
    admiteFiltros,
    onNuevo,
  }: {
    id: string;
    /** Los repositorios en los que se puede copiar (sin los importados). */
    repos: { id: string; nombre: string; destino: string }[];
    equipo: Equipo;
    equipos: Equipo[];
    catalogo: DestinoCatalogo[];
    copias: CopiaConfig[];
    /** La copia que elige (para «También lo usa…»). */
    copia: string;
    value: string;
    admiteFiltros: boolean;
    onNuevo: (p: PedidoRepoNuevo) => void;
  } = $props();

  interface Opcion {
    id: string;
    nombre: string;
    destino: string;
    tipo: TipoCorto | null;
    nube: boolean;
    tambien: string[];
  }
  const opciones = $derived.by<Opcion[]>(() =>
    repos.map((r) => {
      const p0 = pasosDelRepo(equipo, r.id, equipos)[0];
      const d = equipo.resumen?.destinos?.find((x) => x.id === r.destino || x.nombre === r.destino);
      return {
        id: r.id,
        nombre: r.nombre,
        destino: p0?.texto ?? d?.nombre ?? "Se crea al guardar",
        tipo: p0 ? tipoDePaso(p0, equipos, catalogo) : null,
        nube: d?.tipo === "nube",
        tambien: copias.filter((k) => k.id !== copia && k.repo === r.id).map((k) => k.nombre),
      };
    }),
  );
  /** Agrupados por destino (los de una nube de este equipo, aparte: «Avanzado»). */
  const grupos = $derived.by(() => {
    const m = new Map<string, Opcion[]>();
    for (const o of opciones.filter((x) => !x.nube)) m.set(o.destino, [...(m.get(o.destino) ?? []), o]);
    return [...m.entries()].map(([destino, l]) => ({ destino, tipo: l[0].tipo, repos: l }));
  });
  const enNube = $derived(opciones.filter((x) => x.nube));
  const nubes = $derived(equipo.resumen?.nubes ?? []);
  let avanzado = $state(untrack(() => opciones.find((o) => o.id === value)?.nube ?? false));

  // «+ Nuevo repositorio»: vacío o con las versiones de otro.
  let nuevo = $state(false);
  let conVersiones = $state(false);
  let origen = $state("");
  let algunas = $state(false);
  let f = $state({ etiquetas: "", ultimos_dias: "" as number | string, desde: "" });
  const conOrigen = $derived((equipo.resumen?.repositorios ?? []).filter((r) => (r.versiones ?? 1) > 0));
  const filtro = $derived(algunas ? filtroParaOrden(admiteFiltros ? f : { etiquetas: f.etiquetas }) : null);
  const errorF = $derived(algunas && admiteFiltros ? errorFiltro(f) : null);
  const listoNuevo = $derived(!conVersiones || (!!origen && !errorF && (!algunas || !!filtro)));
  function seguir() {
    const o = conOrigen.find((r) => r.id === origen);
    onNuevo(conVersiones && o ? { origen: { repo: o.id, nombre: o.nombre, filtro } } : {});
    nuevo = false;
  }
</script>

{#snippet fila(o: Opcion)}
  <label class="fila" class:on={value === o.id}>
    <input type="radio" name="repo-{id}" value={o.id} checked={value === o.id} onchange={() => (value = o.id)} />
    <Database size={15} />
    <span class="nombre">{o.nombre}</span>
    {#if o.tambien.length}<span class="faint tambien">También {lista(o.tambien.map((x) => `«${x}»`))}</span>{/if}
  </label>
{/snippet}

<div class="elegir-repo">
  {#if grupos.length}
    <div class="grupos" role="radiogroup" aria-label="Repositorio">
      {#each grupos as g (g.destino)}
        <div class="grupo">
          <p class="destino">{#if g.tipo}<TipoDestino {...g.tipo} soloIcono />{/if}<span>{g.destino}</span></p>
          {#each g.repos as o (o.id)}{@render fila(o)}{/each}
        </div>
      {/each}
    </div>
  {:else if !enNube.length}
    <p class="faint vacio">Este equipo aún no tiene repositorios.</p>
  {/if}

  {#if !nuevo}
    <button type="button" class="btn btn-sm nuevo" onclick={() => (nuevo = true)}><Plus size={14} />Nuevo repositorio</button>
  {:else}
    <div class="panel-nuevo">
      <div class="segmented inline" role="radiogroup" aria-label="Cómo empieza el repositorio nuevo">
        <button type="button" role="radio" aria-checked={!conVersiones} class:on={!conVersiones} onclick={() => (conVersiones = false)}>Vacío</button>
        <button type="button" role="radio" aria-checked={conVersiones} class:on={conVersiones} disabled={!conOrigen.length} use:tip={conOrigen.length ? "Trae las versiones de otro repositorio de este equipo" : "No hay otro repositorio con versiones"} onclick={() => (conVersiones = true)}><History size={14} />Con versiones de otro</button>
      </div>
      {#if conVersiones}
        <div class="field">
          <label class="field-label" for="nr-origen-{id}">De</label>
          <select id="nr-origen-{id}" class="input" bind:value={origen}>
            <option value="" disabled>Elige un repositorio</option>
            {#each conOrigen as r (r.id)}<option value={r.id}>{r.nombre}</option>{/each}
          </select>
        </div>
        <div class="segmented inline" role="radiogroup" aria-label="Qué versiones traer">
          <button type="button" role="radio" aria-checked={!algunas} class:on={!algunas} onclick={() => (algunas = false)}>Todas</button>
          <button type="button" role="radio" aria-checked={algunas} class:on={algunas} onclick={() => (algunas = true)}>Solo algunas</button>
        </div>
        {#if algunas}
          <div class="filtro">
            <div class="field">
              <label class="field-label" for="nr-et-{id}">Etiqueta</label>
              <input id="nr-et-{id}" class="input" bind:value={f.etiquetas} placeholder="diaria, semanal" />
            </div>
            {#if admiteFiltros}
              <div class="field">
                <label class="field-label" for="nr-dias-{id}">Últimos días</label>
                <input id="nr-dias-{id}" class="input num" type="number" min="1" max="3650" bind:value={f.ultimos_dias} />
              </div>
              <div class="field">
                <label class="field-label" for="nr-desde-{id}">Desde</label>
                <input id="nr-desde-{id}" class="input" type="date" bind:value={f.desde} />
              </div>
            {/if}
          </div>
          {#if errorF}<p class="error-campo">{errorF}</p>{/if}
        {/if}
      {/if}
      <div class="acciones">
        <button type="button" class="btn btn-ghost btn-sm" onclick={() => (nuevo = false)}>Cancelar</button>
        <button type="button" class="btn btn-primary btn-sm" disabled={!listoNuevo} onclick={seguir}>Destino y contraseña…</button>
      </div>
    </div>
  {/if}

  <details class="avanzado" bind:open={avanzado}>
    <summary>Avanzado: nubes de {equipo.nombre}</summary>
    <div class="notice notice-warn"><TriangleAlert size={16} /><p>Este equipo guardará la credencial de la nube y podrá borrar en ella. <Ayuda id="nube-propia" /></p></div>
    {#if enNube.length}
      <div class="grupo" role="radiogroup" aria-label="Repositorios en una nube de {equipo.nombre}">
        {#each enNube as o (o.id)}{@render fila(o)}{/each}
      </div>
    {/if}
    {#each nubes as n (n.nombre)}
      {@const uso = usoNubeDirecta({ equipo, nombre: n.nombre, tipo: n.tipo }, equipo)}
      <button type="button" class="btn btn-sm nube" disabled={!uso.ok} use:tip={uso.motivo ?? "Un repositorio nuevo en esta nube"} onclick={() => onNuevo({ nube: n.nombre })}><Cloud size={14} />Nuevo repositorio en {n.nombre}</button>
    {:else}
      <p class="faint vacio">No hay nubes conectadas en {equipo.nombre}.</p>
    {/each}
  </details>
</div>

<style>
  .elegir-repo {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .grupos {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .grupo {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .destino {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 0 2px;
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-2);
  }
  .fila {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
    min-height: 40px;
    padding: 6px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    cursor: pointer;
  }
  .fila:hover {
    background: var(--surface-2);
  }
  .fila.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .fila:focus-within {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .fila :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .nombre {
    font-weight: 550;
  }
  .tambien {
    font-size: var(--fs-xs);
  }
  .vacio {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .nuevo,
  .nube {
    align-self: flex-start;
  }
  .panel-nuevo {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-3);
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
  }
  .filtro {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: var(--sp-3);
  }
  .acciones {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .avanzado {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .avanzado[open] {
    display: flex;
  }
  .avanzado > summary {
    font-size: var(--fs-sm);
    color: var(--text-2);
    cursor: pointer;
  }
  .avanzado[open] > :not(summary) {
    margin-top: var(--sp-2);
  }
</style>
