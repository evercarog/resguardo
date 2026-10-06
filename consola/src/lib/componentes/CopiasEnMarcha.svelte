<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Aviso global de lo que está en marcha en el cliente (v1.25), en la barra
  // lateral y en la cabecera del móvil. Cuenta todas las tareas («2 copias en
  // marcha») y da el porcentaje de todas juntas; debajo, cada una con el suyo
  // y su enlace (a la copia, o al equipo si no es una copia). Los porcentajes
  // salen de progreso.svelte.ts, igual que los de las filas y las tarjetas.
  import { LoaderCircle } from "@lucide/svelte";
  import { actual } from "$lib/estado.svelte";
  import { pctConjunto, pctVisible, pulso, todasEnMarcha } from "$lib/progreso.svelte";
  import { textoFase } from "$lib/textoProgreso";
  import { plural } from "$lib/formato";
  import type { TareaEnMarcha } from "$lib/tipos";

  let { compacto = false, alNavegar }: { compacto?: boolean; alNavegar?: () => void } = $props();

  /** Como mucho se listan estas; el resto, «y N más». */
  const MAX = 3;
  const lista = $derived(todasEnMarcha());
  const soloCopias = $derived(lista.every((x) => x.tarea.tipo === "copia"));
  const texto = $derived(soloCopias ? `${plural(lista.length, "copia", "copias")} en marcha` : `${plural(lista.length, "tarea", "tareas")} en marcha`);
  const una = $derived(lista.length === 1 ? lista[0] : null);
  const pct = $derived(pctConjunto(lista, pulso.ahora));
  const nombreEquipo = (id: string) => actual.equipos.find((e) => e.id === id)?.nombre ?? "un equipo";
  // v1.4x: un «Mover a otro sitio…» (de esta consola o de otra), por el repositorio que se mueve.
  const queEs = (t: TareaEnMarcha) => (t.mover ? `Mover «${t.nombre_origen ?? t.nombre ?? t.repo}»` : t.nombre ? t.nombre : textoFase(t));
  function hrefDe(x: { equipo: string; tarea: TareaEnMarcha }) {
    const base = `/c/${actual.id}/equipos/${encodeURIComponent(x.equipo)}`;
    if (x.tarea.tipo === "copia" && x.tarea.copia) return `${base}/copias/${encodeURIComponent(x.tarea.copia)}`;
    if (x.tarea.tipo === "historial" || x.tarea.tipo === "retencion") return `${base}/repositorios/${encodeURIComponent(x.tarea.origen ?? x.tarea.repo)}`;
    return base;
  }
  /** Con una, a ella; con varias del mismo equipo, a ese equipo; si no, a la lista de equipos (cada uno con su chip). */
  const href = $derived.by(() => {
    if (una) return hrefDe(una);
    const equipos = new Set(lista.map((x) => x.equipo));
    return equipos.size === 1 ? `/c/${actual.id}/equipos/${encodeURIComponent([...equipos][0])}` : `/c/${actual.id}/equipos`;
  });
  const detalle = $derived(
    lista
      .map((x) => {
        const p = pctVisible(x.equipo, x.tarea, pulso.ahora);
        return `${x.tarea.nombre ? `«${x.tarea.nombre}»` : textoFase(x.tarea)} en ${nombreEquipo(x.equipo)}${p != null ? ` (${p} %)` : ""}`;
      })
      .join("; "),
  );
</script>

