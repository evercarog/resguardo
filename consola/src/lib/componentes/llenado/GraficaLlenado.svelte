<script lang="ts">
  // La gráfica de «¿Cuándo se llena?»: lo ocupado los últimos 60 días (línea
  // continua con su área), la línea recta del ritmo actual hacia delante
  // (discontinua), la capacidad (discontinua, rotulada) y el punto en que se
  // llenaría. Una serie, un eje, tinta neutra; el color de estado va en el
  // chip de al lado con su icono. Al pasar el ratón o con las flechas (tras
  // enfocarla), una guía y el dato, leído también en una región viva; debajo,
  // «Ver los datos» con la misma serie en una tabla. En oscuro, el área lleva
  // un brillo suave.
  import { bytes, fechaCorta } from "$lib/formato";
  import { valorEn, mesYAno, type Prevision } from "$lib/llenado";

  let { p, ahora, alto = 120 }: { p: Prevision; ahora: number; alto?: number } = $props();

  const DIA = 86_400_000;
  let ancho = $state(0);
  const M = { izq: 54, der: 10, arr: 14, abj: 20 };
  const x0 = $derived(ahora - 60 * DIA);
  const x1 = $derived(
    p.lleno ? Math.min(p.lleno + Math.max(20 * DIA, (p.lleno - ahora) * 0.08), ahora + 5 * 365 * DIA) : p.porDia ? ahora + 365 * DIA : ahora + 7 * DIA,
  );
  const fin = $derived(p.lleno && p.lleno < x1 ? p.lleno : x1);
  // Eje de una línea (no de barras): empieza cerca de lo mínimo para que se vea cómo crece; rotulado.
  const rango = $derived.by(() => {
    const alFinal = valorEn(p, fin, ahora) ?? 0;
    const vs = p.serie.map((s) => s.v);
    const bajo = Math.min(...vs, p.usado ?? Infinity);
    const alto_ = p.total != null ? Math.max(p.total, alFinal) : Math.max(alFinal, ...vs);
    const margen = Math.max(1, (alto_ - bajo) * 0.18);
    return { min: Math.max(0, bajo - margen), max: alto_ + margen * 0.5 };
  });
  /** Los rótulos del eje con los decimales que hagan falta para que no digan lo mismo («2,199 TB» y «2,2 TB»). */
  function eje(v: number): string {
    const otro = v === rango.min ? (p.total ?? rango.max) : rango.min;
    if (bytes(v) !== bytes(otro)) return bytes(v);
    const u = ["B", "KB", "MB", "GB", "TB", "PB"];
    let i = 0;
    let x = v;
    while (x >= 1000 && i < u.length - 1) (x /= 1000), i++;
    return `${x.toLocaleString("es", { maximumFractionDigits: 3 })} ${u[i]}`;
  }
  const W = $derived(Math.max(160, ancho));
  const X = (t: number) => M.izq + ((t - x0) / (x1 - x0)) * (W - M.izq - M.der);
  const Y = (v: number) => M.arr + (1 - (v - rango.min) / (rango.max - rango.min)) * (alto - M.arr - M.abj);
  const fmtDia = new Intl.DateTimeFormat("es", { day: "numeric", month: "short" });
  const historia = $derived(p.serie.map((s, i) => `${i ? "L" : "M"}${X(s.t).toFixed(1)},${Y(s.v).toFixed(1)}`).join(""));
  const area = $derived(p.serie.length ? `${historia}L${X(p.serie.at(-1)!.t).toFixed(1)},${Y(rango.min)}L${X(p.serie[0].t).toFixed(1)},${Y(rango.min)}Z` : "");
  const proyeccion = $derived(p.usado != null && p.porDia ? `M${X(ahora)},${Y(p.usado)}L${X(fin)},${Y(valorEn(p, fin, ahora) ?? p.usado)}` : "");
  const id = $props.id();

  // Guía y dato al pasar el ratón o con las flechas.
  let sobre = $state<number | null>(null);
  const paso = $derived((x1 - x0) / 30);
  function mover(ev: PointerEvent) {
    const r = (ev.currentTarget as SVGElement).getBoundingClientRect();
    const t = x0 + ((ev.clientX - r.left - M.izq) / (W - M.izq - M.der)) * (x1 - x0);
    sobre = Math.min(x1, Math.max(x0, t));
  }
  function tecla(ev: KeyboardEvent) {
    if (!["ArrowLeft", "ArrowRight", "Home", "End", "Escape"].includes(ev.key)) return;
    ev.preventDefault();
    if (ev.key === "Escape") return void (sobre = null);
    if (ev.key === "Home") return void (sobre = x0);
    if (ev.key === "End") return void (sobre = fin);
    sobre = Math.min(x1, Math.max(x0, (sobre ?? ahora) + (ev.key === "ArrowLeft" ? -paso : paso)));
  }
  const dato = $derived.by(() => {
    if (sobre === null) return null;
    const v = valorEn(p, sobre, ahora);
    if (v == null) return null;
    const previsto = sobre > ahora;
    const lleno = p.total != null && v >= p.total;
    return { t: sobre, v: Math.min(v, p.total ?? v), texto: `${fechaCorta(new Date(sobre).toISOString())}: ${lleno ? "lleno" : bytes(Math.min(v, p.total ?? v))}${previsto ? " (previsto)" : ""}` };
  });
  const resumen = $derived(
    `${p.nombre}: ocupa ${bytes(p.usado)}${p.total != null ? ` de ${bytes(p.total)}` : ""}. ${p.frase} Es aproximado: no descuenta lo que quita la retención.${p.porDia ? " Usa las flechas para recorrer la gráfica." : ""}`,
  );
  // Los datos en tabla: cada 10 días de la historia y cada mes de la previsión.
  const filas = $derived.by(() => {
    const xs: { t: number; v: number; previsto: boolean }[] = [];
    for (let i = 0; i < p.serie.length; i += 10) xs.push({ ...p.serie[i], previsto: false });
    if (p.usado != null) xs.push({ t: ahora, v: p.usado, previsto: false });
    if (p.porDia && p.usado != null) {
      const meses = Math.min(60, Math.ceil((fin - ahora) / (30.44 * DIA)));
      for (let m = 1; m <= meses; m++) {
        const t = ahora + m * 30.44 * DIA;
        const v = valorEn(p, t, ahora)!;
        xs.push({ t, v: p.total != null ? Math.min(v, p.total) : v, previsto: true });
        if (p.total != null && v >= p.total) break;
      }
    }
    return xs;
  });
