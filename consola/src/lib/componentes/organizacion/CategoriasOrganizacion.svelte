<script lang="ts">
  // Plan 0.8.0, D2 (14ss); 0.8.1, B2: las categorías de etiqueta (un solo valor por
  // equipo), en «Organización». Datos comunes: las ven igual todas las consolas.
  import { Check, Pencil, Plus, Trash2, TriangleAlert, X } from "@lucide/svelte";
  import Ayuda from "../Ayuda.svelte";
  import BotonCargando from "../BotonCargando.svelte";
  import { actual } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { plural } from "$lib/formato";
  import { etiquetasDe, limpiarEtiqueta } from "$lib/etiquetas.svelte";
  import { equiposConVariasDe, type CategoriaEtiqueta } from "$lib/sedes";
  import { guardarCategoria, quitarCategoria } from "$lib/organizacion.svelte";

  let { editar }: { editar: boolean } = $props();

  const org = $derived(actual.organizacion);
  const categorias = $derived([...org.categorias].sort((a, b) => a.nombre.localeCompare(b.nombre, "es")));
  const todasEtiquetas = $derived(etiquetasDe(actual.equipos).map((t) => t.nombre));
  let cat = $state<{ id?: string; nombre: string; etiquetas: string[]; nueva: string } | null>(null);
  let ocupado = $state(false);
  let error = $state("");
  const errorCat = $derived(cat && !limpiarEtiqueta(cat.nombre) ? "El nombre: de 1 a 32 caracteres, sin comas." : null);
  /** Otra categoría que ya tiene esa etiqueta (una etiqueta, en una sola categoría). */
  const deOtra = (t: string) => categorias.find((c) => c.id !== cat?.id && c.etiquetas.some((x) => x.toLowerCase() === t.toLowerCase()));
  function anadirEtiqueta() {
    if (!cat) return;
    const t = limpiarEtiqueta(cat.nueva);
    if (!t || cat.etiquetas.some((x) => x.toLowerCase() === t.toLowerCase())) return;
    cat.etiquetas = [...cat.etiquetas, t];
    cat.nueva = "";
  }
  const alternar = (t: string) => cat && (cat.etiquetas = cat.etiquetas.some((x) => x.toLowerCase() === t.toLowerCase()) ? cat.etiquetas.filter((x) => x.toLowerCase() !== t.toLowerCase()) : [...cat.etiquetas, t]);
  async function guardarCat(e: SubmitEvent) {
    e.preventDefault();
    if (!cat || errorCat) return;
    ocupado = true;
    error = "";
    try {
      await guardarCategoria(actual.id, { id: cat.id, nombre: cat.nombre, etiquetas: cat.etiquetas.filter((t) => !deOtra(t)) });
      avisar(`Categoría «${cat.nombre.trim()}» guardada.`);
      cat = null;
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }
  let quitando = $state<CategoriaEtiqueta | null>(null);
  async function quitarCat(c: CategoriaEtiqueta) {
    ocupado = true;
    try {
      await quitarCategoria(actual.id, c.id);
      avisar(`Categoría «${c.nombre}» quitada. Sus etiquetas siguen en los equipos, ahora sueltas.`);
      quitando = null;
    } catch (err) {
      fallo(err);
    } finally {
      ocupado = false;
    }
  }
</script>

<p class="faint intro">Un grupo de etiquetas con un solo valor por equipo, como «Tipo»: Servidor o Puesto. Para agrupar por niveles sin que ningún equipo salga dos veces. <Ayuda id="categorias-etiqueta" /></p>
{#if categorias.length}
  <ul class="org-lista">
    {#each categorias as c (c.id)}
      {@const varias = equiposConVariasDe(c, actual.equipos)}
      <li>
        <span class="txt">
          <strong>{c.nombre}</strong>
          <span class="faint">{c.etiquetas.join(", ") || "sin etiquetas"}</span>
          {#if varias.length}<span class="aviso"><TriangleAlert size={12} />{plural(varias.length, "equipo tiene", "equipos tienen")} más de una: {varias.map((e) => e.nombre).join(", ")}. Se agrupan por la primera.</span>{/if}
        </span>
        {#if editar}
          {#if quitando?.id === c.id}
            <span class="conf">
              <button class="btn btn-sm btn-ghost" onclick={() => (quitando = null)}>No</button>
              <button class="btn btn-sm btn-danger" disabled={ocupado} onclick={() => quitarCat(c)}>Sí, quitar</button>
            </span>
          {:else}
            <span class="acc">
              <button class="icon-btn" aria-label="Cambiar {c.nombre}" onclick={() => (cat = { id: c.id, nombre: c.nombre, etiquetas: [...c.etiquetas], nueva: "" })}><Pencil size={15} /></button>
              <button class="icon-btn" aria-label="Quitar {c.nombre}" onclick={() => (quitando = c)}><Trash2 size={15} /></button>
            </span>
          {/if}
        {/if}
      </li>
    {/each}
  </ul>
{:else}
  <p class="vacio">Aún no hay categorías: las etiquetas son todas sueltas.</p>
{/if}
{#if editar}
  {#if cat}
    <form class="form org-editor" onsubmit={guardarCat}>
      <div class="field">
        <label class="field-label" for="ct-nombre">Nombre</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input id="ct-nombre" class="input" bind:value={cat.nombre} maxlength="32" placeholder="Tipo" autofocus />
        {#if cat.nombre && errorCat}<p class="error-campo">{errorCat}</p>{/if}
      </div>
      <fieldset class="field">
        <legend class="field-label">Sus etiquetas (los valores)</legend>
        <div class="chips">
          {#each [...new Set([...todasEtiquetas, ...cat.etiquetas])] as t (t)}
            {@const otra = deOtra(t)}
            <label class="chip-op" class:on={cat.etiquetas.some((x) => x.toLowerCase() === t.toLowerCase())} class:apagada={!!otra}>
              <input type="checkbox" disabled={!!otra} checked={cat.etiquetas.some((x) => x.toLowerCase() === t.toLowerCase())} onchange={() => alternar(t)} />{t}{#if otra}<span class="sr-only"> (ya en «{otra.nombre}»)</span>{/if}
            </label>
          {/each}
        </div>
        <div class="con-boton">
          <input class="input" bind:value={cat.nueva} maxlength="32" placeholder="Otra etiqueta" aria-label="Otra etiqueta para la categoría" onkeydown={(e) => e.key === "Enter" && (e.preventDefault(), anadirEtiqueta())} />
          <button type="button" class="btn btn-sm" onclick={anadirEtiqueta}><Plus size={14} />Añadir</button>
        </div>
        <span class="field-hint">Una etiqueta va en una sola categoría. Al poner una a un equipo, se le quita la otra de la misma categoría.</span>
      </fieldset>
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <div class="org-acciones">
        <button type="button" class="btn btn-ghost" onclick={() => (cat = null)}><X size={14} />Cancelar</button>
        <BotonCargando class="btn btn-primary" disabled={!!errorCat} cargando={ocupado} textoCargando="Guardando…"><Check size={14} />Guardar</BotonCargando>
      </div>
    </form>
  {:else}
    <button class="btn btn-sm" onclick={() => (cat = { nombre: "", etiquetas: [], nueva: "" })}><Plus size={14} />Añadir una categoría</button>
  {/if}
{/if}

<style>
  fieldset {
    border: none;
    padding: 0;
    margin: 0;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 8px;
  }
  .chip-op {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 28px;
    padding: 0 10px;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  .chip-op.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .chip-op.apagada {
    opacity: 0.55;
    cursor: not-allowed;
  }
  .con-boton {
    display: flex;
    gap: 8px;
  }
</style>
