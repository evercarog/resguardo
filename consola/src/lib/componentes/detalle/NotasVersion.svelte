<script lang="ts">
  // Plan 0.8.0, F2: en el detalle de una versión, «Conservar para siempre»
  // (14cc) y los comentarios (14ww, parte 2). Los guarda el equipo y los ven
  // todas sus consolas; nunca van al repositorio. Quién, cuándo y texto; cambiar
  // o borrar uno, solo quien lo escribió. «Conservar» pide la clave de
  // administración y un motivo (también al quitarlo, que espera).
  import { untrack } from "svelte";
  import { MessageSquare, Pencil, Pin, PinOff, Plus, Trash2 } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { app, puede } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { pedirAlEquipo } from "$lib/pedirAlEquipo";
  import { tip } from "$lib/tooltip";
  import { admiteComentarios, admiteConservar, AVISO_SECRETOS, comentariosDe, conservadaDe, conservadaEnAlmacen, errorComentario, esMio, MAX_TEXTO } from "$lib/versionesNotas";
  import type { Cliente, Equipo, RepositorioResumen } from "$lib/tipos";
  import Tiempo from "../Tiempo.svelte";
  import BotonCargando from "../BotonCargando.svelte";
  import ConservarVersion from "./ConservarVersion.svelte";

  let { cliente, equipo: e0, equipos, repo, version }: { cliente: Cliente; equipo: Equipo; equipos: Equipo[]; repo: RepositorioResumen; version: string } = $props();

  // Lo que dice el equipo, al día tras cada cambio (sin esperar a que la página se recargue).
  let fresco = $state<Equipo | null>(null);
  const equipo = $derived(fresco && fresco.id === e0.id ? fresco : e0);
  async function recargar() {
    try {
      fresco = await api.equipo(cliente.id, e0.id);
    } catch {
      /* se verá al recargar */
    }
  }

  const rol = $derived(cliente.rol);
  const escribe = $derived(puede.ordenar(rol) && admiteComentarios(equipo));
  const comentarios = $derived(comentariosDe(equipo, repo.id, version));
  const conservada = $derived(conservadaDe(equipo, repo.id, version));
  const enAlmacen = $derived(conservada ? conservadaEnAlmacen(equipo, repo, equipos, version) : null);

  let nuevo = $state("");
  let anadiendo = $state(false);
  let editando = $state<string | null>(null);
  let textoEdicion = $state("");
  let enviando = $state(false);
  const errorNuevo = $derived(errorComentario(nuevo));
  const errorEdicion = $derived(errorComentario(textoEdicion));

  async function enviar(cuerpo: Record<string, unknown>, ok: string) {
    enviando = true;
    try {
      const r = await pedirAlEquipo(cliente, equipo, "comentario_version", { repo: repo.id, version, autor: app.cuenta?.id ?? null, ...cuerpo });
      avisar(r.hecha ? ok : r.texto);
      if (r.hecha) await recargar();
      return true;
    } catch (err) {
      fallo(err);
      return false;
    } finally {
      enviando = false;
    }
  }
  async function anadir(ev: SubmitEvent) {
    ev.preventDefault();
    if (!nuevo.trim() || errorNuevo) return;
    if (await enviar({ texto: nuevo.trim() }, "Comentario guardado: lo verán todas las consolas del equipo.")) {
      nuevo = "";
      anadiendo = false;
    }
  }
  async function guardarEdicion(ev: SubmitEvent, id: string) {
    ev.preventDefault();
    if (!textoEdicion.trim() || errorEdicion) return;
    if (await enviar({ id, texto: textoEdicion.trim() }, "Comentario cambiado.")) editando = null;
  }
  const borrar = (id: string) => enviar({ id, texto: "" }, "Comentario borrado.");

  let conservando = $state<null | "si" | "no">(null);
  // Si cambia la versión que se mira, se cierra lo que estuviera a medias.
  $effect(() => {
    void version;
    untrack(() => ((editando = null), (anadiendo = false), (nuevo = ""), (conservando = null)));
  });
</script>

