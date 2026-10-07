<script lang="ts">
  import { tip } from "$lib/tooltip";
  // «Desde otras consolas» en Órdenes (v1.49, docs/consolas-multiples.md §5.8): lo que
  // los equipos con más de una consola cuentan en su historial de las órdenes que les
  // mandaron las demás (qué, desde cuál, quién y cómo acabó). Solo se lee: cada orden
  // se lleva desde su consola. Sin la dirección de ninguna consola.
  import { onMount, untrack } from "svelte";
  import { Network } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { enFondo } from "$lib/actividad.svelte";
  import { seguirCambios } from "$lib/vivo.svelte";
  import { actual, app, reloj } from "$lib/estado.svelte";
  import { fechaLarga, relativo } from "$lib/formato";
  import { nombreOrden } from "$lib/salud";
  import { mensajeOrden } from "$lib/textosEquipo";
  import { ordenesDeOtras, RESULTADO_ORDEN, resultadoOrden } from "$lib/espera";
  import type { EntradaHistorial, Equipo } from "$lib/tipos";
  import Chip from "./Chip.svelte";

  let { equipo = "" }: { equipo?: string } = $props();

  type Fila = EntradaHistorial & { equipoNombre: string; equipoId: string };
  let filas = $state<Fila[]>([]);
  let vuelta = 0;

  /** Los equipos que se gestionan también desde otra consola (los demás no tienen nada que contar aquí). */
  const conOtras = (l: Equipo[]) => l.filter((e) => (e.resumen?.consolas?.length ?? 0) > 1 && (!equipo || e.id === equipo));

  async function cargar() {
    const mia = ++vuelta;
    const propia = app.servidor?.identidad ?? "";
    const equipos = conOtras(actual.equipos);
    if (!actual.id || !propia || !equipos.length) {
      filas = [];
      return;
    }
    const leidos = await Promise.all(
      equipos.map(async (e) => ({ equipo: e, historial: await api.historialEquipo(actual.id, e.id, { tipo: ["orden"], limite: 50 }).catch(() => [] as EntradaHistorial[]) })),
    );
    if (mia === vuelta) filas = ordenesDeOtras(leidos, propia).slice(0, 50);
  }
  $effect(() => {
    void actual.id;
    void equipo;
    void actual.equipos.length;
    untrack(() => void cargar());
  });
  // Con el canal en vivo, cuando un equipo sube su historial; sin él, cada minuto.
  onMount(() => seguirCambios(() => enFondo(cargar), { ms: 60_000, toca: (x) => x.t === "historial" }));
</script>

{#if filas.length}
  <section class="otras" aria-labelledby="t-otras">
    <div class="section-head">
      <h2 id="t-otras"><Network size={16} />Desde otras consolas</h2>
    </div>
    <p class="faint intro">Lo que los equipos cuentan de las órdenes que les mandaron sus otras consolas. Se llevan desde allí; las que esperan su turno se pueden cancelar arriba.</p>
    <div class="card p-0">
      <ul>
        {#each filas as h (h.id)}
          {@const r = RESULTADO_ORDEN[resultadoOrden(h)] ?? { texto: resultadoOrden(h) || "—", tono: "neutral" as const }}
          <li>
            <span class="texto">
              <span><strong>{h.descripcion ?? nombreOrden(h.orden ?? "")}</strong> en {h.equipoNombre}</span>
              <span class="faint">
                Desde {h.consola ? `«${h.consola}»` : "otra consola"}{#if h.por}{" · "}pedida por {h.por}{/if}
                {#if h.cancelada_desde}{" · "}cancelada desde «{h.cancelada_desde}»{/if}
                {#if h.mensaje && resultadoOrden(h) !== "hecha"}{" · "}{mensajeOrden(h.orden, h.mensaje)}{/if}
              </span>
            </span>
            <span class="lado">
              <Chip pequeno tono={r.tono} texto={r.texto} />
              <time class="faint" datetime={h.hora} use:tip={fechaLarga(h.hora)}>{relativo(h.hora, reloj.ahora)}</time>
            </span>
          </li>
        {/each}
      </ul>
    </div>
  </section>
{/if}

<style>
  .otras {
    display: flex;
    flex-direction: column;
    margin-top: var(--sp-5);
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .intro {
    margin: 0 0 var(--sp-3);
    font-size: var(--fs-sm);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2) var(--sp-4);
    padding: 10px var(--sp-4);
  }
  li + li {
    border-top: 1px solid var(--border);
  }
  .texto {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .texto .faint {
    font-size: var(--fs-sm);
  }
  .lado {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    font-size: var(--fs-sm);
  }
</style>
