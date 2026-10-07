<script lang="ts">
  // Espejos de un equipo (plan 0.7.26, bloque 4; docs/espejo.md «Trabajos de espejo»):
  // los que hace el almacén con lo que guarda y los que hace el propio equipo con
  // los repositorios de sus discos, al nivel del editor de copias. El espejo no
  // necesita contraseñas (copia los archivos cifrados tal cual); los cambios van al
  // equipo firmados con la clave de administración.
  //
  // ?nuevo=1 (con repo, zona, destino y quien) abre el editor con eso elegido (desde
  // una copia o la página de un destino); ?editar=<id> abre ese espejo.
  import { page } from "$app/state";
  import { untrack } from "svelte";
  import { CloudUpload, HardDrive, Info } from "@lucide/svelte";
  import Migas from "$lib/componentes/Migas.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import ListaEspejos from "$lib/componentes/espejos/ListaEspejos.svelte";
  import * as api from "$lib/api";
  import { actual, cargarCliente } from "$lib/estado.svelte";
  import { seguirCambios, tocaEquipo } from "$lib/vivo.svelte";
  import { claveNube, PRINCIPAL } from "$lib/destinos";
  import { admiteEspejoEquipo, admiteTrabajos, opcionesRepos, trabajosDelEquipo, type TrabajoEspejo } from "$lib/espejoTrabajos";
  import type { EquipoDetalle } from "$lib/tipos";

  const c = $derived(page.params.c ?? "");
  const id = $derived(page.params.e ?? "");
  let equipo = $state<EquipoDetalle | null>(null);
  let error = $state("");

  $effect(() => {
    const [cc, ee] = [c, id];
    untrack(() => cargar(cc, ee));
  });
  function cargar(cc = c, ee = id) {
    api.equipo(cc, ee).then((e) => ee === id && (equipo = e), (e) => (error = e.message));
  }
  // El resultado de cada vuelta (y de cada orden) al día, sin recargar.
  $effect(() => {
    const [cc, ee] = [c, id];
    return untrack(() =>
      seguirCambios(() => api.equipo(cc, ee).then((e) => ee === id && (equipo = e), () => {}), {
        ms: 0,
        toca: (x) => (x.t === "informe" || x.t === "equipo" || x.t === "orden") && tocaEquipo(x, ee),
      }),
    );
  });

  const almacen = $derived(!!equipo?.resumen?.guarda_copias?.activo);
  const deAlmacen = $derived(almacen && admiteTrabajos(equipo));
  const delEquipo = $derived(admiteEspejoEquipo(equipo));
  // En un almacén, «Hechos por este equipo» solo si tiene repositorios en sus discos (o espejos ya puestos).
  const propios = $derived(trabajosDelEquipo(equipo));
  const reposLocales = $derived(!!equipo && opcionesRepos(equipo, "equipo", []).length > 0);
  const equipos = $derived(equipo ? [...actual.equipos.filter((x) => x.id !== equipo!.id), equipo] : actual.equipos);

  /** Lo pedido al llegar (una vez): un espejo nuevo con esto elegido, o uno para cambiar. */
  const q = untrack(() => page.url.searchParams);
  const pedidoQuien = q.get("quien") === "equipo" ? "equipo" : q.get("quien") === "almacen" ? "almacen" : null;
  function preelegido(): Partial<TrabajoEspejo> | null {
    if (q.get("nuevo") !== "1") return null;
    const p: Partial<TrabajoEspejo> = {};
    const repo = q.get("repo");
    if (repo) p.que = { tipo: "repos", repos: [repo] };
    const zona = q.get("zona");
    if (zona && zona !== PRINCIPAL) p.zona = zona;
    const destino = q.get("destino");
    // La clave del catálogo: «zona:<almacén>:<zona>» o «nube:<equipo>:<nombre>».
    const m = destino ? /^(zona|nube):([^:]+):(.+)$/.exec(destino) : null;
    if (m && m[2] === id && m[1] === "zona") p.adonde = { tipo: "zona", carpeta: m[3] };
    else if (m && m[2] === id) {
      const n = [...(equipo?.resumen?.nubes ?? []), ...(equipo?.resumen?.guarda_copias?.nubes ?? [])].find((x) => claveNube(id, x.nombre) === destino);
      if (n) p.adonde = { tipo: "nube", nube: n.nombre, carpeta: "Resguardo" };
    }
    // Desde una copia: después de cada copia nueva de ese repositorio.
    if (repo) p.cuando = { tras_copia: true };
    return p;
  }
  // Se leen cuando aparece cada lista (con el equipo ya cargado).
  // (Cada lista abre «editar» solo si ese espejo es suyo.)
  const inicialAlmacen = $derived(equipo ? { nuevo: pedidoQuien !== "equipo" ? preelegido() : null, editar: q.get("editar") } : null);
  const inicialEquipo = $derived(equipo ? { nuevo: pedidoQuien === "equipo" || (!almacen && pedidoQuien !== "almacen") ? preelegido() : null, editar: q.get("editar") } : null);

  function terminado() {
    cargar();
    void cargarCliente(c);
  }
