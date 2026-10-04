<script lang="ts">
  // La línea de tiempo de las versiones («máquina del tiempo», docs/diseno.md
  // §4): a todo lo ancho, con escala (día, semana, mes, año), arrastre, la
  // rueda horizontal y las flechas. Cada versión es una marca del color y la
  // forma de su copia (tres colores validados para daltonismo; las demás en
  // tinta neutra); las que la próxima retención quitaría, huecas y tenues;
  // debajo, una franja por regla de la retención con lo que guarda cada una.
  // Al pasar el ratón o con el foco, su fecha, tamaño y copia; al pulsar (o
  // Intro), se elige. Es un solo punto de parada (listbox con
  // aria-activedescendant); la lista de siempre sigue debajo.
  import { ChevronLeft, ChevronRight, CalendarClock } from "@lucide/svelte";
  import type { Regla } from "$lib/tipos";
  import { bytes, plural } from "$lib/formato";
  import { huecosDeCopia, marcasEje, NOMBRE_MOTIVO, retencionDe, ZOOM, ZOOMS, zoomInicial, type VersionLinea, type Zoom } from "$lib/lineaTiempo";
  import { PERIODOS, textoRegla } from "$lib/retencion";

  interface Props {
    versiones: VersionLinea[];
    copias?: { id: string; nombre: string }[];
    regla?: Regla | null;
    /** Quién aplica la retención (un almacén), si no es el equipo. */
    quien?: string | null;
    ahora: number;
    seleccion?: string | null;
    alElegir: (id: string) => void;
    /** «Elegir» en Restaurar; «Ver» en el repositorio. */
    verbo?: string;
    etiqueta: string;
  }
  let { versiones, copias = [], regla = null, quien = null, ahora, seleccion = null, alElegir, verbo = "Elegir", etiqueta }: Props = $props();

  const id = $props.id();
  const lista = $derived([...versiones].sort((a, b) => Date.parse(a.hora) - Date.parse(b.hora)).map((v) => ({ ...v, t: Date.parse(v.hora) })));
  const huecos = $derived(huecosDeCopia(copias, lista.map((v) => v.copia)));
  const nombreCopia = (c: string | null | undefined) => copias.find((k) => k.id === c)?.nombre ?? (c ? c : "Sin copia");
  const leyenda = $derived.by(() => {
    const out: { hueco: 0 | 1 | 2 | 3; texto: string }[] = [];
    for (const [c, h] of huecos) if (h < 3) out.push({ hueco: h, texto: nombreCopia(c) });
    if ([...huecos.values()].includes(3) || lista.some((v) => !v.copia)) out.push({ hueco: 3, texto: "Otras" });
    return out;
  });
  const hueco = (c: string | null | undefined) => (c ? (huecos.get(c) ?? 3) : 3);
  const motivos = $derived(retencionDe(lista, regla));
  const quitaria = $derived(motivos ? [...motivos.values()].filter((m) => m === null).length : 0);

  // La ventana visible: `ancho` ms terminando en `fin`.
  let zoom = $state<Zoom>("mes");
  let fin = $state(0);
  let iniciado = false;
  $effect(() => {
    if (iniciado || !lista.length) return;
    iniciado = true;
    zoom = zoomInicial(lista.map((v) => v.t), ahora);
    const sel = seleccion ? lista.find((v) => v.id === seleccion) : null;
    fin = sel && sel.t < ahora - ZOOM[zoom].ms * 0.8 ? sel.t + ZOOM[zoom].ms / 2 : ahora + ZOOM[zoom].ms * 0.03;
  });
  const anchoMs = $derived(ZOOM[zoom].ms);
  const desde = $derived(fin - anchoMs);
  const primera = $derived(lista[0]?.t ?? ahora);
  function limitar(f: number) {
    return Math.min(ahora + anchoMs * 0.25, Math.max(primera + anchoMs * 0.1, f));
  }
  const pct = (t: number) => ((t - desde) / anchoMs) * 100;
  const visibles = $derived(lista.filter((v) => v.t >= desde - anchoMs * 0.02 && v.t <= fin + anchoMs * 0.02));
  const marcas = $derived(marcasEje(desde, fin, zoom));
  function cambiarZoom(z: Zoom) {
    // Se mantiene el centro (o la versión activa).
    const centro = activa ? activa.t : fin - anchoMs / 2;
    zoom = z;
    fin = limitar(centro + ZOOM[z].ms / 2);
  }
  const mover = (fraccion: number) => (fin = limitar(fin + anchoMs * fraccion));

  // Franjas de la retención: de la más antigua a la más reciente que guarda cada regla (en la ventana).
  const franjas = $derived.by(() => {
    if (!motivos) return [];
    return PERIODOS.map((p) => {
      const xs = lista.filter((v) => motivos.get(v.id) === p);
      return { p, xs, n: xs.length };
    }).filter((f) => f.n);
  });

  // La versión activa (con el teclado o el ratón) y su tarjeta.
  let activaId = $state<string | null>(null);
  let sobreId = $state<string | null>(null);
  const activa = $derived(lista.find((v) => v.id === (sobreId ?? activaId)) ?? null);
  const fmtLargo = new Intl.DateTimeFormat("es", { weekday: "long", day: "numeric", month: "long", year: "numeric", hour: "2-digit", minute: "2-digit" });
  function motivoTexto(vid: string): string | null {
    if (!motivos) return null;
    const m = motivos.get(vid);
    return m ? `Se queda (${NOMBRE_MOTIVO[m]})` : "La próxima retención la quitaría";
  }
  function describir(v: (typeof lista)[number]): string {
    return [
      fmtLargo.format(v.t),
      v.bytes != null ? bytes(v.bytes) : null,
      v.anadido != null ? `${bytes(v.anadido)} nuevos` : null,
      v.archivos != null ? plural(v.archivos, "archivo", "archivos") : null,
      `copia «${nombreCopia(v.copia)}»`,
      motivoTexto(v.id)?.replace(/^./, (x) => x.toLowerCase()),
      v.id === seleccion ? "elegida" : null,
    ]
      .filter(Boolean)
      .join(", ");
  }
  function activar(v: (typeof lista)[number] | undefined) {
    if (!v) return;
    activaId = v.id;
    if (v.t < desde + anchoMs * 0.05 || v.t > fin - anchoMs * 0.05) fin = limitar(v.t + anchoMs / 2);
  }
  function tecla(ev: KeyboardEvent) {
    const i = lista.findIndex((v) => v.id === activaId);
    const k = ev.key;
    if (k === "ArrowLeft" || k === "ArrowUp") activar(lista[i < 0 ? lista.length - 1 : Math.max(0, i - 1)]);
    else if (k === "ArrowRight" || k === "ArrowDown") activar(lista[i < 0 ? lista.length - 1 : Math.min(lista.length - 1, i + 1)]);
    else if (k === "Home") activar(lista[0]);
    else if (k === "End") activar(lista.at(-1));
    else if (k === "PageUp") mover(-0.8);
    else if (k === "PageDown") mover(0.8);
    else if (k === "+" || k === "-") {
      const j = ZOOMS.indexOf(zoom) + (k === "+" ? -1 : 1);
      if (ZOOMS[j]) cambiarZoom(ZOOMS[j]);
    } else if ((k === "Enter" || k === " ") && activaId) alElegir(activaId);
    else return;
    ev.preventDefault();
  }
  function alEnfocar() {
    if (!activaId) activar(lista.find((v) => v.id === seleccion) ?? lista.at(-1));
  }

  // Arrastrar para moverse (y la rueda horizontal). Un clic sin arrastre elige.
  let pista = $state<HTMLDivElement | null>(null);
  let arrastre: { x: number; fin: number; movido: boolean } | null = null;
  function bajar(ev: PointerEvent) {
    if (ev.button !== 0) return;
    arrastre = { x: ev.clientX, fin, movido: false };
    pista?.setPointerCapture(ev.pointerId);
  }
  function moverPuntero(ev: PointerEvent) {
    if (!arrastre || !pista) return;
    const dx = ev.clientX - arrastre.x;
    if (Math.abs(dx) > 3) arrastre.movido = true;
    if (arrastre.movido) fin = limitar(arrastre.fin - (dx / pista.clientWidth) * anchoMs);
  }
  function subir(ev: PointerEvent) {
    const a = arrastre;
    arrastre = null;
    if (a && !a.movido) {
      const el = (ev.target as HTMLElement).closest<HTMLElement>("[data-v]");
      if (el?.dataset.v) {
        activaId = el.dataset.v;
        alElegir(el.dataset.v);
      }
    }
  }
  function rueda(ev: WheelEvent) {
    if (!pista || Math.abs(ev.deltaX) <= Math.abs(ev.deltaY)) return;
    ev.preventDefault();
    fin = limitar(fin + (ev.deltaX / pista.clientWidth) * anchoMs);
  }
  const fmtCorto = new Intl.DateTimeFormat("es", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
</script>

{#if lista.length}
  <div class="linea">
    <div class="herr">
      <div class="segmentos" role="group" aria-label="Escala">
        {#each ZOOMS as z (z)}
          <button type="button" aria-pressed={zoom === z} onclick={() => cambiarZoom(z)}>{ZOOM[z].texto}</button>
        {/each}
      </div>
      <div class="nav">
        <button type="button" class="icon-btn" aria-label="Antes" onclick={() => mover(-0.6)}><ChevronLeft size={16} /></button>
        <button type="button" class="btn btn-sm btn-ghost" onclick={() => (fin = limitar(ahora + anchoMs * 0.03))}><CalendarClock size={14} />Hoy</button>
        <button type="button" class="icon-btn" aria-label="Después" onclick={() => mover(0.6)}><ChevronRight size={16} /></button>
      </div>
    </div>

    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="pista"
      bind:this={pista}
      role="listbox"
      tabindex="0"
      aria-label="{etiqueta}. Flechas: versión anterior o siguiente; Intro: {verbo.toLowerCase()}; + y −: escala; Re Pág y Av Pág: moverse."
      aria-activedescendant={activaId ? `${id}-${activaId}` : undefined}
      onkeydown={tecla}
      onfocus={alEnfocar}
      onpointerdown={bajar}
      onpointermove={moverPuntero}
      onpointerup={subir}
      onpointercancel={() => (arrastre = null)}
      onwheel={rueda}
    >
      <div class="rejilla" aria-hidden="true">
        {#each marcas as m (m.t)}
          <span class="m" class:fuerte={m.fuerte} style:left="{pct(m.t)}%"><span class="m-txt">{m.texto}</span></span>
        {/each}
        {#if ahora >= desde && ahora <= fin}<span class="ahora" style:left="{pct(ahora)}%"><span>ahora</span></span>{/if}
      </div>
      {#each visibles as v (v.id)}
        {@const motivo = motivos?.get(v.id)}
        <div
          id="{id}-{v.id}"
          class="marca h{hueco(v.copia)}"
          class:quita={motivos && motivo === null}
          class:elegida={v.id === seleccion}
          class:activa={v.id === activa?.id}
          role="option"
          tabindex="-1"
          aria-selected={v.id === seleccion}
          aria-label={describir(v)}
          data-v={v.id}
          style:left="{pct(v.t)}%"
          onpointerenter={() => (sobreId = v.id)}
          onpointerleave={() => (sobreId = null)}
        >
          <span class="forma"></span>
        </div>
      {/each}
      {#if activa && activa.t >= desde && activa.t <= fin}
        {@const m = motivoTexto(activa.id)}
        <div class="ficha" style:left="{Math.min(88, Math.max(12, pct(activa.t)))}%" aria-hidden="true">
          <strong class="num">{fmtCorto.format(activa.t)}</strong>
          <span class="num">{[activa.bytes != null ? bytes(activa.bytes) : null, activa.anadido != null ? `+${bytes(activa.anadido)}` : null].filter(Boolean).join(" · ")}</span>
          <span><span class="mini h{hueco(activa.copia)}"><span class="forma"></span></span>{nombreCopia(activa.copia)}</span>
          {#if m}<span class="faint">{m}</span>{/if}
        </div>
      {/if}
    </div>

    {#if franjas.length}
      <div class="franjas" aria-hidden="true">
        {#each franjas as f (f.p)}
          {@const a = Math.max(0, pct(f.xs[0].t))}
          {@const b = Math.min(100, pct(f.xs.at(-1)!.t))}
          <div class="franja">
            <span class="f-et">{f.p} · {f.n}</span>
            <div class="f-pista">
              {#if b >= 0 && a <= 100}<span class="f-tramo" style:left="{a}%" style:width="{Math.max(0.6, b - a)}%"></span>{/if}
              {#each f.xs.filter((v) => v.t >= desde && v.t <= fin) as v (v.id)}<span class="f-punto" style:left="{pct(v.t)}%"></span>{/each}
            </div>
          </div>
        {/each}
      </div>
    {/if}

    <!-- Fuera del listbox: lo que tiene el foco, leído. -->
    <span class="sr-only" aria-live="polite" aria-atomic="true">{activaId && lista.find((v) => v.id === activaId) ? describir(lista.find((v) => v.id === activaId)!) : ""}</span>

    <div class="leyenda">
      {#if leyenda.length > 1 || leyenda[0]?.hueco !== 3}
        {#each leyenda as l (l.hueco + l.texto)}<span class="l"><span class="mini h{l.hueco}"><span class="forma"></span></span>{l.texto}</span>{/each}
      {/if}
      {#if regla}
        <span class="l"><span class="mini quita h3"><span class="forma"></span></span>La próxima retención la quitaría · {quitaria}</span>
        <span class="faint regla">Simulado con la retención {quien ? `que aplica ${quien}` : "del repositorio"}: {textoRegla(regla)}.</span>
      {/if}
    </div>
  </div>
{/if}

<style>
  .linea {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
  }
  .herr {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .segmentos {
    display: inline-flex;
    padding: 2px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .segmentos button {
    min-height: 26px;
    padding: 0 10px;
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 500;
    color: var(--text-2);
    background: none;
    border: 0;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .segmentos button[aria-pressed="true"] {
    color: var(--text-1);
    background: var(--surface);
    box-shadow: var(--shadow-sm), 0 0 0 1px var(--border);
  }

  /* La pista: el eje abajo, las marcas encima. */
  .pista {
    position: relative;
    height: 112px;
    overflow: hidden;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: grab;
    touch-action: pan-y;
    user-select: none;
  }
  .pista:active {
    cursor: grabbing;
  }
  .pista:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .rejilla {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .m {
    position: absolute;
    top: 0;
    bottom: 0;
    border-left: 1px solid var(--graf-rejilla);
  }
  .m.fuerte {
    border-left-color: var(--graf-base);
  }
  .m-txt {
    position: absolute;
    bottom: 4px;
    left: 4px;
    font-size: 10.5px;
    font-variant-numeric: tabular-nums;
    color: var(--graf-eje);
    white-space: nowrap;
  }
  .ahora {
    position: absolute;
    top: 0;
    bottom: 0;
    border-left: 1px dashed var(--graf-guia);
  }
  .ahora span {
    position: absolute;
    top: 3px;
    right: 4px;
    font-size: 10.5px;
    color: var(--text-3);
  }

  /* Cada versión: una línea fina del color de su copia y, arriba, su forma. */
  .marca {
    position: absolute;
    top: 22px;
    width: 12px;
    height: 58px;
    margin-left: -6px;
    cursor: pointer;
  }
  .marca::before {
    position: absolute;
    top: 8px;
    bottom: 0;
    left: 5px;
    width: 2px;
    content: "";
    background: var(--c);
    border-radius: 2px;
  }
  .forma {
    position: absolute;
    top: 0;
    left: 2px;
    width: 8px;
    height: 8px;
    background: var(--c);
    border: 1.5px solid var(--c);
    border-radius: 999px;
  }
  .h1 .forma {
    border-radius: 1px;
  }
  .h2 .forma {
    border-radius: 1px;
    rotate: 45deg;
    scale: 0.9;
  }
  .h3 .forma {
    border-radius: 0;
    height: 3px;
    top: 3px;
  }
  .marca.quita {
    opacity: 0.45;
  }
  .marca.quita::before {
    background: repeating-linear-gradient(to bottom, var(--c) 0 3px, transparent 3px 6px);
  }
  .quita .forma {
    background: var(--bg-subtle);
  }
  .marca.activa::before,
  .marca.elegida::before {
    top: 4px;
    width: 3px;
    left: 4.5px;
  }
  .marca.elegida .forma {
    box-shadow: 0 0 0 2px var(--surface), 0 0 0 4px var(--accent);
  }
  .marca.activa {
    z-index: 2;
    opacity: 1;
  }
  .marca.activa .forma {
    scale: 1.25;
  }
  /* Colores de las copias: los de las gráficas en vivo (--onda-1…3, validados
   * para daltonismo en claro y en oscuro); la cuarta en adelante, tinta tenue. */
  .h0 {
    --c: var(--onda-1);
  }
  .h1 {
    --c: var(--onda-2);
  }
  .h2 {
    --c: var(--onda-3);
  }
  .h3 {
    --c: var(--text-3);
  }

  .ficha {
    position: absolute;
    top: 6px;
    z-index: 3;
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 6px 10px;
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    color: var(--text-1);
    white-space: nowrap;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-md);
    translate: 12px 0;
    pointer-events: none;
  }
  .ficha .mini {
    margin-right: 5px;
  }

  /* Franjas de la retención. */
  .franjas {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .franja {
    display: grid;
    grid-template-columns: 112px minmax(0, 1fr);
    align-items: center;
    gap: var(--sp-2);
  }
  .f-et {
    font-size: var(--fs-xs);
    color: var(--text-2);
    text-align: right;
  }
  .f-pista {
    position: relative;
    height: 10px;
    overflow: hidden;
  }
  .f-tramo {
    position: absolute;
    top: 4px;
    height: 2px;
    background: var(--border-strong);
    border-radius: 2px;
  }
  .f-punto {
    position: absolute;
    top: 2px;
    width: 6px;
    height: 6px;
    margin-left: -3px;
    background: var(--text-2);
    border-radius: 999px;
  }

  .leyenda {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 16px;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .l {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .mini {
    position: relative;
    display: inline-block;
    width: 10px;
    height: 10px;
    vertical-align: -1px;
  }
  .mini .forma {
    top: 1px;
    left: 1px;
  }
  .mini.h3 .forma {
    top: 4px;
  }
  .mini.quita .forma {
    background: transparent;
    border-color: var(--text-3);
    border-style: dashed;
    height: 8px;
    top: 1px;
    border-radius: 999px;
  }
  .regla {
    flex-basis: 100%;
  }
  @media (max-width: 640px) {
    .franja {
      grid-template-columns: 88px minmax(0, 1fr);
    }
  }
  @media (prefers-reduced-motion: no-preference) {
    .marca .forma {
      transition: scale var(--dur-fast) var(--ease);
    }
  }
</style>
