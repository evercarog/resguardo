<script lang="ts">
  // Plan 0.8.0, D1 y D2: las sedes del cliente (nombre, notas y la principal) y las
  // categorías de etiqueta (un solo valor por equipo). Datos comunes: los ven igual
  // todas las consolas. Solo administradores y propietarios.
  import { Building2, Check, Pencil, Plus, Tags, Trash2, TriangleAlert, X } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import Ayuda from "./Ayuda.svelte";
  import BotonCargando from "./BotonCargando.svelte";
  import { actual } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { plural } from "$lib/formato";
  import { etiquetasDe } from "$lib/etiquetas.svelte";
  import { equiposConVariasDe, errorSede, sedePorDefecto, type CategoriaEtiqueta, type Sede } from "$lib/sedes";
  import { guardarCategoria, guardarSede, quitarCategoria, quitarSede } from "$lib/organizacion.svelte";
  import { limpiarEtiqueta } from "$lib/etiquetas.svelte";

  let { onclose }: { onclose: () => void } = $props();

  const org = $derived(actual.organizacion);
  const sedes = $derived([...org.sedes].sort((a, b) => a.nombre.localeCompare(b.nombre, "es")));
  const porDefecto = $derived(sedePorDefecto(org));
  const cuantos = (s: Sede) => actual.equipos.filter((e) => org.equipos[e.id] === s.id).length;
  const destinosDe = (s: Sede) => Object.values(org.destinos).filter((x) => x === s.id).length;

  // --- Sedes ---
  let editando = $state<{ id?: string; nombre: string; notas: string; principal: boolean } | null>(null);
  let ocupado = $state(false);
  let error = $state("");
  const errorEd = $derived(editando ? errorSede(editando.nombre, editando.notas) : null);
  const repetida = $derived(!!editando && sedes.some((s) => s.id !== editando!.id && s.nombre.trim().toLowerCase() === editando!.nombre.trim().toLowerCase()));

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    if (!editando || errorEd || repetida) return;
    ocupado = true;
    error = "";
    try {
      const nueva = !editando.id;
      await guardarSede(actual.id, { ...editando, principal: editando.principal || (nueva && !sedes.length) });
      avisar(nueva ? `Sede «${editando.nombre.trim()}» añadida.` : `Sede «${editando.nombre.trim()}» guardada.`);
      editando = null;
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }
  let quitando = $state<Sede | null>(null);
  async function quitar(s: Sede) {
    ocupado = true;
    try {
      await quitarSede(actual.id, s.id);
      avisar(`Sede «${s.nombre}» quitada: sus equipos y destinos vuelven a la principal.`);
      quitando = null;
    } catch (err) {
      fallo(err);
    } finally {
      ocupado = false;
    }
  }

  // --- Categorías ---
  const categorias = $derived([...org.categorias].sort((a, b) => a.nombre.localeCompare(b.nombre, "es")));
  const todasEtiquetas = $derived(etiquetasDe(actual.equipos).map((t) => t.nombre));
  let cat = $state<{ id?: string; nombre: string; etiquetas: string[]; nueva: string } | null>(null);
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
  async function quitarCat(c: CategoriaEtiqueta) {
    ocupado = true;
    try {
      await quitarCategoria(actual.id, c.id);
      avisar(`Categoría «${c.nombre}» quitada. Sus etiquetas siguen en los equipos, ahora sueltas.`);
    } catch (err) {
      fallo(err);
    } finally {
      ocupado = false;
    }
  }
  let pestana = $state<"sedes" | "categorias">("sedes");
</script>