</script>

<svelte:head><title>Espejos de {equipo?.nombre ?? "equipo"} · Resguardo Server</title></svelte:head>

<div class="page">
  <Migas items={[{ texto: "Equipos", href: `/c/${c}/equipos` }, { texto: equipo?.nombre ?? "Equipo", href: `/c/${c}/equipos/${id}` }, { texto: "Espejos" }]} />
  <div class="page-top">
    <div>
      <h1>Espejos de {equipo?.nombre ?? "…"}</h1>
      <p>Copias exactas de los repositorios en otro disco o en una nube. Sin contraseñas: se copian los archivos cifrados tal cual. <Ayuda id="espejo" /></p>
    </div>
  </div>

  {#if !equipo || !actual.cliente}
    {#if error}<div class="notice notice-danger"><p>{error}</p></div>{:else}<Cargando />{/if}
  {:else}
    {#if almacen}
      <section class="seccion" aria-labelledby="t-alm">
        <div class="section-head"><h2 id="t-alm" class="section-title"><HardDrive size={18} />Hechos por el almacén</h2></div>
        {#if deAlmacen}
          <p class="faint intro">Lo que guarda {equipo.nombre} de otros equipos, a otra zona, a otro disco o a una nube conectada aquí.</p>
          <ListaEspejos cliente={actual.cliente} hace={equipo} quien="almacen" {equipos} inicial={inicialAlmacen} alTerminar={terminado} />
        {:else}
          <div class="notice notice-info"><Info size={16} /><p>Este almacén tiene un agente anterior: su espejo se cambia en <a href="/c/{c}/equipos/{id}#guarda">su ficha</a>. Actualízalo para tener varios espejos, cadenas y «Igual que el origen».</p></div>
        {/if}
      </section>
    {/if}
    {#if !almacen || propios.length || reposLocales}
    <section class="seccion" aria-labelledby="t-eq">
      <div class="section-head"><h2 id="t-eq" class="section-title"><CloudUpload size={18} />Hechos por este equipo</h2></div>
      {#if delEquipo}
        <p class="faint intro">Los repositorios de sus propios discos, a otro disco o a una nube conectada en {equipo.nombre}. Funciona solo con el agente.</p>
        <ListaEspejos cliente={actual.cliente} hace={equipo} quien="equipo" {equipos} inicial={inicialEquipo} alTerminar={terminado} />
      {:else}
        <div class="notice notice-info"><Info size={16} /><p>Actualiza el agente de {equipo.nombre} para que haga espejos de los repositorios de sus discos.</p></div>
      {/if}
    </section>
    {/if}
  {/if}
</div>

<style>
  .seccion {
    display: grid;
    gap: 10px;
    margin-top: 18px;
  }
  .intro {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .seccion h2 {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
</style>
