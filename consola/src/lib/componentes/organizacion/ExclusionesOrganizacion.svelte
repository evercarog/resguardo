<script lang="ts">
  // 0.8.1 (B2, plan-mejoras 15y): los grupos de exclusiones del cliente («No copiar»):
  // nombre y reglas, una por línea. Vienen unos de partida (se guardan al tocarlos) y se
  // pueden crear, renombrar, cambiar y quitar. Datos comunes (`exclusiones:<id>`): los
  // ven igual todas las consolas. Solo los datos: usarlos en el editor de copias es
  // otra parte (las copias guardan sus propias reglas). Editar: administradores y propietarios.
  import { Check, ChevronDown, Pencil, Plus, RefreshCw, RotateCcw, Trash2, TriangleAlert, X } from "@lucide/svelte";
  // 0.8.1 (D2): qué copias usan cada grupo y aplicarles un cambio.
  import { copiasQueUsan } from "$lib/exclusionesCopia";
  import AplicarGrupoEnCopias from "./AplicarGrupoEnCopias.svelte";
  import BotonCargando from "../BotonCargando.svelte";
  import { actual } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { plural } from "$lib/formato";
  import { ADMITE_EXCLUSIONES, equiposQueGuardan } from "$lib/datosComunes";
  import { gruposDePartidaQuitados, gruposExclusiones, MAX_GRUPOS, MAX_LARGO_REGLA, MAX_NOMBRE_GRUPO, MAX_REGLAS_GRUPO, valorGrupo, type GrupoExclusiones } from "$lib/gruposExclusiones";
  import { guardarGrupoExclusiones, quitarGrupoExclusiones, reponerGruposDePartida } from "$lib/organizacion.svelte";

  let { editar }: { editar: boolean } = $props();

  const grupos = $derived(gruposExclusiones(actual));
  const quitados = $derived(gruposDePartidaQuitados(actual.organizacion));
  const admitidos = $derived(actual.organizacion.exclusionesAdmitidas);
  /** Equipos que guardan los datos comunes pero aún no los grupos (agente anterior). */
  const sinGuardar = $derived(equiposQueGuardan(actual.equipos).filter((e) => !e.resumen?.admite?.includes(ADMITE_EXCLUSIONES)));

  let editando = $state<{ id?: string; nombre: string; reglas: string; dePartida: boolean } | null>(null);
  let abiertos = $state<string[]>([]);
  let ocupado = $state(false);
  let error = $state("");
  const comprobado = $derived(editando ? valorGrupo(editando.nombre, editando.reglas) : null);
  const errorEd = $derived(comprobado && "error" in comprobado ? comprobado.error : null);
  const nReglas = $derived(editando ? editando.reglas.split(/\r?\n/).filter((l) => l.trim()).length : 0);
  const repetido = $derived(!!editando && grupos.some((g) => g.id !== editando!.id && g.nombre.trim().toLowerCase() === editando!.nombre.trim().toLowerCase()));

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    if (!editando || errorEd || repetido) return;
    ocupado = true;
    error = "";
    try {
      const nuevo = !editando.id;
      await guardarGrupoExclusiones(actual.id, { id: editando.id, nombre: editando.nombre, reglas: editando.reglas });
      const n = editando.id ? copiasQueUsan(actual.equipos, { id: editando.id, nombre: editando.nombre, reglas: [] }).length : 0;
      avisar(nuevo ? `Grupo «${editando.nombre.trim()}» creado.` : n ? `Grupo «${editando.nombre.trim()}» guardado. Lo ${n === 1 ? "usa 1 copia" : `usan ${n} copias`}: abajo puedes aplicarles el cambio.` : `Grupo «${editando.nombre.trim()}» guardado.`);
      editando = null;
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }
  let quitando = $state<GrupoExclusiones | null>(null);
  async function quitar(g: GrupoExclusiones) {
    ocupado = true;
    try {
      await quitarGrupoExclusiones(actual.id, g.id);
      avisar(`Grupo «${g.nombre}» quitado. Las copias que ya usaban sus reglas las conservan.`);
      quitando = null;
    } catch (err) {
      fallo(err);
    } finally {
      ocupado = false;
    }
  }
  async function reponer() {
    ocupado = true;
    try {
      await reponerGruposDePartida(actual.id);
      avisar(quitados.length === 1 ? `«${quitados[0].nombre}» vuelve a estar.` : "Los grupos de partida vuelven a estar.");
    } catch (err) {
      fallo(err);
    } finally {
      ocupado = false;
    }
  }
  const usosDe = (g: GrupoExclusiones) => copiasQueUsan(actual.equipos, g);
  const viejasDe = (g: GrupoExclusiones) => usosDe(g).filter((x) => !x.alDia);
  let aplicarA = $state<GrupoExclusiones | null>(null);
  const alternar = (id: string) => (abiertos = abiertos.includes(id) ? abiertos.filter((x) => x !== id) : [...abiertos, id]);
  const VISTAS = 8;
