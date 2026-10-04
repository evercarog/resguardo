<script lang="ts">
  import { tip } from "$lib/tooltip";
  // «Antes de copiar» de una copia (ganchos de plantilla, v1.10). Solo dos
  // plantillas cerradas, dichas en llano; cada campo se comprueba con las
  // reglas del agente y el error sale al momento. Un agente anterior a 0.7.2
  // rechazaría la configuración: entonces solo se explica qué hacer.
  import { CircleAlert, Database, FolderClock, FolderOpen, Plus, Trash2 } from "@lucide/svelte";
  import type { Cliente, Equipo, Gancho } from "$lib/tipos";
  import { errorCarpetaDestino, errorGancho, MAX_BASES, MAX_GANCHOS, VERSION_GANCHOS } from "$lib/ganchos";
  import Ayuda from "./Ayuda.svelte";
  import ElegirCarpetas from "./ElegirCarpetas.svelte";

  let {
    ganchos = $bindable(),
    admite,
    version,
    id,
    cliente,
    equipo,
    prueba,
  }: {
    ganchos: Gancho[];
    admite: boolean;
    version: string | null;
    id: string;
    /** Con el equipo y la prueba de administración, «Explorar…» elige la carpeta en el propio equipo. */
    cliente?: Cliente;
    equipo?: Equipo;
    prueba?: Uint8Array;
  } = $props();
  /** El paso cuya carpeta se está eligiendo. */
  let explorando = $state<number | null>(null);
  const win = $derived(/windows/i.test(equipo?.so ?? "windows"));

  function anadir(tipo: Gancho["tipo"]) {
    ganchos = [...ganchos, tipo === "sqlserver" ? { tipo, instancia: ".", bases: [""], carpeta: "C:\\ResguardoVolcados" } : { tipo, carpeta: "", horas: 26, extension: null }];
  }
  const quitar = (i: number) => (ganchos = ganchos.filter((_, j) => j !== i));
  const bases = (t: string) =>
    t
      .split(/[\n,;]+/)
      .map((x) => x.trim())
      .filter(Boolean);
</script>

