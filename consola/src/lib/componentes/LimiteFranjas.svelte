<script lang="ts">
  // Plan 0.8.0, F4 (14t): límite de velocidad según la hora (en «Más opciones»
  // del espejo y de la copia derivada). Una lista de franjas con sus días, de
  // qué hora a qué hora y su límite; fuera de ellas, el de siempre.
  import { Gauge, Plus, X } from "@lucide/svelte";
  import { errorFranjas, franjaNueva, franjasEnFrase, MAX_FRANJAS } from "$lib/franjas";
  import { DIAS_CORTOS } from "$lib/retencion";
  import type { FranjaVelocidad } from "$lib/tipos";

  let { id, franjas = $bindable([]), porDefecto = null, admite = true, equipo = "" }: { id: string; franjas: FranjaVelocidad[]; porDefecto?: number | null; admite?: boolean; equipo?: string } = $props();

  const error = $derived(errorFranjas(franjas));
  const poner = (i: number, cambio: Partial<FranjaVelocidad>) => (franjas = franjas.map((f, j) => (j === i ? { ...f, ...cambio } : f)));
  const alternarDia = (i: number, d: number) => poner(i, { dias: franjas[i].dias.includes(d) ? franjas[i].dias.filter((x) => x !== d) : [...franjas[i].dias, d].sort((a, b) => a - b) });
</script>

<div class="franjas" role="group" aria-labelledby="{id}-t">
  <span class="field-label" id="{id}-t"><Gauge size={14} />Velocidad según la hora</span>
  {#if !admite}
    <p class="faint">Actualiza el agente{equipo ? ` de ${equipo}` : ""} para limitar la velocidad según la hora.</p>
  {:else}
    {#each franjas as f, i (i)}
      <div class="franja">
        <div class="dias" role="group" aria-label="Días de la franja {i + 1}">
          {#each DIAS_CORTOS as d, j (d)}
            <button type="button" class="dia" class:on={f.dias.includes(j + 1)} aria-pressed={f.dias.includes(j + 1)} onclick={() => alternarDia(i, j + 1)}>{d}</button>
          {/each}
        </div>
        <label>de <input class="input hora" type="time" value={f.desde} aria-label="Desde (franja {i + 1})" onchange={(e) => poner(i, { desde: e.currentTarget.value })} /></label>
        <label>a <input class="input hora" type="time" value={f.hasta} aria-label="Hasta (franja {i + 1})" onchange={(e) => poner(i, { hasta: e.currentTarget.value })} /></label>
        <label><input class="input num" type="number" min="0" placeholder="sin límite" value={f.kib || ""} aria-label="KiB/s (franja {i + 1}); vacío, sin límite" oninput={(e) => poner(i, { kib: Math.trunc(Number(e.currentTarget.value) || 0) })} /> KiB/s</label>
        <button type="button" class="icon-btn mini" aria-label="Quitar la franja {i + 1}" onclick={() => (franjas = franjas.filter((_, j) => j !== i))}><X size={14} /></button>
      </div>
    {/each}
    {#if franjas.length < MAX_FRANJAS}
      <button type="button" class="btn btn-sm btn-ghost" onclick={() => (franjas = [...franjas, franjaNueva()])}><Plus size={14} />{franjas.length ? "Otra franja" : "Limitar en unas horas"}</button>
    {/if}
    {#if error}<p class="err" role="alert">{error}</p>{:else if franjas.length}<p class="faint frase">{franjasEnFrase(franjas, porDefecto)}. Cada subida empieza con el límite de su hora.</p>{/if}
  {/if}
</div>

<style>
  .franjas {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .field-label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .franja {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .dias {
    display: inline-flex;
    gap: 2px;
  }
  .dia {
    width: 28px;
    height: 28px;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--text-2);
    font: inherit;
    font-size: var(--fs-xs);
    cursor: pointer;
  }
  .dia.on {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .hora {
    width: 7.5em;
  }
  .num {
    width: 7em;
  }
  .err {
    margin: 0;
    color: var(--bad);
    font-size: var(--fs-sm);
  }
  .frase {
    margin: 0;
    font-size: var(--fs-sm);
  }
</style>
