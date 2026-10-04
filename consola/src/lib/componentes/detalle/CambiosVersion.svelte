<script lang="ts">
  // «Qué cambió»: los archivos nuevos, cambiados y borrados entre una versión
  // y la anterior de la misma copia (o la que se elija), con su tamaño y cuánto
  // cambió, agrupados por carpeta, con búsqueda, filtro y orden. Los nombres
  // los lee el equipo (`restic diff`) y llegan cifrados por la sesión; antes de
  // abrirla se ven los recuentos del informe.
  import { untrack } from "svelte";
  import { ArrowDownUp, FileClock, FileMinus, FilePen, FilePlus, FileQuestion, Folder, History, LoaderCircle, Search, TriangleAlert } from "@lucide/svelte";
  import type { AccesoRepo } from "$lib/accesoRepo.svelte";
  import type { MensajeEquipo } from "$lib/sesion";
  import {
    agruparPorCarpeta,
    cuentaFiltro,
    deltaDe,
    filtrarCambios,
    FILTROS,
    partesRuta,
    prefijoComun,
    rutaLegible,
    TEXTO_FILTRO,
    TEXTO_TIPO,
    unirPaginas,
    type Cambio,
    type Diferencias,
    type FiltroCambios,
    type OrdenCambios,
    type PaginaCambios,
  } from "$lib/detalle";
  import { bytes, fechaCorta, fechaLarga, numero, plural } from "$lib/formato";
  import { tip } from "$lib/tooltip";
  import type { VersionInforme } from "$lib/tipos";
  import DesbloquearRepo from "./DesbloquearRepo.svelte";
  import RestaurarArchivo from "./RestaurarArchivo.svelte";
  import { ir } from "./navegar";
  import "./pulsable.css";

  let {
    acceso,
    nombreRepo,
    version,
    anterior,
    con,
    filtro,
    versiones,
    puedeRestaurar,
    alVerArchivo,
  }: {
    acceso: AccesoRepo;
    nombreRepo: string;
    /** La versión (del informe, o solo su id si ya no está en él). */
    version: VersionInforme | { id: string; hora?: string };
    /** La anterior de la misma copia según el informe (el equipo la busca si falta). */
    anterior: VersionInforme | null;
    /** Comparar con esta otra (de la URL). */
    con: string | null;
    filtro: FiltroCambios;
    versiones: VersionInforme[];
    puedeRestaurar: boolean;
    alVerArchivo: (ruta: string) => void;
  } = $props();

  const desde = $derived(con ?? anterior?.id ?? null);
  let dif = $state<Diferencias | null>(null);
  let cargando = $state(false);
  let progreso = $state("");
  let error = $state("");
  let texto = $state("");
  let orden = $state<OrdenCambios>("ruta");
  let agrupar = $state(true);
  const PASO = 300;
  let limite = $state(PASO);
  let restaurando = $state<string | null>(null);

  const puede = $derived(acceso.abierta && acceso.admite("diferencias"));
  $effect(() => {
    const [h, d, listo] = [version.id, desde, puede];
    if (!listo) return;
    untrack(() => void cargar(h, d));
  });

  async function cargar(hasta: string, de: string | null) {
    dif = null;
    error = "";
    cargando = true;
    progreso = `${acceso.equipo.nombre} compara las dos versiones…`;
    const clave = `dif|${de ?? ""}|${hasta}`;
    try {
      const paginas: PaginaCambios[] = [];
      let indice: number | null = 0;
      while (indice != null) {
        const p: MensajeEquipo & PaginaCambios = await acceso.pedir<MensajeEquipo & PaginaCambios>("diferencias", { hasta, ...(de ? { desde: de } : {}), indice }, `${clave}|${indice}`);
        if (hasta !== version.id || de !== desde) return;
        paginas.push(p);
        if (p.primera) break;
        const siguiente: number | null | undefined = p.siguiente;
        indice = typeof siguiente === "number" && siguiente > indice ? siguiente : null;
        if (indice != null) progreso = `Recibiendo la lista… ${numero(indice)} de ${numero(p.total ?? 0)}`;
      }
      dif = unirPaginas(paginas);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      cargando = false;
    }
  }

  const lista = $derived(dif ? filtrarCambios(dif.cambios, filtro, texto, orden) : []);
  const visibles = $derived(lista.slice(0, limite));
  const comun = $derived(dif ? prefijoComun(dif.cambios.map((c) => c.ruta)) : "/");
  const grupos = $derived(agrupar ? agruparPorCarpeta(visibles, orden) : []);
  const relativa = (carpeta: string) => (comun !== "/" && carpeta.startsWith(comun) ? carpeta.slice(comun.length).replace(/^\//, "") || "(esta carpeta)" : rutaLegible(carpeta));
  const ICONO = { nuevo: FilePlus, cambiado: FilePen, borrado: FileMinus, metadatos: FilePen, otro: FileQuestion };
  const TONO = { nuevo: "ok", cambiado: "info", borrado: "bad", metadatos: "neutral", otro: "neutral" };
  const fmtDelta = (d: number | null) => (d == null ? "" : d === 0 ? "igual" : `${d > 0 ? "+" : "−"}${bytes(Math.abs(d))}`);
  /** Para restaurar un archivo borrado, la versión en la que aún estaba. */
  const versionPara = (c: Cambio) => (c.tipo === "borrado" ? (dif?.desde ?? desde ?? version.id) : version.id);
  const cuandoDe = (id: string | null) => versiones.find((v) => id && (v.id === id || v.id.startsWith(id) || id.startsWith(v.id)))?.hora ?? null;
  const elegibles = $derived(versiones.filter((v) => v.id !== version.id && (!("hora" in version) || !version.hora || Date.parse(v.hora) < Date.parse(version.hora))));
  // Antes de abrir: lo que ya dice el informe.
  const delInforme = $derived("archivos_nuevos" in version ? version : null);

  function elegirFiltro(f: FiltroCambios) {
    limite = PASO;
    ir({ filtro: f }, true);
  }
</script>

<section class="cambios" aria-labelledby="t-cambios">
  <div class="cab">
    <h3 id="t-cambios" class="sr-only">Qué cambió</h3>
    <p class="entre">
      {#if dif?.primera}Es la primera versión de su copia: no hay otra anterior con la que compararla.
      {:else}
        De <strong>{desde ? fechaCorta(cuandoDe(desde) ?? dif?.desdeCuando) : "la versión anterior de la misma copia"}</strong>{#if desde}{" "}<span class="pastilla mono" title={desde}>{desde.slice(0, 8)}</span>{/if}
        a <strong>{fechaCorta("hora" in version ? version.hora : null)}</strong> <span class="pastilla mono" title={version.id}>{version.id.slice(0, 8)}</span>
      {/if}
    </p>
    {#if elegibles.length}
      <label class="comparar">
        <span class="faint">Comparar con</span>
        <select class="input" value={con ?? ""} onchange={(e) => ir({ con: e.currentTarget.value || null, filtro: "todos" })}>
          <option value="">La anterior de su copia{anterior ? ` (${fechaCorta(anterior.hora)})` : ""}</option>
          {#each elegibles as v (v.id)}<option value={v.id}>{fechaCorta(v.hora)} · {v.id.slice(0, 8)}</option>{/each}
        </select>
      </label>
    {/if}
  </div>

  {#if !acceso.abierta}
    {#if delInforme && (delInforme.archivos_nuevos != null || delInforme.archivos_cambiados != null)}
      <p class="del-informe">Según el informe del equipo: <strong>{plural(delInforme.archivos_nuevos ?? 0, "archivo nuevo", "archivos nuevos")}</strong> y <strong>{plural(delInforme.archivos_cambiados ?? 0, "cambiado", "cambiados")}</strong>{#if delInforme.archivos_sin_cambios != null}; {plural(delInforme.archivos_sin_cambios, "siguió igual", "siguieron igual")}{/if}. Los nombres, con la contraseña:</p>
    {/if}
    <DesbloquearRepo {acceso} {nombreRepo} para="ver qué archivos cambiaron" />
  {:else if !acceso.admite("diferencias")}
    <div class="notice notice-info"><p>El agente de {acceso.equipo.nombre} todavía no sabe comparar versiones. Actualízalo para ver aquí qué archivos cambiaron. Mientras, puedes explorar cada versión en Restaurar.</p></div>
  {:else if error}
    <div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>
    <button class="btn btn-sm" onclick={() => cargar(version.id, desde)}>Volver a intentarlo</button>
  {:else if cargando || !dif}
    <p class="espera" role="status"><LoaderCircle size={15} class="spin" />{progreso}</p>
  {:else if !dif.primera}
    <div class="filtros" role="group" aria-label="Qué cambios ver">
      {#each FILTROS as f (f)}
        <button class="chip-filtro f-{f}" class:on={filtro === f} aria-pressed={filtro === f} onclick={() => elegirFiltro(f)}>
          {TEXTO_FILTRO[f]} <span class="num">{numero(cuentaFiltro(dif.resumen, f))}</span>
        </button>
      {/each}
    </div>
    <p class="faint totales num">
      {#if dif.resumen.carpetas_nuevas || dif.resumen.carpetas_borradas}{[dif.resumen.carpetas_nuevas ? plural(dif.resumen.carpetas_nuevas, "carpeta nueva", "carpetas nuevas") : null, dif.resumen.carpetas_borradas ? plural(dif.resumen.carpetas_borradas, "carpeta borrada", "carpetas borradas") : null].filter(Boolean).join(" · ")} · {/if}
      {#if dif.resumen.bytes_anadidos != null}datos nuevos {bytes(dif.resumen.bytes_anadidos)}{/if}{#if dif.resumen.bytes_quitados}{" "}· quitados {bytes(dif.resumen.bytes_quitados)}{/if}
      {#if dif.resumen.metadatos}{" "}· {plural(dif.resumen.metadatos, "solo con otra fecha o permisos", "solo con otra fecha o permisos")}{/if}
    </p>
    {#if dif.recortado}<p class="notice notice-info"><span>Hay muchísimos cambios: aquí están los primeros {numero(dif.cambios.length)} (los recuentos son de todos).</span></p>{/if}

    <div class="herramientas">
      <label class="buscar">
        <Search size={14} />
        <span class="sr-only">Buscar por nombre o carpeta</span>
        <input class="input" type="search" placeholder="Buscar por nombre o carpeta" bind:value={texto} oninput={() => (limite = PASO)} />
      </label>
      <label class="ordenar">
        <ArrowDownUp size={14} />
        <span class="sr-only">Ordenar</span>
        <select class="input" bind:value={orden}>
          <option value="ruta">Por carpeta y nombre</option>
          <option value="tamano">Por tamaño</option>
          <option value="delta">Por cuánto cambió</option>
        </select>
      </label>
      <label class="agrupar"><input type="checkbox" bind:checked={agrupar} />Agrupar por carpeta</label>
    </div>

    {#if !lista.length}
      <p class="faint vacio">{texto ? "Nada con ese nombre entre estos cambios." : filtro === "todos" ? "Ningún archivo cambió entre estas dos versiones." : `Ningún archivo ${TEXTO_FILTRO[filtro].toLowerCase().replace(/s$/, "")} entre estas dos versiones.`}</p>
    {:else}
      {#if comun !== "/"}<p class="faint comun">En <span class="selectable">{rutaLegible(comun)}</span></p>{/if}
      {#snippet fila(c: Cambio)}
        {@const Icono = ICONO[c.tipo]}
        {@const d = deltaDe(c)}
        <li class="archivo">
          <span class="ic tone-{TONO[c.tipo]}" use:tip={TEXTO_TIPO[c.tipo]}><Icono size={15} /></span>
          <span class="nombre selectable" title={rutaLegible(c.ruta)}>{agrupar ? partesRuta(c.ruta).nombre : rutaLegible(c.ruta)}</span>
          <span class="tam num">
            {#if c.bytes != null || c.bytes_antes != null}<span use:tip={c.tipo === "borrado" ? "Lo que ocupaba" : "Lo que ocupa"}>{bytes(c.bytes ?? c.bytes_antes)}</span>{/if}
            {#if d != null && c.tipo === "cambiado"}<small class:sube={d > 0} use:tip={`Antes: ${bytes(c.bytes_antes)}`}>{fmtDelta(d)}</small>{/if}
          </span>
          <span class="acc">
            <button class="icon-btn" use:tip={"Ver versiones de este archivo"} aria-label="Ver las versiones de {partesRuta(c.ruta).nombre}" onclick={() => alVerArchivo(c.ruta)}><FileClock size={15} /></button>
            {#if puedeRestaurar}
              <button class="icon-btn" use:tip={"Restaurar este archivo"} aria-label="Restaurar {partesRuta(c.ruta).nombre}" aria-expanded={restaurando === c.ruta} onclick={() => (restaurando = restaurando === c.ruta ? null : c.ruta)}><History size={15} /></button>
            {/if}
          </span>
          {#if restaurando === c.ruta}
            <div class="rest"><RestaurarArchivo {acceso} version={versionPara(c)} cuando={cuandoDe(versionPara(c))} ruta={c.ruta} tamano={c.bytes ?? c.bytes_antes} /></div>
          {/if}
        </li>
      {/snippet}
      {#if agrupar}
        {#each grupos as g (g.carpeta)}
          <details class="grupo" open={grupos.length <= 12}>
            <summary>
              <Folder size={14} />
              <span class="carpeta selectable">{relativa(g.carpeta)}</span>
              <span class="faint num cuenta">{[g.nuevos ? `${numero(g.nuevos)} nuevos` : null, g.cambiados ? `${numero(g.cambiados)} cambiados` : null, g.borrados ? `${numero(g.borrados)} borrados` : null].filter(Boolean).join(" · ")}{dif.conTamanos ? ` · ${bytes(g.bytes)}` : ""}</span>
            </summary>
            <ul>
              {#each g.cambios as c (c.ruta)}{@render fila(c)}{/each}
            </ul>
          </details>
        {/each}
      {:else}
        <ul>
          {#each visibles as c (c.ruta)}{@render fila(c)}{/each}
        </ul>
      {/if}
      {#if lista.length > limite}
        <button class="btn btn-ghost mas" onclick={() => (limite += PASO)}>Mostrar más ({numero(lista.length - limite)} más)</button>
      {/if}
      {#if !dif.conTamanos}<p class="faint pie">Sin tamaños: el equipo no los midió.</p>{/if}
    {/if}
  {/if}
  {#if acceso.abierta && dif && !cargando}<p class="faint pie" use:tip={fechaLarga(new Date().toISOString())}>Leído ahora de {acceso.equipo.nombre}, cifrado de extremo a extremo.</p>{/if}
</section>

<style>
  .cambios {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .cab {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2) var(--sp-4);
  }
  .entre {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .comparar {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-xs);
  }
  .comparar .input,
  .ordenar .input {
    width: auto;
    height: 30px;
    padding: 0 8px;
    font-size: var(--fs-xs);
  }
  .del-informe {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .filtros {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip-filtro {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 32px;
    padding: 0 12px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-2);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    cursor: pointer;
  }
  .chip-filtro:hover {
    border-color: var(--text-3);
  }
  .chip-filtro.on {
    color: var(--accent-text);
    background: var(--accent-soft);
    border-color: var(--accent);
  }
  .chip-filtro:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .totales {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .herramientas {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
  }
  .buscar,
  .ordenar {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--text-3);
  }
  .buscar {
    flex: 1 1 220px;
  }
  .buscar .input {
    height: 32px;
  }
  .agrupar {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .comun {
    margin: 0;
    font-size: var(--fs-xs);
    overflow-wrap: anywhere;
  }
  .grupo summary {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 36px;
    padding: 4px 0;
    font-size: var(--fs-sm);
    font-weight: 500;
    cursor: pointer;
    border-top: 1px solid var(--border);
  }
  .grupo summary:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .carpeta {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .cuenta {
    margin-left: auto;
    flex: none;
    font-size: var(--fs-xs);
    font-weight: 400;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .grupo ul {
    padding-left: 20px;
  }
  .archivo {
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: var(--sp-2);
    min-height: 36px;
    padding: 2px 0;
    border-top: 1px solid var(--border);
    font-size: var(--fs-sm);
  }
  .ic {
    display: inline-flex;
    color: var(--c, var(--text-3));
  }
  .ic.tone-ok {
    --c: var(--ok);
  }
  .ic.tone-bad {
    --c: var(--bad);
  }
  .ic.tone-info {
    --c: var(--info);
  }
  .nombre {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .tam {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .tam small {
    color: var(--text-3);
  }
  .tam small.sube {
    color: var(--text-2);
  }
  .acc {
    display: flex;
    gap: 2px;
  }
  .rest {
    grid-column: 2 / -1;
    padding-bottom: 6px;
  }
  .mas {
    align-self: center;
  }
  .vacio,
  .pie {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
</style>