</script>

<p class="faint intro">Reglas de «No copiar» con nombre, para dejar fuera de las copias lo que no es de trabajo (música, vídeos, instaladores…). Cada copia guarda sus propias reglas: cambiar un grupo no cambia las copias que ya lo usan.</p>
{#if !admitidos}
  <div class="notice notice-info"><p>El servidor de esta consola aún no guarda los grupos de exclusiones: se verán los de partida, pero para cambiarlos hay que actualizarlo.</p></div>
{/if}
<ul class="org-lista">
  {#each grupos as g (g.id)}
    {@const abierto = abiertos.includes(g.id)}
    <li class="grupo">
      <span class="txt">
        <strong>{g.nombre}{#if g.dePartida}<span class="pastilla">de partida</span>{/if}</strong>
        <span class="reglas" id="reglas-{g.id}">
          {#each abierto ? g.reglas : g.reglas.slice(0, VISTAS) as r (r)}<code>{r}</code>{/each}
          {#if g.reglas.length > VISTAS}
            <button type="button" class="mas" aria-expanded={abierto} aria-controls="reglas-{g.id}" onclick={() => alternar(g.id)}>
              {abierto ? "Ver menos" : `y ${g.reglas.length - VISTAS} más`}<ChevronDown size={12} class={abierto ? "girado" : ""} />
            </button>
          {/if}
        </span>
        <span class="faint">{plural(g.reglas.length, "regla", "reglas")}{#if usosDe(g).length}{" · "}lo usa{usosDe(g).length === 1 ? "" : "n"} {plural(usosDe(g).length, "copia", "copias")}{/if}</span>
        <!-- 15y: las copias que lo usan con las reglas de antes: se les puede aplicar (una orden por equipo). -->
        {#if viejasDe(g).length}
          <span class="viejas">
            <TriangleAlert size={13} />
            <span>{plural(viejasDe(g).length, "copia tiene", "copias tienen")} sus reglas de antes ({viejasDe(g).slice(0, 3).map((x) => `«${x.copia.nombre}» de ${x.equipo.nombre}`).join(", ")}{viejasDe(g).length > 3 ? "…" : ""}).</span>
            {#if editar}<button type="button" class="btn btn-sm" onclick={() => (aplicarA = g)}><RefreshCw size={13} />Aplicarles el cambio</button>{/if}
          </span>
        {/if}
      </span>
      {#if editar && admitidos}
        {#if quitando?.id === g.id}
          <span class="conf">
            <button class="btn btn-sm btn-ghost" onclick={() => (quitando = null)}>No</button>
            <button class="btn btn-sm btn-danger" disabled={ocupado} onclick={() => quitar(g)}>Sí, quitar</button>
          </span>
        {:else}
          <span class="acc">
            <button class="icon-btn" aria-label="Cambiar «{g.nombre}»" onclick={() => (editando = { id: g.id, nombre: g.nombre, reglas: g.reglas.join("\n"), dePartida: g.dePartida })}><Pencil size={15} /></button>
            <button class="icon-btn" aria-label="Quitar «{g.nombre}»" onclick={() => (quitando = g)}><Trash2 size={15} /></button>
          </span>
        {/if}
      {/if}
    </li>
  {:else}
    <li><span class="txt faint">No hay grupos. Crea uno con sus reglas.</span></li>
  {/each}
</ul>
{#if editar && admitidos}
  {#if editando}
    <form class="form org-editor" onsubmit={guardar}>
      <div class="field">
        <label class="field-label" for="gx-nombre">Nombre</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input id="gx-nombre" class="input" bind:value={editando.nombre} maxlength={MAX_NOMBRE_GRUPO} placeholder="Archivos de obra antiguos" autofocus aria-invalid={!!(editando.nombre && repetido)} />
        {#if repetido}<p class="error-campo">Ya hay un grupo con ese nombre.</p>{/if}
      </div>
      <div class="field">
        <label class="field-label" for="gx-reglas">Reglas <span class="faint">(una por línea)</span></label>
        <textarea id="gx-reglas" class="input mono" rows="8" bind:value={editando.reglas} placeholder={"*.mp3\nDownloads/*.exe"} aria-invalid={!!(editando.reglas.trim() && errorEd)} aria-describedby="gx-ayuda"></textarea>
        {#if editando.reglas.trim() && editando.nombre.trim() && errorEd}<p class="error-campo">{errorEd}</p>{/if}
        <span class="field-hint" id="gx-ayuda">
          Como en «No copiar» de una copia: <code>*.mp3</code> deja fuera todos los .mp3, <code>Downloads/*.exe</code> solo los de las carpetas de descargas. Hasta {MAX_REGLAS_GRUPO} reglas de {MAX_LARGO_REGLA} caracteres · llevas {nReglas}.
          {#if editando.dePartida}Al guardarlo deja de ser «de partida» y se guarda como lo dejes.{/if}
        </span>
      </div>
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <div class="org-acciones">
        <button type="button" class="btn btn-ghost" onclick={() => (editando = null)}><X size={14} />Cancelar</button>
        <BotonCargando class="btn btn-primary" disabled={!!errorEd || repetido} cargando={ocupado} textoCargando="Guardando…"><Check size={14} />{editando.id ? "Guardar" : "Crear"}</BotonCargando>
      </div>
    </form>
  {:else}
    <div class="botones">
      <button class="btn btn-sm" disabled={grupos.length >= MAX_GRUPOS} onclick={() => (editando = { nombre: "", reglas: "", dePartida: false })}><Plus size={14} />Crear un grupo</button>
      {#if quitados.length}<button class="btn btn-sm btn-ghost" disabled={ocupado} onclick={reponer}><RotateCcw size={14} />Volver a poner {quitados.length === 1 ? `«${quitados[0].nombre}»` : `los de partida (${quitados.length})`}</button>{/if}
    </div>
  {/if}
{/if}
{#if aplicarA && actual.cliente}
  <AplicarGrupoEnCopias cliente={actual.cliente} equipos={actual.equipos} grupo={aplicarA} onclose={() => (aplicarA = null)} />
{/if}
{#if sinGuardar.length}
  <p class="faint nota">{plural(sinGuardar.length, "equipo tiene", "equipos tienen")} un agente anterior que aún no guarda estos grupos ({sinGuardar.map((e) => e.nombre).join(", ")}): las demás consolas los verán por los equipos actualizados.</p>
{/if}

<style>
  .reglas {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    align-items: center;
    margin: 2px 0;
  }
  .reglas code {
    font-family: var(--mono);
    font-size: var(--fs-xs);
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    overflow-wrap: anywhere;
  }
  .mas {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    border: none;
    background: none;
    padding: 0 4px;
    font-size: var(--fs-xs);
    color: var(--accent);
    cursor: pointer;
  }
  .mas :global(.girado) {
    transform: rotate(180deg);
  }
  .mono {
    font-family: var(--mono);
    font-size: var(--fs-sm);
    height: auto;
  }
  .field-hint code {
    font-family: var(--mono);
  }
  .viejas {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
    margin-top: 4px;
    font-size: var(--fs-xs);
    color: var(--warn);
  }
  .viejas > span {
    color: var(--text-2);
  }
  .botones {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .nota {
    font-size: var(--fs-xs);
    margin: var(--sp-3) 0 0;
  }
</style>
