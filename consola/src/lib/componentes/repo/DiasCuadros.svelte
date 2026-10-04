<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Cuadros por día (como en la app de escritorio y la web anterior): el más
  // reciente a la derecha. Con versiones, verde; solo «sin cambios», verde
  // suave; avisos, ámbar; solo fallos, rojo; sin copia, pista neutra. Cada
  // cuadro lleva la fecha y el resultado (también para lectores de pantalla).
  import type { Dia } from "$lib/repo";

  let {
    dias,
    tamano = "md",
    elegido = null,
    alElegir,
    etiqueta,
    leyenda = false,
  }: { dias: Dia[]; tamano?: "md" | "mini"; elegido?: string | null; alElegir?: (clave: string | null) => void; etiqueta: string; leyenda?: boolean } = $props();

  // Texto alternativo: el recuento de cada resultado («9 con versiones, 1 falló…»), no solo el título.
  const NOMBRES: Record<Dia["estado"], [string, string]> = {
    datos: ["con versiones", "con versiones"],
    igual: ["sin cambios", "sin cambios"],
    aviso: ["con avisos", "con avisos"],
    mal: ["falló", "fallaron"],
    nada: ["sin copia", "sin copia"],
  };
  const resumen = $derived.by(() => {
    const n = { datos: 0, igual: 0, aviso: 0, mal: 0, nada: 0 };
    for (const d of dias) n[d.estado]++;
    const partes = (Object.keys(NOMBRES) as Dia["estado"][]).filter((k) => n[k]).map((k) => `${n[k]} ${n[k] === 1 ? "día" : "días"} ${NOMBRES[k][n[k] === 1 ? 0 : 1]}`);
    return `${etiqueta}: ${partes.join(", ")}.`;
  });

  // Con `alElegir`, un solo punto de parada para el tabulador: las flechas van de un día con copias (o
  // intentos) a otro; se puede elegir también un día que solo tuvo fallos, para ver qué pasó.
  const elegibles = $derived(dias.filter((d) => d.n > 0 || d.estado !== "nada").map((d) => d.clave));
  const enfocable = $derived(elegido && elegibles.includes(elegido) ? elegido : elegibles.at(-1));
  function flechas(e: KeyboardEvent) {
    const i = elegibles.indexOf((e.target as HTMLElement).dataset.clave ?? "");
    if (i < 0) return;
    const j = e.key === "ArrowRight" || e.key === "ArrowDown" ? i + 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? i - 1 : e.key === "Home" ? 0 : e.key === "End" ? elegibles.length - 1 : null;
    if (j == null) return;
    e.preventDefault();
    const k = elegibles[Math.max(0, Math.min(elegibles.length - 1, j))];
    (e.currentTarget as HTMLElement).querySelector<HTMLElement>(`[data-clave="${k}"]`)?.focus();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="cuadros {tamano}" role={alElegir ? "group" : "img"} aria-label={alElegir ? `${resumen} Usa las flechas para elegir un día.` : resumen} onkeydown={alElegir ? flechas : undefined}>
  {#each dias as d (d.clave)}
    {#if alElegir}
      <button
        type="button"
        class="c c-{d.estado}"
        class:on={elegido === d.clave}
        data-clave={d.clave}
        tabindex={d.clave === enfocable ? 0 : -1}
        use:tip={d.titulo}
        aria-label={d.titulo}
        aria-pressed={elegido === d.clave}
        disabled={d.n === 0 && d.estado === "nada"}
        onclick={() => alElegir(elegido === d.clave ? null : d.clave)}
      ></button>
    {:else}
      <span class="c c-{d.estado}" use:tip={d.titulo}></span>
    {/if}
  {/each}
</div>
{#if leyenda}
  <p class="leyenda">
    <span><i class="c c-datos"></i>Con versiones</span>
    <span><i class="c c-igual"></i>Sin cambios</span>
    <span><i class="c c-aviso"></i>Con avisos</span>
    <span><i class="c c-mal"></i>Falló</span>
    <span><i class="c c-nada"></i>Sin copia</span>
  </p>
{/if}

<style>
  .cuadros {
    --c: 11px;
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
  }
  .cuadros.mini {
    --c: 8px;
    flex-wrap: nowrap;
  }
  .c {
    display: inline-block;
    flex: none;
    width: var(--c, 10px);
    height: var(--c, 10px);
    padding: 0;
    border: none;
    border-radius: 2px;
    background: var(--surface-3);
  }
  button.c {
    cursor: pointer;
  }
  button.c:disabled {
    cursor: default;
  }
  button.c:hover:not(:disabled) {
    outline: 1px solid var(--text-3);
    outline-offset: 1px;
  }
  .c-datos {
    background: color-mix(in srgb, var(--ok) 85%, transparent);
  }
  .c-igual {
    background: color-mix(in srgb, var(--ok) 40%, transparent);
  }
  .c-aviso {
    background: var(--warn);
  }
  .c-mal {
    background: var(--bad);
  }
  .c.on {
    outline: 2px solid var(--text-1);
    outline-offset: 1px;
  }
  .leyenda {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    margin: 8px 0 0;
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    color: var(--text-3);
  }
  .leyenda span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .leyenda .c {
    --c: 9px;
  }
</style>
