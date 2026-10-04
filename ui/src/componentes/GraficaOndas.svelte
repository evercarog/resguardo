<script lang="ts" module>
  /** Una serie de la gráfica: sus puntos `[instante en ms, valor]`, en orden. */
  export interface SerieOnda {
    id: string;
    nombre: string;
    /** Variable CSS del color (`--onda-1`, `--onda-2`, `--onda-3`). */
    color: string;
    puntos: [number, number][];
  }

  /** «12,4 MB/s», «850 kB/s» (unidades decimales, como restic y la consola). */
  export function porSegundo(v: number): string {
    const u = ["B/s", "kB/s", "MB/s", "GB/s"];
    let i = 0;
    while (v >= 1000 && i < u.length - 1) {
      v /= 1000;
      i++;
    }
    const n = v.toLocaleString("es", { maximumFractionDigits: v < 10 && i > 0 ? 1 : 0 });
    return `${n} ${u[i]}`;
  }
</script>

<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { prefersReducedMotion } from "svelte/motion";

  // Gráfica en vivo «de ondas» (docs/diseno.md, «Gráficas en vivo»): áreas
  // translúcidas con degradado y un brillo suave que avanzan con el tiempo.
  // Una sola escala (nunca dos ejes): las cifras de otra unidad van en otra
  // gráfica. La leyenda dice el valor de ahora y el pico de cada serie; un
  // resumen para lectores de pantalla y una cruz al pasar el ratón con los
  // valores de ese instante. Se pinta a 30 fps como mucho, solo si se ve; con
  // «reducir movimiento», solo cuando llegan datos.
  interface Props {
    series: SerieOnda[];
    /** Nombre de la gráfica (para el resumen accesible). */
    titulo: string;
    /** Segundos que se ven (por defecto, 5 minutos). */
    ventana?: number;
    alto?: number;
    formato?: (v: number) => string;
    /** Retraso con el que se pinta (ms): da tiempo a que llegue la muestra siguiente y la onda fluye sin saltos. */
    retraso?: number;
    /** Sin leyenda (si la pone quien la usa). */
    sinLeyenda?: boolean;
  }
  let { series, titulo, ventana = 300, alto = 140, formato = porSegundo, retraso = 2500, sinLeyenda = false }: Props = $props();

  let lienzo = $state<HTMLCanvasElement>();
  let caja = $state<HTMLDivElement>();
  let ancho = $state(0);
  let cursor = $state<number | null>(null);
  let visible = true;
  let maxSuave = 0;
  let marco = 0;
  let ultimoPintado = 0;
  let colores: Record<string, string> = {};
  let tinta = { linea: "#888", texto: "#888" };
  let leidoColores = 0;

  const ahoraPintado = () => Date.now() - retraso;

  const enVentana = (s: SerieOnda, desde: number) => s.puntos.filter((p) => p[0] >= desde - 5000);
  const actual = (s: SerieOnda) => (s.puntos.length ? s.puntos[s.puntos.length - 1][1] : 0);
  const pico = (s: SerieOnda) => {
    const desde = Date.now() - ventana * 1000;
    return s.puntos.reduce((m, p) => (p[0] >= desde ? Math.max(m, p[1]) : m), 0);
  };
  const resumen = $derived(
    `${titulo}: ` +
      (series.some((s) => s.puntos.length)
        ? series.map((s) => `${s.nombre} ${formato(actual(s))} ahora, pico ${formato(pico(s))}`).join("; ")
        : "sin datos todavía"),
  );

  function leerColores() {
    if (!caja) return;
    const cs = getComputedStyle(caja);
    for (const s of series) colores[s.color] = cs.getPropertyValue(s.color).trim() || "#2a78d6";
    tinta = { linea: cs.getPropertyValue("--border").trim() || "#ddd", texto: cs.getPropertyValue("--text-3").trim() || "#888" };
    leidoColores = Date.now();
  }

  /** Curva suave que no se pasa de los puntos (Fritsch–Carlson, monótona por tramos). */
  function curva(ctx: CanvasRenderingContext2D, xs: number[], ys: number[]) {
    const n = xs.length;
    if (n < 2) return;
    const d: number[] = [];
    const m: number[] = [];
    for (let i = 0; i < n - 1; i++) d.push((ys[i + 1] - ys[i]) / (xs[i + 1] - xs[i] || 1));
    m.push(d[0]);
    for (let i = 1; i < n - 1; i++) m.push(d[i - 1] * d[i] <= 0 ? 0 : (d[i - 1] + d[i]) / 2);
    m.push(d[n - 2]);
    for (let i = 0; i < n - 1; i++) {
      if (d[i] === 0) {
        m[i] = 0;
        m[i + 1] = 0;
        continue;
      }
      const a = m[i] / d[i];
      const b = m[i + 1] / d[i];
      const h = a * a + b * b;
      if (h > 9) {
        const t = 3 / Math.sqrt(h);
        m[i] = t * a * d[i];
        m[i + 1] = t * b * d[i];
      }
    }
    for (let i = 0; i < n - 1; i++) {
      const dx = (xs[i + 1] - xs[i]) / 3;
      ctx.bezierCurveTo(xs[i] + dx, ys[i] + m[i] * dx, xs[i + 1] - dx, ys[i + 1] - m[i + 1] * dx, xs[i + 1], ys[i + 1]);
    }
  }

  function pintar() {
    if (!lienzo || !ancho) return;
    if (Date.now() - leidoColores > 1000) leerColores();
    const dpr = window.devicePixelRatio || 1;
    const w = ancho;
    const h = alto;
    if (lienzo.width !== Math.round(w * dpr) || lienzo.height !== Math.round(h * dpr)) {
      lienzo.width = Math.round(w * dpr);
      lienzo.height = Math.round(h * dpr);
    }
    const ctx = lienzo.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    const ahora = ahoraPintado();
    const desde = ahora - ventana * 1000;
    const x = (t: number) => ((t - desde) / (ventana * 1000)) * w;
    // La escala se acerca poco a poco al máximo (sin saltos al llegar un pico).
    const objetivo = Math.max(1, ...series.flatMap((s) => enVentana(s, desde).map((p) => p[1]))) * 1.18;
    maxSuave = maxSuave ? maxSuave + (objetivo - maxSuave) * (prefersReducedMotion.current ? 1 : 0.12) : objetivo;
    const arriba = 6;
    const y = (v: number) => h - 1 - (v / maxSuave) * (h - arriba - 1);
    ctx.strokeStyle = tinta.linea;
    ctx.lineWidth = 1;
    ctx.fillStyle = tinta.texto;
    ctx.font = "11px system-ui, sans-serif";
    ctx.textBaseline = "bottom";
    // Rejilla: dos líneas en las grandes, una en las bajas (sin cifras encimadas).
    for (const f of h >= 90 ? [1 / 3, 2 / 3] : [2 / 3]) {
      const yy = Math.round(h - f * (h - arriba)) + 0.5;
      ctx.beginPath();
      ctx.moveTo(0, yy);
      ctx.lineTo(w, yy);
      ctx.stroke();
      ctx.fillText(formato((f * maxSuave) / 1.0), 4, yy - 2);
    }
    // De la más alta a la más baja: las pequeñas quedan delante y se ven.
    const orden = [...series].sort((a, b) => pico(b) - pico(a));
    for (const s of orden) {
      const pts = enVentana(s, desde);
      if (pts.length < 2) continue;
      const color = colores[s.color] || "#2a78d6";
      const xs = pts.map((p) => x(p[0]));
      const ys = pts.map((p) => y(p[1]));
      // Área con degradado.
      const g = ctx.createLinearGradient(0, arriba, 0, h);
      g.addColorStop(0, color + "8c");
      g.addColorStop(0.55, color + "33");
      g.addColorStop(1, color + "05");
      ctx.beginPath();
      ctx.moveTo(xs[0], h);
      ctx.lineTo(xs[0], ys[0]);
      curva(ctx, xs, ys);
      ctx.lineTo(xs[xs.length - 1], h);
      ctx.closePath();
      ctx.fillStyle = g;
      ctx.fill();
      // La línea, con brillo.
      ctx.save();
      ctx.shadowColor = color;
      ctx.shadowBlur = 10;
      ctx.strokeStyle = color;
      ctx.lineWidth = 2;
      ctx.lineJoin = "round";
      ctx.beginPath();
      ctx.moveTo(xs[0], ys[0]);
      curva(ctx, xs, ys);
      ctx.stroke();
      ctx.restore();
    }
    // La cruz al pasar el ratón.
    if (cursor !== null) {
      ctx.strokeStyle = tinta.texto;
      ctx.setLineDash([3, 3]);
      ctx.beginPath();
      ctx.moveTo(cursor + 0.5, 0);
      ctx.lineTo(cursor + 0.5, h);
      ctx.stroke();
      ctx.setLineDash([]);
    }
    ultimoPintado = Date.now();
  }

  function bucle() {
    marco = 0;
    const animar = !prefersReducedMotion.current && visible && !document.hidden;
    if (animar && Date.now() - ultimoPintado >= 33) pintar();
    if (animar) marco = requestAnimationFrame(bucle);
  }

  function arrancar() {
    if (!marco) marco = requestAnimationFrame(bucle);
  }

  // Datos nuevos (o tema, o tamaño): se pinta ya y, si toca, sigue fluyendo.
  $effect(() => {
    void series;
    void ancho;
    void cursor;
    pintar();
    arrancar();
  });

  /** Lo que valía cada serie en el instante bajo el ratón. */
  const enCursor = $derived.by(() => {
    if (cursor === null || !ancho) return null;
    const t = ahoraPintado() - ventana * 1000 + (cursor / ancho) * ventana * 1000;
    return series.map((s) => {
      let mejor: [number, number] | null = null;
      for (const p of s.puntos) if (!mejor || Math.abs(p[0] - t) < Math.abs(mejor[0] - t)) mejor = p;
      return { s, v: mejor && Math.abs(mejor[0] - t) < 10_000 ? mejor[1] : null };
    });
  });

  let observador: ResizeObserver | undefined;
  let interseccion: IntersectionObserver | undefined;
  const alCambiarVisibilidad = () => arrancar();
  onMount(() => {
    observador = new ResizeObserver((e) => (ancho = Math.floor(e[0].contentRect.width)));
    if (caja) observador.observe(caja);
    interseccion = new IntersectionObserver((e) => {
      visible = e[0].isIntersecting;
      if (visible) arrancar();
    });
    if (caja) interseccion.observe(caja);
    document.addEventListener("visibilitychange", alCambiarVisibilidad);
    leerColores();
  });
  onDestroy(() => {
    observador?.disconnect();
    interseccion?.disconnect();
    if (typeof document !== "undefined") document.removeEventListener("visibilitychange", alCambiarVisibilidad);
    if (marco) cancelAnimationFrame(marco);
  });

  function mover(e: PointerEvent) {
    const r = lienzo!.getBoundingClientRect();
    cursor = Math.max(0, Math.min(r.width, e.clientX - r.left));
  }
