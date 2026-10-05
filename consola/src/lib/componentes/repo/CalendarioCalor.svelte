<script lang="ts">
  // El «calendario de calor» de las versiones (docs/diseno.md §4): como el
  // historial de contribuciones, una casilla por tramo de tiempo, más intensa
  // cuantas más versiones (cuatro escalones de un solo tono, de claro a
  // oscuro); rayada si la próxima retención las quitaría todas. En 7, 30 y 60
  // días, columnas = días y filas = horas (la cabecera de cada día se pulsa
  // para ver el día entero); en un año, semanas × días de la semana; en el
  // móvil, una tira de días que se desliza. Es una rejilla (`grid`) con un
  // solo punto de parada: flechas, Inicio y Fin, Re Pág y Av Pág (una
  // semana), Intro o espacio filtran la bitácora a esa casilla.
  import { tick } from "svelte";
  import { plural } from "$lib/formato";
  import type { Calendario, Celda } from "$lib/lineaTiempo";

  interface Props {
    cal: Calendario;
    /** El tramo que filtra la bitácora (se marca). */
    filtro: { desde: number; hasta: number } | null;
    /** La hora de la versión elegida (su casilla lleva un punto). */
    elegida: number | null;
    alFiltrar: (c: Celda, texto: string) => void;
    etiqueta: string;
    /** El id del resumen en texto (aria-describedby). */
    descrito?: string;
  }
  let { cal, filtro, elegida, alFiltrar, etiqueta, descrito }: Props = $props();

  const horas = $derived(cal.modo === "horas");
  const F = $derived(cal.filas.length);
  const N = $derived(cal.columnas.length);
  const fmtDia = new Intl.DateTimeFormat("es", { weekday: "short", day: "numeric", month: "short" });
  const fmtDiaAno = new Intl.DateTimeFormat("es", { weekday: "short", day: "numeric", month: "short", year: "numeric" });
  const fmtHora = new Intl.DateTimeFormat("es", { hour: "2-digit", minute: "2-digit" });

  /** La casilla en (fila, columna); la fila −1 es la cabecera de los días. */
  function celda(f: number, c: number): Celda | null {
    return f < 0 ? (cal.columnas[c]?.dia ?? null) : (cal.celdas[f]?.[c] ?? null);
  }
  function porClave(k: string): { f: number; c: number } | null {
    const m = /^(h|\d+)-(\d+)$/.exec(k);
    return m ? { f: m[1] === "h" ? -1 : Number(m[1]), c: Number(m[2]) } : null;
  }
  const clave = (f: number, c: number) => (f < 0 ? `h-${c}` : `${f}-${c}`);

  /** El texto de una casilla (aria-label y globo). */
  /** «mar, 29 sept» o, en una casilla de horas, «mar, 29 sept, 14:00–15:00». */
  function cuandoDe(x: Celda, cabecera = false): string {
    const d = new Date(x.desde);
    const dia = (d.getFullYear() === new Date().getFullYear() ? fmtDia : fmtDiaAno).format(d);
    return cabecera || cal.modo !== "horas" ? dia : `${dia}, ${fmtHora.format(x.desde)}–${fmtHora.format(x.hasta)}`;
  }
  function texto(x: Celda, cabecera = false): string {
    const cuando = cuandoDe(x, cabecera);
    if (x.fuera) return `${cuando}: fuera del periodo`;
    if (x.futura) return `${cuando}: todavía no`;
    const n = x.ids.length;
    const partes = [n ? plural(n, "versión", "versiones") : "sin versiones"];
    if (n && x.quitan) partes.push(x.quitan === n ? (n === 1 ? "la quitaría la próxima retención" : "las quitaría todas la próxima retención") : `${x.quitan} las quitaría la próxima retención`);
    if (cabecera) partes.push("ver el día entero");
    return `${cuando}: ${partes.join(", ")}`;
  }
  const enFiltro = (x: Celda) => !!filtro && x.desde >= filtro.desde && x.hasta <= filtro.hasta;
  const conElegida = (x: Celda) => elegida != null && elegida >= x.desde && elegida < x.hasta;

  // La casilla activa (roving tabindex): la de la elegida, la de ahora o la última.
  let activa = $state<string | null>(null);
  const inicial = $derived.by(() => {
    for (let f = 0; f < F; f++) for (let c = N - 1; c >= 0; c--) if (elegida != null && conElegida(cal.celdas[f][c])) return clave(f, c);
    for (let f = 0; f < F; f++) for (let c = 0; c < N; c++) if (cal.celdas[f][c].ahora) return clave(f, c);
    return clave(F - 1, N - 1);
  });
  const enfocable = $derived(activa && porClave(activa) && celda(porClave(activa)!.f, porClave(activa)!.c) ? activa : inicial);

  let rejilla = $state<HTMLDivElement | null>(null);
  let desliza = $state<HTMLDivElement | null>(null);
  let caja = $state<HTMLDivElement | null>(null);
  async function mover(f: number, c: number) {
    const fmin = horas ? -1 : 0;
    f = Math.max(fmin, Math.min(F - 1, f));
    c = Math.max(0, Math.min(N - 1, c));
    activa = clave(f, c);
    await tick();
    const el = rejilla?.querySelector<HTMLElement>(`[data-k="${activa}"]`);
    el?.focus();
    el?.scrollIntoView({ block: "nearest", inline: "nearest" });
    mostrar(el ?? null);
  }
  function tecla(ev: KeyboardEvent) {
    const p = porClave((ev.target as HTMLElement).dataset.k ?? "");
    if (!p) return;
    const k = ev.key;
    if (k === "ArrowLeft") mover(p.f, p.c - 1);
    else if (k === "ArrowRight") mover(p.f, p.c + 1);
    else if (k === "ArrowUp") mover(p.f - 1, p.c);
    else if (k === "ArrowDown") mover(p.f + 1, p.c);
    else if (k === "Home") mover(ev.ctrlKey ? (horas ? -1 : 0) : p.f, 0);
    else if (k === "End") mover(ev.ctrlKey ? F - 1 : p.f, N - 1);
    else if (k === "PageUp") mover(p.f, p.c - 7);
    else if (k === "PageDown") mover(p.f, p.c + 7);
    else if (k === "Enter" || k === " ") elegir(p.f, p.c);
    else if (k === "Escape") sobre = null;
    else return;
    ev.preventDefault();
  }
  function elegir(f: number, c: number) {
    const x = celda(f, c);
    if (!x || x.fuera || x.futura) return;
    activa = clave(f, c);
    alFiltrar(x, cuandoDe(x, f < 0));
  }
  function pulsar(ev: MouseEvent) {
    const p = porClave((ev.target as HTMLElement).closest<HTMLElement>("[data-k]")?.dataset.k ?? "");
    if (p) elegir(p.f, p.c);
  }

  // El globo: uno solo, fuera de lo que se desliza (no se recorta).
  let sobre = $state<{ x: number; y: number; texto: string } | null>(null);
  function mostrar(el: HTMLElement | null) {
    const p = el && porClave(el.dataset.k ?? "");
    const x = p && celda(p.f, p.c);
    if (!el || !x || !caja) return (sobre = null);
    const r = el.getBoundingClientRect();
    const b = caja.getBoundingClientRect();
    sobre = { x: r.left + r.width / 2 - b.left, y: r.top - b.top, texto: texto(x, p!.f < 0) };
  }
  const encima = (ev: PointerEvent) => mostrar((ev.target as HTMLElement).closest<HTMLElement>("[data-k]"));

  // En la tira y en el año, lo más reciente a la vista (a la derecha).
  $effect(() => {
    void cal;
    if (desliza) desliza.scrollLeft = desliza.scrollWidth;
  });

  // Rótulos de abajo (el día del mes): todos si caben; si no, los lunes y el 1.
  let ancho = $state(0);
  const colPx = $derived(N ? (ancho - (horas ? 44 : cal.modo === "dias" ? 22 : 0)) / N : 0);
  // Si no caben todos: los lunes, el 1 (si no queda pegado a un lunes) y hoy.
  const pies = $derived.by(() => {
    if (colPx >= 15) return cal.columnas.map((c) => (c.hoy && colPx >= 30 ? "Hoy" : c.pie));
    const out = cal.columnas.map((c) => {
      const d = new Date(c.desde);
      return c.hoy || d.getDay() === 1 || d.getDate() === 1 ? c.pie : null;
    });
    // Dos rótulos pegados: se queda el del 1 o el de hoy.
    for (let i = 1; i < out.length; i++)
      for (let j = Math.max(0, i - 2); j < i; j++)
        if (out[i] && out[j]) {
          const fuerte = (k: number) => cal.columnas[k].hoy || new Date(cal.columnas[k].desde).getDate() === 1;
          if (fuerte(i)) out[j] = null;
          else out[i] = null;
        }
    return out;
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="cal m-{cal.modo}" class:apretado={N >= 45} bind:this={caja} style:--n={N} onpointerleave={() => (sobre = null)}>
  <div class="desliza" bind:this={desliza} bind:clientWidth={ancho}>
    <div class="dentro">
      <!-- Los meses, arriba. -->
      <div class="pista meses" aria-hidden="true">
        <span></span>
        {#each cal.columnas as col, c (col.desde)}<span class="mes" style:grid-column={c + 2}>{col.mes ?? ""}</span>{/each}
      </div>
      <!-- El foco va a las casillas (roving tabindex), no a la rejilla. -->
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_interactive_supports_focus -->
      <div class="rejilla" role="grid" aria-label={etiqueta} aria-describedby={descrito} aria-rowcount={F + (horas ? 1 : 0)} aria-colcount={N + 1} bind:this={rejilla} onkeydown={tecla} onclick={pulsar} onpointerover={encima} onfocusin={(e) => (e.target as HTMLElement).matches(":focus-visible") && mostrar(e.target as HTMLElement)} onfocusout={() => (sobre = null)}>
        {#if horas}
          <div class="cf" role="row">
            <span class="etq" role="columnheader"><span class="sr-only">Hora</span></span>
            {#each cal.columnas as col, c (col.desde)}
              {@const k = clave(-1, c)}
              <span
                class="cab"
                class:hoy={col.hoy}
                class:finde={col.finde}
                class:en-filtro={!!col.dia && enFiltro(col.dia)}
                role="columnheader"
                tabindex={enfocable === k ? 0 : -1}
                data-k={k}
                aria-label={col.dia ? texto(col.dia, true) : undefined}>{col.inicial}</span
              >
            {/each}
          </div>
        {/if}
        {#each cal.filas as fila, f (f)}
          <div class="cf" role="row">
            <span class="etq" role="rowheader"><span aria-hidden="true">{fila.corto ?? ""}</span><span class="sr-only">{fila.texto}</span></span>
            {#each cal.celdas[f] as x, c (x.k)}
              {@const k = clave(f, c)}
              <span
                class="c n{x.nivel}"
                class:quita={x.ids.length > 0 && x.quitan === x.ids.length}
                class:futura={x.futura}
                class:fuera={x.fuera}
                class:ahora={x.ahora}
                class:en-filtro={enFiltro(x)}
                class:elegida={conElegida(x)}
                role="gridcell"
                tabindex={enfocable === k ? 0 : -1}
                aria-selected={enFiltro(x)}
                aria-disabled={x.fuera || x.futura || undefined}
                data-k={k}
                aria-label={texto(x)}
              ></span>
            {/each}
          </div>
        {/each}
      </div>
      {#if cal.modo !== "dias"}
        <!-- El día del mes, abajo. -->
        <div class="pista pies" aria-hidden="true">
          <span></span>
          {#each cal.columnas as col, c (col.desde)}
            <span class="pie" class:hoy={col.hoy} style:grid-column={c + 2}>{pies[c] ?? ""}</span>
          {/each}
        </div>
      {/if}
    </div>
  </div>
  {#if sobre}
    <div class="graf-tip globo" style:left="{sobre.x}px" style:top="{sobre.y}px" aria-hidden="true">{sobre.texto}</div>
  {/if}
</div>

<style>
  .cal {
    /* La escala de calor (--calor-0…4) la pone la línea de tiempo (LineaTiempoVersiones). */
    --raya: var(--text-3);
    --etq-ancho: 44px;
    --col: minmax(7px, 1fr);
    --alto: 12px;
    --hueco: 3px;
    position: relative;
    min-width: 0;
  }
  .m-dias {
    --etq-ancho: 18px;
    --col: minmax(9px, 15px);
  }
  .m-tira {
    --etq-ancho: 0px;
    --col: 20px;
    --alto: 26px;
  }
  .apretado {
    --hueco: 2px;
  }
  .desliza {
    overflow-x: auto;
    overscroll-behavior-x: contain;
    scrollbar-width: thin;
  }
  .m-tira .desliza,
  .m-dias .desliza {
    padding-bottom: 4px;
  }
  .dentro {
    display: flex;
    flex-direction: column;
    gap: var(--hueco);
    min-width: max-content;
  }
  .m-horas .dentro {
    min-width: calc(var(--etq-ancho) + var(--n) * 10px);
  }
  .pista,
  .cf {
    display: grid;
    grid-template-columns: var(--etq-ancho) repeat(var(--n), var(--col));
    gap: var(--hueco);
  }
  .rejilla {
    display: flex;
    flex-direction: column;
    gap: var(--hueco);
  }
  .rejilla:focus-within .c:focus-visible,
  .cab:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
    position: relative;
    z-index: 1;
  }
  .meses,
  .pies {
    font-size: 10.5px;
    line-height: 14px;
    color: var(--graf-eje);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .mes {
    overflow: visible;
    font-weight: 500;
    color: var(--text-2);
  }
  .pie {
    text-align: center;
  }
  .pie.hoy {
    justify-self: center;
    min-width: max-content;
    padding: 0 5px;
    font-weight: 600;
    color: var(--accent-text);
    background: var(--accent-soft);
    border-radius: 999px;
  }
  .etq {
    position: relative;
    font-size: 10.5px;
    line-height: 1;
    color: var(--graf-eje);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .etq > span[aria-hidden] {
    position: absolute;
    top: 50%;
    right: 6px;
    translate: 0 -50%;
  }

  /* La cabecera de cada día (vista por horas): su inicial; «Hoy» en una píldora del acento. Se pulsa. */
  .cab {
    display: grid;
    place-items: center;
    height: 18px;
    font-size: 10.5px;
    font-weight: 500;
    color: var(--text-3);
    border-radius: 4px;
    cursor: pointer;
    user-select: none;
  }
  .cab.finde {
    color: color-mix(in srgb, var(--text-3) 70%, transparent);
  }
  .cab:hover {
    background: var(--surface-2);
    color: var(--text-1);
  }
  .cab.hoy {
    font-weight: 700;
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .cab.en-filtro {
    box-shadow: inset 0 -2px 0 var(--accent);
    color: var(--text-1);
  }

  /* Las casillas. */
  .c {
    display: block;
    height: var(--alto);
    border-radius: 3px;
    background: var(--calor-0);
    cursor: pointer;
  }
  .m-dias .c {
    height: auto;
    aspect-ratio: 1;
    border-radius: 2px;
  }
  .n1 {
    background: var(--calor-1);
  }
  .n2 {
    background: var(--calor-2);
  }
  .n3 {
    background: var(--calor-3);
  }
  .n4 {
    background: var(--calor-4);
  }
  /* Todas las de la casilla las quitaría la próxima retención: rayada en tinta tenue, sin color. */
  .c.quita {
    background: repeating-linear-gradient(135deg, color-mix(in srgb, var(--raya) 80%, transparent) 0 1.5px, var(--calor-0) 1.5px 4px);
  }
  .c.futura,
  .c.fuera {
    background: transparent;
    box-shadow: inset 0 0 0 1px var(--border);
    cursor: default;
  }
  .c.fuera {
    box-shadow: none;
  }
  /* Ahora mismo: un recuadro en tinta. */
  .c.ahora {
    box-shadow: inset 0 0 0 1.5px var(--text-2);
  }
  .c:not(.futura):not(.fuera):hover {
    box-shadow: inset 0 0 0 1.5px var(--text-1);
  }
  /* Lo que filtra la bitácora: un recuadro del acento. */
  .c.en-filtro {
    box-shadow:
      0 0 0 1.5px var(--surface),
      0 0 0 3px var(--accent);
    position: relative;
    z-index: 1;
  }
  /* La de la versión elegida: un punto en el centro. */
  .c.elegida {
    background-image: radial-gradient(circle, var(--surface) 0 2.5px, var(--text-1) 2.5px 3.5px, transparent 3.5px);
  }

  .globo {
    translate: -50% calc(-100% - 6px);
    pointer-events: none;
    z-index: 5;
  }
</style>
