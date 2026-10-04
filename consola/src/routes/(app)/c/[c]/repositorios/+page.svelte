<script lang="ts">
  // Repositorios y destinos del cliente: dónde se guardan las copias y cómo de
  // protegidas están. Los datos salen de lo que informa cada equipo (sin rutas).
  import { tip } from "$lib/tooltip";
  import { bytesRepo, destinoDe, estadoRepo, informeDe, nVersiones, pruebaRestauracion, verificacion } from "$lib/repo";
  import { cargarInformes, ultimos } from "$lib/informes.svelte";
  import { ChevronRight, Cloud, Database, FlaskConical, HardDrive, Lock, Monitor, Plus, Server, ShieldCheck } from "@lucide/svelte";
  import { actual, puede, reloj } from "$lib/estado.svelte";
  import { bytes, numero, plural } from "$lib/formato";
  import type { DestinoResumen, Equipo, RepositorioResumen } from "$lib/tipos";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import Cifra from "$lib/componentes/Cifra.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import PendienteItem from "$lib/componentes/PendienteItem.svelte";
  import { pendientesDe } from "$lib/pendientes.svelte";
  import NuevoRepositorio from "$lib/componentes/NuevoRepositorio.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";

  let nuevo = $state(false);
  /** Repositorios (y destinos nuevos) en camino: se ven en su sitio mientras el equipo los crea. */
  const enCamino = $derived(pendientesDe("repositorios"));
  const destinosEnCamino = $derived([...pendientesDe("destinos"), ...enCamino.filter((p) => p.destinoNuevo && !destinos.some((d) => d.nombre === p.destinoNuevo))]);
  const administra = $derived(puede.administrar(actual.cliente?.rol));
  const conEquipos = $derived(actual.equipos.some((e) => e.confirmado && e.modo !== "trasladado"));

  const destinos = $derived.by(() => {
    const m = new Map<string, DestinoResumen & { equipos: Set<string> }>();
    for (const e of actual.equipos)
      for (const d of e.resumen?.destinos ?? []) {
        const x = m.get(d.id) ?? { ...d, equipos: new Set<string>() };
        x.equipos.add(e.nombre);
        m.set(d.id, x);
      }
    return [...m.values()].sort((a, b) => a.nombre.localeCompare(b.nombre));
  });
  $effect(() => void cargarInformes(actual.id, actual.equipos.map((e) => e.id)));
  // Con lo que diga el último informe de cada equipo si el resumen no lo trae (agentes anteriores a 0.7.4).
  const repos = $derived(
    actual.equipos.flatMap((e) =>
      (e.resumen?.repositorios ?? []).map((r) => {
        const inf = informeDe(ultimos.porEquipo[e.id], r.id);
        return {
          ...r,
          // El id del destino (el agente da su nombre): para agruparlo con los demás.
          destino: destinoDe(e.resumen?.destinos, r)?.id ?? r.destino,
          equipo: e,
          versiones: nVersiones(r, inf),
          bytes: bytesRepo(r, inf) ?? undefined,
          verificado: verificacion(r, inf)?.ultima ?? null,
          prueba_restauracion: pruebaRestauracion(r, inf)?.ultima ?? null,
          estado: estadoRepo(r, inf, e.resumen?.copias ?? [], reloj.ahora),
        };
      }),
    ) as (RepositorioResumen & { equipo: Equipo; estado: ReturnType<typeof estadoRepo> })[],
  );
  const almacenes = $derived(actual.equipos.filter((e) => e.resumen?.guarda_copias?.activo));
  /** Los destinos que no son un equipo de este cliente que guarda copias. */
  // Un agente que no dice `equipo_almacen` nombra el destino como el equipo que guarda copias: ese ya tiene su tarjeta.
  const otros = $derived(destinos.filter((d) => !almacenes.some((a) => a.id === d.equipo_almacen || (!d.equipo_almacen && d.tipo === "rest" && d.nombre === a.nombre))));
  const total = $derived(repos.reduce((n, r) => n + (r.bytes ?? 0), 0));
  const nombreDestino = (id: string) => destinos.find((d) => d.id === id)?.nombre ?? id;
  /** Los repositorios que guardan en un destino (o en un almacén, por cualquiera de sus destinos). */
  const reposEn = (ids: string[]) => repos.filter((r) => ids.includes(r.destino));
  const idsAlmacen = (a: Equipo) => destinos.filter((d) => d.equipo_almacen === a.id || (!d.equipo_almacen && d.tipo === "rest" && d.nombre === a.nombre)).map((d) => d.id);
  const ICONO = { rest: Server, local: HardDrive, s3: Cloud, b2: Cloud, sftp: Server, otro: Database };
  const TIPO = { rest: "Servidor de copias", local: "Disco o carpeta del equipo", s3: "S3", b2: "Backblaze B2", sftp: "SFTP", otro: "Otro" };

  const DIA = 86_400_000;
  const reciente = (iso: string | null | undefined, dias: number) => !!iso && reloj.ahora - Date.parse(iso) < dias * DIA;
  const verificados = $derived(repos.filter((r) => reciente(r.verificado, 8)).length);
  const probados = $derived(repos.filter((r) => reciente(r.prueba_restauracion, 35)).length);
  const conProblemas = $derived(repos.filter((r) => r.estado.tono === "bad" || r.estado.tono === "warn").length);
