<script lang="ts">
  // El panel de detalle de un repositorio (en su página y en la de cada
  // copia): lo que diga la URL (`?v=…&vista=…`, ver `$lib/detalle`) se abre en
  // un panel lateral con migas y «Atrás». Las vistas con nombres de archivo
  // («Qué cambió», «Lo que más ocupa», las versiones de un archivo) usan una
  // sola sesión cifrada con el equipo, que se abre con la contraseña del
  // repositorio la primera vez y se cierra al salir de la página.
  import { onDestroy } from "svelte";
  import { page } from "$app/state";
  import { AccesoRepo } from "$lib/accesoRepo.svelte";
  import { anteriorDeLaCopia, leerSeleccion, migasDe, partesRuta, versionPorId, vueltaPorHora, type Seleccion } from "$lib/detalle";
  import { fechaLarga, fechaCorta } from "$lib/formato";
  import { versionesDe } from "$lib/repo";
  import { infCopia } from "$lib/copia";
  import type { Cliente, CopiaResumen, EntradaHistorial, Equipo, RepoInforme, RepositorioResumen, VersionInforme } from "$lib/tipos";
  import Cajon from "./Cajon.svelte";
  import CambiosVersion from "./CambiosVersion.svelte";
  import DetalleVersion from "./DetalleVersion.svelte";
  import DetalleVuelta from "./DetalleVuelta.svelte";
  import EspacioRepo from "./EspacioRepo.svelte";
  import EstadoRepo from "./EstadoRepo.svelte";
  import OcupaVersion from "./OcupaVersion.svelte";
  import VersionesArchivo from "./VersionesArchivo.svelte";
  import { atras, cerrar, ir } from "./navegar";

  let {
    cliente,
    equipo,
    repo,
    inf,
    copias,
    historial = [],
    puedeRestaurar,
    ahora,
    soloCopia = null,
  }: {
    cliente: Cliente;
    equipo: Equipo;
    repo: RepositorioResumen;
    inf: RepoInforme | null;
    copias: CopiaResumen[];
    historial?: EntradaHistorial[];
    puedeRestaurar: boolean;
    ahora: number;
    /** En la página de una copia: «¿Por qué?» cuenta solo sus vueltas. */
    soloCopia?: string | null;
  } = $props();

  const sel: Seleccion = $derived(leerSeleccion(page.url.searchParams));
  const versiones = $derived(versionesDe(inf));
  const ejecuciones = $derived(inf?.ejecuciones ?? []);
  const version = $derived(versionPorId(versiones, sel.version));
  const vuelta = $derived(vueltaPorHora(ejecuciones, sel.vuelta));
  const base = $derived(`/c/${cliente.id}/equipos/${equipo.id}`);
  const enlaceCopia = (k: string) => `${base}/copias/${encodeURIComponent(k)}`;
  const enlaceRestaurar = (v: VersionInforme, todo: boolean) => `/c/${cliente.id}/restaurar?${new URLSearchParams({ equipo: equipo.id, repo: repo.id, version: v.id, ...(todo ? { todo: "1" } : {}) })}`;

  // Una sesión para todo el panel (y mientras dure la página).
  let acceso: AccesoRepo | null = null;
  function elAcceso(): AccesoRepo {
    if (!acceso || acceso.equipo.id !== equipo.id || acceso.repo !== repo.id) {
      acceso?.cerrar();
      acceso = new AccesoRepo(cliente, equipo, repo.id);
    }
    return acceso;
  }
  onDestroy(() => acceso?.cerrar());

  /** Las versiones de un archivo (sin URL: es un nombre de archivo). */
  let archivo = $state<string | null>(null);
  let claveArchivo = "";
  $effect(() => {
    const k = page.url.search;
    if (k !== claveArchivo) {
      claveArchivo = k;
      archivo = null;
    }
  });

  const nombreVersion = (id: string) => {
    const v = versionPorId(versiones, id);
    return v ? `Versión del ${fechaCorta(v.hora)}` : `Versión ${id.slice(0, 8)}`;
  };
  const migas = $derived([
    ...migasDe(sel, nombreVersion).map((m) => ({ texto: m.texto, ir: () => ir(m.sel) })),
    ...(archivo ? [{ texto: partesRuta(archivo).nombre, ir: undefined }] : []),
  ]);
  const titulo = $derived.by(() => {
    if (archivo) return "Versiones de este archivo";
    switch (sel.vista) {
      case "version":
        return version ? `Versión del ${fechaLarga(version.hora)}` : `Versión ${sel.version?.slice(0, 8)}`;
      case "cambios":
        return "Qué cambió";
      case "ocupa":
        return "Lo que más ocupa";
      case "espacio":
        return `Espacio de «${repo.nombre}»`;
      case "vuelta":
        return vuelta?.resultado === "fallo" ? "Una copia que falló" : "Detalle de la copia";
      case "estado":
        return "¿Por qué este estado?";
      default:
        return "";
    }
  });
  function alAtras() {
    if (archivo) archivo = null;
    else atras(migas.at(-2)?.ir);
  }
</script>

{#if sel.vista}
  <Cajon {titulo} {migas} alAtras={alAtras} alCerrar={cerrar}>
    {#if archivo}
      <VersionesArchivo acceso={elAcceso()} ruta={archivo} {puedeRestaurar} />
    {:else if sel.vista === "version"}
      {#if version}
        <DetalleVersion {version} {versiones} {ejecuciones} {historial} {copias} repo={repo.id} {enlaceCopia} {enlaceRestaurar} {puedeRestaurar} />
      {:else}
        <p class="faint">Esa versión ya no está en el informe del equipo (es de hace más de 60 días o la quitó la retención). Puedes ver qué cambió en ella si aún está en el repositorio.</p>
        <button class="btn" onclick={() => ir({ vista: "cambios" })}>Qué cambió</button>
      {/if}
    {:else if sel.vista === "cambios" && sel.version}
      <CambiosVersion
        acceso={elAcceso()}
        nombreRepo={repo.nombre}
        version={version ?? { id: sel.version }}
        anterior={version ? anteriorDeLaCopia(versiones, version) : null}
        con={sel.con}
        filtro={sel.filtro}
        {versiones}
        {puedeRestaurar}
        alVerArchivo={(r) => (archivo = r)}
      />
    {:else if sel.vista === "ocupa" && sel.version}
      <OcupaVersion acceso={elAcceso()} nombreRepo={repo.nombre} version={sel.version} alVerArchivo={(r) => (archivo = r)} />
    {:else if sel.vista === "espacio"}
      <EspacioRepo {repo} {inf} {ahora} />
    {:else if sel.vista === "vuelta"}
      {#if vuelta}
        <DetalleVuelta {vuelta} {versiones} {ejecuciones} {historial} {copias} repo={repo.id} {enlaceCopia} />
      {:else}
        <p class="faint">Esa copia ya no está en el informe del equipo (solo trae los últimos 60 días).</p>
      {/if}
    {:else if sel.vista === "estado"}
      <EstadoRepo inf={soloCopia ? infCopia(inf, soloCopia) : inf} {ahora} />
    {/if}
  </Cajon>
{/if}
