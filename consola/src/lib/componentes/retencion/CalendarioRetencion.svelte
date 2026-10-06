<script lang="ts">
  // El calendario de «Retención en detalle»: las últimas semanas y las que
  // vienen, con lo que se eliminó (el día de cada vuelta: un cuadro relleno con
  // cuántas quitó; con borde rojo si alguna falló) y lo que se eliminará (el día
  // en que dejan de entrar en la regla: un cuadro con borde discontinuo). Forma,
  // número y texto, nunca solo el color. Pulsar un día con algo lo elige (y
  // filtra las listas de abajo); otra vez, lo quita.
  import { claveDia } from "$lib/retencionDetalle";

  type Marca = { quitadas: number; vueltas: number; fallos: number; previstas: number };
  let {
    marcas,
    ahora,
    atras = 35,
    adelante = 30,
    dia = null,
    alDia,
  }: {
    marcas: Map<string, Marca>;
    ahora: number;
    /** Días hacia atrás y hacia delante. */
    atras?: number;
    adelante?: number;
    dia?: string | null;
    alDia: (k: string | null) => void;
  } = $props();

  const INICIALES = ["L", "M", "X", "J", "V", "S", "D"];
  const fmtLargo = new Intl.DateTimeFormat("es", { weekday: "long", day: "numeric", month: "long" });
  const fmtMes = new Intl.DateTimeFormat("es", { month: "short" });

  const semanas = $derived.by(() => {
    const hoy = new Date(ahora);
    const desde = new Date(hoy.getFullYear(), hoy.getMonth(), hoy.getDate() - atras);
    const hasta = new Date(hoy.getFullYear(), hoy.getMonth(), hoy.getDate() + adelante);
    // Desde el lunes de la primera semana.
    const lunes = new Date(desde.getFullYear(), desde.getMonth(), desde.getDate() - ((desde.getDay() + 6) % 7));
    const out: { mes: string | null; dias: { k: string; d: Date; dentro: boolean; hoy: boolean; futuro: boolean; m: Marca | null }[] }[] = [];
    let mesAnterior = -1;
    for (let s = new Date(lunes); s <= hasta; s = new Date(s.getFullYear(), s.getMonth(), s.getDate() + 7)) {
      const dias = Array.from({ length: 7 }, (_, i) => {
        const d = new Date(s.getFullYear(), s.getMonth(), s.getDate() + i);
        const k = claveDia(d.getTime());
        return { k, d, dentro: d >= desde && d <= hasta, hoy: k === claveDia(ahora), futuro: d.getTime() > ahora, m: marcas.get(k) ?? null };
      });
      const primero = dias.find((x) => x.d.getDate() === 1) ?? (out.length ? null : dias[0]);
      const mes = primero && primero.d.getMonth() !== mesAnterior ? fmtMes.format(primero.d) : null;
      if (primero) mesAnterior = primero.d.getMonth();
      out.push({ mes, dias });
    }
    return out;
  });

  function etiqueta(x: { d: Date; m: Marca | null; hoy: boolean }) {
    const partes = [`${fmtLargo.format(x.d)}${x.hoy ? " (hoy)" : ""}`];
    if (x.m?.vueltas) partes.push(`${x.m.vueltas === 1 ? "se aplicó la retención" : `se aplicó la retención ${x.m.vueltas} veces`}: se ${x.m.quitadas === 1 ? "eliminó 1 versión" : `eliminaron ${x.m.quitadas} versiones`}${x.m.fallos ? ` (${x.m.fallos === 1 ? "una falló" : `${x.m.fallos} fallaron`})` : ""}`);
    if (x.m?.previstas) partes.push(`se ${x.m.previstas === 1 ? "eliminará 1 versión" : `eliminarán ${x.m.previstas} versiones`} (simulado)`);
    return partes.join(". ");
  }
</script>

