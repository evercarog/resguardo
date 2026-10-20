<script lang="ts">
  // 0.8.1 (B2, plan-mejoras 15a): todas las etiquetas del cliente, con cuántos equipos
  // la llevan (enlace a Equipos filtrado por ella), su color y su categoría. Renombrar,
  // unir y borrar en todos sus equipos (CambiarEtiqueta), y su color, plantilla y avisos
  // (AjustesEtiqueta, el de siempre). Editar: administradores y propietarios.
  import { BellRing, Combine, LayoutTemplate, PenLine, Settings2, Trash2 } from "@lucide/svelte";
  import EtiquetaChip from "../EtiquetaChip.svelte";
  import AjustesEtiqueta from "../AjustesEtiqueta.svelte";
  import MenuAcciones from "../MenuAcciones.svelte";
  import CambiarEtiqueta from "./CambiarEtiqueta.svelte";
  import { actual } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { plural } from "$lib/formato";
  import { ajusteDe, etiquetasDe, filtroEtiqueta, mismaEtiqueta, NOMBRES_COLOR } from "$lib/etiquetas.svelte";
  import { colorDe, conAvisos } from "$lib/etiquetasGrupos";
  import { categoriaDe } from "$lib/sedes";
  import { guardarOrganizacion } from "$lib/organizacion.svelte";
  import { conPedidas } from "$lib/gestionEtiquetas.svelte";

  let { editar }: { editar: boolean } = $props();

  const todas = $derived(etiquetasDe(conPedidas(actual.equipos)).sort((a, b) => a.nombre.localeCompare(b.nombre, "es", { sensitivity: "base" })));
  const categorias = $derived([...actual.organizacion.categorias].sort((a, b) => a.nombre.localeCompare(b.nombre, "es")));

  let ajustar = $state<{ nombre: string; n: number } | null>(null);
  let cambiar = $state<{ modo: "renombrar" | "unir" | "borrar"; nombre: string } | null>(null);
  let guardandoCat = $state("");

  /** Poner la etiqueta en otra categoría (o en ninguna): sale de la suya y entra en la elegida. */
  async function ponerCategoria(nombre: string, id: string) {
    const entradas: { clave: string; valor: unknown }[] = [];
    for (const c of categorias) {
      const tiene = c.etiquetas.some((t) => mismaEtiqueta(t, nombre));
      if (tiene && c.id !== id) entradas.push({ clave: `etiqueta.categoria:${c.id}`, valor: { nombre: c.nombre, etiquetas: c.etiquetas.filter((t) => !mismaEtiqueta(t, nombre)) } });
      if (!tiene && c.id === id) entradas.push({ clave: `etiqueta.categoria:${c.id}`, valor: { nombre: c.nombre, etiquetas: [...c.etiquetas, nombre] } });
    }
    if (!entradas.length) return;
    guardandoCat = nombre;
    try {
      await guardarOrganizacion(actual.id, entradas);
      const c = categorias.find((x) => x.id === id);
      avisar(c ? `«${nombre}» ahora es de la categoría «${c.nombre}».` : `«${nombre}» ya no es de ninguna categoría.`);
    } catch (err) {
      fallo(err);
    } finally {
      guardandoCat = "";
    }
  }
</script>

<p class="faint intro">Las etiquetas de los equipos de este cliente. Cambiarlas aquí las cambia en todos sus equipos: una orden a cada uno, que queda en su historial y que ven igual todas sus consolas.</p>
{#if todas.length}
  <ul class="org-lista">
    {#each todas as t (t.nombre)}
      {@const a = ajusteDe(t.nombre, actual.etiquetas)}
      {@const cat = categoriaDe(actual.organizacion, t.nombre)}
      <li class="et-fila">
        <span class="txt">
          <span class="linea">
            <EtiquetaChip nombre={t.nombre} />
            <a class="n" href="/c/{actual.id}/equipos" onclick={() => filtroEtiqueta.poner(t.nombre)}>{plural(t.n, "equipo", "equipos")}</a>
          </span>
          <span class="faint">
            {NOMBRES_COLOR[colorDe(t.nombre, actual.etiquetas)]}{a?.color == null ? " (automático)" : ""}
            {#if !editar || !categorias.length}{" · "}{cat ? `categoría «${cat.nombre}»` : "sin categoría"}{/if}
            {#if a?.plantilla}<span class="dato"><LayoutTemplate size={12} />plantilla</span>{/if}
            {#if conAvisos(a?.avisos)}<span class="dato"><BellRing size={12} />avisos propios</span>{/if}
          </span>
        </span>
        {#if editar}
          <span class="acc">
            {#if categorias.length}
              <select class="input cat" aria-label="Categoría de «{t.nombre}»" value={cat?.id ?? ""} disabled={guardandoCat === t.nombre} onchange={(e) => ponerCategoria(t.nombre, e.currentTarget.value)}>
                <option value="">Sin categoría</option>
                {#each categorias as c (c.id)}<option value={c.id}>{c.nombre}</option>{/each}
              </select>
            {/if}
            <button type="button" class="btn btn-sm" onclick={() => (ajustar = t)} aria-label="Color, plantilla y avisos de «{t.nombre}»"><Settings2 size={14} />Ajustar</button>
            <MenuAcciones
              etiqueta="Más acciones para «{t.nombre}»"
              texto=""
              grupos={[
                [
                  { texto: "Renombrar…", icono: PenLine, onclick: () => (cambiar = { modo: "renombrar", nombre: t.nombre }) },
                  { texto: "Unir con otra…", icono: Combine, onclick: () => (cambiar = { modo: "unir", nombre: t.nombre }), disabled: todas.length < 2, detalle: todas.length < 2 ? "No hay otra etiqueta" : undefined },
                ],
                [{ texto: `Quitar de ${plural(t.n, "equipo", "equipos")}…`, icono: Trash2, peligro: true, onclick: () => (cambiar = { modo: "borrar", nombre: t.nombre }) }],
              ]}
            />
          </span>
        {/if}
      </li>
    {/each}
  </ul>
{:else}
  <p class="vacio">Ningún equipo tiene etiquetas todavía. Se ponen en la ficha de cada equipo o con «Varios a la vez» en Equipos.</p>
{/if}

{#if ajustar}<AjustesEtiqueta nombre={ajustar.nombre} n={ajustar.n} onclose={() => (ajustar = null)} />{/if}
{#if cambiar}<CambiarEtiqueta modo={cambiar.modo} nombre={cambiar.nombre} onclose={() => (cambiar = null)} />{/if}

<style>
  .linea {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .n {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .n:hover {
    color: var(--accent);
  }
  .dato {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    margin-left: 8px;
  }
  .cat {
    width: auto;
    min-width: 0;
    max-width: 150px;
    height: 28px;
    padding-top: 0;
    padding-bottom: 0;
    font-size: var(--fs-sm);
  }
</style>
