<script lang="ts">
  // 16e: la barra de filtros de los equipos, igual en Equipos (lista y mapa) y en Estado.
  // Los filtros se combinan (entre clases, «y»; dentro de una clase, «cualquiera»), se ven
  // como chips quitables, se recuerdan por persona en este navegador y van en la dirección
  // (para compartir un enlace con la vista filtrada). Con vistas guardadas con nombre.
  import { untrack, type Snippet } from "svelte";
  import { page } from "$app/state";
  import { replaceState } from "$app/navigation";
  import { Bookmark, BookmarkPlus, ChevronDown, Settings2, SlidersHorizontal, X } from "@lucide/svelte";
  import type { Equipo } from "$lib/tipos";
  import { actual, puede, reloj } from "$lib/estado.svelte";
  import { contextoFiltros, filtrosEquipos } from "$lib/filtrosEquipos.svelte";
  import {
    aDireccion,
    alternar,
    chipsDe,
    cuantosFiltros,
    deDireccion,
    deTexto,
    direccionConFiltros,
    errorNombreVista,
    estadosDe,
    ESTADOS_FILTRO,
    MAX_NOMBRE_VISTA,
    NOMBRE_ESTADO,
    textoCuenta,
    vacios,
    vistaActual,
    type FiltrosEquipos,
  } from "$lib/filtrosEquipos";
  import { etiquetasDe, mismaEtiqueta } from "$lib/etiquetas.svelte";
  import { opcionesFiltroInfra, textoFiltroInfra, leerFiltroInfra } from "$lib/infra";
  import { sedePorId } from "$lib/sedes";
  import EtiquetaChip from "./EtiquetaChip.svelte";
  import GestionarEtiquetas from "./GestionarEtiquetas.svelte";

  let {
    equipos,
    vistos,
    total = equipos.length,
    children,
  }: {
    /** Todos los equipos (para las opciones y sus cuentas). */
    equipos: Equipo[];
    /** Cuántos pasan los filtros (y la búsqueda, si la hay). */
    vistos: number;
    total?: number;
    /** Lo que va delante en la misma fila (el buscador de la página). */
    children?: Snippet;
  } = $props();

  // La dirección manda al llegar (un enlace compartido); luego, lo elegido se escribe en ella.
  untrack(() => {
    if (direccionConFiltros(page.url.searchParams)) filtrosEquipos.poner(deDireccion(page.url.searchParams));
  });
  $effect(() => {
    const f = filtrosEquipos.valor;
    untrack(() => {
      const url = new URL(page.url);
      const q = aDireccion(f, url.searchParams).toString();
      if (q === url.searchParams.toString()) return;
      url.search = q;
      try {
        replaceState(url, page.state);
      } catch {
        /* el enrutador aún no está listo: no pasa nada, se escribe en el siguiente cambio */
      }
    });
  });

  const f = $derived(filtrosEquipos.valor);
  const poner = (x: FiltrosEquipos) => filtrosEquipos.poner(x);
  const n = $derived(cuantosFiltros(f));
  let abierto = $state(false);
  let ajustar = $state(false);

  const ctx = $derived(contextoFiltros());
  const org = $derived(actual.organizacion);
  const contar = <T,>(xs: T[]) => {
    const m = new Map<T, number>();
    for (const x of xs) m.set(x, (m.get(x) ?? 0) + 1);
    return m;
  };
  const porOrden = (a: string, b: string) => a.localeCompare(b, "es", { numeric: true });

  // Las opciones, solo las que tienen algún equipo (o ya están elegidas).
  const estados = $derived.by(() => {
    const m = new Map<string, number>();
    for (const e of equipos) for (const s of estadosDe(e, reloj.ahora)) m.set(s, (m.get(s) ?? 0) + 1);
    return ESTADOS_FILTRO.filter((s) => m.get(s) || f.estados.includes(s)).map((s) => ({ id: s, n: m.get(s) ?? 0 }));
  });
  const sedes = $derived.by(() => {
    if (!org.sedes.length) return [];
    const m = contar(equipos.map((e) => ctx.sede?.(e) ?? ""));
    return org.sedes.map((s) => ({ id: s.id, nombre: s.nombre, n: m.get(s.id) ?? 0 })).filter((s) => s.n || f.sedes.includes(s.id));
  });
  const etiquetas = $derived(etiquetasDe(equipos));
  /** Las etiquetas, por categoría (las que no tienen, al final). */
  const gruposEt = $derived.by(() => {
    const usadas = new Set<string>();
    const out: { nombre: string | null; ets: { nombre: string; n: number }[] }[] = [];
    for (const c of org.categorias) {
      const ets = etiquetas.filter((t) => c.etiquetas.some((x) => mismaEtiqueta(x, t.nombre)));
      ets.forEach((t) => usadas.add(t.nombre));
      if (ets.length) out.push({ nombre: c.nombre, ets });
    }
    const resto = etiquetas.filter((t) => !usadas.has(t.nombre));
    if (resto.length) out.push({ nombre: out.length ? "Otras" : null, ets: resto });
    return out;
  });
  const sistemas = $derived([...contar(equipos.map((e) => ctx.sistema!(e)))].sort((a, b) => porOrden(a[0], b[0])));
  const tipos = $derived([...contar(equipos.map((e) => ctx.tipo!(e)))].sort((a, b) => porOrden(a[0], b[0])));
  const versiones = $derived([...contar(equipos.map((e) => e.version_agente).filter(Boolean))].sort((a, b) => porOrden(b[0], a[0])));
  const almacenes = $derived(equipos.filter((e) => e.rol === "almacenamiento").length);
  const opcionesInfra = $derived(opcionesFiltroInfra(actual.infra));

  const chips = $derived(
    chipsDe(f, {
      sede: (id) => sedePorId(org, id)?.nombre ?? null,
      infra: (v) => textoFiltroInfra(leerFiltroInfra(v, actual.infra), actual.infra) || null,
    }),
  );

  // --- Vistas guardadas ---
  const vistas = $derived(filtrosEquipos.vistas);
  const vista = $derived(vistaActual(vistas, f));
  let guardando = $state(false);
  let nombreVista = $state("");
  const errorVista = $derived(errorNombreVista(nombreVista, f));
  let verError = $state(false);
  function guardarVista(ev: SubmitEvent) {
    ev.preventDefault();
    verError = true;
    if (errorVista) return;
    verError = false;
    filtrosEquipos.guardarVista(nombreVista);
    guardando = false;
    nombreVista = "";
  }
  function elegirVista(nombre: string) {
    const v = vistas.find((x) => x.nombre === nombre);
    if (v) poner(deTexto(v.filtros));
  }
