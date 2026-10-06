<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Un hueco «en camino» en la lista donde aparecerá el resultado de una
  // orden: qué es, en qué punto está (enviada, esperando al equipo o sin
  // conexión, aplicándose, lista o con su error) y, si es de las que esperan,
  // cuándo se aplicará. Ver lib/pendientes.svelte.ts.
  import { Check, Clock, LoaderCircle, TriangleAlert, WifiOff, X } from "@lucide/svelte";
  import { actual, reloj } from "$lib/estado.svelte";
  import { cuentaAtras } from "$lib/formato";
  import { conError, quitarPendiente, type Pendiente } from "$lib/pendientes.svelte";
  import { porQueNoSeAplico } from "$lib/espera";

  let { p, forma = "fila", conEquipo = false }: { p: Pendiente; forma?: "fila" | "tarjeta"; conEquipo?: boolean } = $props();

  const o = $derived(p.orden);
  const conectado = $derived(actual.equipos.find((e) => e.id === p.equipo)?.conectado ?? true);
  // v1.49: también «entregada» antes de su hora (el equipo la tiene en espera).
  const esperaSeguridad = $derived((o.estado === "pendiente" || o.estado === "entregada") && !!o.not_before && Date.parse(o.not_before) > reloj.ahora);
  const mal = $derived(conError(o));
  const estado = $derived<"espera" | "sin_conexion" | "camino" | "marcha" | "hecha" | "mal">(
    mal ? "mal" : o.estado === "hecha" ? "hecha" : esperaSeguridad ? "espera" : o.estado === "en_marcha" ? "marcha" : !conectado ? "sin_conexion" : "camino",
  );
  const VERBO: Record<string, string> = { crear_repositorio: "Creando", importar_repositorio: "Importando", conectar_nube: "Conectando", quitar_nube: "Desconectando" };
  const texto = $derived.by(() => {
    const eq = p.equipoNombre;
    switch (estado) {
      case "mal":
        return porQueNoSeAplico(o) ?? o.mensaje ?? `${eq} no la aplicó.`;
      case "hecha":
        return "Listo.";
      case "espera":
        return `Espera de seguridad: se aplicará dentro de ${cuentaAtras(o.not_before!, reloj.ahora)}. Se puede cancelar en Órdenes.`;
      case "marcha":
        return `${VERBO[o.tipo] ?? "Aplicando"} en ${eq}…`;
      case "sin_conexion":
        return `${eq} está sin conexión: se aplicará cuando vuelva.`;
      default:
        return o.estado === "entregada" ? `${eq} la ha recibido…` : `${VERBO[o.tipo] ?? "Enviada"} · esperando a ${eq}…`;
    }
  });
</script>

<div class="pendiente {forma}" class:mal class:hecha={estado === "hecha"} role="status" aria-live="polite">
  <span class="ic">
    {#if estado === "mal"}<TriangleAlert size={16} />
    {:else if estado === "hecha"}<Check size={16} />
    {:else if estado === "espera"}<Clock size={16} />
    {:else if estado === "sin_conexion"}<WifiOff size={16} />
    {:else}<LoaderCircle size={16} class="spin" />{/if}
  </span>
  <span class="texto">
    <span class="titulo">{p.titulo}{#if conEquipo}<span class="faint">{" · "}{p.equipoNombre}</span>{/if}</span>
    <span class="estado">{texto}{#if estado === "espera" || estado === "mal"} <a class="link" href="/c/{p.cliente}/ordenes">Ver en Órdenes</a>{/if}</span>
  </span>
  {#if estado === "mal" || estado === "hecha"}
    <button type="button" class="icon-btn" aria-label="Quitar este aviso" use:tip={"Quitar"} onclick={() => quitarPendiente(o.id)}><X size={14} /></button>
  {/if}
</div>

<style>
  .pendiente {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-height: 44px;
    padding: 10px var(--sp-4);
    background: color-mix(in srgb, var(--accent) 5%, var(--surface));
    border: 1px dashed color-mix(in srgb, var(--accent) 45%, var(--border));
    border-radius: var(--radius);
  }
  .pendiente.tarjeta {
    align-items: flex-start;
    padding: var(--sp-4);
    border-radius: var(--radius-lg);
  }
  .pendiente.mal {
    background: color-mix(in srgb, var(--bad) 6%, var(--surface));
    border: 1px solid color-mix(in srgb, var(--bad) 40%, var(--border));
  }
  .pendiente.hecha {
    background: color-mix(in srgb, var(--ok) 6%, var(--surface));
    border: 1px solid color-mix(in srgb, var(--ok) 35%, var(--border));
  }
  .ic {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    color: var(--accent-text);
    background: var(--accent-soft);
    border-radius: var(--radius);
  }
  .mal .ic {
    color: var(--bad);
    background: color-mix(in srgb, var(--bad) var(--soft), transparent);
  }
  .hecha .ic {
    color: var(--ok);
    background: color-mix(in srgb, var(--ok) var(--soft), transparent);
  }
  .texto {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1;
    min-width: 0;
  }
  .titulo {
    font-weight: 500;
    overflow-wrap: anywhere;
  }
  .estado {
    font-size: var(--fs-sm);
    color: var(--text-2);
    overflow-wrap: anywhere;
  }
  .mal .estado {
    color: var(--bad);
  }
</style>