</script>

<svelte:head><title>Repositorios y destinos · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <CabeceraPagina
    titulo="Repositorios y destinos"
    icono={Database}
    migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${actual.id}` }, { texto: "Repositorios y destinos" }]}
    resumen={actual.cargado
      ? `${plural(repos.length, "repositorio", "repositorios")} en ${plural(almacenes.length + otros.length, "destino", "destinos")} · ${bytes(total)} protegidos${enCamino.length ? ` · ${plural(enCamino.length, "cambio en camino", "cambios en camino")}` : ""}`
      : "Cargando…"}
  >
    {#snippet acciones()}
      {#if administra && conEquipos}
        <button class="btn btn-primary" onclick={() => (nuevo = true)}><Plus size={16} />Nuevo repositorio</button>
      {/if}
    {/snippet}
  </CabeceraPagina>

  {#if !actual.cargado || !actual.cliente}
    <Esqueleto forma="cifras" n={4} />
    <section><div class="section-head"><h2>Destinos</h2></div><Esqueleto forma="tarjetas" n={2} /></section>
    <section><div class="section-head"><h2>Repositorios</h2></div><Esqueleto forma="tabla" n={3} /></section>
  {:else if !conEquipos && !repos.length}
    <div class="card">
      <Vacio icono={Monitor} ilustracion="sin-equipos" titulo="Primero, un equipo" texto="Los repositorios viven en los equipos: añade uno y podrás crear dónde guardar sus copias.">
        {#if administra}<a class="btn btn-primary" href="/c/{actual.id}/emparejar"><Plus size={16} />Añadir equipo</a>{/if}
      </Vacio>
    </div>
  {:else}
    {#if repos.length}
      <div class="cifras" role="list" aria-label="Cifras de los repositorios">
        <Cifra icono={Database} etiqueta="Repositorios" valor={numero(repos.length)} sub={conProblemas ? plural(conProblemas, "necesita atención", "necesitan atención") : "todos al día"} mal={conProblemas > 0} />
        <Cifra icono={HardDrive} etiqueta="Protegido" valor={total ? bytes(total) : "—"} sub="en {plural(almacenes.length + otros.length, 'destino', 'destinos')}" />
        <Cifra icono={ShieldCheck} etiqueta="Verificados" valor={numero(verificados)} de="de {repos.length}" sub="en los últimos 7 días" />
        <Cifra icono={FlaskConical} etiqueta="Restauración probada" valor={numero(probados)} de="de {repos.length}" sub="en el último mes" />
      </div>
    {/if}

    <section>
      <div class="section-head"><h2>Destinos <span class="count">· {almacenes.length + otros.length}</span> <Ayuda id="destino" /></h2></div>
      {#if destinos.length || almacenes.length || destinosEnCamino.length}
        <div class="rejilla destinos">
          {#each destinosEnCamino as p (p.orden.id + "d")}<PendienteItem p={p.destinoNuevo ? { ...p, titulo: `Destino «${p.destinoNuevo}»` } : p} forma="tarjeta" conEquipo />{/each}
          {#each almacenes as a (a.id)}
            {@const suyos = reposEn(idsAlmacen(a))}
            <a class="card tile destino" href="/c/{actual.id}/equipos/{a.id}">
              <span class="tile-cab">
                <span class="tile-ic"><Server size={16} /></span>
                <span class="tile-nombre"><strong>{a.nombre}</strong><span>Almacén · puerto <span class="pastilla mono">{a.resumen?.guarda_copias?.puerto ?? "—"}</span></span></span>
                <ChevronRight size={16} class="flecha" />
              </span>
              <p class="tile-linea num">
                {plural(a.resumen?.guarda_copias?.usuarios ?? 0, "equipo copia aquí", "equipos copian aquí")}{#if suyos.length}{" · "}{plural(suyos.length, "repositorio", "repositorios")} · {bytes(suyos.reduce((n, r) => n + (r.bytes ?? 0), 0))}{/if}
              </p>
              <span class="tile-chips"><span class="badge badge-sm tone-ok" use:tip={"Los equipos pueden añadir copias, pero no borrarlas: protege contra el ransomware."}><Lock size={11} />Solo añadir</span>{#if a.resumen?.guarda_copias?.solo_red_local}<span class="badge badge-sm tone-neutral">Solo red local</span>{/if}</span>
            </a>
          {/each}
          {#each otros as d (d.id)}
            {@const Icono = ICONO[d.tipo] ?? Database}
            {@const suyos = reposEn([d.id])}
            <div class="card tile destino">
              <span class="tile-cab">
                <span class="tile-ic"><Icono size={16} /></span>
                <span class="tile-nombre"><strong>{d.nombre}</strong><span>{TIPO[d.tipo] ?? d.tipo}{#if d.donde && d.donde !== d.nombre}{" · "}<span class="pastilla mono">{d.donde}</span>{/if}</span></span>
              </span>
              <p class="tile-linea num">
                {#if suyos.length}{plural(suyos.length, "repositorio", "repositorios")} · {bytes(suyos.reduce((n, r) => n + (r.bytes ?? 0), 0))}{:else}Sin repositorios todavía{/if} · lo usa{d.equipos.size > 1 ? "n" : ""} {[...d.equipos].join(", ")}
              </p>
              {#if d.inmutable}<span class="tile-chips"><span class="badge badge-sm tone-ok"><Lock size={11} />Inmutable<Ayuda id="inmutable" /></span></span>{/if}
            </div>
          {/each}
        </div>
      {:else}
        <div class="card">
          <Vacio icono={Cloud} titulo="Sin destinos todavía" texto="Un destino es dónde se guardan las copias: un almacén (un equipo que guarda copias de otros), un disco o la nube. Se elige al crear el repositorio.">
            {#if administra && conEquipos}<button class="btn btn-primary" onclick={() => (nuevo = true)}><Plus size={16} />Nuevo repositorio</button>{/if}
          </Vacio>
        </div>
      {/if}
      <p class="nota"><ShieldCheck size={13} />Lo recomendado: una copia en la oficina (rápida para restaurar) y otra externa e inmutable en la nube.</p>
    </section>

    <section>
      <div class="section-head"><h2>Repositorios <span class="count">· {repos.length}</span> <Ayuda id="repositorio" /></h2></div>
      {#if enCamino.length}
        <div class="en-camino">{#each enCamino as p (p.orden.id)}<PendienteItem {p} conEquipo />{/each}</div>
      {/if}
      {#if repos.length}
        <div class="card p-0 desplazable solo-ancho-tabla">
          <table class="tabla">
            <caption class="sr-only">Repositorios</caption>
            <thead><tr><th scope="col">Repositorio</th><th scope="col">Estado</th><th scope="col">Equipo · destino</th><th scope="col" class="der">Versiones</th><th scope="col" class="der">Tamaño</th><th scope="col">Verificado</th><th scope="col">Restauración probada</th></tr></thead>
            <tbody>
              {#each repos as r (r.equipo.id + r.id)}
                <tr>
                  <td>
                    <a class="enlace-repo" href="/c/{actual.id}/equipos/{r.equipo.id}/repositorios/{encodeURIComponent(r.id)}"><Database size={14} /><strong>{r.nombre}</strong></a>{#if r.solo_lectura} <span class="badge badge-sm tone-neutral" use:tip={"Importado de otro equipo: se puede explorar y restaurar, pero ninguna copia escribe en él."}>Solo lectura</span>{/if}
                    {#if r.retencion}<span class="faint pequeno bloque">{r.retencion}</span>{/if}
                  </td>
                  <td><Chip pequeno tono={r.estado.tono} texto={r.estado.texto} /></td>
                  <td><a class="enlace-eq" href="/c/{actual.id}/equipos/{r.equipo.id}">{r.equipo.nombre}</a><span class="faint pequeno bloque">{nombreDestino(r.destino)}</span></td>
                  <td class="num der">{numero(nVersiones(r))}</td>
                  <td class="num der">{bytes(r.bytes)}</td>
                  <td class:faint={!reciente(r.verificado, 8)}><Tiempo iso={r.verificado} nada="nunca" /></td>
                  <td class:faint={!reciente(r.prueba_restauracion, 35)}><Tiempo iso={r.prueba_restauracion} nada="nunca" /></td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
        <!-- En móvil, una lista en vez de la tabla (que obligaría a desplazarse de lado). -->
        <div class="card p-0 lista solo-movil">
          {#each repos as r (r.equipo.id + r.id)}
            <a class="fila" href="/c/{actual.id}/equipos/{r.equipo.id}/repositorios/{encodeURIComponent(r.id)}">
              <span class="fila-texto">
                <span class="fila-titulo">{r.nombre}{#if r.solo_lectura} <span class="badge badge-sm tone-neutral">Solo lectura</span>{/if}</span>
                <span class="fila-sub">{r.equipo.nombre} · {nombreDestino(r.destino)}</span>
                <span class="fila-sub num">{numero(nVersiones(r))} versiones · {bytes(r.bytes)} · verificado <Tiempo iso={r.verificado} nada="nunca" /></span>
              </span>
              <Chip pequeno tono={r.estado.tono} texto={r.estado.texto} />
              <ChevronRight size={16} />
            </a>
          {/each}
        </div>
      {:else if !enCamino.length}
        <div class="card">
          <Vacio icono={Database} ilustracion="sin-repos" titulo="Sin repositorios todavía" texto="Un repositorio guarda las versiones de las copias, cifradas, en un destino. Crea el primero y elige qué copiar.">
            {#if administra && conEquipos}<button class="btn btn-primary" onclick={() => (nuevo = true)}><Plus size={16} />Nuevo repositorio</button>{/if}
          </Vacio>
        </div>
      {/if}
    </section>
  {/if}
</div>

{#if nuevo && actual.cliente}
  <NuevoRepositorio cliente={actual.cliente} equipos={actual.equipos} destinos={destinos} onclose={() => (nuevo = false)} />
{/if}

<style>
  .en-camino {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin-bottom: var(--sp-3);
  }
  .solo-movil {
    display: none;
  }
  @media (max-width: 640px) {
    .solo-movil {
      display: block;
    }
    .solo-ancho-tabla {
      display: none;
    }
  }
  .rejilla.destinos {
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 280px), 1fr));
  }
  a.destino:hover {
    border-color: var(--border-strong);
    box-shadow: var(--shadow-sm);
  }
  .destino :global(.flecha) {
    flex: none;
    color: var(--text-3);
  }
  .enlace-repo {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: inherit;
    text-decoration: none;
  }
  .enlace-repo :global(svg) {
    color: var(--text-3);
  }
  .enlace-repo:hover strong {
    text-decoration: underline;
  }
  .enlace-eq {
    color: var(--text-1);
    text-decoration: none;
  }
  .enlace-eq:hover {
    text-decoration: underline;
  }
  .bloque {
    display: block;
    margin-top: 2px;
  }
  .der {
    text-align: right;
  }
  .nota {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: var(--sp-3) 0 0;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .pequeno {
    font-size: var(--fs-xs);
  }
</style>
