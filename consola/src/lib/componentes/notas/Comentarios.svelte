<script lang="ts">
  import { untrack } from "svelte";
  // Comentarios de un objeto: una bitácora («cambié el disco el 3/10», «si
  // falla, llamar a…»). Cualquiera del cliente los lee; técnicos o más
  // escriben. Cada cual cambia o borra los suyos durante 15 minutos; después
  // se quedan (se añade otro). Una persona propietaria puede borrar cualquiera.
  import { MessageSquare, Pencil, Trash2 } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { actual } from "$lib/estado.svelte";
  import { fallo } from "$lib/avisos.svelte";
  import { AVISO_SECRETOS, asegurarIndice, cargarNotas, claveNota, detalles, errorTextoNota, MAX_TEXTO, notas, ponerDetalle, puedeEscribirNotas } from "$lib/notas.svelte";
  import type { ComentarioNota, TipoNota } from "$lib/tipos";
  import { tip } from "$lib/tooltip";
  import Tiempo from "../Tiempo.svelte";
  import BotonCargando from "../BotonCargando.svelte";
  import TextoNota from "./TextoNota.svelte";

  let { tipo, objeto, titulo = "Comentarios", sinTarjeta = false }: { tipo: TipoNota; objeto: string; titulo?: string; sinTarjeta?: boolean } = $props();

  /** Cuántos se ven sin pulsar «Ver los anteriores». */
  const VISIBLES = 5;

  const d = $derived(detalles[claveNota(tipo, objeto)]);
  const lista = $derived(d?.comentarios ?? []);
  let todos = $state(false);
  const vistos = $derived(todos ? lista : lista.slice(-VISIBLES));
  const escribe = $derived(puedeEscribirNotas(actual.cliente?.rol));
  let nuevo = $state("");
  let enviando = $state(false);
  let editandoId = $state<string | null>(null);
  let textoEdicion = $state("");
  let borrandoId = $state<string | null>(null);
  let ocupado = $state(false);
  let error = $state("");
  const idBase = $derived(`com-${tipo}-${objeto.replace(/[^A-Za-z0-9_-]/g, "_")}`);

  $effect(() => {
    const c = actual.id;
    const [t, o] = [tipo, objeto];
    if (!c) return;
    error = "";
    untrack(() => void asegurarIndice(c).then(() => cargarNotas(c, t, o).catch((e) => (error = (e as Error).message))));
  });

  function poner(cambio: (l: ComentarioNota[]) => ComentarioNota[]) {
    if (d) ponerDetalle(actual.id, { ...d, comentarios: cambio(d.comentarios) });
  }

  async function comentar(e: SubmitEvent) {
    e.preventDefault();
    if (!nuevo.trim() || errorTextoNota(nuevo)) return;
    enviando = true;
    try {
      const k = await api.comentar(actual.id, tipo, objeto, nuevo);
      poner((l) => [...l, k]);
      nuevo = "";
    } catch (err) {
      fallo(err);
    } finally {
      enviando = false;
    }
  }
  async function guardarEdicion(k: ComentarioNota) {
    if (!textoEdicion.trim() || errorTextoNota(textoEdicion)) return;
    ocupado = true;
    try {
      const nuevoK = await api.editarComentario(actual.id, k.id, textoEdicion);
      poner((l) => l.map((x) => (x.id === k.id ? nuevoK : x)));
      editandoId = null;
    } catch (err) {
      fallo(err);
      // Pasados los 15 minutos ya no se puede: se relee para que lo diga.
      void cargarNotas(actual.id, tipo, objeto).catch(() => {});
    } finally {
      ocupado = false;
    }
  }
  async function borrar(k: ComentarioNota) {
    ocupado = true;
    try {
      await api.borrarComentario(actual.id, k.id);
      poner((l) => l.filter((x) => x.id !== k.id));
      borrandoId = null;
    } catch (err) {
      fallo(err);
      void cargarNotas(actual.id, tipo, objeto).catch(() => {});
    } finally {
      ocupado = false;
    }
  }
  /** Ctrl+Intro envía; Escape deja de editar. */
  function teclas(e: KeyboardEvent, enviar: () => void, cancelar?: () => void) {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) (e.preventDefault(), enviar());
    else if (e.key === "Escape" && cancelar) (e.stopPropagation(), cancelar());
  }
  const inicial = (n: string) => (n.trim()[0] ?? "?").toUpperCase();
</script>

