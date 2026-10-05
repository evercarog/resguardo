<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Órdenes destructivas esperando su turno: «Pendiente: … · Cancelar».
  // Cualquiera del cliente (salvo «solo lectura») puede cancelarlas.
  import { onMount, untrack } from "svelte";
  import { Clock, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { enFondo } from "$lib/actividad.svelte";
  import { seguirCambios } from "$lib/vivo.svelte";
  import { actual, cargarCliente, puede, reloj } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { cuentaAtras, fechaLarga } from "$lib/formato";
  import { nombreOrden } from "$lib/salud";
  import type { Orden } from "$lib/tipos";
  import Ayuda from "./Ayuda.svelte";

  let ordenes = $state<Orden[]>([]);
  let cancelando = $state<string | null>(null);
  let confirmar = $state<string | null>(null);

  async function cargar() {
    if (!actual.id) return;
    try {
      ordenes = await api.ordenesPendientes(actual.id);
    } catch {
      /* se reintenta en la siguiente vuelta */
    }
  }
  onMount(() => {
    void cargar();
    // Con el canal en vivo, cuando cambia una orden; sin él, cada 20 s.
    return seguirCambios(() => enFondo(cargar), { ms: 20_000, toca: (x) => x.t === "orden" });
  });
  // Al cambiar los pendientes del resumen (otra persona canceló o pidió algo), se recarga.
  $effect(() => {
    void actual.pendientes;
    untrack(() => void cargar());
  });

  const equipo = (id?: string) => actual.equipos.find((e) => e.id === id)?.nombre ?? "un equipo";

  async function cancelar(o: Orden) {
    cancelando = o.id;
    try {
      await api.cancelarOrden(actual.id, o.id);
      avisar(`Cancelada: «${nombreOrden(o.tipo)}» en ${equipo(o.equipo)}.`);
      confirmar = null;
      await Promise.all([cargar(), cargarCliente(actual.id, { silencioso: true })]);
    } catch (e) {
      fallo(e);
    } finally {
      cancelando = null;
    }
  }
</script>

{#if ordenes.length}
  <section class="card pendientes" aria-labelledby="t-pend">
    <div class="cab">
      <Clock size={16} />
      <h2 id="t-pend" class="section-title">Órdenes esperando su turno · {ordenes.length}</h2>
      <Ayuda id="espera" />
    </div>
    <ul>
      {#each ordenes as o (o.id)}
        <li>
          <span class="texto">
            <span><strong>{nombreOrden(o.tipo)}</strong> en {equipo(o.equipo)}</span>
            <span class="faint">Pedida por {o.emitida_por.nombre} · se aplica en <time datetime={o.not_before} use:tip={fechaLarga(o.not_before)}>{cuentaAtras(o.not_before!, reloj.ahora)}</time></span>
          </span>
          {#if puede.ordenar(actual.cliente?.rol)}
            {#if confirmar === o.id}
              <span class="conf">
                <button class="btn btn-sm btn-ghost" onclick={() => (confirmar = null)}>No</button>
                <button class="btn btn-sm btn-danger" disabled={cancelando === o.id} onclick={() => cancelar(o)}>{cancelando === o.id ? "Cancelando…" : "Sí, cancelar"}</button>
              </span>
            {:else}
              <button class="btn btn-sm" onclick={() => (confirmar = o.id)}><X size={14} />Cancelar</button>
            {/if}
          {/if}
        </li>
      {/each}
    </ul>
    <p class="faint nota">Si no esperabas alguna, cancélala y cambia la clave de administración.</p>
  </section>
{/if}

<style>
  .pendientes {
    padding: var(--sp-4) var(--sp-5);
    border-color: color-mix(in srgb, var(--warn) 30%, var(--border));
    background: color-mix(in srgb, var(--warn) 4%, var(--surface));
  }
  .cab {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--warn);
  }
  .cab h2 {
    color: var(--text-1);
  }
  ul {
    display: flex;
    flex-direction: column;
    margin: var(--sp-3) 0 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2) var(--sp-4);
    padding: 10px 0;
    border-top: 1px solid var(--border);
  }
  .texto {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .texto .faint {
    font-size: var(--fs-sm);
  }
  .conf {
    display: flex;
    gap: 6px;
  }
  .nota {
    margin: var(--sp-2) 0 0;
    font-size: var(--fs-xs);
  }
</style>
