<script lang="ts">
  // «Lo que se eliminó»: cada vuelta de la retención (del equipo, del almacén
  // o de la copia externa) con quién la aplicó, la regla, las cifras y, al
  // abrirla, las versiones que quitó y por qué no las guardó la regla.
  import { ChevronDown, ChevronRight } from "@lucide/svelte";
  import { bytes, fechaLarga, numero, plural } from "$lib/formato";
  import { resumenRegla } from "$lib/retencion";
  import { claveDia, quienAplico, quienCorto, textoSeVa, type FilaVersion, type VueltaRetencion } from "$lib/retencionDetalle";
  import Chip from "../Chip.svelte";
  import Tiempo from "../Tiempo.svelte";
  import TablaVersiones from "./TablaVersiones.svelte";

  let {
    vueltas,
    nombres,
    nombreCopia,
    copia = null,
    dia = null,
  }: {
    vueltas: VueltaRetencion[];
    nombres: { equipo: string; almacen?: string | null };
    /** El nombre de una copia por su id. */
    nombreCopia: (id: string | null) => string | null;
    /** Solo lo de esta copia (id). */
    copia?: string | null;
    /** Solo las vueltas de este día («AAAA-MM-DD»). */
    dia?: string | null;
  } = $props();

  const id = $props.id();
  let abiertas = $state<Set<string>>(new Set());
  const alternar = (k: string) => {
    const s = new Set(abiertas);
    if (s.has(k)) s.delete(k);
    else s.add(k);
    abiertas = s;
  };

  // Con una copia elegida: las vueltas que quitaron algo suyo (y las que no dicen qué quitaron).
  const vistas = $derived(
    vueltas.filter((v) => (!dia || claveDia(Date.parse(v.hora)) === dia) && (!copia || !v.versiones.length || v.versiones.some((x) => x.copia === copia))),
  );
  const filas = (v: VueltaRetencion): FilaVersion[] =>
    v.versiones
      .filter((x) => !copia || x.copia === copia)
      .map((x) => ({ id: x.id, hora: x.hora, copia: nombreCopia(x.copia), bytes: x.bytes, porque: textoSeVa(x.motivo, x.hora ? new Date(x.hora) : null, v.regla), tono: "va" }));
</script>

{#if !vistas.length}
  <p class="faint vacio">
    {vueltas.length ? "Nada con lo elegido." : "Todavía no hay nada anotado. Cada vez que se aplique la retención (desde la consola, a su hora en el almacén o al subir la copia externa) se guardará aquí lo que quitó."}
  </p>
{:else}
  <ol class="vueltas">
    {#each vistas as v (v.id)}
      {@const abierta = abiertas.has(v.id)}
      {@const lista = filas(v)}
      <li class="vuelta" class:fallo={!v.ok}>
        <div class="linea">
          <button class="abrir" aria-expanded={abierta} aria-controls="{id}-{v.id}" onclick={() => alternar(v.id)} disabled={!v.versiones.length && !v.mensaje && !v.regla}>
            {#if abierta}<ChevronDown size={16} />{:else}<ChevronRight size={16} />{/if}
            <span class="cuando">{fechaLarga(v.hora)}</span>
          </button>
          <span class="faint rel"><Tiempo iso={v.hora} /></span>
          <span class="badge badge-sm tone-neutral">{quienCorto(v, nombres.almacen)}</span>
          {#if v.por === "automatica"}<span class="badge badge-sm tone-info">Automática</span>{/if}
          {#if !v.ok}<Chip pequeno tono="bad" texto="Falló" />{/if}
        </div>
        <p class="cifras">
          {#if v.quitadas !== null}
            <strong>{v.quitadas === 0 ? "No sobraba ninguna versión" : `${plural(v.quitadas, "versión quitada", "versiones quitadas")}`}</strong>
          {:else if !v.ok}
            <strong>Falló{v.mensaje ? `: ${v.mensaje}` : ""}</strong>
          {:else}
            <strong>No se sabe qué quitó</strong>
          {/if}
          {#if v.quedan !== null}<span>· quedan {numero(v.quedan)}</span>{/if}
          {#if v.liberado !== null && (v.quitadas ?? 0) > 0}<span>· {bytes(v.liberado)} liberados</span>{/if}
          {#if v.sospechosas}<span>· {plural(v.sospechosas, "versión sospechosa sin tocar", "versiones sospechosas sin tocar")}</span>{/if}
        </p>
        {#if abierta}
          <div class="detalle" id="{id}-{v.id}">
            <p class="faint">{quienAplico(v, nombres)}{v.regla ? ` ${resumenRegla(v.regla)}` : ""}</p>
            {#if v.mensaje}<p class="msg" class:mal={!v.ok}>{v.mensaje}</p>{/if}
            {#if v.sospechosas}
              <p class="faint">El almacén no tocó {v.sospechosas === 1 ? "una versión cuya hora" : `${v.sospechosas} versiones cuya hora`} no cuadra con su subida (traídas de otro repositorio, o el reloj del equipo): ni se quitan ni desplazan a otras.</p>
            {/if}
            {#if v.versiones.length}
              <TablaVersiones filas={lista} titulo="Versiones que quitó la retención el {fechaLarga(v.hora)}" vacio="Ninguna de esta copia." />
            {/if}
            {#if v.mas}
              <p class="faint">{v.compactada ? `De esta vez (de las antiguas) solo se guardan las cifras: quitó ${plural(v.mas, "versión", "versiones")}.` : `Y ${plural(v.mas, "versión más", "versiones más")} que no caben en la lista.`}</p>
            {/if}
          </div>
        {/if}
      </li>
    {/each}
  </ol>
{/if}

<style>
  .vacio {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .vueltas {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
  .vuelta {
    padding: var(--sp-3) 0;
    border-top: 1px solid var(--border);
  }
  .vuelta:first-child {
    border-top: none;
    padding-top: 0;
  }
  .vuelta.fallo {
    box-shadow: inset 3px 0 0 var(--bad);
    padding-left: var(--sp-3);
  }
  .linea {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 8px;
  }
  .abrir {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 4px 2px 0;
    border: none;
    background: none;
    font: inherit;
    font-weight: 600;
    color: var(--text-1);
    cursor: pointer;
    text-align: left;
  }
  .abrir:disabled {
    cursor: default;
  }
  .abrir:focus-visible {
    outline: 2px solid var(--focus, var(--accent));
    border-radius: var(--radius-sm);
  }
  .cuando::first-letter {
    text-transform: uppercase;
  }
  .rel {
    font-size: var(--fs-xs);
  }
  .cifras {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 6px;
    margin: 4px 0 0 22px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .cifras strong {
    color: var(--text-1);
    font-weight: 600;
  }
  .detalle {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin: var(--sp-3) 0 0 22px;
  }
  .detalle p {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .msg.mal {
    color: var(--bad);
  }
  @media (max-width: 640px) {
    .cifras,
    .detalle {
      margin-left: 0;
    }
  }
</style>
