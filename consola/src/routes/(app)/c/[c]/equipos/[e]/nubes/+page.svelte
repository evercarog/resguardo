<script lang="ts">
  // Nubes conectadas en un equipo (plan 0.8.1, B3; plan-mejoras 15s), como la página de sus
  // espejos: cada nube con su tipo, bucket y carpeta base, si es inmutable, qué la usa, lo que
  // ocupa y el estado de la conexión, y su página propia. Las credenciales nunca salen del equipo.
  import { page } from "$app/state";
  import { untrack } from "svelte";
  import { Cloud, Plus } from "@lucide/svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import ConectarNube from "$lib/componentes/ConectarNube.svelte";
  import ConectarDestino from "$lib/componentes/ConectarDestino.svelte";
  import NubesConectadas from "$lib/componentes/almacen/NubesConectadas.svelte";
  import * as api from "$lib/api";
  import { actual, puede } from "$lib/estado.svelte";
  import { seguirCambios, tocaEquipo } from "$lib/vivo.svelte";
  import { catalogoDe, cargarCatalogo } from "$lib/catalogoDestinos.svelte";
  import { nubesVista } from "$lib/nubesAlmacen";
  import { nubesPorAnular } from "$lib/nubesEquipo";
  import { admiteMasDestinos } from "$lib/espejo";
  import { fechaLarga, plural } from "$lib/formato";
  import type { EquipoDetalle } from "$lib/tipos";

  const c = $derived(page.params.c ?? "");
  const id = $derived(page.params.e ?? "");
  let equipo = $state<EquipoDetalle | null>(null);
  let error = $state("");
  let conectar = $state(false);

  $effect(() => {
    const [cc, ee] = [c, id];
    untrack(() => cargar(cc, ee));
  });
  function cargar(cc = c, ee = id) {
    api.equipo(cc, ee).then((e) => ee === id && (equipo = e), (e) => (error = e.message));
  }
  $effect(() => {
    const [cc, ee] = [c, id];
    return untrack(() =>
      seguirCambios(() => api.equipo(cc, ee).then((e) => ee === id && (equipo = e), () => {}), {
        ms: 0,
        toca: (x) => (x.t === "informe" || x.t === "equipo" || x.t === "orden") && tocaEquipo(x, ee),
      }),
    );
  });
  $effect(() => {
    const cc = c;
    if (cc) untrack(() => void cargarCatalogo(cc));
  });

  const equipos = $derived(equipo ? [...actual.equipos.filter((x) => x.id !== equipo!.id), equipo] : actual.equipos);
  const nubes = $derived(equipo ? nubesVista(equipo, equipos, catalogoDe(c), c) : []);
  const porAnular = $derived(nubesPorAnular(equipo));
  const admin = $derived(!!actual.cliente && puede.administrar(actual.cliente.rol));
</script>

<svelte:head><title>Nubes de {equipo?.nombre ?? "equipo"} · Resguardo Server</title></svelte:head>

<div class="page">
  <CabeceraPagina
    titulo="Nubes de {equipo?.nombre ?? '…'}"
    icono={Cloud}
    migas={[{ texto: "Equipos", href: `/c/${c}/equipos` }, { texto: equipo?.nombre ?? "Equipo", href: `/c/${c}/equipos/${id}` }, { texto: "Nubes conectadas" }]}
    resumen={equipo ? (nubes.length ? `${plural(nubes.length, "nube conectada", "nubes conectadas")} en ${equipo.nombre}. Sus credenciales se quedan en el equipo.` : `${equipo.nombre} no tiene ninguna nube conectada.`) : "Cargando…"}
  >
    {#snippet acciones()}
      {#if admin && equipo && actual.cliente}<button class="btn btn-primary" onclick={() => (conectar = true)}><Plus size={16} />Conectar una nube</button>{/if}
    {/snippet}
  </CabeceraPagina>

  {#if !equipo || !actual.cliente}
    {#if error}<div class="notice notice-danger"><p>{error}</p></div>{:else}<Cargando />{/if}
  {:else}
    <section aria-labelledby="t-nubes">
      <div class="section-head"><h2 id="t-nubes">Nubes <span class="count">· {nubes.length}</span> <Ayuda id="espejo" /></h2></div>
      {#if nubes.length}
        <div class="card p"><NubesConectadas cliente={c} equipo={equipo.id} {nubes} /></div>
        <p class="faint nota">Para desconectar una, entra en su página: solo se puede si nada la usa.</p>
      {:else}
        <div class="card"><Vacio icono={Cloud} titulo="Ninguna nube conectada" texto="Conecta Dropbox, Backblaze B2, S3, SFTP, una carpeta de red o WebDAV para usarla como destino de un espejo o de una copia." /></div>
      {/if}
      {#each porAnular as n (n.nombre)}
        <div class="notice notice-warn"><p><strong>«{n.nombre}»</strong>: credenciales borradas de este equipo; no se pudo anular el permiso en {n.tipo === "dropbox" ? "Dropbox" : "el proveedor"} todavía: se reintentará{#if n.hasta}{" "}hasta el {fechaLarga(n.hasta)}{/if}.</p></div>
      {/each}
    </section>
  {/if}
</div>

{#if conectar && equipo && actual.cliente}
  {#if admiteMasDestinos(equipo)}
    <ConectarDestino cliente={actual.cliente} {equipo} onclose={() => ((conectar = false), cargar())} />
  {:else}
    <ConectarNube cliente={actual.cliente} {equipo} onclose={() => ((conectar = false), cargar())} />
  {/if}
{/if}

<style>
  section {
    display: grid;
    gap: 10px;
    margin-top: var(--sp-6);
  }
  .nota {
    margin: 0;
    font-size: var(--fs-sm);
  }
</style>