</script>

<figure class="llenado">
  <div class="marco" bind:clientWidth={ancho}>
    {#if ancho > 0}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <svg
        width={W}
        height={alto}
        role="img"
        tabindex="0"
        aria-label={resumen}
        onpointermove={mover}
        onpointerleave={() => (sobre = null)}
        onblur={() => (sobre = null)}
        onkeydown={tecla}
      >
        <defs>
          <linearGradient id="area-{id}" x1="0" x2="0" y1="0" y2="1">
            <stop offset="0" class="area-arriba" />
            <stop offset="1" class="area-abajo" />
          </linearGradient>
        </defs>
        <!-- Base y referencias -->
        <line class="base" x1={M.izq} x2={W - M.der} y1={Y(rango.min)} y2={Y(rango.min)} />
        <text class="eje" x={M.izq - 6} y={Y(rango.min)} dy="0.32em" text-anchor="end">{rango.min ? eje(rango.min) : "0"}</text>
        {#if p.total != null}
          <line class="capacidad" x1={M.izq} x2={W - M.der} y1={Y(p.total)} y2={Y(p.total)} />
          <text class="eje" x={M.izq - 6} y={Y(p.total)} dy="0.32em" text-anchor="end">{eje(p.total)}</text>
          <text class="rotulo" x={M.izq + 4} y={Y(p.total) - 4}>Capacidad</text>
        {:else}
          <text class="eje" x={M.izq - 6} y={M.arr} dy="0.32em" text-anchor="end">{bytes(rango.max)}</text>
        {/if}
        <line class="hoy" x1={X(ahora)} x2={X(ahora)} y1={M.arr - 4} y2={Y(rango.min)} />
        <!-- Historia (área y línea) y previsión (discontinua) -->
        {#if area}<path class="area" d={area} fill="url(#area-{id})" />{/if}
        {#if historia}<path class="linea" d={historia} />{/if}
        {#if proyeccion}<path class="prevision" d={proyeccion} />{/if}
        {#if p.lleno && p.lleno <= x1 && p.total != null}
          <circle class="lleno" cx={X(p.lleno)} cy={Y(p.total)} r="3.5" />
          <text class="rotulo" x={X(p.lleno) - 7} y={Y(p.total) - 5} text-anchor="end">lleno · {mesYAno(p.lleno)}</text>
        {/if}
        <!-- Fechas: el principio, hoy y el final -->
        {#if X(ahora) - M.izq > 64}<text class="eje" x={M.izq} y={alto - 4}>{fmtDia.format(x0)}</text>{/if}
        <text class="eje" x={X(ahora)} y={alto - 4} text-anchor="middle">hoy</text>
        <text class="eje" x={W - M.der} y={alto - 4} text-anchor="end">{mesYAno(x1)}</text>
        {#if dato}
          <line class="guia" x1={X(dato.t)} x2={X(dato.t)} y1={M.arr - 4} y2={Y(rango.min)} />
          <circle class="punto" cx={X(dato.t)} cy={Y(dato.v)} r="3" />
        {/if}
      </svg>
      {#if dato}
        <div class="tip num" style:left="{Math.min(Math.max(X(dato.t), 70), W - 70)}px" aria-hidden="true">{dato.texto}</div>
      {/if}
    {/if}
  </div>
  <span class="sr-only" aria-live="polite" aria-atomic="true">{dato?.texto ?? ""}</span>
  <details class="datos">
    <summary>Ver los datos</summary>
    <div class="desplazable">
      <table class="tabla">
        <caption class="sr-only">Espacio ocupado en {p.nombre}, por fecha</caption>
        <thead><tr><th scope="col">Fecha</th><th scope="col" class="der">Ocupado</th><th scope="col">Dato</th></tr></thead>
        <tbody>
          {#each filas as f (f.t)}
            <tr><td class="num">{fechaCorta(new Date(f.t).toISOString())}</td><td class="num der">{bytes(f.v)}</td><td class="faint">{f.previsto ? "previsto" : "medido"}</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
  </details>
</figure>

<style>
  .llenado {
    margin: 0;
    min-width: 0;
  }
  .marco {
    position: relative;
    min-width: 0;
  }
  svg {
    display: block;
    overflow: visible;
    border-radius: var(--radius-sm);
  }
  svg:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .base {
    stroke: var(--border-strong);
  }
  .capacidad {
    stroke: var(--text-3);
    stroke-dasharray: 2 3;
  }
  .hoy {
    stroke: var(--border-strong);
  }
  .guia {
    stroke: var(--text-3);
    stroke-dasharray: 2 2;
  }
  .linea {
    fill: none;
    stroke: var(--text-2);
    stroke-width: 1.5;
    stroke-linejoin: round;
  }
  .prevision {
    fill: none;
    stroke: var(--text-2);
    stroke-width: 1.5;
    stroke-dasharray: 5 4;
  }
  .area-arriba {
    stop-color: var(--text-3);
    stop-opacity: 0.2;
  }
  .area-abajo {
    stop-color: var(--text-3);
    stop-opacity: 0.02;
  }
  /* En oscuro, un brillo sobrio bajo la línea (como un gráfico en vivo). */
  :global(:root[data-theme="dark"]) .area-arriba,
  :global(:root[data-theme="black"]) .area-arriba {
    stop-color: var(--accent);
    stop-opacity: 0.28;
  }
  :global(:root[data-theme="dark"]) .linea,
  :global(:root[data-theme="black"]) .linea {
    filter: drop-shadow(0 0 3px color-mix(in srgb, var(--accent) 45%, transparent));
  }
  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-theme="light"])) .area-arriba {
      stop-color: var(--accent);
      stop-opacity: 0.28;
    }
    :global(:root:not([data-theme="light"])) .linea {
      filter: drop-shadow(0 0 3px color-mix(in srgb, var(--accent) 45%, transparent));
    }
  }
  .lleno {
    fill: var(--surface);
    stroke: var(--text-1);
    stroke-width: 1.5;
  }
  .punto {
    fill: var(--accent);
    stroke: var(--surface);
    stroke-width: 1.5;
  }
  .eje,
  .rotulo {
    font-size: 10.5px;
    font-variant-numeric: tabular-nums;
    fill: var(--text-3);
  }
  .rotulo {
    fill: var(--text-2);
  }
  .tip {
    position: absolute;
    top: -6px;
    padding: 3px 8px;
    font-size: var(--fs-xs);
    color: var(--bg);
    white-space: nowrap;
    background: var(--text-1);
    border-radius: 6px;
    translate: -50% -100%;
    pointer-events: none;
  }
  .datos {
    margin-top: 6px;
    font-size: var(--fs-xs);
  }
  .datos summary {
    color: var(--text-2);
    cursor: pointer;
  }
  .datos .tabla {
    margin-top: 6px;
  }
  .der {
    text-align: right;
  }
</style>