</script>

<div class="ondas" bind:this={caja}>
  {#if !sinLeyenda}
    <ul class="leyenda">
      {#each series as s (s.id)}
        <li>
          <span class="muestra" style:background={`var(${s.color})`}></span>
          <span class="nombre">{s.nombre}</span>
          <span class="num valor">{formato(actual(s))}</span>
          <span class="num pico">pico {formato(pico(s))}</span>
        </li>
      {/each}
    </ul>
  {/if}
  <div class="lienzo" style:height={`${alto}px`} role="img" aria-label={resumen}>
    <canvas
      bind:this={lienzo}
      style:width="100%"
      style:height={`${alto}px`}
      aria-hidden="true"
      onpointermove={mover}
      onpointerleave={() => (cursor = null)}
    ></canvas>
    {#if enCursor && cursor !== null}
      <div class="tip" style:left={`${Math.min(cursor + 10, Math.max(0, ancho - 150))}px`} aria-hidden="true">
        {#each enCursor as { s, v } (s.id)}
          <div><span class="muestra" style:background={`var(${s.color})`}></span>{s.nombre}: <b class="num">{v === null ? "—" : formato(v)}</b></div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .ondas {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }
  .leyenda {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1) var(--sp-4);
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    color: var(--text-2);
  }
  .leyenda li {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .muestra {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 3px;
    margin-right: 2px;
    flex: none;
  }
  .nombre {
    color: var(--text-2);
  }
  .valor {
    color: var(--text-1);
    font-weight: 600;
  }
  .pico {
    color: var(--text-3);
  }
  .num {
    font-variant-numeric: tabular-nums;
  }
  .lienzo {
    position: relative;
    border-radius: var(--radius);
    overflow: hidden;
  }
  canvas {
    display: block;
    touch-action: none;
  }
  .tip {
    position: absolute;
    top: 6px;
    pointer-events: none;
    padding: 6px 8px;
    border-radius: var(--radius-sm);
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-md);
    font-size: var(--fs-xs);
    color: var(--text-2);
    white-space: nowrap;
  }
  .tip div {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .tip b {
    color: var(--text-1);
  }
</style>
