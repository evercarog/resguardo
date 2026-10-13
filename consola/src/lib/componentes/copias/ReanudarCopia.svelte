<script lang="ts">
  // Plan 0.8.0, F1 (14pp): una copia completada (hizo lo que tenía previsto) se
  // puede reanudar. Antes, cómo sigue: sin fin, N veces más, hasta un día o, una
  // de «una sola vez», otra vez ahora. Lo demás (la clave de administración y el
  // envío) lo hace el editor de copias, con la copia abierta en su resumen.
  import { CalendarCheck, Play } from "@lucide/svelte";
  import { comoSeguirParam, diaDe, MAX_VECES, type ComoSeguir } from "$lib/copiaGuiada";
  import type { CopiaResumen } from "$lib/tipos";

  let { copia, href, oncerrar }: { copia: CopiaResumen; /** La dirección del editor con la copia (`…/copias?copia=<id>`). */ href: string; oncerrar: () => void } = $props();

  const unaVez = $derived(!!copia.termina?.una_vez);
  let modo = $state<ComoSeguir["modo"]>("veces");
  let veces = $state(10);
  let hasta = $state(diaDe(Date.now() + 30 * 86_400_000));
  const como = $derived<ComoSeguir>(modo === "veces" ? { modo, veces } : modo === "hasta" ? { modo, hasta } : { modo });
  const vale = $derived(modo !== "veces" || (Number.isInteger(veces) && veces >= 1 && veces <= MAX_VECES));
  const destino = $derived(`${href}${href.includes("?") ? "&" : "?"}reanudar=${encodeURIComponent(comoSeguirParam(como))}`);
</script>

<div class="reanudar">
  <p class="titulo"><CalendarCheck size={16} />¿Cómo sigue «{copia.nombre}»?</p>
  <div class="opciones" role="radiogroup" aria-label="Cómo sigue la copia">
    {#if unaVez}
      <label class="op"><input type="radio" bind:group={modo} value="una_vez" /> Otra vez, ahora (una sola vez más)</label>
    {/if}
    <label class="op"><input type="radio" bind:group={modo} value="veces" /> <span>Después de</span> <input class="input num" type="number" min="1" max={MAX_VECES} bind:value={veces} aria-label="Copias más" onfocus={() => (modo = "veces")} /> <span>copias más</span></label>
    <label class="op"><input type="radio" bind:group={modo} value="hasta" /> <span>Hasta el</span> <input class="input" type="date" bind:value={hasta} aria-label="Último día" onfocus={() => (modo = "hasta")} /></label>
    <label class="op"><input type="radio" bind:group={modo} value="nunca" /> Sin fin{unaVez ? " (con horario: lo eliges en el editor)" : ""}</label>
  </div>
  <p class="faint nota">La cuenta vuelve a empezar. Te pedirá la clave de administración para enviarlo al equipo.</p>
  <div class="acciones">
    <button type="button" class="btn btn-ghost" onclick={oncerrar}>Cancelar</button>
    {#if vale}<a class="btn btn-primary" href={destino}><Play size={16} />Seguir</a>{:else}<button type="button" class="btn btn-primary" disabled>Seguir</button>{/if}
  </div>
</div>

<style>
  .reanudar {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-4);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
  }
  .titulo {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-weight: 600;
  }
  .opciones {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .op {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .op .input {
    width: auto;
  }
  .num {
    width: 6em !important;
  }
  .nota {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
