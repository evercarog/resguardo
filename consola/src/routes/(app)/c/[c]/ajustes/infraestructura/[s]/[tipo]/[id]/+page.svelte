<script lang="ts">
  // 0.8.3 (16d): la página de un disco físico o de un conjunto (o disco virtual) anotado en
  // Infraestructura: su ficha, lo que vive en él (equipos, volúmenes, destinos y zonas) y
  // qué se perdería si falla. Solo con lo anotado: Resguardo no ve los discos.
  import { untrack } from "svelte";
  import { page } from "$app/state";
  import { HardDrive, Layers, Monitor, TriangleAlert } from "@lucide/svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import { actual } from "$lib/estado.svelte";
  import { catalogoDe, cargarCatalogo } from "$lib/catalogoDestinos.svelte";
  import { destinosParaPasos } from "$lib/cadenas";
  import { hrefDestino } from "$lib/fichaDestino";
  import { ESTADOS_CONJUNTO, ESTADOS_DISCO, TIPOS_DISCO, detalleDisco, marcaYModelo, textoConjunto } from "$lib/infra";
  import { fichaSoporte, hrefSoporte } from "$lib/infraFicha";
  import { bytes } from "$lib/formato";

  const c = $derived(actual.id);
  $effect(() => {
    const cc = c;
    if (cc) untrack(() => void cargarCatalogo(cc));
  });
  const tipo = $derived(page.params.tipo === "conjunto" ? "conjunto" : page.params.tipo === "disco" ? "disco" : null);
  const destinos = $derived(
    destinosParaPasos(actual.equipos, catalogoDe(c)).map((d) => ({
      clave: d.clave,
      nombre: d.nombre,
      equipo: d.zona?.almacen.id ?? (d.destino?.tipo === "local" && !d.destino.red ? (actual.equipos.find((e) => d.equipos.includes(e.nombre))?.id ?? null) : null),
    })),
  );
  const ficha = $derived(
    tipo ? fichaSoporte(actual.infra, page.params.s ?? "", tipo, page.params.id ?? "", actual.equipos.filter((e) => e.confirmado && e.modo !== "trasladado"), destinos) : null,
  );
  const nombre = $derived(ficha?.disco?.nombre ?? ficha?.conjunto?.nombre ?? "");
  const volver = $derived(`/c/${c}/ajustes/infraestructura`);
  const datosDisco = $derived.by(() => {
    const d = ficha?.disco;
    if (!d) return [];
    return [
      ["Marca y modelo", marcaYModelo(d.marca, d.modelo) || null],
      ["Número de serie", d.serie],
      ["Tipo", d.tipo ? (TIPOS_DISCO.find((t) => t.id === d.tipo)?.nombre ?? null) : null],
      ["Tamaño", d.tamano ? bytes(d.tamano) : null],
      ["Bahía o ranura", d.bahia],
      ["Estado anotado", d.estado ? (ESTADOS_DISCO.find((e) => e.id === d.estado)?.nombre ?? null) : null],
      ["Notas", d.notas],
    ].filter((x): x is [string, string] => !!x[1]);
  });
  const datosConjunto = $derived.by(() => {
    const k = ficha?.conjunto;
    if (!k) return [];
    return [
      ["Forma", textoConjunto(k)],
      ["Capacidad", k.capacidad ? bytes(k.capacidad) : null],
      ["Estado anotado", k.estado ? (ESTADOS_CONJUNTO.find((e) => e.id === k.estado)?.nombre ?? null) : null],
      ["Notas", k.notas],
    ].filter((x): x is [string, string] => !!x[1]);
  });
</script>

<svelte:head><title>{nombre || "Disco"} · Infraestructura · {actual.cliente?.nombre ?? "Resguardo"}</title></svelte:head>

