<script lang="ts">
  // Las etiquetas del cliente con sus ajustes (v1.52, tarea 6): color,
  // plantilla para los equipos nuevos y avisos. Cada una se cambia en su
  // diálogo (AjustesEtiqueta). Se abre desde el filtro de etiquetas.
  import { BellRing, LayoutTemplate, Settings2, Tags } from "@lucide/svelte";
  import { untrack } from "svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { actual } from "$lib/estado.svelte";
  import { ajusteDe, colorDe, conAvisos, etiquetasDe, NOMBRES_COLOR } from "$lib/etiquetasGrupos";
  import { plural } from "$lib/formato";
  import EtiquetaChip from "./EtiquetaChip.svelte";
  import AjustesEtiqueta from "./AjustesEtiqueta.svelte";

  let { onclose, inicial = null }: { onclose: () => void; inicial?: string | null } = $props();

  const todas = $derived(etiquetasDe(actual.equipos));
  // Con `inicial`, directamente a esa etiqueta (y al cerrarla, se cierra todo).
  let editar = $state<{ nombre: string; n: number } | null>(
    untrack(() => (inicial ? { nombre: inicial, n: etiquetasDe(actual.equipos).find((t) => t.nombre.toLowerCase() === inicial.toLowerCase())?.n ?? 0 } : null)),
  );
</script>

{#if editar}
  <AjustesEtiqueta nombre={editar.nombre} n={editar.n} onclose={() => (inicial ? onclose() : (editar = null))} />
{:else}
  <Modal labelledby="t-etiquetas-cliente" {onclose} width={560}>
    <div class="dlg-title">
      <span class="ticon"><Tags size={18} /></span>
      <div>
        <h2 id="t-etiquetas-cliente">Etiquetas de {actual.cliente?.nombre ?? "este cliente"}</h2>
        <p>Elige su color, la plantilla que se propone a sus equipos nuevos y cómo se avisa de ellos. Las etiquetas se ponen en la ficha de cada equipo.</p>
      </div>
    </div>
    {#if todas.length}
      <ul class="lista-et">
        {#each todas as t (t.nombre)}
          {@const a = ajusteDe(t.nombre, actual.etiquetas)}
          <li>
            <span class="nombre"><EtiquetaChip nombre={t.nombre} /><span class="faint">{plural(t.n, "equipo", "equipos")}</span></span>
            <span class="que faint">
              <span>{NOMBRES_COLOR[colorDe(t.nombre, actual.etiquetas)]}{a?.color == null ? " (automático)" : ""}</span>
              {#if a?.plantilla}<span class="dato"><LayoutTemplate size={12} />plantilla</span>{/if}
              {#if conAvisos(a?.avisos)}<span class="dato"><BellRing size={12} />{a?.avisos?.importancia === "critico" ? "como críticos" : "avisos propios"}</span>{/if}
            </span>
            <button type="button" class="btn btn-sm" onclick={() => (editar = t)} aria-label="Ajustar «{t.nombre}»"><Settings2 size={14} />Ajustar</button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="faint">Ningún equipo tiene etiquetas todavía.</p>
    {/if}
    <footer>
      <button type="button" class="btn btn-primary" onclick={onclose}>Cerrar</button>
    </footer>
  </Modal>
{/if}

<style>
  .lista-et {
    display: flex;
    flex-direction: column;
    max-height: 55vh;
    overflow: auto;
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .lista-et li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 12px;
    padding: 10px var(--sp-3);
    border-top: 1px solid var(--border);
    font-size: var(--fs-sm);
  }
  .lista-et li:first-child {
    border-top: none;
  }
  .nombre {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .que {
    display: inline-flex;
    flex: 1;
    flex-wrap: wrap;
    gap: 4px 10px;
    min-width: 140px;
  }
  .dato {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
</style>