{#if notas.disponible}
  <section class="comentarios" class:card={!sinTarjeta} class:p={!sinTarjeta} aria-labelledby="{idBase}-t">
    <h2 class="section-title" id="{idBase}-t"><MessageSquare size={16} />{titulo}{#if lista.length}<span class="count">· {lista.length}</span>{/if}</h2>
    {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
    {#if lista.length > vistos.length}
      <button type="button" class="btn btn-sm btn-ghost mas" onclick={() => (todos = true)}>Ver {lista.length - vistos.length === 1 ? "el anterior" : `los ${lista.length - vistos.length} anteriores`}</button>
    {/if}
    {#if vistos.length}
      <ol class="lista">
        {#each vistos as k (k.id)}
          <li class="com">
            <span class="avatar" aria-hidden="true">{inicial(k.autor.nombre)}</span>
            <div class="cuerpo">
              <p class="cab">
                <strong>{k.autor.nombre || "Alguien"}</strong>
                <span class="faint"><Tiempo iso={k.creado} />{#if k.editado}<span use:tip={"Cambiado después de escribirlo"}> · editado</span>{/if}</span>
                {#if (k.editable || k.borrable) && editandoId !== k.id && borrandoId !== k.id}
                  <span class="acc">
                    {#if k.editable}<button type="button" class="icon-btn" aria-label="Cambiar este comentario" use:tip={`Cambiar (durante ${d?.minutos_cambio ?? 15} minutos)`} onclick={() => ((editandoId = k.id), (textoEdicion = k.texto))}><Pencil size={14} /></button>{/if}
                    {#if k.borrable}<button type="button" class="icon-btn" aria-label="Borrar este comentario" use:tip={"Borrar"} onclick={() => (borrandoId = k.id)}><Trash2 size={14} /></button>{/if}
                  </span>
                {/if}
              </p>
              {#if editandoId === k.id}
                <textarea class="input" rows="3" bind:value={textoEdicion} aria-label="Cambiar el comentario" maxlength={MAX_TEXTO + 200} onkeydown={(e) => teclas(e, () => guardarEdicion(k), () => (editandoId = null))}></textarea>
                {#if errorTextoNota(textoEdicion)}<p class="error-campo" role="alert">{errorTextoNota(textoEdicion)}</p>{/if}
                <div class="fila-acc">
                  <button type="button" class="btn btn-sm btn-ghost" onclick={() => (editandoId = null)}>Cancelar</button>
                  <BotonCargando type="button" class="btn btn-sm btn-primary" cargando={ocupado} disabled={!textoEdicion.trim()} onclick={() => guardarEdicion(k)}>Guardar</BotonCargando>
                </div>
              {:else}
                <TextoNota texto={k.texto} />
              {/if}
              {#if borrandoId === k.id}
                <div class="fila-acc" role="group" aria-label="Confirmar">
                  <span class="faint">¿Borrar este comentario?</span>
                  <button type="button" class="btn btn-sm btn-ghost" onclick={() => (borrandoId = null)}>No</button>
                  <BotonCargando type="button" class="btn btn-sm btn-danger" cargando={ocupado} onclick={() => borrar(k)}>Borrar</BotonCargando>
                </div>
              {/if}
            </div>
          </li>
        {/each}
      </ol>
    {:else if d}
      <p class="faint vacio">{escribe ? "Sin comentarios. Anota aquí lo que se hizo y lo que conviene saber: queda con fecha y nombre." : "Sin comentarios."}</p>
    {/if}
    {#if escribe && d}
      <form class="nuevo" onsubmit={comentar}>
        <label class="sr-only" for="{idBase}-nuevo">Nuevo comentario</label>
        <textarea
          id="{idBase}-nuevo"
          class="input"
          rows={nuevo.includes("\n") ? 3 : 1}
          bind:value={nuevo}
          placeholder="Añadir un comentario…"
          maxlength={MAX_TEXTO + 200}
          aria-describedby="{idBase}-ayuda"
          onkeydown={(e) => teclas(e, () => (e.currentTarget as HTMLTextAreaElement).form?.requestSubmit())}
        ></textarea>
        <BotonCargando type="submit" class="btn btn-sm" cargando={enviando} disabled={!nuevo.trim() || !!errorTextoNota(nuevo)}>Comentar</BotonCargando>
      </form>
      <p class="faint ayuda" id="{idBase}-ayuda">{errorTextoNota(nuevo) ?? `${AVISO_SECRETOS} Ctrl+Intro para enviar.`}</p>
    {/if}
  </section>
{/if}

<style>
  .comentarios {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  .comentarios .section-title {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .section-title .count {
    font-weight: 400;
    color: var(--text-3);
  }
  .mas {
    align-self: flex-start;
  }
  .lista {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .com {
    display: flex;
    gap: 10px;
    min-width: 0;
    font-size: var(--fs-sm);
  }
  .avatar {
    display: inline-grid;
    place-items: center;
    flex: none;
    width: 26px;
    height: 26px;
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--accent-text);
    background: var(--accent-soft);
    border-radius: 999px;
  }
  .cuerpo {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .cab {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 2px 8px;
    margin: 0;
  }
  .acc {
    display: inline-flex;
    margin-left: auto;
  }
  .acc .icon-btn {
    width: 26px;
    height: 26px;
  }
  .fila-acc {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
  }
  .nuevo {
    display: flex;
    align-items: flex-end;
    gap: 8px;
  }
  .nuevo textarea {
    flex: 1;
    min-width: 0;
    resize: vertical;
  }
  .ayuda,
  .vacio {
    margin: 0;
    font-size: var(--fs-xs);
  }
</style>
