<script lang="ts">
  // Avisos por etiqueta de una persona (v1.52, tarea 6): para los equipos con
  // una etiqueta, otra cosa que lo general («de los de Servidores, todo»; «de
  // los de Pruebas, nada»). Si un equipo tiene varias con preferencia, vale lo
  // que pida cualquiera de ellas.
  import { Tag } from "@lucide/svelte";
  import { mismaEtiqueta } from "$lib/etiquetasGrupos";
  import type { PrefEtiqueta, Severidad } from "$lib/tipos";

  let { etiquetas, valor = $bindable([]), id = "avisos-et" }: { etiquetas: string[]; valor?: PrefEtiqueta[]; id?: string } = $props();

  const OPCIONES: [string, string, Severidad[] | null][] = [
    ["", "Como el resto", null],
    ["todo", "Todo", ["critico", "importante", "informativo"]],
    ["ci", "Crítico e importante", ["critico", "importante"]],
    ["c", "Solo crítico", ["critico"]],
    ["nada", "Nada al momento", []],
  ];
  function opcionDe(t: string): string {
    const p = valor.find((x) => mismaEtiqueta(x.etiqueta, t));
    if (!p) return "";
    const k = [...p.inmediatos].sort().join(",");
    return OPCIONES.find(([, , s]) => s && [...s].sort().join(",") === k)?.[0] ?? "ci";
  }
  function poner(t: string, op: string) {
    const s = OPCIONES.find(([k]) => k === op)?.[2];
    const resto = valor.filter((x) => !mismaEtiqueta(x.etiqueta, t));
    valor = s ? [...resto, { etiqueta: t, inmediatos: [...s] }] : resto;
  }
</script>

{#if etiquetas.length}
  <fieldset class="grupo-et">
    <legend><Tag size={14} />Por etiqueta</legend>
    <p class="faint pequeno">De los equipos con estas etiquetas, en lugar de lo general.</p>
    {#each etiquetas as t, i (t)}
      <div class="fila-et">
        <label for="{id}-{i}">{t}</label>
        <select id="{id}-{i}" class="input" value={opcionDe(t)} onchange={(e) => poner(t, e.currentTarget.value)}>
          {#each OPCIONES as [k, texto] (k)}<option value={k}>{texto}</option>{/each}
        </select>
      </div>
    {/each}
  </fieldset>
{/if}

<style>
  .grupo-et {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    border: none;
  }
  .grupo-et legend {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 4px;
    font-weight: 600;
  }
  .pequeno {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .fila-et {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 6px 12px;
    font-size: var(--fs-sm);
  }
  .fila-et label {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .fila-et .input {
    width: auto;
    min-width: 180px;
  }
</style>