<div class="page">
  {#if !ficha}
    <CabeceraPagina titulo="No está anotado" icono={HardDrive} migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${c}` }, { texto: "Infraestructura", href: volver }]} />
    <Vacio titulo="Ese disco o conjunto no está en la infraestructura" texto="Puede que lo hayan quitado o que aún no se haya cargado."><a class="btn btn-sm" href={volver}>Ir a Infraestructura</a></Vacio>
  {:else}
    <CabeceraPagina
      titulo={nombre}
      icono={ficha.tipo === "disco" ? HardDrive : Layers}
      migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${c}` }, { texto: "Infraestructura", href: volver }, { texto: ficha.servidor.nombre, href: volver }, { texto: nombre }]}
      resumen={ficha.tipo === "disco" ? `Disco físico de «${ficha.servidor.nombre}»${detalleDisco(ficha.disco!) ? ` · ${detalleDisco(ficha.disco!)}` : ""}` : `Conjunto de discos de «${ficha.servidor.nombre}» · ${textoConjunto(ficha.conjunto!)}`}
    />

    <div class="rejilla">
      <section class="card p" aria-labelledby="t-ficha">
        <h2 class="section-title" id="t-ficha">Ficha</h2>
        <dl class="datos">
          {#each ficha.tipo === "disco" ? datosDisco : datosConjunto as [k, v] (k)}
            <dt>{k}</dt>
            <dd class:mono={k === "Número de serie"}>{v}</dd>
          {/each}
        </dl>
        {#if ficha.tipo === "disco" && ficha.conjuntosDelDisco.length}
          <p class="faint">Forma parte de
            {#each ficha.conjuntosDelDisco as k, n (k.id)}{n ? ", " : " "}<a class="link" href={hrefSoporte(c, ficha.servidor.id, "conjunto", k.id)}>{k.nombre}</a>{/each}.</p>
        {/if}
        {#if ficha.tipo === "conjunto"}
          {#if ficha.discosDelConjunto.length}
            <h3 class="sub">Sus discos</h3>
            <ul class="lista-s">
              {#each ficha.discosDelConjunto as d (d.id)}
                <li><HardDrive size={14} aria-hidden="true" /><a class="link" href={hrefSoporte(c, ficha.servidor.id, "disco", d.id)}>{d.nombre}</a><span class="faint">{[detalleDisco(d), d.serie ? `S/N ${d.serie}` : null].filter(Boolean).join(" · ")}</span></li>
              {/each}
            </ul>
          {:else}
            <p class="faint">Sus discos no están anotados uno a uno (en Infraestructura → el servidor → «Cambiar»).</p>
          {/if}
        {/if}
      </section>

      <section class="card p" aria-labelledby="t-falla">
        <h2 class="section-title" id="t-falla"><TriangleAlert size={16} />Si falla</h2>
        <ul class="falla">
          {#each ficha.siFalla as f (f)}<li>{f}</li>{/each}
        </ul>
        <p class="faint pequeno">Con lo anotado en Infraestructura: Resguardo no comprueba los discos.</p>
      </section>
    </div>

    <section class="card p" aria-labelledby="t-dentro">
      <h2 class="section-title" id="t-dentro">Lo que vive en él</h2>
      {#if !ficha.equipos.length && !ficha.destinos.length}
        <p class="faint">Nada anotado todavía. En Infraestructura, elige este {ficha.tipo === "disco" ? "disco" : "conjunto"} para un equipo, un volumen o un destino.</p>
      {:else}
        {#if ficha.equipos.length}
          <h3 class="sub">Equipos y volúmenes (datos originales)</h3>
          <ul class="lista-s">
            {#each ficha.equipos as x (x.equipo.id)}
              <li><Monitor size={14} aria-hidden="true" /><a class="link" href="/c/{c}/equipos/{x.equipo.id}">{x.equipo.nombre}</a><span class="faint">{x.entero ? "el equipo entero" : ""}{x.entero && x.volumenes.length ? " · " : ""}{x.volumenes.length ? `${x.volumenes.join(", ")}` : ""}</span></li>
            {/each}
          </ul>
        {/if}
        {#if ficha.destinos.length}
          <h3 class="sub">Destinos y zonas (copias)</h3>
          <ul class="lista-s">
            {#each ficha.destinos as d (d.clave)}
              <li><HardDrive size={14} aria-hidden="true" /><a class="link" href={hrefDestino(c, d.clave)}>{d.nombre}</a></li>
            {/each}
          </ul>
        {/if}
      {/if}
    </section>
  {/if}
</div>

<style>
  .rejilla {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: var(--sp-4);
    margin-bottom: var(--sp-4);
  }
  .datos {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px var(--sp-3);
    margin: 0 0 var(--sp-3);
  }
  .datos dt {
    color: var(--text-2);
  }
  .datos dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .sub {
    font-size: var(--fs-sm);
    font-weight: 600;
    margin: var(--sp-3) 0 var(--sp-2);
  }
  .lista-s {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .lista-s li {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .falla {
    margin: 0 0 var(--sp-2);
    padding-left: 1.2em;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .pequeno {
    font-size: var(--fs-sm);
    margin: 0;
  }
</style>
