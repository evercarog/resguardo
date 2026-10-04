<script lang="ts">
  // Historial: las copias de los últimos 14 días (barras: correctas, con
  // avisos, fallidas) y lo último que terminó de cada tarea.
  import { CircleAlert, CircleCheck, TriangleAlert } from "@lucide/svelte";
  import { bytes, relativo } from "$lib/formato";
  import { vivo } from "./puente.svelte";
  import { TIPOS } from "./estado";
  import type { Dia } from "./tipos";

  const dias = $derived<Dia[]>(vivo.datos?.ventana?.historial ?? []);
  const hechas = $derived([...(vivo.datos?.bandeja?.hechas ?? [])].sort((a, b) => b.cuando.localeCompare(a.cuando)));
  const max = $derived(Math.max(1, ...dias.map((d) => d.ok + d.aviso + d.fallo)));
  const total = $derived(dias.reduce((s, d) => ({ ok: s.ok + d.ok, aviso: s.aviso + d.aviso, fallo: s.fallo + d.fallo }), { ok: 0, aviso: 0, fallo: 0 }));
  const etiquetaDia = (iso: string) => new Date(`${iso}T12:00:00`).toLocaleDateString("es", { weekday: "narrow" });
  const ALTO = 72;
  let foco = $state<Dia | null>(null);
</script>

<div class="v-pila">
  <section class="v-tarjeta">
    <h2 class="v-titulo">Últimos 14 días</h2>
    {#if dias.length}
      <ul class="leyenda v-mini">
        <li><CircleCheck size={13} aria-hidden="true" class="ok" />{total.ok} correctas</li>
        <li><TriangleAlert size={13} aria-hidden="true" class="warn" />{total.aviso} con avisos</li>
        <li><CircleAlert size={13} aria-hidden="true" class="bad" />{total.fallo} fallidas</li>
      </ul>
      <div class="barras" role="img" aria-label={`Copias de los últimos 14 días: ${total.ok} correctas, ${total.aviso} con avisos y ${total.fallo} fallidas.`}>
        {#each dias as d (d.dia)}
          {@const n = d.ok + d.aviso + d.fallo}
          <div class="col" onpointerenter={() => (foco = d)} onpointerleave={() => (foco = null)} aria-hidden="true">
            <div class="pila" style:height={`${(n / max) * ALTO}px`}>
              {#if d.fallo}<span class="seg bad" style:flex={d.fallo}></span>{/if}
              {#if d.aviso}<span class="seg warn" style:flex={d.aviso}></span>{/if}
              {#if d.ok}<span class="seg ok" style:flex={d.ok}></span>{/if}
            </div>
            <span class="v-mini dia">{etiquetaDia(d.dia)}</span>
          </div>
        {/each}
      </div>
      <p class="v-mini detalle" aria-live="polite">
        {#if foco}{new Date(`${foco.dia}T12:00:00`).toLocaleDateString("es", { weekday: "long", day: "numeric", month: "long" })}: {foco.ok} correctas, {foco.aviso} con avisos, {foco.fallo} fallidas{:else}&nbsp;{/if}
      </p>
    {:else}
      <p class="v-sub">Abre esta ventana con el servicio en marcha para ver el historial.</p>
    {/if}
  </section>

  {#if hechas.length}
    <section class="v-tarjeta">
      <h2 class="v-titulo">Lo último de cada tarea</h2>
      <ul class="hechas">
        {#each hechas as h (h.clave)}
          <li>
            <span class="r" data-r={h.resultado}>
              {#if h.resultado === "ok"}<CircleCheck size={15} aria-hidden="true" />{:else if h.resultado === "warning"}<TriangleAlert size={15} aria-hidden="true" />{:else}<CircleAlert size={15} aria-hidden="true" />{/if}
            </span>
            <span class="v-cortar"><b>{h.nombre}</b> <span class="v-mini">{TIPOS[h.tipo] ?? h.tipo}</span></span>
            <span class="v-mini cuando">{relativo(h.cuando)}{h.bytes ? ` · ${bytes(h.bytes)}` : ""}</span>
          </li>
        {/each}
      </ul>
    </section>
  {/if}
</div>

<style>
  .leyenda {
    display: flex;
    gap: var(--sp-3);
    margin: var(--sp-2) 0 var(--sp-3);
    padding: 0;
    list-style: none;
  }
  .leyenda li {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .leyenda :global(.ok),
  .seg.ok {
    color: var(--ok);
    background: currentColor;
  }
  .leyenda :global(.ok),
  .leyenda :global(.warn),
  .leyenda :global(.bad) {
    background: none;
  }
  .leyenda :global(.warn) {
    color: var(--warn);
  }
  .leyenda :global(.bad) {
    color: var(--bad);
  }
  .barras {
    display: grid;
    grid-template-columns: repeat(14, 1fr);
    align-items: end;
    gap: 4px;
    height: 92px;
  }
  .col {
    display: grid;
    justify-items: center;
    align-content: end;
    gap: 4px;
    height: 100%;
  }
  .pila {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    max-width: 18px;
    min-height: 2px;
    border-radius: 4px 4px 0 0;
    overflow: hidden;
    background: var(--surface-3);
  }
  .seg {
    min-height: 3px;
  }
  .seg.ok {
    background: var(--ok);
  }
  .seg.warn {
    background: var(--warn);
  }
  .seg.bad {
    background: var(--bad);
  }
  .dia {
    text-transform: uppercase;
  }
  .detalle {
    margin: var(--sp-2) 0 0;
  }
  .hechas {
    display: grid;
    gap: var(--sp-2);
    margin: var(--sp-3) 0 0;
    padding: 0;
    list-style: none;
  }
  .hechas li {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
  }
  .cuando {
    margin-left: auto;
    flex: none;
  }
  .r {
    display: grid;
    flex: none;
  }
  .r[data-r="ok"] {
    color: var(--ok);
  }
  .r[data-r="warning"] {
    color: var(--warn);
  }
  .r[data-r="error"] {
    color: var(--bad);
  }
</style>