<!-- Sin región «status»: el porcentaje cambia cada pocos segundos y no debe anunciarse. -->
{#if lista.length && actual.id}
  {#if compacto}
    <a class="chip" {href} use:tip={detalle} aria-label="{texto}: {detalle}" onclick={alNavegar}>
      <LoaderCircle size={13} class="spin" aria-hidden="true" />
      <span class="num">{lista.length > 1 ? `${lista.length}${pct != null ? ` · ${pct} %` : ""}` : pct != null ? `${pct} %` : "En marcha"}</span>
    </a>
  {:else}
    <div class="en-marcha" role="group" aria-label={texto}>
      <a class="cab" {href} use:tip={detalle} aria-label="{texto}: {detalle}" onclick={alNavegar}>
        <LoaderCircle size={14} class="spin" aria-hidden="true" />
        <span class="texto">{texto}</span>
        {#if pct != null}<span class="pct num">{pct} %</span>{/if}
        {#if una}<span class="sub">{queEs(una.tarea)} · {nombreEquipo(una.equipo)}</span>{/if}
        <span class="mini" class:indeterminada={pct == null} aria-hidden="true"><span style:width={pct == null ? undefined : `${pct}%`}></span></span>
      </a>
      {#if !una}
        <ul class="items">
          {#each lista.slice(0, MAX) as x (`${x.equipo}|${x.tarea.tipo}|${x.tarea.repo}|${x.tarea.copia ?? ""}`)}
            {@const p = pctVisible(x.equipo, x.tarea, pulso.ahora)}
            <li>
              <a href={hrefDe(x)} onclick={alNavegar}>
                <span class="item-texto">{queEs(x.tarea)}<span class="faint"> · {nombreEquipo(x.equipo)}</span></span>
                <span class="num item-pct">{p != null ? `${p} %` : "…"}</span>
              </a>
            </li>
          {/each}
          {#if lista.length > MAX}<li><a class="mas" {href} onclick={alNavegar}>y {plural(lista.length - MAX, "otra", "otras")}</a></li>{/if}
        </ul>
      {/if}
    </div>
  {/if}
{/if}

<style>
  .en-marcha {
    display: grid;
    background: var(--info-soft);
    border: 1px solid color-mix(in srgb, var(--info) 30%, transparent);
    border-radius: var(--radius);
    animation: rise var(--dur-slow) var(--ease-out) both;
  }
  .en-marcha:hover {
    border-color: color-mix(in srgb, var(--info) 55%, transparent);
  }
  .cab {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 2px 8px;
    padding: 8px 10px;
    font-size: var(--fs-sm);
    font-weight: 550;
    color: var(--text-1);
    text-decoration: none;
    border-radius: inherit;
  }
  .en-marcha :global(svg) {
    color: var(--info);
  }
  .texto {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pct {
    font-weight: 650;
  }
  .sub {
    grid-column: 2 / -1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-xs);
    font-weight: 400;
    color: var(--text-2);
  }
  .mini {
    grid-column: 1 / -1;
    position: relative;
    height: 4px;
    margin-top: 4px;
    overflow: hidden;
    background: color-mix(in srgb, var(--info) 18%, transparent);
    border-radius: 999px;
  }
  .mini span {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--info);
    border-radius: inherit;
    transition: width 0.9s linear;
  }
  .mini.indeterminada span {
    width: 35%;
    animation: vaiven 1.6s ease-in-out infinite;
  }
  @keyframes vaiven {
    from {
      left: -35%;
    }
    to {
      left: 100%;
    }
  }
  .items {
    display: grid;
    margin: 0;
    padding: 0 4px 4px;
    list-style: none;
    border-top: 1px solid color-mix(in srgb, var(--info) 20%, transparent);
  }
  .items a {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 26px;
    padding: 2px 6px;
    font-size: var(--fs-xs);
    color: var(--text-1);
    text-decoration: none;
    border-radius: var(--radius-sm);
  }
  .items a:hover {
    background: color-mix(in srgb, var(--info) 10%, transparent);
  }
  .item-texto {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .item-pct {
    flex: none;
    font-weight: 600;
  }
  .mas {
    color: var(--text-2) !important;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 9px;
    font-size: var(--fs-xs);
    font-weight: 550;
    color: var(--text-1);
    text-decoration: none;
    background: var(--info-soft);
    border: 1px solid color-mix(in srgb, var(--info) 30%, transparent);
    border-radius: 999px;
  }
  .chip :global(svg) {
    color: var(--info);
  }
  @media (prefers-reduced-motion: reduce) {
    .mini span {
      transition: none;
    }
    .mini.indeterminada span {
      left: 0;
      width: 100%;
      opacity: 0.35;
      animation: none;
    }
  }
</style>
