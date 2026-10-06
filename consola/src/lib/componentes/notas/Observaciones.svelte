<script lang="ts">
  import { untrack } from "svelte";
  // Observaciones de un objeto (equipo, repositorio, copia, destino o cliente),
  // arriba en su página: el texto, quién lo cambió y cuándo, y «Editar» para
  // técnicos o más. Sin observaciones, solo un botón discreto para añadirlas
  // (y nada para quien solo lee). En claro en el servidor: ver lib/notas.svelte.ts.
  import { NotebookPen, Pencil, Plus } from "@lucide/svelte";
  import { actual } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { asegurarIndice, cargarNotas, claveNota, detalles, errorTextoNota, guardarObservacion, notas, puedeEscribirNotas } from "$lib/notas.svelte";
  import type { TipoNota } from "$lib/tipos";
  import { tip } from "$lib/tooltip";
  import Tiempo from "../Tiempo.svelte";
  import BotonCargando from "../BotonCargando.svelte";
  import CampoObservaciones from "./CampoObservaciones.svelte";
  import TextoNota from "./TextoNota.svelte";
  import { admiteDatosEquipo, alcanceDatos, variasConsolas } from "$lib/datosEquipo";
  import { pedirAlEquipo } from "$lib/pedirAlEquipo";

  let { tipo, objeto, compacto = false }: { tipo: TipoNota; objeto: string; compacto?: boolean } = $props();

  const d = $derived(detalles[claveNota(tipo, objeto)]);
  const obs = $derived(d?.observacion ?? null);
  const escribe = $derived(puedeEscribirNotas(actual.cliente?.rol));
  let editando = $state(false);
  let texto = $state("");
  let guardando = $state(false);
  const idCampo = $derived(`obs-${tipo}-${objeto.replace(/[^A-Za-z0-9_-]/g, "_")}`);
  /** v1.4x: la observación de un equipo que la guarda él (igual en todas sus consolas). */
  const delEquipo = $derived(tipo === "equipo" ? (actual.equipos.find((e) => e.id === objeto && admiteDatosEquipo(e)) ?? null) : null);

  $effect(() => {
    const c = actual.id;
    const [t, o] = [tipo, objeto];
    if (!c) return;
    untrack(() => void asegurarIndice(c).then(() => cargarNotas(c, t, o).catch(() => {})));
  });

  function editar() {
    texto = obs?.texto ?? "";
    editando = true;
  }
  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    if (errorTextoNota(texto)) return;
    guardando = true;
    try {
      if (delEquipo && actual.cliente) {
        // v1.4x: la del equipo la guarda el equipo y la ven igual todas sus consolas.
        const limpio = texto.replace(/\r\n?/g, "\n").trim();
        if (limpio !== (obs?.texto ?? "")) {
          const r = await pedirAlEquipo(actual.cliente, delEquipo, "observacion_equipo", { texto: limpio });
          avisar(r.hecha ? (limpio ? "Observaciones guardadas en el equipo." : "Observaciones quitadas en el equipo.") + (variasConsolas(delEquipo) ? " Las verán igual todas sus consolas." : "") : r.texto);
          await cargarNotas(actual.id, tipo, objeto);
        }
      } else {
        const cambio = await guardarObservacion(actual.id, tipo, objeto, texto);
        if (cambio) avisar(texto.trim() ? "Observaciones guardadas." : "Observaciones quitadas.");
      }
      editando = false;
    } catch (err) {
      fallo(err);
    } finally {
      guardando = false;
    }
  }
  function teclas(e: KeyboardEvent) {
    if (e.key === "Escape") (e.stopPropagation(), (editando = false));
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) (e.currentTarget as HTMLFormElement).requestSubmit();
  }
</script>

{#if notas.disponible && d}
  {#if editando}
    <!-- Ctrl+Intro guarda y Escape cancela desde el campo (las teclas suben hasta el formulario). -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <form class="obs editando" onsubmit={guardar} onkeydown={teclas}>
      <CampoObservaciones id={idCampo} bind:valor={texto} filas={4} />
      {#if tipo === "equipo"}{@const eq = actual.equipos.find((e) => e.id === objeto)}{#if eq && alcanceDatos(eq)}<p class="alcance faint">{alcanceDatos(eq)}</p>{/if}{/if}
      <div class="acciones">
        <button type="button" class="btn btn-sm btn-ghost" onclick={() => (editando = false)}>Cancelar</button>
        <BotonCargando type="submit" class="btn btn-sm btn-primary" cargando={guardando} disabled={!!errorTextoNota(texto)}>Guardar</BotonCargando>
      </div>
    </form>
  {:else if obs}
    <section class="obs" class:compacto aria-label="Observaciones">
      <span class="icono" aria-hidden="true"><NotebookPen size={16} /></span>
      <div class="cuerpo">
        <TextoNota texto={obs.texto} />
        <p class="meta faint">{obs.por || "Alguien"} · <Tiempo iso={obs.actualizada} /></p>
      </div>
      {#if escribe}
        <button type="button" class="icon-btn" aria-label="Editar las observaciones" use:tip={"Editar las observaciones"} onclick={editar}><Pencil size={15} /></button>
      {/if}
    </section>
  {:else if escribe}
    <button type="button" class="btn btn-sm btn-ghost anadir" onclick={editar}><Plus size={14} />Añadir observaciones</button>
  {/if}
{/if}

<style>
  .obs {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: var(--sp-2) var(--sp-3);
    background: color-mix(in srgb, var(--warn) 7%, var(--surface, transparent));
    border: 1px solid color-mix(in srgb, var(--warn) 30%, var(--border));
    border-radius: var(--radius);
    font-size: var(--fs-sm);
    min-width: 0;
  }
  .obs.editando {
    flex-direction: column;
    align-items: stretch;
  }
  .obs.compacto {
    padding: 6px 10px;
  }
  .icono {
    flex: none;
    margin-top: 2px;
    color: var(--warn);
  }
  .cuerpo {
    flex: 1;
    min-width: 0;
    max-height: 14em;
    overflow: auto;
  }
  .alcance {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .meta {
    margin: 4px 0 0;
    font-size: var(--fs-xs);
  }
  .acciones {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .anadir {
    align-self: flex-start;
  }
</style>