<div class="ganchos">
  <div class="cab">
    <span class="field-label">Antes de copiar <span class="faint opc">(opcional)</span></span>
    <Ayuda id="ganchos" />
  </div>

  {#if !admite}
    <p class="faint nota">
      Volcar bases de datos de SQL Server o vigilar las copias propias de una aplicación necesita el agente {VERSION_GANCHOS} o posterior{version ? ` (este equipo tiene la ${version})` : ""}. Actualiza el agente para usar esto.
    </p>
  {:else}
    {#each ganchos as g, i (i)}
      {@const err = errorGancho(g)}
      <div class="gancho" class:mal={!!err}>
        <div class="g-cab">
          {#if g.tipo === "sqlserver"}<Database size={16} /><strong>Volcar bases de datos de SQL Server antes de copiar</strong>
          {:else}<FolderClock size={16} /><strong>Avisar si las copias propias de una aplicación no están al día</strong>{/if}
          <button type="button" class="icon-btn" aria-label="Quitar este paso" use:tip={"Quitar"} onclick={() => quitar(i)}><Trash2 size={14} /></button>
        </div>
        {#if g.tipo === "sqlserver"}
          <p class="faint expl">Cada base se vuelca (copia «COPY_ONLY», que no estorba a las copias propias de SQL Server) en la carpeta, se copia con lo demás y el volcado se borra al terminar. Si un volcado falla, la copia sale como fallida.</p>
          <div class="fila-campos">
            <div class="field">
              <label class="field-label" for="g-bases-{id}-{i}">Bases de datos</label>
              <textarea id="g-bases-{id}-{i}" class="input mono" rows="2" spellcheck="false" value={g.bases.join("\n")} oninput={(e) => (g.bases = bases(e.currentTarget.value))} placeholder="WO_Empresa&#10;Contabilidad 2026"></textarea>
              <span class="field-hint">Una por línea, hasta {MAX_BASES}.</span>
            </div>
            <div class="field">
              <label class="field-label" for="g-inst-{id}-{i}">Instancia</label>
              <input id="g-inst-{id}-{i}" class="input mono" bind:value={g.instancia} placeholder="." spellcheck="false" />
              <span class="field-hint">«.» es la predeterminada; si no, «EQUIPO\SQLEXPRESS» o «EQUIPO,1433».</span>
            </div>
          </div>
          <div class="field">
            <label class="field-label" for="g-carp-{id}-{i}">Carpeta para los volcados</label>
            <div class="con-boton">
              <input id="g-carp-{id}-{i}" class="input mono" bind:value={g.carpeta} spellcheck="false" />
              {#if equipo && cliente}<button type="button" class="btn" onclick={() => (explorando = i)}><FolderOpen size={15} />Explorar…</button>{/if}
            </div>
            <span class="field-hint">En el propio equipo, con sitio para los volcados. La cuenta del equipo necesita permiso de copia en cada base: <a href="/ayuda#ganchos">cómo darlo</a>.</span>
          </div>
        {:else}
          <p class="faint expl">Para programas que hacen sus propias copias (por ejemplo, World Office): si el archivo más nuevo de su carpeta es más viejo que lo indicado, la copia sale «con avisos». No cambia lo que se copia.</p>
          <div class="fila-campos">
            <div class="field">
              <label class="field-label" for="g-carp-{id}-{i}">Carpeta de sus copias</label>
              <div class="con-boton">
                <input id="g-carp-{id}-{i}" class="input mono" bind:value={g.carpeta} placeholder="D:\WO\Copias" spellcheck="false" />
                {#if equipo && cliente}<button type="button" class="btn" onclick={() => (explorando = i)}><FolderOpen size={15} />Explorar…</button>{/if}
              </div>
            </div>
            <div class="field corto2">
              <label class="field-label" for="g-horas-{id}-{i}">Avisar tras</label>
              <div class="con-unidad"><input id="g-horas-{id}-{i}" class="input num" type="number" min="1" max="720" bind:value={g.horas} /><span class="faint">horas</span></div>
            </div>
            <div class="field corto2">
              <label class="field-label" for="g-ext-{id}-{i}">Solo archivos</label>
              <input
                id="g-ext-{id}-{i}"
                class="input mono"
                value={g.extension ?? ""}
                oninput={(e) => (g.extension = e.currentTarget.value.trim() || null)}
                placeholder=".bak (opcional)"
                spellcheck="false"
              />
            </div>
          </div>
        {/if}
        {#if err}<p class="error-campo" role="alert"><CircleAlert size={14} />{err}</p>{/if}
      </div>
    {/each}
    {#if ganchos.length < MAX_GANCHOS}
      <div class="anadir">
        <button type="button" class="btn btn-sm btn-ghost" onclick={() => anadir("sqlserver")}><Plus size={14} />Volcar bases de datos de SQL Server antes de copiar</button>
        <button type="button" class="btn btn-sm btn-ghost" onclick={() => anadir("carpeta_reciente")}><Plus size={14} />Avisar si la carpeta de copias de una aplicación lleva más de N horas sin archivos nuevos</button>
      </div>
    {/if}
  {/if}
</div>

{#if explorando !== null && ganchos[explorando] && equipo && cliente}
  {@const g = ganchos[explorando]}
  <ElegirCarpetas
    {cliente}
    {equipo}
    {prueba}
    unica
    titulo={g.tipo === "sqlserver" ? `Carpeta para los volcados en ${equipo.nombre}` : `Carpeta de las copias de la aplicación en ${equipo.nombre}`}
    iniciales={g.carpeta.trim() ? [g.carpeta.trim()] : []}
    validar={g.tipo === "sqlserver" ? (r) => errorCarpetaDestino(r, win) : undefined}
    onclose={() => (explorando = null)}
    alElegir={(rutas) => {
      if (explorando !== null && ganchos[explorando] && rutas[0]) ganchos[explorando].carpeta = rutas[0];
      explorando = null;
    }}
  />
{/if}

<style>
  .ganchos {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .cab {
    display: flex;
    align-items: center;
  }
  .opc {
    font-weight: 400;
  }
  .nota {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .gancho {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-4);
    background: var(--surface-2);
    border: 1px solid transparent;
    border-radius: var(--radius);
  }
  .gancho.mal {
    border-color: color-mix(in srgb, var(--bad) 40%, transparent);
  }
  .g-cab {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .g-cab strong {
    flex: 1;
    font-weight: 500;
  }
  .expl {
    margin: 0;
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
  }
  .fila-campos {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 180px), 1fr));
    gap: var(--sp-3);
  }
  .con-unidad {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .con-unidad .input {
    width: 90px;
  }
  .error-campo {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
  }
  .anadir {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .anadir .btn {
    height: auto;
    min-height: 28px;
    white-space: normal;
    text-align: left;
  }
</style>