<div class="cal" role="group" aria-label="Calendario de la retención">
  <div class="semana cab" aria-hidden="true">
    <span></span>
    {#each INICIALES as i (i)}<span class="ini">{i}</span>{/each}
  </div>
  {#each semanas as s, n (n)}
    <div class="semana">
      <span class="mes">{s.mes ?? ""}</span>
      {#each s.dias as x (x.k)}
        {#if !x.dentro}
          <span class="dia fuera" aria-hidden="true"></span>
        {:else if x.m && (x.m.vueltas || x.m.previstas)}
          <button
            class="dia con"
            class:hoy={x.hoy}
            class:futuro={x.futuro}
            class:elegido={dia === x.k}
            aria-pressed={dia === x.k}
            aria-label={etiqueta(x)}
            title={etiqueta(x)}
            onclick={() => alDia(dia === x.k ? null : x.k)}
          >
            <span class="num-dia">{x.d.getDate()}</span>
            {#if x.m.vueltas}<span class="marca se-fue" class:fallo={x.m.fallos > 0}>{x.m.fallos && !x.m.quitadas ? "!" : `−${x.m.quitadas}`}</span>{/if}
            {#if x.m.previstas}<span class="marca se-ira">−{x.m.previstas}</span>{/if}
          </button>
        {:else}
          <span class="dia" class:hoy={x.hoy} class:futuro={x.futuro} title={etiqueta(x)}><span class="num-dia">{x.d.getDate()}</span></span>
        {/if}
      {/each}
    </div>
  {/each}
  <p class="leyenda">
    <span><span class="marca se-fue">−n</span> se eliminó (el día que se aplicó la retención)</span>
    <span><span class="marca se-fue fallo">!</span> falló al aplicarla</span>
    <span><span class="marca se-ira">−n</span> se eliminará (simulado: el día que deja de entrar en la regla)</span>
  </p>
</div>

<style>
  .cal {
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-width: 560px;
  }
  .semana {
    display: grid;
    grid-template-columns: 9ch repeat(7, minmax(0, 1fr));
    gap: 3px;
    align-items: stretch;
  }
  .ini {
    text-align: center;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .mes {
    font-size: var(--fs-xs);
    color: var(--text-3);
    align-self: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dia {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: flex-start;
    gap: 1px;
    min-height: 44px;
    padding: 3px 2px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    border: 1px solid transparent;
    font: inherit;
    color: var(--text-2);
  }
  .dia.futuro {
    background: transparent;
    border-color: var(--border);
  }
  .dia.fuera {
    background: transparent;
  }
  .dia.hoy {
    border-color: var(--accent);
    border-width: 2px;
  }
  button.dia {
    cursor: pointer;
    transition: border-color var(--dur-fast) var(--ease);
  }
  button.dia:hover {
    border-color: var(--text-3);
  }
  button.dia:focus-visible {
    outline: 2px solid var(--focus, var(--accent));
    outline-offset: 1px;
  }
  .dia.elegido {
    background: var(--accent-soft);
    border-color: var(--accent);
  }
  .num-dia {
    font-size: var(--fs-xs);
    line-height: 1.2;
  }
  .marca {
    display: inline-block;
    min-width: 3ch;
    padding: 0 3px;
    border-radius: 4px;
    font-size: 11px;
    line-height: 15px;
    font-variant-numeric: tabular-nums;
    text-align: center;
  }
  .se-fue {
    background: var(--text-2);
    color: var(--surface);
  }
  .se-fue.fallo {
    background: var(--bad);
    color: var(--bad-contrast, #fff);
  }
  .se-ira {
    border: 1.5px dashed var(--warn);
    color: var(--text-1);
    line-height: 12px;
  }
  .leyenda {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    margin: 8px 0 0;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .leyenda > span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  @media (max-width: 480px) {
    .semana {
      grid-template-columns: 5ch repeat(7, minmax(0, 1fr));
    }
    .mes {
      font-size: 10px;
    }
  }
</style>
