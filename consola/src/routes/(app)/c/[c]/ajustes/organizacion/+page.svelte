<script lang="ts">
  // Plan 0.8.1, B2 (15a, 15y): «Organización» (antes, el diálogo «Sedes y
  // categorías» de Equipos): sedes, categorías, etiquetas y grupos de exclusiones.
  // Todo son datos comunes del cliente: lo ven igual todas sus consolas. La ve
  // quien puede ordenar (técnicos o más); cambiarla, administradores y propietarios.
  import { Building2, FolderX, Tags, Tag } from "@lucide/svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import IndicePagina from "$lib/componentes/IndicePagina.svelte";
  import SedesOrganizacion from "$lib/componentes/organizacion/SedesOrganizacion.svelte";
  import CategoriasOrganizacion from "$lib/componentes/organizacion/CategoriasOrganizacion.svelte";
  import EtiquetasOrganizacion from "$lib/componentes/organizacion/EtiquetasOrganizacion.svelte";
  import ExclusionesOrganizacion from "$lib/componentes/organizacion/ExclusionesOrganizacion.svelte";
  import { actual, puede } from "$lib/estado.svelte";

  const editar = $derived(puede.administrar(actual.cliente?.rol));
  const SECCIONES = [
    { id: "sedes", texto: "Sedes" },
    { id: "categorias", texto: "Categorías" },
    { id: "etiquetas", texto: "Etiquetas" },
    { id: "exclusiones", texto: "Grupos de exclusiones" },
  ];
</script>

<div class="page organizacion">
  <CabeceraPagina titulo="Organización" icono={Building2} migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${actual.id}` }, { texto: "Ajustes" }, { texto: "Organización" }]} resumen="Sedes, categorías, etiquetas y grupos de exclusiones. Lo ven igual todas las consolas del cliente." />

  {#if !actual.cargado || !actual.cliente}
    <Cargando />
  {:else}
    <IndicePagina items={SECCIONES} />
    {#if !editar}<div class="notice notice-info"><p>Solo los administradores y los propietarios del cliente pueden cambiar esto.</p></div>{/if}

    <section class="card p" id="sedes" aria-labelledby="t-sedes">
      <h2 class="section-title" id="t-sedes"><Building2 size={16} />Sedes</h2>
      <SedesOrganizacion {editar} />
    </section>

    <section class="card p" id="categorias" aria-labelledby="t-categorias">
      <h2 class="section-title" id="t-categorias"><Tags size={16} />Categorías de etiqueta</h2>
      <CategoriasOrganizacion {editar} />
    </section>

    <section class="card p" id="etiquetas" aria-labelledby="t-etiquetas">
      <h2 class="section-title" id="t-etiquetas"><Tag size={16} />Etiquetas</h2>
      <EtiquetasOrganizacion {editar} />
    </section>

    <section class="card p" id="exclusiones" aria-labelledby="t-exclusiones">
      <h2 class="section-title" id="t-exclusiones"><FolderX size={16} />Grupos de exclusiones</h2>
      <ExclusionesOrganizacion {editar} />
    </section>
  {/if}
</div>

<style>
  .organizacion section {
    margin-bottom: var(--sp-4);
    scroll-margin-top: 72px;
  }
  .section-title {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 var(--sp-2);
  }
  /* Lo común a las cuatro secciones (sus componentes, en lib/componentes/organizacion). */
  .organizacion :global(.intro) {
    font-size: var(--fs-sm);
    margin: 0 0 var(--sp-3);
  }
  .organizacion :global(.org-lista) {
    list-style: none;
    margin: 0 0 var(--sp-3);
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .organizacion :global(.org-lista > li) {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    padding: 10px 12px;
    border-top: 1px solid var(--border);
  }
  .organizacion :global(.org-lista > li:first-child) {
    border-top: none;
  }
  .organizacion :global(.org-lista .txt) {
    flex: 1 1 160px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .organizacion :global(.org-lista .txt strong) {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .organizacion :global(.org-lista .txt .faint) {
    font-size: var(--fs-xs);
  }
  .organizacion :global(.org-lista .acc),
  .organizacion :global(.org-lista .conf) {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    margin-left: auto;
  }
  /* Etiquetas: categoría, «Ajustar» y «…» en una sola línea también a 375 px. */
  .organizacion :global(.org-lista .et-fila .acc) {
    flex-wrap: nowrap;
    min-width: 0;
  }
  .organizacion :global(.aviso) {
    display: inline-flex;
    gap: 4px;
    align-items: center;
    font-size: var(--fs-xs);
    color: var(--warn);
  }
  .organizacion :global(.vacio) {
    font-size: var(--fs-sm);
    color: var(--text-2);
    margin: 0 0 var(--sp-3);
  }
  .organizacion :global(.org-editor) {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--sp-4);
    margin-bottom: var(--sp-3);
  }
  .organizacion :global(.org-acciones) {
    display: flex;
    justify-content: flex-end;
    flex-wrap: wrap;
    gap: 8px;
  }
  .organizacion :global(.check) {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: var(--fs-sm);
  }
</style>
