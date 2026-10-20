<script lang="ts">
  // Plan 0.8.0, D1 (14ll); 0.8.1, B2: las sedes del cliente (nombre, notas y la
  // principal), en «Organización» (antes, el diálogo «Sedes y categorías»).
  // Datos comunes: los ven igual todas las consolas. Editar: administradores y propietarios.
  import { Check, Pencil, Plus, Trash2, TriangleAlert, X } from "@lucide/svelte";
  import Ayuda from "../Ayuda.svelte";
  import BotonCargando from "../BotonCargando.svelte";
  import { actual } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { plural } from "$lib/formato";
  import { errorSede, sedePorDefecto, type Sede } from "$lib/sedes";
  import { guardarSede, quitarSede } from "$lib/organizacion.svelte";

  let { editar }: { editar: boolean } = $props();

  const org = $derived(actual.organizacion);
  const sedes = $derived([...org.sedes].sort((a, b) => a.nombre.localeCompare(b.nombre, "es")));
  const porDefecto = $derived(sedePorDefecto(org));
  const cuantos = (s: Sede) => actual.equipos.filter((e) => org.equipos[e.id] === s.id).length;
  const destinosDe = (s: Sede) => Object.values(org.destinos).filter((x) => x === s.id).length;

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
</script>

<p class="faint intro">Los sitios del cliente: cada equipo y cada destino está en uno. Una copia está fuera del sitio si su destino está en otra sede que el equipo que copia; la nube, siempre. <Ayuda id="sedes" /></p>
{#if sedes.length}
  <ul class="org-lista">
    {#each sedes as s (s.id)}
      <li>
        <span class="txt">
          <strong>{s.nombre}{#if porDefecto?.id === s.id}<span class="pastilla">principal</span>{/if}</strong>
          <span class="faint">{plural(cuantos(s), "equipo asignado", "equipos asignados")}{#if destinosDe(s)}{" · "}{plural(destinosDe(s), "destino", "destinos")}{/if}{#if s.notas}{" · "}{s.notas}{/if}</span>
        </span>
        {#if editar}
          {#if quitando?.id === s.id}
            <span class="conf">
              <button class="btn btn-sm btn-ghost" onclick={() => (quitando = null)}>No</button>
              <button class="btn btn-sm btn-danger" disabled={ocupado} onclick={() => quitar(s)}>Sí, quitar</button>
            </span>
          {:else}
            <span class="acc">
              <button class="icon-btn" aria-label="Cambiar {s.nombre}" onclick={() => (editando = { id: s.id, nombre: s.nombre, notas: s.notas ?? "", principal: s.principal })}><Pencil size={15} /></button>
              <button class="icon-btn" aria-label="Quitar {s.nombre}" onclick={() => (quitando = s)}><Trash2 size={15} /></button>
            </span>
          {/if}
        {/if}
      </li>
    {/each}
  </ul>
{:else}
  <p class="vacio">Sin sedes, todo funciona como siempre: el tipo de cada destino (Local, Fuera del sitio o Nube) dice si está fuera.</p>
{/if}
{#if editar}
  {#if editando}
    <form class="form org-editor" onsubmit={guardar}>
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
      <div class="org-acciones">
        <button type="button" class="btn btn-ghost" onclick={() => (editando = null)}><X size={14} />Cancelar</button>
        <BotonCargando class="btn btn-primary" disabled={!!errorEd || repetida} cargando={ocupado} textoCargando="Guardando…"><Check size={14} />{editando.id ? "Guardar" : "Añadir"}</BotonCargando>
      </div>
    </form>
  {:else}
    <button class="btn btn-sm" onclick={() => (editando = { nombre: "", notas: "", principal: !sedes.length })}><Plus size={14} />Añadir una sede</button>
  {/if}
{/if}
