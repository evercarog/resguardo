<script lang="ts">
  // Órdenes con su estado, quién las pidió, la respuesta del equipo y si esa
  // respuesta está firmada por él. Las que aún se pueden cancelar, con su botón.
  import { ShieldAlert, ShieldCheck, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { resultadoFirmado } from "$lib/cripto/claves";
  import { actual, cargarCliente, puede, reloj } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { cuentaAtras, fechaLarga } from "$lib/formato";
  import { ESTADO_ORDEN, nombreOrden } from "$lib/salud";
  import type { Equipo, Orden } from "$lib/tipos";
  import Chip from "./Chip.svelte";
  import { iconoOrden } from "$lib/iconos";
  import Tiempo from "./Tiempo.svelte";

  let { ordenes, equipos, mostrarEquipo = false, alCambiar }: { ordenes: Orden[]; equipos: Equipo[]; mostrarEquipo?: boolean; alCambiar?: () => void } = $props();
  let cancelando = $state<string | null>(null);

  /** El detalle es un JSON en texto, firmado con el resultado (api-servidor.md §5). */
  function detalle(o: Orden): string {
    if (!o.detalle) return "";
    try {
      const d = JSON.parse(o.detalle) as Record<string, unknown>;
      if (d.sellado) return "Con detalle cifrado para quien la envió";
      if (typeof d.espera_min_horas === "number") return `Espera del equipo: ${d.espera_min_horas} h`;
      if (typeof d.trozos === "number") return `${d.trozos} ${d.trozos === 1 ? "trozo" : "trozos"} en el relé`;
      return "";
    } catch {
      return "";
    }
  }
  const equipoDe = (o: Orden) => equipos.find((e) => e.id === o.equipo) ?? (equipos.length === 1 ? equipos[0] : undefined);
  const esperando = (o: Orden) => o.estado === "pendiente" && !!o.not_before && Date.parse(o.not_before) > reloj.ahora;
  const cancelable = (o: Orden) => o.estado === "pendiente" && (esperando(o) || !o.not_before);

  async function cancelar(o: Orden) {
    cancelando = o.id;
    try {
      await api.cancelarOrden(actual.id, o.id);
      avisar(`Orden cancelada: «${nombreOrden(o.tipo)}».`);
      alCambiar?.();
      void cargarCliente(actual.id, { silencioso: true });
    } catch (e) {
      fallo(e);
    } finally {
      cancelando = null;
    }
  }
</script>

<ul class="ordenes">
  {#each ordenes as o (o.id)}
    {@const eq = equipoDe(o)}
    {@const firmada = eq ? resultadoFirmado(eq.sign_pub, o) : false}
    {@const Ic = iconoOrden(o.tipo)}
    <li>
      <span class="ic tone-{esperando(o) ? 'warn' : ESTADO_ORDEN[o.estado].tono}" aria-hidden="true"><Ic size={15} /></span>
      <div class="cuerpo">
      <div class="cab">
        <span class="titulo">
          <strong>{nombreOrden(o.tipo)}</strong>
          {#if mostrarEquipo && eq}<a class="eq" href="/c/{actual.id}/equipos/{eq.id}?tab=ordenes">{eq.nombre}</a>{/if}
        </span>
        {#if esperando(o)}
          <Chip tono="warn" texto="Se aplica en {cuentaAtras(o.not_before!, reloj.ahora)}" />
        {:else}
          <Chip tono={ESTADO_ORDEN[o.estado].tono} texto={ESTADO_ORDEN[o.estado].texto} girando={o.estado === "en_marcha"} />
        {/if}
      </div>
      <p class="meta faint">
        N.º {o.seq} · {o.emitida_por.nombre} · <Tiempo iso={o.emitida} />
        {#if o.not_before}{" · "}espera hasta <time datetime={o.not_before}>{fechaLarga(o.not_before)}</time>{/if}
      </p>
      {#if o.mensaje}<p class="mensaje">{o.mensaje}</p>{/if}
      <div class="pie">
        {#if o.firma_agente}
          <span class="firma" class:ok={firmada}>
            {#if firmada}<ShieldCheck size={13} />Firmada por el equipo{:else}<ShieldAlert size={13} />Firma no válida{/if}
          </span>
        {/if}
        {#if detalle(o)}<span class="faint det">{detalle(o)}</span>{/if}
        {#if cancelable(o) && puede.ordenar(actual.cliente?.rol)}
          <button class="btn btn-sm cancelar" disabled={cancelando === o.id} onclick={() => cancelar(o)}><X size={14} />{cancelando === o.id ? "Cancelando…" : "Cancelar"}</button>
        {/if}
      </div>
      </div>
    </li>
  {/each}
</ul>

<style>
  .ordenes {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    gap: var(--sp-3);
    padding: 12px var(--sp-4);
    border-top: 1px solid var(--border);
    transition: background var(--dur-fast) var(--ease);
  }
  li:hover {
    background: color-mix(in srgb, var(--surface-2) 60%, transparent);
  }
  .ic {
    display: grid;
    flex: none;
    place-items: center;
    width: 30px;
    height: 30px;
    margin-top: 1px;
    color: var(--tone);
    background: color-mix(in srgb, var(--tone) var(--soft), transparent);
    border-radius: var(--radius);
  }
  .cuerpo {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .eq {
    color: var(--text-2);
    text-decoration: none;
  }
  .eq::before {
    content: "en ";
    color: var(--text-3);
  }
  .eq:hover {
    color: var(--text-1);
    text-decoration: underline;
  }
  li:first-child {
    border-top: none;
  }
  .cab {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 6px 12px;
  }
  .titulo {
    display: flex;
    flex-wrap: wrap;
    gap: 0 6px;
  }
  .titulo strong {
    font-weight: 600;
  }
  p {
    margin: 0;
  }
  .meta {
    font-size: var(--fs-sm);
  }
  .mensaje {
    font-size: var(--fs-sm);
    color: var(--text-1);
  }
  .pie {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    font-size: var(--fs-xs);
  }
  .pie:empty {
    display: none;
  }
  .firma {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--bad);
    font-weight: 500;
  }
  .firma.ok {
    color: var(--ok);
  }
  .cancelar {
    margin-left: auto;
  }
</style>