{#if admiteConservar(equipo) || admiteComentarios(equipo) || comentarios.length || conservada}
  <section class="notas-version" aria-labelledby="t-notas-v">
    <h3 id="t-notas-v">Comentarios y conservar</h3>

    {#if conservada}
      <div class="conservada" role="status">
        <Pin size={15} />
        <div>
          <p><strong>Se conserva para siempre:</strong> «{conservada.motivo}». La retención nunca la quita{enAlmacen === true ? " (tampoco la del almacén)" : enAlmacen === false ? "; el almacén aún no lo sabe" : ""}.</p>
          <p class="faint quien">{conservada.por ?? "Alguien"}{conservada.consola ? ` desde «${conservada.consola}»` : ""} · <Tiempo iso={conservada.cuando} /></p>
        </div>
        {#if puede.administrar(rol) && admiteConservar(equipo)}
          <button class="btn btn-sm btn-ghost" onclick={() => (conservando = "no")}><PinOff size={14} />Dejar de conservar…</button>
        {/if}
      </div>
    {:else if puede.administrar(rol) && admiteConservar(equipo)}
      <div class="fila-conservar">
        <button class="btn btn-sm" onclick={() => (conservando = "si")} use:tip={"La retención nunca la quitará (p. ej. antes de liberar espacio en el origen)"}><Pin size={14} />Conservar para siempre…</button>
      </div>
    {/if}

    {#if comentarios.length}
      <ul class="comentarios">
        {#each comentarios as c (c.id)}
          <li>
            <MessageSquare size={14} />
            <div class="cuerpo">
              {#if editando === c.id}
                <form class="editar" onsubmit={(ev) => guardarEdicion(ev, c.id)}>
                  <label class="sr-only" for="ed-{c.id}">Comentario</label>
                  <textarea id="ed-{c.id}" class="input" rows="2" maxlength={MAX_TEXTO} bind:value={textoEdicion}></textarea>
                  {#if errorEdicion}<p class="error" role="alert">{errorEdicion}</p>{/if}
                  <div class="acciones">
                    <button type="button" class="btn btn-sm btn-ghost" onclick={() => (editando = null)}>Cancelar</button>
                    <BotonCargando class="btn btn-sm btn-primary" disabled={!textoEdicion.trim() || !!errorEdicion} cargando={enviando} textoCargando="Guardando…">Guardar</BotonCargando>
                  </div>
                </form>
              {:else}
                <p class="texto">{c.texto}</p>
                <p class="faint quien">
                  {c.por ?? "Alguien"}{c.consola ? ` desde «${c.consola}»` : ""} · <Tiempo iso={c.cuando} />{#if c.editado}{" "}(cambiado){/if}
                  {#if esMio(c, app.cuenta?.id) && escribe}
                    <button class="icon-btn mini" aria-label="Cambiar el comentario" use:tip={"Cambiar"} onclick={() => ((editando = c.id), (textoEdicion = c.texto))}><Pencil size={13} /></button>
                    <button class="icon-btn mini" aria-label="Borrar el comentario" use:tip={"Borrar"} disabled={enviando} onclick={() => borrar(c.id)}><Trash2 size={13} /></button>
                  {/if}
                </p>
              {/if}
            </div>
          </li>
        {/each}
      </ul>
    {/if}

    {#if escribe}
      {#if anadiendo}
        <form class="nuevo" onsubmit={anadir}>
          <label for="nuevo-com-{version}">Comentario</label>
          <textarea id="nuevo-com-{version}" class="input" rows="2" maxlength={MAX_TEXTO} bind:value={nuevo} placeholder="Por ejemplo: versión buena antes de migrar el programa"></textarea>
          <p class="faint aviso">{AVISO_SECRETOS} <span class="num">{[...nuevo].length}/{MAX_TEXTO}</span></p>
          {#if nuevo && errorNuevo}<p class="error" role="alert">{errorNuevo}</p>{/if}
          <div class="acciones">
            <button type="button" class="btn btn-sm btn-ghost" onclick={() => ((anadiendo = false), (nuevo = ""))}>Cancelar</button>
            <BotonCargando class="btn btn-sm btn-primary" disabled={!nuevo.trim() || !!errorNuevo} cargando={enviando} textoCargando="Guardando…">Guardar comentario</BotonCargando>
          </div>
        </form>
      {:else}
        <button class="btn btn-sm btn-ghost" onclick={() => (anadiendo = true)}><Plus size={14} />Añadir un comentario</button>
      {/if}
    {:else if !comentarios.length && !admiteComentarios(equipo)}
      <p class="faint">Actualiza el agente de {equipo.nombre} para comentar las versiones.</p>
    {/if}
  </section>
{/if}

{#if conservando}
  <ConservarVersion {cliente} {equipo} {equipos} {repo} {version} conservar={conservando === "si"} onclose={() => (conservando = null)} alEnviar={() => setTimeout(recargar, 4000)} />
{/if}

<style>
  .notas-version {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .notas-version > :global(.btn) {
    align-self: flex-start;
  }
  .conservada {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 8px;
    padding: var(--sp-3);
    border-radius: var(--radius);
    background: var(--accent-soft);
    color: var(--text-1);
  }
  .conservada > :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--accent-text);
  }
  .conservada > div {
    flex: 1 1 14rem;
    min-width: 0;
  }
  .conservada p,
  .comentarios p {
    margin: 0;
  }
  .quien {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    font-size: var(--fs-xs);
  }
  .comentarios {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .comentarios li {
    display: flex;
    gap: 8px;
  }
  .comentarios li > :global(svg) {
    flex: none;
    margin-top: 3px;
    color: var(--text-3);
  }
  .cuerpo {
    flex: 1;
    min-width: 0;
  }
  .texto {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .nuevo,
  .editar {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .nuevo label {
    font-size: var(--fs-sm);
    font-weight: 550;
  }
  .aviso {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    margin: 0;
    font-size: var(--fs-xs);
  }
  .error {
    margin: 0;
    color: var(--bad);
    font-size: var(--fs-sm);
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