<Modal labelledby="t-organizar" {onclose} width={620}>
  <div class="dlg-title">
    <span class="ticon"><Building2 size={18} /></span>
    <div>
      <h2 id="t-organizar">Sedes y categorías</h2>
      <p>Lo ven igual todas las consolas del cliente.</p>
    </div>
  </div>
  <!-- G5: dos botones de verdad con aria-pressed (antes, pestañas sin flechas ni panel). -->
  <div class="segmented" role="group" aria-label="Qué organizar">
    <button type="button" aria-pressed={pestana === "sedes"} class:on={pestana === "sedes"} onclick={() => (pestana = "sedes")}><Building2 size={14} />Sedes</button>
    <button type="button" aria-pressed={pestana === "categorias"} class:on={pestana === "categorias"} onclick={() => (pestana = "categorias")}><Tags size={14} />Categorías de etiqueta</button>
  </div>

  {#if pestana === "sedes"}
    <p class="faint intro">Los sitios del cliente: cada equipo y cada destino está en uno. Una copia está fuera del sitio si su destino está en otra sede que el equipo que copia; la nube, siempre. <Ayuda id="sedes" /></p>
    {#if sedes.length}
      <ul class="lista">
        {#each sedes as s (s.id)}
          <li>
            <span class="txt">
              <strong>{s.nombre}</strong>{#if porDefecto?.id === s.id}<span class="pastilla">principal</span>{/if}
              <span class="faint">{plural(cuantos(s), "equipo asignado", "equipos asignados")}{#if destinosDe(s)} · {plural(destinosDe(s), "destino", "destinos")}{/if}{#if s.notas} · {s.notas}{/if}</span>
            </span>
            {#if quitando?.id === s.id}
              <span class="conf">
                <button class="btn btn-sm btn-ghost" onclick={() => (quitando = null)}>No</button>
                <button class="btn btn-sm btn-danger" disabled={ocupado} onclick={() => quitar(s)}>Sí, quitar</button>
              </span>
            {:else}
              <button class="icon-btn" aria-label="Cambiar {s.nombre}" onclick={() => (editando = { id: s.id, nombre: s.nombre, notas: s.notas ?? "", principal: s.principal })}><Pencil size={15} /></button>
              <button class="icon-btn" aria-label="Quitar {s.nombre}" onclick={() => (quitando = s)}><Trash2 size={15} /></button>
            {/if}
          </li>
        {/each}
      </ul>
    {:else}
      <p class="vacio">Sin sedes, todo funciona como siempre: el tipo de cada destino (Local, Fuera del sitio o Nube) dice si está fuera.</p>
    {/if}
    {#if editando}
      <form class="form editor" onsubmit={guardar}>
        <div class="field">
          <label class="field-label" for="sd-nombre">Nombre</label>
          <!-- svelte-ignore a11y_autofocus -->
          <input id="sd-nombre" class="input" bind:value={editando.nombre} maxlength="60" placeholder="Oficina Norte" autofocus aria-invalid={!!(editando.nombre && (errorEd || repetida))} />
          {#if repetida}<p class="error-campo">Ya hay una sede con ese nombre.</p>{:else if editando.nombre && errorEd}<p class="error-campo">{errorEd}</p>{/if}
        </div>
        <div class="field">
          <label class="field-label" for="sd-notas">Notas <span class="faint">(opcional)</span></label>
          <textarea id="sd-notas" class="input" rows="2" maxlength="500" bind:value={editando.notas} placeholder="Dónde está, quién se encarga…"></textarea>
        </div>
        <label class="check"><input type="checkbox" bind:checked={editando.principal} />Principal: la de los equipos que aún no tienen sede</label>
        {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
        <div class="acciones">
          <button type="button" class="btn btn-ghost" onclick={() => (editando = null)}><X size={14} />Cancelar</button>
          <BotonCargando class="btn btn-primary" disabled={!!errorEd || repetida} cargando={ocupado} textoCargando="Guardando…"><Check size={14} />{editando.id ? "Guardar" : "Añadir"}</BotonCargando>
        </div>
      </form>
    {:else}
      <button class="btn btn-sm" onclick={() => (editando = { nombre: "", notas: "", principal: !sedes.length })}><Plus size={14} />Añadir una sede</button>
    {/if}
  {:else}
    <p class="faint intro">Un grupo de etiquetas con un solo valor por equipo, como «Tipo»: Servidor o Puesto. Para agrupar por niveles sin que ningún equipo salga dos veces. <Ayuda id="categorias-etiqueta" /></p>
    {#if categorias.length}
      <ul class="lista">
        {#each categorias as c (c.id)}
          {@const varias = equiposConVariasDe(c, actual.equipos)}
          <li>
            <span class="txt">
              <strong>{c.nombre}</strong>
              <span class="faint">{c.etiquetas.join(", ") || "sin etiquetas"}</span>
              {#if varias.length}<span class="aviso"><TriangleAlert size={12} />{plural(varias.length, "equipo tiene", "equipos tienen")} más de una: {varias.map((e) => e.nombre).join(", ")}. Se agrupan por la primera.</span>{/if}
            </span>
            <button class="icon-btn" aria-label="Cambiar {c.nombre}" onclick={() => (cat = { id: c.id, nombre: c.nombre, etiquetas: [...c.etiquetas], nueva: "" })}><Pencil size={15} /></button>
            <button class="icon-btn" aria-label="Quitar {c.nombre}" disabled={ocupado} onclick={() => quitarCat(c)}><Trash2 size={15} /></button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="vacio">Aún no hay categorías: las etiquetas son todas sueltas.</p>
    {/if}
    {#if cat}
      <form class="form editor" onsubmit={guardarCat}>
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
        <div class="acciones">
          <button type="button" class="btn btn-ghost" onclick={() => (cat = null)}><X size={14} />Cancelar</button>
          <BotonCargando class="btn btn-primary" disabled={!!errorCat} cargando={ocupado} textoCargando="Guardando…"><Check size={14} />Guardar</BotonCargando>
        </div>
      </form>
    {:else}
      <button class="btn btn-sm" onclick={() => (cat = { nombre: "", etiquetas: [], nueva: "" })}><Plus size={14} />Añadir una categoría</button>
    {/if}
  {/if}
  <footer>
    <button class="btn btn-ghost" onclick={onclose}>Cerrar</button>
  </footer>
</Modal>

<style>
  .segmented {
    margin-bottom: var(--sp-3);
  }
  .intro {
    font-size: var(--fs-sm);
    margin: 0 0 var(--sp-3);
  }
  .lista {
    list-style: none;
    margin: 0 0 var(--sp-3);
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .lista li {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border-top: 1px solid var(--border);
  }
  .lista li:first-child {
    border-top: none;
  }
  .txt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .txt strong {
    display: inline-flex;
    gap: 6px;
    align-items: center;
  }
  .txt .faint {
    font-size: var(--fs-xs);
  }
  .aviso {
    display: inline-flex;
    gap: 4px;
    align-items: center;
    font-size: var(--fs-xs);
    color: var(--warn);
  }
  .vacio {
    font-size: var(--fs-sm);
    color: var(--text-2);
    margin: 0 0 var(--sp-3);
  }
  .editor {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--sp-4);
    margin-bottom: var(--sp-3);
  }
  .acciones {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .check {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: var(--fs-sm);
  }
  .conf {
    display: inline-flex;
    gap: 4px;
  }
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
  footer {
    display: flex;
    justify-content: flex-end;
    margin-top: var(--sp-4);
  }
</style>
