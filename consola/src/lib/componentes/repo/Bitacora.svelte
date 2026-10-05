<script lang="ts">
  // La «bitácora» de las versiones (docs/diseno.md §4): una lista vertical por
  // días con un riel a la izquierda. Cada versión es una fila: su marca (color
  // y forma de su copia, hueca si la próxima retención la quitaría), la hora,
  // lo nuevo en una píldora mono («+154 MB»), la copia y si se conserva. Con
  // `acciones` (el repositorio), pulsar la elige y debajo salen sus acciones;
  // sin ellas (Restaurar), pulsar es elegirla. Las filas son botones de verdad
  // (Tab) y las flechas ↑↓ van de una a otra.
  import type { Snippet } from "svelte";
  import { ChevronRight } from "@lucide/svelte";
  import { bytes, plural } from "$lib/formato";
  import { NOMBRE_MOTIVO, nombreDia, type VersionLinea } from "$lib/lineaTiempo";
  import type { Periodo } from "$lib/retencion";
  import FormaCopia from "./FormaCopia.svelte";

  type V = VersionLinea & { t: number };
  interface Props {
    dias: { dia: number; vs: V[] }[];
    ahora: number;
    motivos: Map<string, Periodo | null> | null;
    hueco: (copia: string | null | undefined) => 0 | 1 | 2 | 3;
    nombreCopia: (copia: string | null | undefined) => string;
    seleccion: string | null;
    alElegir: (id: string) => void;
    acciones?: Snippet<[string]>;
    /** La más reciente de todas (lleva su distintivo). */
    reciente?: string | null;
    /** Las que han llegado mientras se miraba (se iluminan un momento). */
    nuevas: Set<string>;
    etiqueta: string;
  }
  let { dias, ahora, motivos, hueco, nombreCopia, seleccion, alElegir, acciones, reciente = null, nuevas, etiqueta }: Props = $props();

  const fmtHora = new Intl.DateTimeFormat("es", { hour: "2-digit", minute: "2-digit" });
  const fmtLargo = new Intl.DateTimeFormat("es", { weekday: "long", day: "numeric", month: "long", hour: "2-digit", minute: "2-digit" });
  const quita = (id: string) => !!motivos && motivos.get(id) === null;
  function estado(id: string): string | null {
    if (!motivos) return null;
    const m = motivos.get(id);
    return m ? `se conserva · ${NOMBRE_MOTIVO[m]}` : "la quitará la retención";
  }
  function describir(v: V): string {
    return [
      fmtLargo.format(v.t),
      v.anadido != null ? `${bytes(v.anadido)} nuevos` : v.bytes != null ? bytes(v.bytes) : null,
      v.archivos != null ? plural(v.archivos, "archivo", "archivos") : null,
      `copia «${nombreCopia(v.copia)}»`,
      estado(v.id),
      v.id === reciente ? "la más reciente" : null,
      ...(v.etiquetas ?? []),
    ]
      .filter(Boolean)
      .join(", ");
  }
  let lista = $state<HTMLOListElement | null>(null);
  function flechas(ev: KeyboardEvent) {
    if (ev.key !== "ArrowDown" && ev.key !== "ArrowUp") return;
    const filas = [...(lista?.querySelectorAll<HTMLElement>(".fv") ?? [])];
    const i = filas.indexOf(ev.target as HTMLElement);
    if (i < 0) return;
    ev.preventDefault();
    filas[Math.max(0, Math.min(filas.length - 1, i + (ev.key === "ArrowDown" ? 1 : -1)))]?.focus();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<ol class="bitacora" aria-label={etiqueta} bind:this={lista} onkeydown={flechas}>
  {#each dias as d (d.dia)}
    {@const quitan = d.vs.filter((v) => quita(v.id)).length}
    <li class="dia">
      <h3 class="dia-cab">
        <span class="dia-nombre">{nombreDia(d.dia, ahora)}</span>
        <span class="dia-cuenta num">{plural(d.vs.length, "versión", "versiones")}{quitan ? ` · la retención quitará ${quitan}` : ""}</span>
      </h3>
      <ol class="riel">
        {#each d.vs as v (v.id)}
          {@const q = quita(v.id)}
          {@const elegida = v.id === seleccion}
          <li class="v" id="version-{v.id}" class:elegida class:quita={q} class:nueva={nuevas.has(v.id)}>
            <span class="nodo"><FormaCopia hueco={hueco(v.copia)} quita={q} tamano={10} /></span>
            <button
              type="button"
              class="fv"
              aria-label={describir(v)}
              aria-expanded={acciones ? elegida : undefined}
              aria-current={elegida ? "true" : undefined}
              onclick={() => alElegir(v.id)}
            >
              <span class="hora num" aria-hidden="true">{fmtHora.format(v.t)}</span>
              {#if v.anadido != null}<span class="pildora" aria-hidden="true">+{bytes(v.anadido)}</span>{:else if v.bytes != null}<span class="pildora" aria-hidden="true">{bytes(v.bytes)}</span>{/if}
              <span class="copia" aria-hidden="true">{nombreCopia(v.copia)}</span>
              {#if v.id === reciente}<span class="badge badge-sm tone-accent" aria-hidden="true">La más reciente</span>{/if}
              {#each v.etiquetas ?? [] as t (t)}<span class="badge badge-sm tone-info" aria-hidden="true">{t}</span>{/each}
              <span class="hueco"></span>
              {#if v.archivos != null}<span class="dato num" aria-hidden="true">{plural(v.archivos, "archivo", "archivos")}</span>{/if}
              {#if estado(v.id)}<span class="estado" aria-hidden="true">{estado(v.id)}</span>{/if}
              <ChevronRight size={15} class="flecha" aria-hidden="true" />
            </button>
            {#if acciones && elegida}
              <div class="acciones">{@render acciones(v.id)}</div>
            {/if}
          </li>
        {/each}
      </ol>
    </li>
  {/each}
</ol>

<style>
  .bitacora,
  .riel {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .bitacora {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .dia-cab {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 10px;
    margin: 0 0 4px;
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--text-1);
  }
  .dia-nombre::first-letter {
    text-transform: uppercase;
  }
  .dia-cuenta {
    font-size: var(--fs-xs);
    font-weight: 400;
    color: var(--text-3);
  }
  /* El riel: una raya a la izquierda; cada versión, su marca encima. */
  .riel {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding-left: 22px;
  }
  .riel::before {
    content: "";
    position: absolute;
    top: 6px;
    bottom: 6px;
    left: 7px;
    width: 2px;
    border-radius: 2px;
    background: var(--border);
  }
  .v {
    position: relative;
  }
  .nodo {
    position: absolute;
    top: 0;
    left: -22px;
    display: grid;
    place-items: center;
    width: 16px;
    height: 36px;
    pointer-events: none;
  }
  /* Un aro del fondo separa la marca del riel. */
  .nodo :global(.forma) {
    box-shadow: 0 0 0 2.5px var(--surface);
  }
  .fv {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-height: 36px;
    padding: 4px 10px;
    font: inherit;
    font-size: var(--fs-sm);
    text-align: left;
    color: var(--text-1);
    background: none;
    border: 1px solid transparent;
    border-radius: var(--radius);
    cursor: pointer;
  }
  .fv:hover {
    background: var(--surface-2);
  }
  .fv:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 0;
  }
  .elegida > .fv {
    background: var(--accent-soft);
    border-color: color-mix(in srgb, var(--accent) 35%, transparent);
  }
  .hora {
    flex: none;
    width: 3.2em;
    font-weight: 600;
  }
  .quita .hora {
    font-weight: 500;
    color: var(--text-2);
  }
  .pildora {
    flex: none;
    padding: 0 7px;
    font-family: var(--mono);
    font-size: 11.5px;
    line-height: 18px;
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
    white-space: nowrap;
  }
  .copia {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-2);
  }
  .hueco {
    flex: 1;
  }
  .dato,
  .estado {
    flex: none;
    font-size: var(--fs-xs);
    color: var(--text-3);
    white-space: nowrap;
  }
  .fv :global(.flecha) {
    flex: none;
    color: var(--text-3);
    opacity: 0;
  }
  .fv:hover :global(.flecha),
  .fv:focus-visible :global(.flecha) {
    opacity: 1;
  }
  .elegida > .fv :global(.flecha) {
    opacity: 1;
    rotate: 90deg;
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 8px 10px 6px;
  }
  @media (max-width: 640px) {
    .fv {
      flex-wrap: wrap;
      row-gap: 2px;
      min-height: 44px;
    }
    .estado {
      flex-basis: 100%;
      order: 9;
      padding-left: calc(3.2em + 10px);
    }
    .dato {
      display: none;
    }
  }
  @media (prefers-reduced-motion: no-preference) {
    .nueva > .fv {
      animation: llega 1.6s var(--ease) both;
    }
    .fv :global(.flecha) {
      transition:
        opacity var(--dur-fast) var(--ease),
        rotate var(--dur-fast) var(--ease);
    }
  }
  @keyframes llega {
    from {
      background: color-mix(in srgb, var(--accent) 22%, transparent);
    }
  }
</style>
