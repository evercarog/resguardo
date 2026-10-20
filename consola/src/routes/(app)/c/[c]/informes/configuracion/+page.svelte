<script lang="ts">
  // 0.8.1 (H5, plan-mejoras 15tt): el informe de la configuración de copias, en PDF
  // (para leer, imprimir y mandar al cliente) y CSV (una fila por copia y sitio, para
  // Excel). Se arma en el navegador (lib/informeConfig.ts): con la clave de
  // administración salen también las carpetas, las exclusiones y lo de «antes de
  // copiar», que van cifrados en el servidor. Sin contraseñas ni claves.
  import { untrack } from "svelte";
  import { FileDown, ListTree, Sheet } from "@lucide/svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import CampoClave from "$lib/componentes/CampoClave.svelte";
  import { actual, app, reloj } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { cargarInformes, ultimos } from "$lib/informes.svelte";
  import { catalogoDe, cargarCatalogo } from "$lib/catalogoDestinos.svelte";
  import { cargarFuncionales, listaFuncionales } from "$lib/funcionales.svelte";
  import { gruposExclusiones } from "$lib/gruposExclusiones";
  import { sedeDeEquipo } from "$lib/sedes";
  import { copiasInforme, csvConfig, documentoConfig } from "$lib/informeConfig";
  import { configsDescifradas, bajar } from "$lib/informeConfigCargar";
  import { pdfDe } from "$lib/pdfTexto";
  import { nombreArchivo } from "$lib/csv";
  import { fechaLarga, plural } from "$lib/formato";
  import { diaLocal } from "$lib/pruebasFuncionales";
  import type { Configuracion } from "$lib/tipos";

  const equiposCliente = $derived(actual.equipos.filter((e) => e.confirmado !== false && e.modo !== "trasladado"));
  $effect(() => {
    const [cc, ids] = [actual.id, actual.equipos.map((e) => e.id)];
    if (!cc) return;
    untrack(() => {
      void cargarInformes(cc, ids);
      void cargarCatalogo(cc);
      void cargarFuncionales(cc);
    });
  });

  // Qué equipos: todos, los de una sede o uno.
  let alcance = $state("todos");
  const sedes = $derived(actual.organizacion?.sedes ?? []);
  const elegidos = $derived.by(() => {
    if (alcance.startsWith("sede:")) {
      const id = alcance.slice(5);
      return equiposCliente.filter((e) => sedeDeEquipo(actual.organizacion, e.id).sede?.id === id);
    }
    if (alcance.startsWith("equipo:")) return equiposCliente.filter((e) => e.id === alcance.slice(7));
    return equiposCliente;
  });
  const textoAlcance = $derived(
    alcance.startsWith("sede:") ? `Sede ${sedes.find((s) => s.id === alcance.slice(5))?.nombre ?? ""}` : alcance.startsWith("equipo:") ? `Solo ${elegidos[0]?.nombre ?? ""}` : "Todos los equipos",
  );
  const nCopias = $derived(elegidos.reduce((n, e) => n + (e.resumen?.copias?.length ?? 0), 0));

  let clave = $state("");
  let ocupado = $state(false);
  let paso = $state("");
  let faltan = $state<string[]>([]);

  async function hacer(formato: "pdf" | "csv") {
    if (!actual.cliente) return;
    ocupado = true;
    faltan = [];
    try {
      let configs: Record<string, Configuracion | null> = {};
      if (clave) {
        const r = await configsDescifradas(actual.cliente, elegidos, clave, (t) => (paso = t));
        configs = r.configs;
        faltan = r.faltan;
      }
      paso = "Preparando el informe…";
      const copias = copiasInforme({
        equipos: elegidos,
        todos: actual.equipos,
        informes: ultimos.porEquipo,
        catalogo: catalogoDe(actual.id),
        organizacion: actual.organizacion,
        funcionales: listaFuncionales(actual.id),
        infra: actual.infra,
        configs,
        grupos: gruposExclusiones(actual),
        ahora: reloj.ahora,
      });
      const hoy = diaLocal(reloj.ahora);
      const nombre = nombreArchivo(["Resguardo", actual.cliente.nombre, "configuración de copias", hoy], formato);
      if (formato === "csv") await bajar(csvConfig(copias), "text/csv;charset=utf-8", nombre);
      else {
        const generado = `Generado el ${fechaLarga(new Date(reloj.ahora).toISOString())}${app.cuenta?.nombre ? ` por ${app.cuenta.nombre}` : ""}`;
        await bajar(pdfDe(documentoConfig(copias, { cliente: actual.cliente.nombre, generado, alcance: textoAlcance, conClave: !!clave })) as BlobPart, "application/pdf", nombre);
      }
      avisar(formato === "pdf" ? "Informe en PDF listo." : "Informe en CSV listo.");
    } catch (e) {
      avisar((e as Error).message, "bad");
    } finally {
      clave = "";
      paso = "";
      ocupado = false;
    }
  }
</script>

<svelte:head><title>Configuración de copias · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <CabeceraPagina
    titulo="Configuración de copias"
    icono={ListTree}
    migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${actual.id}` }, { texto: "Informes", href: `/c/${actual.id}/informes` }, { texto: "Configuración de copias" }]}
    resumen="Qué se copia de cada equipo, cada cuánto, adónde, qué se conserva y cómo está, para leerlo, imprimirlo o mandarlo al cliente. Sin contraseñas ni claves."
  />

  <section class="card p" aria-labelledby="t-conf">
    <h2 class="section-title" id="t-conf">Descargar el informe</h2>
    <form
      class="form"
      onsubmit={(e) => {
        e.preventDefault();
        void hacer("pdf");
      }}
    >
      <div class="field">
        <label class="field-label" for="conf-alcance">Qué equipos</label>
        <select id="conf-alcance" class="input" bind:value={alcance}>
          <option value="todos">Todos los equipos ({equiposCliente.length})</option>
          {#if sedes.length}
            <optgroup label="Por sede">
              {#each sedes as s (s.id)}<option value="sede:{s.id}">Sede {s.nombre}</option>{/each}
            </optgroup>
          {/if}
          <optgroup label="Un equipo">
            {#each equiposCliente as e (e.id)}<option value="equipo:{e.id}">{e.nombre}</option>{/each}
          </optgroup>
        </select>
        <p class="hint">{plural(elegidos.length, "equipo", "equipos")} · {plural(nCopias, "copia", "copias")}</p>
      </div>
      <CampoClave
        id="conf-clave"
        etiqueta="Clave de administración (opcional)"
        bind:value={clave}
        ayuda="Con la clave salen también las carpetas, lo que no se copia y lo de «antes de copiar»: van cifrados y solo se abren aquí, en tu navegador. Sin ella, el informe dice cuántas carpetas hay."
      />
      {#if faltan.length}
        <div class="notice notice-warn" role="status"><p>Sin su configuración: {faltan.join(" ")}</p></div>
      {/if}
      <div class="acciones">
        <button type="submit" class="btn btn-primary" disabled={ocupado || !elegidos.length}><FileDown size={16} />Descargar en PDF</button>
        <button type="button" class="btn" disabled={ocupado || !elegidos.length} onclick={() => hacer("csv")}><Sheet size={16} />Descargar en CSV</button>
        {#if ocupado}<span class="faint" role="status">{paso}</span>{/if}
      </div>
      <p class="hint">El CSV tiene una fila por copia y por cada sitio de su camino (su repositorio, cada copia derivada y cada espejo), separada por «;», para abrirla en Excel o LibreOffice.</p>
    </form>
  </section>
</div>

<style>
  .acciones {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
</style>
