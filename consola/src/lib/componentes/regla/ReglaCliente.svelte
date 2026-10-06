<script lang="ts">
  // La regla 3-2-1-1-0 de todas las copias de un cliente (tarea 8c): cuántas
  // la cumplen y cuáles no, con lo que les falta. En Estado, las que dejaron de
  // cumplir salen como aviso no urgente (no van a «Necesita atención»). En
  // Informes, la tabla entera (para enseñársela al cliente).
  import { untrack } from "svelte";
  import { ChevronRight, Clock, ShieldCheck } from "@lucide/svelte";
  import Ayuda from "../Ayuda.svelte";
  import TiraRegla from "./TiraRegla.svelte";
  import { plural } from "$lib/formato";
  import { catalogoDe, cargarCatalogo } from "$lib/catalogoDestinos.svelte";
  import { cuentaRegla, fraseRegla, PARTES, reglasDelCliente, type IdParte } from "$lib/regla321";
  import type { Equipo, Informe } from "$lib/tipos";

  let {
    cliente,
    equipos,
    todos = equipos,
    informes,
    ahora,
    modo = "estado",
  }: { cliente: string; equipos: Equipo[]; todos?: Equipo[]; informes: Record<string, Informe | null | undefined>; ahora: number; modo?: "estado" | "informe" } = $props();

  $effect(() => {
    const cc = cliente;
    if (cc) untrack(() => void cargarCatalogo(cc));
  });
  const reglas = $derived(reglasDelCliente(equipos, informes, catalogoDe(cliente), ahora, todos));
  const cuenta = $derived(cuentaRegla(reglas));
  // Las que no cumplen, primero las que dejaron de cumplir.
  const noCumplen = $derived(reglas.filter((r) => !r.regla.cumple).sort((a, b) => Number(b.regla.dejo_de_cumplir) - Number(a.regla.dejo_de_cumplir) || a.copia.nombre.localeCompare(b.copia.nombre)));
  const dejaron = $derived(noCumplen.filter((r) => r.regla.dejo_de_cumplir));
  const ORDEN: IdParte[] = ["copias", "soportes", "fuera", "inmutable", "errores"];
  let verTodas = $state(false);
  const lista = $derived(modo === "informe" ? reglas : verTodas ? noCumplen : noCumplen.slice(0, 5));
</script>

{#if reglas.length}
  <section class="card p regla-cliente" aria-labelledby="t-regla-cliente">
    <div class="cab">
      <h2 class="section-title" id="t-regla-cliente"><ShieldCheck size={16} />Regla 3-2-1-1-0 <Ayuda id="regla-321" /></h2>
      <span class="cuenta num"><strong>{cuenta.cumplen}</strong> de {plural(cuenta.total, "copia la cumple", "copias la cumplen")}</span>
    </div>
    <div class="barra" role="img" aria-label="{cuenta.cumplen} de {cuenta.total} copias cumplen la regla">
      {#if cuenta.cumplen}<span class="tone-ok" style:flex={cuenta.cumplen}></span>{/if}
      {#if cuenta.dejaron}<span class="tone-warn" style:flex={cuenta.dejaron}></span>{/if}
      {#if cuenta.total - cuenta.cumplen - cuenta.dejaron}<span class="tone-neutral" style:flex={cuenta.total - cuenta.cumplen - cuenta.dejaron}></span>{/if}
    </div>
    {#if cuenta.total > cuenta.cumplen}
      <p class="faltan faint">
        Lo que más falta:
        {ORDEN.filter((p) => cuenta.faltan[p])
          .sort((a, b) => cuenta.faltan[b] - cuenta.faltan[a])
          .map((p) => `${PARTES[p].titulo} (${cuenta.faltan[p]})`)
          .join(" · ")}
      </p>
    {/if}

    {#if modo === "estado" && dejaron.length}
      <div class="notice notice-info suave" role="status">
        <Clock size={16} />
        <p>
          {plural(dejaron.length, "copia dejó", "copias dejaron")} de cumplir la regla: {dejaron
            .slice(0, 3)
            .map((r) => `«${r.copia.nombre}» de ${r.equipo.nombre}`)
            .join(", ")}{dejaron.length > 3 ? "…" : ""}. No es urgente: su configuración está bien, pero algo no está al día.
        </p>
      </div>
    {/if}

    {#if lista.length}
      <ul class="filas">
        {#each lista as r (r.equipo.id + r.copia.id)}
          <li>
            <a class="fila" href="/c/{cliente}/equipos/{r.equipo.id}/copias/{encodeURIComponent(r.copia.id)}#sec-regla">
              <TiraRegla rc={r} {cliente} {ahora} compacta />
              <span class="f-nombre"><strong>{r.copia.nombre}</strong><span class="faint">{" · "}{r.equipo.nombre}</span></span>
              <span class="f-frase" class:dejo={r.regla.dejo_de_cumplir}>{fraseRegla(r.regla)}</span>
              <ChevronRight size={14} class="flecha" />
            </a>
          </li>
        {/each}
      </ul>
      {#if modo === "estado" && noCumplen.length > 5}
        <button class="btn btn-sm btn-ghost" onclick={() => (verTodas = !verTodas)}>{verTodas ? "Ver menos" : `Ver las ${noCumplen.length}`}</button>
      {/if}
    {:else if modo === "estado"}
      <p class="faint">Todas las copias la cumplen.</p>
    {/if}
    <p class="faint nota">Es una guía, no una obligación: dice qué le falta a cada copia y dónde se arregla.</p>
  </section>
{/if}

<style>
  .regla-cliente {
    display: grid;
    gap: var(--sp-3);
  }
  .cab {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--sp-2);
    flex-wrap: wrap;
  }
  .cab h2 {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
  }
  .cuenta {
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .cuenta strong {
    font-size: 1.25rem;
    color: var(--text-1);
  }
  .barra {
    display: flex;
    height: 6px;
    border-radius: 999px;
    overflow: hidden;
    gap: 2px;
    background: var(--surface-2);
  }
  .barra span {
    background: var(--tone);
  }
  .faltan,
  .nota {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .suave {
    margin: 0;
  }
  .filas {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
  }
  .fila {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) minmax(0, 1.3fr) auto;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-2) 0;
    border-bottom: 1px solid var(--border);
    color: inherit;
    text-decoration: none;
    font-size: var(--fs-sm);
  }
  .fila:hover .f-nombre strong {
    text-decoration: underline;
  }
  .f-frase {
    color: var(--text-2);
    font-size: var(--fs-xs);
  }
  .f-frase.dejo {
    color: var(--text-1);
  }
  .fila :global(.flecha) {
    color: var(--text-3);
  }
  @media (max-width: 640px) {
    .fila {
      grid-template-columns: auto minmax(0, 1fr) auto;
    }
    .f-frase {
      grid-column: 1 / -1;
      grid-row: 2;
    }
  }
</style>
