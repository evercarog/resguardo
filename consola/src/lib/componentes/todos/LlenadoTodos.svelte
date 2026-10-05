<script lang="ts">
  // «¿Cuándo se llena?» de todos los clientes: cada almacén, destino y
  // destino del espejo con su cliente, lo que ocupa, a qué ritmo crece y
  // cuándo se llenaría (aproximado), con su gráfica. Lo que se llena antes,
  // arriba; con muchos, los 6 primeros y «Ver todos».
  import { Cloud, Gauge, HardDrive, Server } from "@lucide/svelte";
  import type { PrevisionGlobal } from "$lib/global";
  import type { IconoLlenado } from "$lib/llenado";
  import { bytes, fechaLarga, relativo } from "$lib/formato";
  import { tip } from "$lib/tooltip";
  import Chip from "../Chip.svelte";
  import MarcaCliente from "../MarcaCliente.svelte";
  import GraficaLlenado from "../llenado/GraficaLlenado.svelte";

  let { lista, ahora }: { lista: PrevisionGlobal[]; ahora: number } = $props();
  const ICONO: Record<IconoLlenado, typeof Server> = { almacen: Server, servidor: Server, disco: HardDrive, nube: Cloud };
  const PRIMEROS = 6;
  let todas = $state(false);
  const vistas = $derived(todas ? lista : lista.slice(0, PRIMEROS));
</script>

{#if lista.length}
  <section aria-labelledby="t-llenado-todos">
    <div class="section-head">
      <h2 id="t-llenado-todos">¿Cuándo se llena? <span class="count">· {lista.length} · aproximado</span></h2>
    </div>
    <div class="card p-0 lista">
      {#each vistas as p (p.clave)}
        {@const Ic = ICONO[p.icono]}
        <div class="fila-llenado">
          <div class="cab">
            <span class="d-ic" aria-hidden="true"><Ic size={16} /></span>
            <span class="txt">
              {#if p.href}<a class="nombre" href={p.href}>{p.nombre}</a>{:else}<span class="nombre">{p.nombre}</span>{/if}
              <span class="sub"><MarcaCliente nombre={p.cliente.nombre} marca={p.cliente.marca} tam={14} /><a href="/c/{p.cliente.id}">{p.cliente.nombre}</a>{" · "}{p.sub}</span>
            </span>
            {#if p.tono === "warn" || p.tono === "bad" || p.tono === "ok"}<Chip pequeno tono={p.tono} texto={p.estado} />{/if}
          </div>
          <div class="cuerpo">
            <div class="cifras-ll">
              <p class="frase">{p.frase}</p>
              {#if p.usado != null}
                <p class="faint num medidas">
                  Ocupa {bytes(p.usado)}{#if p.total != null}{" "}de {bytes(p.total)} · {bytes(p.libre)} libres{/if}
                  {#if p.medido}<span use:tip={`Medido ${fechaLarga(p.medido)}`}>{" "}· medido {relativo(p.medido, ahora)}</span>{/if}
                </p>
              {/if}
              {#if p.porDia}<p class="faint nota"><Gauge size={12} aria-hidden="true" />Con lo que añadieron las versiones de los últimos {p.dias} días; no descuenta lo que quita la retención.</p>{/if}
            </div>
            {#if p.serie.length && p.porDia != null}
              <div class="graf"><GraficaLlenado {p} {ahora} /></div>
            {/if}
          </div>
        </div>
      {/each}
      {#if lista.length > PRIMEROS}
        <div class="mas">
          <button type="button" class="btn btn-sm btn-ghost" aria-expanded={todas} onclick={() => (todas = !todas)}>{todas ? "Ver menos" : `Ver todos (${lista.length})`}</button>
        </div>
      {/if}
    </div>
  </section>
{/if}

<style>
  .lista > * + * {
    border-top: 1px solid var(--border);
  }
  .fila-llenado {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-4) var(--sp-5);
  }
  .cab {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }
  .d-ic {
    display: grid;
    flex: none;
    place-items: center;
    width: 32px;
    height: 32px;
    color: var(--text-2);
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .txt {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  .nombre {
    font-weight: 500;
    color: var(--text-1);
    overflow-wrap: anywhere;
  }
  a.nombre {
    text-decoration: none;
  }
  a.nombre:hover {
    text-decoration: underline;
  }
  .sub {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0 5px;
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    color: var(--text-3);
  }
  .sub a {
    color: var(--text-2);
  }
  .cuerpo {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr);
    gap: var(--sp-5);
    align-items: start;
    padding-left: 44px;
  }
  .cifras-ll p {
    margin: 0;
  }
  .frase {
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
    color: var(--text-1);
  }
  .medidas,
  .nota {
    margin-top: 6px !important;
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
  }
  .nota {
    display: flex;
    gap: 5px;
    align-items: flex-start;
  }
  .nota :global(svg) {
    flex: none;
    margin-top: 2px;
  }
  .graf {
    min-width: 0;
  }
  .mas {
    display: flex;
    justify-content: center;
    padding: var(--sp-2);
  }
  @media (max-width: 760px) {
    .cuerpo {
      grid-template-columns: minmax(0, 1fr);
      padding-left: 0;
    }
    .fila-llenado {
      padding: var(--sp-4);
    }
  }
</style>
