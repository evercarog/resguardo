<script lang="ts">
  import { tip } from "$lib/tooltip";
  // «Antes de copiar» de una copia (ganchos de plantilla, v1.10). Solo dos
  // plantillas cerradas, dichas en llano; cada campo se comprueba con las
  // reglas del agente y el error sale al momento. Un agente anterior a 0.7.2
  // rechazaría la configuración: entonces solo se explica qué hacer.
  import { CircleAlert, Database, FolderClock, FolderOpen, ListChecks, LoaderCircle, Plus, Trash2 } from "@lucide/svelte";
  import type { Cliente, Equipo, Gancho, GanchoSqlserver } from "$lib/tipos";
  import { admiteTodasLasBases, admiteVerificarVolcado, errorCarpetaDestino, errorGancho, MAX_BASES, MAX_EXCLUIDAS, MAX_GANCHOS, VERSION_GANCHOS } from "$lib/ganchos";
  import { elegibles, estadoBase, type BaseInstancia } from "$lib/basesSql";
  import { pedirBases } from "$lib/basesSqlPedir";
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
  /** 14ccc: el agente comprueba cada volcado (y se puede apagar) desde «verificar_volcado». */
  const conVerificar = $derived(admiteVerificarVolcado(equipo));
  /** 15u: «Todas las bases» y «Ver las bases de la instancia», con un agente que los entiende. */
  const conTodas = $derived(admiteTodasLasBases(equipo));

  // 15u: «Ver las bases de la instancia», por paso (una petición al equipo que no cambia nada).
  let listas = $state<Record<number, { cargando?: boolean; error?: string; bases?: BaseInstancia[] }>>({});
  async function verBases(i: number) {
    const g = ganchos[i];
    if (g?.tipo !== "sqlserver" || !equipo || !cliente) return;
    listas[i] = { cargando: true };
    try {
      listas[i] = { bases: await pedirBases(cliente, equipo, g.instancia ?? ".") };
    } catch (e) {
      listas[i] = { error: (e as Error).message };
    }
  }
  const igual = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();
  /** ¿Se volcará esa base con lo elegido? */
  function marcada(g: GanchoSqlserver, b: BaseInstancia): boolean {
    if (g.todas) return b.sistema ? !!g.sistema : !(g.excluir ?? []).some((x) => igual(x, b.nombre));
    return g.bases.some((x) => igual(x, b.nombre));
  }
  function alternar(g: GanchoSqlserver, b: BaseInstancia, si: boolean) {
    if (g.todas) {
      if (b.sistema) g.sistema = si || undefined;
      else g.excluir = si ? (g.excluir ?? []).filter((x) => !igual(x, b.nombre)) : [...(g.excluir ?? []), b.nombre];
    } else g.bases = si ? [...g.bases.filter(Boolean), b.nombre] : g.bases.filter((x) => !igual(x, b.nombre));
  }

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
          <p class="faint expl">Cada base se vuelca (copia «COPY_ONLY», que no estorba a las copias propias de SQL Server) en la carpeta{conVerificar && g.verificar !== false ? ", se verifica" : ""}, se copia con lo demás y el volcado se borra al terminar. Si un volcado falla, la copia sale como fallida.</p>
          {#if conTodas || g.todas}
            <!-- 15u: una lista fija o todas las de la instancia (leídas en cada copia). -->
            <div class="modo" role="radiogroup" aria-label="Qué bases se vuelcan">
              <label class="radio"><input type="radio" name="g-modo-{id}-{i}" checked={!g.todas} onchange={() => (g.todas = undefined)} /> Estas bases</label>
              <label class="radio"><input type="radio" name="g-modo-{id}-{i}" checked={!!g.todas} onchange={() => (g.todas = true)} /> Todas las bases de la instancia</label>
            </div>
          {/if}
          <div class="fila-campos">
            {#if g.todas}
              <div class="field">
                <label class="field-label" for="g-excl-{id}-{i}">Menos estas <span class="faint opc">(opcional)</span></label>
                <textarea id="g-excl-{id}-{i}" class="input mono" rows="2" spellcheck="false" value={(g.excluir ?? []).join("\n")} oninput={(e) => (g.excluir = bases(e.currentTarget.value))} placeholder="Pruebas"></textarea>
                <span class="field-hint">Una por línea, hasta {MAX_EXCLUIDAS}. Se leen de la instancia en cada copia: una base nueva entra sola. Solo las que están en línea; nunca tempdb ni model.</span>
                <label class="check"><input type="checkbox" checked={!!g.sistema} onchange={(e) => (g.sistema = e.currentTarget.checked || undefined)} /> Volcar también master y msdb</label>
              </div>
            {:else}
              <div class="field">
                <label class="field-label" for="g-bases-{id}-{i}">Bases de datos</label>
                <textarea id="g-bases-{id}-{i}" class="input mono" rows="2" spellcheck="false" value={g.bases.join("\n")} oninput={(e) => (g.bases = bases(e.currentTarget.value))} placeholder="Ventas&#10;Nominas"></textarea>
                <span class="field-hint">Una por línea, hasta {MAX_BASES}.{conTodas ? " Si la instancia tiene más bases en línea, la copia te avisa." : ""}</span>
              </div>
            {/if}
            <div class="field">
              <label class="field-label" for="g-inst-{id}-{i}">Instancia</label>
              <input id="g-inst-{id}-{i}" class="input mono" bind:value={g.instancia} placeholder="." spellcheck="false" />
              <span class="field-hint">«.» es la predeterminada; si no, «EQUIPO\SQLEXPRESS» o «EQUIPO,1433».</span>
            </div>
          </div>
          {#if conTodas && equipo && cliente}
            {@const l = listas[i]}
            <div class="ver-bases">
              <button type="button" class="btn btn-sm" disabled={!!l?.cargando} onclick={() => verBases(i)}>
                {#if l?.cargando}<LoaderCircle size={14} class="girando" />Leyendo las bases de {equipo.nombre}…{:else}<ListChecks size={14} />Ver las bases de la instancia{/if}
              </button>
              {#if l?.error}<p class="error-campo" role="alert"><CircleAlert size={14} />{l.error}</p>{/if}
              {#if l?.bases}
                {@const bs = elegibles(l.bases)}
                {#if !bs.length}
                  <p class="faint nota">La instancia no tiene bases que volcar.</p>
                {:else}
                  <ul class="lista-bases" aria-label="Bases de la instancia">
                    {#each bs as b (b.nombre)}
                      {@const fuera = b.estado !== "ONLINE"}
                      <li>
                        <label class="check">
                          <input type="checkbox" checked={marcada(g, b)} onchange={(e) => alternar(g, b, e.currentTarget.checked)} />
                          <span class="mono">{b.nombre}</span>
                          {#if b.sistema}<span class="faint"> · del sistema</span>{/if}
                          {#if fuera}<span class="aviso-base"> · {estadoBase(b.estado)}: no se volcará mientras no esté en línea</span>{/if}
                        </label>
                      </li>
                    {/each}
                  </ul>
                  <span class="field-hint">{g.todas ? "Desmarca las que no quieras volcar." : "Marca las que quieras volcar."}</span>
                {/if}
              {/if}
            </div>
          {/if}
          <div class="field">
            <label class="field-label" for="g-carp-{id}-{i}">Carpeta para los volcados</label>
            <div class="con-boton">
              <input id="g-carp-{id}-{i}" class="input mono" bind:value={g.carpeta} spellcheck="false" />
              {#if equipo && cliente}<button type="button" class="btn" onclick={() => (explorando = i)}><FolderOpen size={15} />Explorar…</button>{/if}
            </div>
            <span class="field-hint">En el propio equipo, con sitio para los volcados. La cuenta del equipo necesita dos permisos: db_backupoperator en cada base (para volcarla) y CREATE ANY DATABASE (para verificar): <a href="/ayuda#guia-ganchos">cómo darlos</a>.</span>
          </div>
          {#if conVerificar}
            <!-- 14ccc: «Más opciones» del paso; activado por defecto (sin el campo, el agente verifica). -->
            <details class="mas">
              <summary>Más opciones{g.verificar === false ? " · sin verificar los volcados" : ""}</summary>
              <label class="switch-row">
                <input type="checkbox" class="switch" checked={g.verificar !== false} onchange={(e) => (g.verificar = e.currentTarget.checked ? undefined : false)} />
                <span>Verificar cada volcado (recomendado)<span class="faint">Al terminar cada volcado, SQL Server lo relee entero y comprueba sus sumas («RESTORE VERIFYONLY»). Si no pasa, la copia sale fallida; si falta el permiso para verificar (CREATE ANY DATABASE), sale con aviso. Tarda más o menos lo que tardó el volcado. <a href="/ayuda#guia-comprobar-volcado">Comprobar los datos de un volcado</a></span></span>
              </label>
            </details>
          {/if}
        {:else}
          <p class="faint expl">Para programas que hacen sus propias copias : si el archivo más nuevo de su carpeta es más viejo que lo indicado, la copia sale «con avisos». No cambia lo que se copia.</p>
          <div class="fila-campos">
            <div class="field">
              <label class="field-label" for="g-carp-{id}-{i}">Carpeta de sus copias</label>
              <div class="con-boton">
                <input id="g-carp-{id}-{i}" class="input mono" bind:value={g.carpeta} placeholder="D:\CopiasApp" spellcheck="false" />
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
  .mas summary {
    cursor: pointer;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .mas[open] summary {
    margin-bottom: var(--sp-3);
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
  .modo {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2) var(--sp-4);
    font-size: var(--fs-sm);
  }
  .check {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .ver-bases {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-2);
  }
  .lista-bases {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 220px;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    list-style: none;
  }
  .aviso-base {
    color: var(--warn);
    font-size: var(--fs-xs);
  }
  .ver-bases :global(.girando) {
    animation: girar 1s linear infinite;
  }
  @keyframes girar {
    to {
      rotate: 360deg;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .ver-bases :global(.girando) {
      animation: none;
    }
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