</script>

<div class="barra">
  {@render children?.()}
  <button type="button" class="btn" aria-expanded={abierto} aria-controls="panel-filtros" onclick={() => (abierto = !abierto)}>
    <SlidersHorizontal size={16} />Filtros{#if n}<span class="n num" aria-label="{n} puestos">{n}</span>{/if}<span class="chev" class:girado={abierto}><ChevronDown size={14} /></span>
  </button>
  {#if vistas.length}
    <select class="input vistas" aria-label="Vistas guardadas" value={vista?.nombre ?? ""} onchange={(ev) => elegirVista(ev.currentTarget.value)}>
      <option value="" disabled>Vistas guardadas…</option>
      {#each vistas as v (v.nombre)}<option value={v.nombre}>{v.nombre}</option>{/each}
    </select>
  {/if}
  <span class="cuenta faint num" aria-live="polite">{textoCuenta(vistos, total)}</span>
</div>

{#if abierto}
  <div class="card panel" id="panel-filtros" role="group" aria-label="Filtros de los equipos">
    {#if estados.length}
      <fieldset>
        <legend>Estado</legend>
        <div class="opciones">
          {#each estados as s (s.id)}
            <button type="button" class="opcion" aria-pressed={f.estados.includes(s.id)} onclick={() => poner({ ...f, estados: alternar(f.estados, s.id) as FiltrosEquipos["estados"] })}>{NOMBRE_ESTADO[s.id]}<span class="cn num">{s.n}</span></button>
          {/each}
        </div>
      </fieldset>
    {/if}
    {#if sedes.length > 1 || f.sedes.length}
      <fieldset>
        <legend>Sede</legend>
        <div class="opciones">
          {#each sedes as s (s.id)}
            <button type="button" class="opcion" aria-pressed={f.sedes.includes(s.id)} onclick={() => poner({ ...f, sedes: alternar(f.sedes, s.id) })}>{s.nombre}<span class="cn num">{s.n}</span></button>
          {/each}
        </div>
      </fieldset>
    {/if}
    {#if etiquetas.length}
      <fieldset>
        <legend>
          Etiquetas
          {#if puede.administrar(actual.cliente?.rol)}<button type="button" class="btn btn-sm btn-ghost ajustar" onclick={() => (ajustar = true)}><Settings2 size={13} />Ajustar</button>{/if}
        </legend>
        {#each gruposEt as g (g.nombre ?? "")}
          <div class="opciones">
            {#if g.nombre}<span class="cat faint">{g.nombre}</span>{/if}
            {#each g.ets as t (t.nombre)}
              <EtiquetaChip nombre={t.nombre} n={t.n} activa={f.etiquetas.some((x) => mismaEtiqueta(x, t.nombre))} onclick={() => poner({ ...f, etiquetas: alternar(f.etiquetas, t.nombre) })} />
            {/each}
          </div>
        {/each}
        {#if f.etiquetas.length > 1}
          <div class="segmented inline modo" role="group" aria-label="Con varias etiquetas">
            <button type="button" class:on={f.modo === "cualquiera"} aria-pressed={f.modo === "cualquiera"} onclick={() => poner({ ...f, modo: "cualquiera" })}>Cualquiera de ellas</button>
            <button type="button" class:on={f.modo === "todas"} aria-pressed={f.modo === "todas"} onclick={() => poner({ ...f, modo: "todas" })}>Todas a la vez</button>
          </div>
        {/if}
      </fieldset>
    {/if}
    {#if sistemas.length > 1 || f.sistemas.length}
      <fieldset>
        <legend>Sistema</legend>
        <div class="opciones">
          {#each sistemas as [s, c] (s)}
            <button type="button" class="opcion" aria-pressed={f.sistemas.includes(s)} onclick={() => poner({ ...f, sistemas: alternar(f.sistemas, s) })}>{s}<span class="cn num">{c}</span></button>
          {/each}
        </div>
      </fieldset>
    {/if}
    {#if tipos.length > 1 || f.tipos.length}
      <fieldset>
        <legend>Tipo de equipo</legend>
        <div class="opciones">
          {#each tipos as [t, c] (t)}
            <button type="button" class="opcion" aria-pressed={f.tipos.includes(t)} onclick={() => poner({ ...f, tipos: alternar(f.tipos, t) })}>{t}<span class="cn num">{c}</span></button>
          {/each}
        </div>
      </fieldset>
    {/if}
    {#if versiones.length > 1 || f.versiones.length}
      <fieldset>
        <legend>Versión del agente</legend>
        <div class="opciones">
          {#each versiones as [v, c] (v)}
            <button type="button" class="opcion" aria-pressed={f.versiones.includes(v)} onclick={() => poner({ ...f, versiones: alternar(f.versiones, v) })}>{v}<span class="cn num">{c}</span></button>
          {/each}
        </div>
      </fieldset>
    {/if}
    {#if almacenes || opcionesInfra.length}
      <fieldset>
        <legend>Más</legend>
        <div class="opciones">
          {#if almacenes}
            <button type="button" class="opcion" aria-pressed={f.almacen} onclick={() => poner({ ...f, almacen: !f.almacen })}>Guardan copias de otros<span class="cn num">{almacenes}</span></button>
          {/if}
          {#if opcionesInfra.length}
            <select class="input input-sm infra" aria-label="En qué servidor físico o disco" value={f.infra} onchange={(ev) => poner({ ...f, infra: ev.currentTarget.value })}>
              <option value="">En cualquier servidor físico</option>
              {#each opcionesInfra as o (o.valor)}<option value={o.valor}>{o.nivel ? "  " : ""}{o.texto}</option>{/each}
            </select>
          {/if}
        </div>
      </fieldset>
    {/if}
  </div>
{/if}

{#if chips.length || vista}
  <div class="activos" role="group" aria-label="Filtros puestos">
    {#if vista}<span class="vista-on"><Bookmark size={13} />{vista.nombre}</span>{/if}
    {#each chips as c (c.clave)}
      <span class="chip-f">{c.texto}<button type="button" class="quitar" aria-label="Quitar «{c.texto}»" onclick={() => poner(c.sin)}><X size={12} /></button></span>
    {/each}
    <button type="button" class="btn btn-sm btn-ghost" onclick={() => poner(vacios())}><X size={14} />Quitar filtros</button>
    {#if vista}
      <button type="button" class="btn btn-sm btn-ghost" onclick={() => filtrosEquipos.quitarVista(vista.nombre)}>Olvidar la vista</button>
    {:else if !guardando}
      <button type="button" class="btn btn-sm btn-ghost" onclick={() => (guardando = true)}><BookmarkPlus size={14} />Guardar como vista</button>
    {/if}
  </div>
  {#if guardando && !vista}
    <form class="guardar" onsubmit={guardarVista}>
      <label class="sr-only" for="nombre-vista">Nombre de la vista</label>
      <!-- svelte-ignore a11y_autofocus -->
      <input id="nombre-vista" class="input input-sm" maxlength={MAX_NOMBRE_VISTA} placeholder="Por ejemplo: Servidores de la planta" bind:value={nombreVista} autofocus />
      <button class="btn btn-sm btn-primary">Guardar</button>
      <button type="button" class="btn btn-sm btn-ghost" onclick={() => ((guardando = false), (nombreVista = ""))}>Cancelar</button>
      {#if verError && errorVista}<span class="error-vista" role="alert">{errorVista}</span>{/if}
      <span class="faint pista">Se guarda en este navegador. Para pasársela a alguien, copia la dirección de la página.</span>
    </form>
  {/if}
{/if}

{#if ajustar}<GestionarEtiquetas onclose={() => (ajustar = false)} />{/if}

<style>
  .barra {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  .n {
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    font-size: var(--fs-xs);
    line-height: 18px;
    text-align: center;
    color: var(--accent-contrast);
    background: var(--accent);
    border-radius: 9px;
  }
  .chev {
    display: inline-flex;
    transition: transform var(--dur-fast) var(--ease);
  }
  .chev.girado {
    transform: rotate(180deg);
  }
  .vistas {
    width: auto;
    max-width: 220px;
  }
  .cuenta {
    margin-left: auto;
    font-size: var(--fs-sm);
  }
  .panel {
    display: grid;
    gap: var(--sp-3);
    padding: var(--sp-4);
  }
  fieldset {
    min-width: 0;
    margin: 0;
    padding: 0;
    border: 0;
  }
  legend {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin-bottom: 6px;
    padding: 0;
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--text-2);
  }
  .opciones {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .opciones + .opciones {
    margin-top: 6px;
  }
  .cat {
    font-size: var(--fs-xs);
    margin-right: 2px;
  }
  .opcion {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 100%;
    min-height: 28px;
    padding: 0 10px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
    cursor: pointer;
    overflow-wrap: anywhere;
    text-align: left;
  }
  .opcion:hover {
    color: var(--text-1);
    border-color: var(--border-strong);
  }
  .opcion[aria-pressed="true"] {
    color: var(--accent-text);
    background: var(--accent-soft);
    border-color: color-mix(in srgb, var(--accent) 40%, transparent);
  }
  .cn {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .modo {
    margin-top: var(--sp-2);
  }
  .ajustar {
    height: 24px;
    font-weight: 500;
  }
  .infra {
    width: auto;
    max-width: 100%;
  }
  .activos {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .chip-f,
  .vista-on {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    max-width: 100%;
    height: 24px;
    padding: 0 4px 0 10px;
    font-size: var(--fs-xs);
    color: var(--text-1);
    background: var(--surface-3);
    border-radius: 999px;
    overflow-wrap: anywhere;
  }
  .vista-on {
    padding-right: 10px;
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .quitar {
    display: inline-grid;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    color: var(--text-3);
    background: none;
    border: 0;
    border-radius: 50%;
    cursor: pointer;
  }
  .quitar:hover {
    color: var(--text-1);
    background: var(--surface-2);
  }
  .guardar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  .guardar .input {
    width: min(280px, 100%);
  }
  .error-vista {
    font-size: var(--fs-sm);
    color: var(--bad);
  }
  .pista {
    font-size: var(--fs-xs);
  }
</style>
