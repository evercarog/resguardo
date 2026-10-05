<script lang="ts">
  // La línea de tiempo de las versiones («máquina del tiempo», docs/diseno.md
  // §4): a todo lo ancho, con escala (día, semana, mes, año), arrastre, la
  // rueda horizontal, las flechas y una tira de toda la historia debajo para
  // moverse. Detrás, un «río» suave con cuántas versiones hay en cada momento
  // (el área de las gráficas: tinta neutra en claro, el acento con brillo en
  // oscuro) y las franjas de la retención («diarias», «semanales»…). Delante,
  // cada versión es una marca del color y la forma de su copia (tres colores
  // validados para daltonismo; las demás en tinta neutra), con un trazo tenue
  // encima tan alto como lo que añadió; las que la próxima retención quitaría,
  // huecas y tenues. Con tres copias o menos, un carril por copia. Si no caben,
  // se juntan en una burbuja con su número (pulsar acerca). Al pasar el ratón o
  // con el foco, su ficha; al pulsar (o Intro), se elige, y la ficha de la
  // elegida lleva sus acciones (`acciones`). Es un solo punto de parada
  // (listbox con aria-activedescendant); la lista de siempre sigue debajo.
  import type { Snippet } from "svelte";
  import { Tween, prefersReducedMotion } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { ChevronLeft, ChevronRight, CalendarClock } from "@lucide/svelte";
  import type { Regla } from "$lib/tipos";
  import { bytes, plural } from "$lib/formato";
  import { agrupar, franjasRetencion, huecosDeCopia, marcasEje, NOMBRE_FRANJA, NOMBRE_MOTIVO, retencionDe, rio, ZOOM, ZOOMS, zoomInicial, type VersionLinea, type Zoom } from "$lib/lineaTiempo";
  import { textoRegla } from "$lib/retencion";

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
    /** Las acciones de la versión elegida, en su ficha («Qué cambió», «Explorar y restaurar»). */
    acciones?: Snippet<[string]>;
  }
  let { versiones, copias = [], regla = null, quien = null, ahora, seleccion = null, alElegir, verbo = "Elegir", etiqueta, acciones }: Props = $props();

  const id = $props.id();
  const lista = $derived([...versiones].sort((a, b) => Date.parse(a.hora) - Date.parse(b.hora)).map((v) => ({ ...v, t: Date.parse(v.hora) })));
  type V = (typeof lista)[number];
  const huecos = $derived(huecosDeCopia(copias, lista.map((v) => v.copia)));
  const nombreCopia = (c: string | null | undefined) => copias.find((k) => k.id === c)?.nombre ?? (c ? c : "Sin copia");
  const hueco = (c: string | null | undefined) => (c ? (huecos.get(c) ?? 3) : 3);
  const leyenda = $derived.by(() => {
    const out: { hueco: 0 | 1 | 2 | 3; texto: string }[] = [];
    for (const [c, h] of huecos) if (h < 3) out.push({ hueco: h, texto: nombreCopia(c) });
    if ([...huecos.values()].includes(3) || lista.some((v) => !v.copia)) out.push({ hueco: 3, texto: "Otras" });
    return out;
  });
  // Un carril por copia si son dos o tres (con «Otras»); si no, todas en la misma base.
  const carriles = $derived<(number | null)[]>(leyenda.length >= 2 && leyenda.length <= 3 ? leyenda.map((l) => l.hueco) : [null]);
  const carrilDe = (v: V) => (carriles[0] === null ? 0 : Math.max(0, carriles.indexOf(hueco(v.copia))));
  const motivos = $derived(retencionDe(lista, regla));
  const quitaria = $derived(motivos ? [...motivos.values()].filter((m) => m === null).length : 0);
  const maxAnadido = $derived(Math.max(1, ...lista.map((v) => v.anadido ?? 0)));

  // La ventana visible: `ancho` ms terminando en `fin`. `zoom` y `fin` son el
  // destino; `vista` llega a ellos con una transición suave (sin ella con
  // «reducir movimiento», y al arrastrar, que sigue al dedo).
  let zoom = $state<Zoom>("mes");
  let fin = $state(0);
  const vista = new Tween({ fin: 0, ancho: ZOOM.mes.ms }, { duration: 0, easing: cubicOut });
  function irA(f: number, animar = true) {
    fin = f;
    void vista.set({ fin: f, ancho: ZOOM[zoom].ms }, { duration: animar && !prefersReducedMotion.current ? 280 : 0 });
  }
  let iniciado = false;
  $effect(() => {
    if (iniciado || !lista.length) return;
    iniciado = true;
    zoom = zoomInicial(lista.map((v) => v.t), ahora);
    const sel = seleccion ? lista.find((v) => v.id === seleccion) : null;
    irA(sel && sel.t < ahora - ZOOM[zoom].ms * 0.8 ? sel.t + ZOOM[zoom].ms / 2 : ahora + ZOOM[zoom].ms * 0.04, false);
  });
  const anchoMs = $derived(ZOOM[zoom].ms);
  const primera = $derived(lista[0]?.t ?? ahora);
  function limitar(f: number, a = anchoMs) {
    return Math.min(ahora + a * 0.25, Math.max(primera + a * 0.1, f));
  }
  function cambiarZoom(z: Zoom) {
    // Se mantiene el centro (o la versión activa).
    const centro = activa && activa.t >= desde && activa.t <= hasta ? activa.t : fin - anchoMs / 2;
    zoom = z;
    irA(limitar(centro + ZOOM[z].ms / 2, ZOOM[z].ms));
  }
  const mover = (fraccion: number) => irA(limitar(fin + anchoMs * fraccion));

  // Geometría (px). `w` lo mide el navegador; el alto es fijo.
  let w = $state(0);
  const ALTO = 196;
  const BASE = ALTO - 33; // la base: debajo, el eje
  const RIO = (BASE - 30) * 0.62; // lo más alto del río: arriba quedan los rótulos de las franjas y la ficha
  const SEP_CARRIL = 18;
  const A = $derived(vista.current.ancho);
  const hasta = $derived(vista.current.fin);
  const desde = $derived(hasta - A);
  const x = (t: number) => ((t - desde) / A) * w;
  const yCarril = (c: number) => BASE - (carriles.length - 1 - c) * SEP_CARRIL;

  const visibles = $derived(lista.filter((v) => v.t >= desde - A * 0.02 && v.t <= hasta + A * 0.02));
  const marcas = $derived(marcasEje(desde, hasta, zoom));

  // El río: versiones por hora, día o semana según la escala (de toda la
  // historia, con tramos fijos: no cambia al arrastrar), en un trazado suave.
  const rioZoom = $derived(rio(lista.map((v) => v.t), ZOOM[zoom].rio, primera, ahora));
  function trazado(xs: number[], ys: number[]): string {
    if (!xs.length) return "";
    let d = `M${xs[0].toFixed(1)},${ys[0].toFixed(1)}`;
    for (let i = 1; i < xs.length; i++) {
      const mx = (xs[i - 1] + xs[i]) / 2;
      d += ` C${mx.toFixed(1)},${ys[i - 1].toFixed(1)} ${mx.toFixed(1)},${ys[i].toFixed(1)} ${xs[i].toFixed(1)},${ys[i].toFixed(1)}`;
    }
    return d;
  }
  function caminoRio(r: { desde: number; paso: number; v: number[] }, de: number, a: number, fx: (t: number) => number, base: number, alto: number) {
    const i0 = Math.max(0, Math.floor((de - r.desde) / r.paso) - 2);
    // Hasta hoy, justo (después no hay versiones); si no, un poco más allá del borde.
    const corta = a >= ahora;
    const i1 = Math.min(r.v.length - 1, corta ? Math.floor((ahora - r.desde) / r.paso - 0.5) : Math.ceil((a - r.desde) / r.paso) + 2);
    const xs: number[] = [];
    const ys: number[] = [];
    for (let i = i0; i <= i1; i++) {
      xs.push(fx(r.desde + (i + 0.5) * r.paso));
      ys.push(base - 1 - r.v[i] * alto);
    }
    if (corta && ys.length) {
      xs.push(fx(ahora));
      ys.push(ys.at(-1)!);
    }
    if (xs.length < 2) return { linea: "", area: "" };
    const linea = trazado(xs, ys);
    return { linea, area: `${linea} L${xs.at(-1)!.toFixed(1)},${base} L${xs[0].toFixed(1)},${base} Z` };
  }
  const rioVista = $derived(w ? caminoRio(rioZoom, desde, Math.min(hasta, ahora), x, BASE, RIO) : { linea: "", area: "" });

  // Las franjas de la retención (de toda la historia) y las que se ven.
  const franjas = $derived(franjasRetencion(lista, motivos, ahora));

  // Las marcas a la vista, juntadas por carril cuando no caben (menos de 10 px),
  // en tramos de unos 28 px alineados al tiempo.
  const enVista = $derived(w ? visibles.map((v) => ({ v, t: v.t, x: x(v.t), c: carrilDe(v) })) : []);
  const grupos = $derived.by(() => {
    const out: { x: number; c: number; xs: typeof enVista }[] = [];
    const paso = (28 * anchoMs) / Math.max(1, w);
    for (let c = 0; c < carriles.length; c++) for (const g of agrupar(enVista.filter((m) => m.c === c), paso, 10)) out.push({ ...g, c });
    return out;
  });
  const juntas = $derived(new Set(grupos.filter((g) => g.xs.length >= 3).flatMap((g) => g.xs.map((m) => m.v.id))));
  // Solo se pintan las sueltas (y la activa y la elegida, aunque estén en una burbuja); cada opción dice su lugar en la lista.
  const posicion = $derived(new Map(lista.map((v, i) => [v.id, i + 1])));

  // La versión activa (con el teclado o el ratón) y su ficha.
  let activaId = $state<string | null>(null);
  let sobreId = $state<string | null>(null);
  let sobreGrupo = $state<{ x: number; y: number; n: number; de: number; a: number } | null>(null);
  const activa = $derived(lista.find((v) => v.id === (sobreId ?? activaId ?? seleccion)) ?? null);
  const elegida = $derived(seleccion ? (lista.find((v) => v.id === seleccion) ?? null) : null);
  const pintadas = $derived.by(() => {
    const out = enVista.filter((m) => !juntas.has(m.v.id) || m.v.id === activa?.id || m.v.id === seleccion);
    // La activa del teclado existe siempre (aria-activedescendant), aunque la ventana aún esté llegando a ella.
    const a = activaId && !out.some((m) => m.v.id === activaId) ? lista.find((v) => v.id === activaId) : null;
    if (a && w) out.push({ v: a, t: a.t, x: x(a.t), c: carrilDe(a) });
    return out;
  });
  const fmtLargo = new Intl.DateTimeFormat("es", { weekday: "long", day: "numeric", month: "long", year: "numeric", hour: "2-digit", minute: "2-digit" });
  const fmtFicha = new Intl.DateTimeFormat("es", { weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
  const fmtDia = new Intl.DateTimeFormat("es", { day: "numeric", month: "short" });
  function motivoTexto(vid: string): string | null {
    if (!motivos) return null;
    const m = motivos.get(vid);
    return m ? `Se queda (${NOMBRE_MOTIVO[m]})` : "La próxima retención la quitaría";
  }
  // Los textos de cada opción, una vez (no en cada fotograma al arrastrar).
  const textos = $derived(new Map(lista.map((v) => [v.id, describir(v)])));
  function describir(v: V): string {
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
  function activar(v: V | undefined) {
    if (!v) return;
    activaId = v.id;
    if (v.t < fin - anchoMs * 0.95 || v.t > fin - anchoMs * 0.05) irA(limitar(v.t + anchoMs / 2));
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

  // Arrastrar para moverse (y la rueda horizontal). Un clic sin arrastre
  // elige; en una burbuja, acerca la escala.
  let pista = $state<HTMLDivElement | null>(null);
  let arrastre: { x: number; fin: number; movido: boolean } | null = null;
  function bajar(ev: PointerEvent) {
    if (ev.button !== 0) return;
    arrastre = { x: ev.clientX, fin, movido: false };
    pista?.setPointerCapture(ev.pointerId);
  }
  function moverPuntero(ev: PointerEvent) {
    if (!arrastre || !w) return;
    const dx = ev.clientX - arrastre.x;
    if (Math.abs(dx) > 3) arrastre.movido = true;
    if (arrastre.movido) irA(limitar(arrastre.fin - (dx / w) * anchoMs), false);
  }
  function subir(ev: PointerEvent) {
    const a = arrastre;
    arrastre = null;
    if (!a || a.movido) return;
    // Con la captura del puntero, el destino es la pista: se busca lo que hay debajo.
    const bajo = document.elementFromPoint(ev.clientX, ev.clientY) as HTMLElement | null;
    const el = bajo?.closest<HTMLElement>("[data-v], [data-g]");
    if (el?.dataset.v) {
      activaId = el.dataset.v;
      alElegir(el.dataset.v);
    } else if (el?.dataset.g) {
      const t = Number(el.dataset.g);
      const j = ZOOMS.indexOf(zoom) - 1;
      if (ZOOMS[j]) {
        zoom = ZOOMS[j];
        irA(limitar(t + ZOOM[zoom].ms / 2));
      }
      sobreGrupo = null;
    }
  }
  function rueda(ev: WheelEvent) {
    if (!w || Math.abs(ev.deltaX) <= Math.abs(ev.deltaY)) return;
    ev.preventDefault();
    irA(limitar(fin + (ev.deltaX / w) * anchoMs), false);
  }

  // La tira de toda la historia (debajo): el río entero y la ventana, que se arrastra.
  const RES = 30;
  const totDesde = $derived(Math.min(primera, ahora - anchoMs) - (ahora - primera) * 0.02);
  const totHasta = $derived(ahora + anchoMs * 0.25);
  const xr = (t: number) => ((t - totDesde) / (totHasta - totDesde)) * w;
  const rioTodo = $derived(rio(lista.map((v) => v.t), Math.max(3_600_000, (totHasta - totDesde) / 120), totDesde, totHasta));
  const rioResumen = $derived(w ? caminoRio(rioTodo, totDesde, ahora, xr, RES - 2, RES - 8) : { linea: "", area: "" });
  let tira: { x: number; fin: number } | null = null;
  let resumenEl = $state<HTMLDivElement | null>(null);
  function bajarTira(ev: PointerEvent) {
    if (ev.button !== 0 || !resumenEl) return;
    const r = resumenEl.getBoundingClientRect();
    const px = ev.clientX - r.left;
    const dentro = px >= xr(fin - anchoMs) - 4 && px <= xr(fin) + 4;
    resumenEl.setPointerCapture(ev.pointerId);
    if (!dentro) {
      const t = totDesde + (px / w) * (totHasta - totDesde);
      irA(limitar(t + anchoMs / 2));
    }
    tira = { x: ev.clientX, fin };
  }
  function moverTira(ev: PointerEvent) {
    if (!tira || !w) return;
    irA(limitar(tira.fin + ((ev.clientX - tira.x) / w) * (totHasta - totDesde)), false);
  }
</script>

{#snippet forma(h: number, quita = false)}<span class="mini h{h}" class:quita><span class="forma"></span></span>{/snippet}

{#if lista.length}
  <div class="linea">
    <div class="herr">
      <div class="segmented inline" role="group" aria-label="Escala">
        {#each ZOOMS as z (z)}
          <button type="button" class:on={zoom === z} aria-pressed={zoom === z} onclick={() => cambiarZoom(z)}>{ZOOM[z].texto}</button>
        {/each}
      </div>
      <div class="nav">
        <button type="button" class="icon-btn" aria-label="Antes" onclick={() => mover(-0.6)}><ChevronLeft size={16} /></button>
        <button type="button" class="btn btn-sm btn-ghost" onclick={() => irA(limitar(ahora + anchoMs * 0.04))}><CalendarClock size={14} />Hoy</button>
        <button type="button" class="icon-btn" aria-label="Después" onclick={() => mover(0.6)}><ChevronRight size={16} /></button>
      </div>
    </div>

    <div class="lienzo" bind:clientWidth={w} style:--alto="{ALTO}px">
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
        {#if w}
          <svg class="fondo" width={w} height={ALTO} viewBox="0 0 {w} {ALTO}" aria-hidden="true">
            <defs>
              <linearGradient id="{id}-rio" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0" class="rio-alto" />
                <stop offset="1" class="rio-bajo" />
              </linearGradient>
            </defs>
            <!-- Franjas de la retención: tintas suaves alternas, con su rótulo arriba. -->
            {#each franjas as f, i (f.p)}
              {@const a = Math.max(-2, x(f.desde))}
              {@const b = Math.min(w + 2, x(f.hasta))}
              {#if b > 0 && a < w}
                <rect class="franja" class:par={i % 2 === 0} x={a} y="0" width={Math.max(0, b - a)} height={BASE} />
                {#if x(f.desde) > 0}<line class="franja-borde" x1={Math.round(a) + 0.5} x2={Math.round(a) + 0.5} y1="0" y2={BASE} />{/if}
              {/if}
            {/each}
            <!-- Rejilla fina y discontinua, una por marca del eje. -->
            {#each marcas as m (m.t)}
              <line class="rejilla" class:fuerte={m.fuerte} x1={Math.round(x(m.t)) + 0.5} x2={Math.round(x(m.t)) + 0.5} y1="22" y2={BASE} />
            {/each}
            <!-- El río. -->
            {#if rioVista.area}
              <path class="rio-area" d={rioVista.area} fill="url(#{id}-rio)" />
              <path class="rio-linea" d={rioVista.linea} />
            {/if}
            <!-- Los carriles (uno por copia) y la base. -->
            {#each carriles as _, c (c)}
              <line class={c === carriles.length - 1 ? "base" : "carril"} x1="0" x2={w} y1={yCarril(c) + 0.5} y2={yCarril(c) + 0.5} />
            {/each}
            <!-- Lo que añadió cada versión: un trazo tenue encima, más alto cuanto más. -->
            {#each pintadas as m (m.v.id)}
              {#if m.v.anadido && !juntas.has(m.v.id)}
                <!-- Con carriles, encima del de arriba (no cruza los demás). -->
                {@const y0 = yCarril(0) - 7}
                <line class="cue h{hueco(m.v.copia)}" class:quita={motivos && motivos.get(m.v.id) === null} x1={m.x} x2={m.x} y1={y0} y2={y0 - 2 - Math.sqrt(m.v.anadido / maxAnadido) * 34} />
              {/if}
            {/each}
            <!-- Guías: la de la elegida (acento) y la de la activa. -->
            {#if elegida && x(elegida.t) >= 0 && x(elegida.t) <= w}
              <line class="guia-elegida" x1={x(elegida.t)} x2={x(elegida.t)} y1="0" y2={BASE} />
            {/if}
            {#if activa && activa.id !== seleccion && x(activa.t) >= 0 && x(activa.t) <= w}
              <line class="guia" x1={x(activa.t)} x2={x(activa.t)} y1="0" y2={BASE} />
            {/if}
            <!-- Hoy: una línea del acento que brilla. -->
            {#if x(ahora) >= 0 && x(ahora) <= w}
              <line class="hoy" x1={x(ahora)} x2={x(ahora)} y1="0" y2={BASE} />
            {/if}
          </svg>

          <div class="eje" aria-hidden="true">
            {#each marcas as m (m.t)}
              {@const mx = x(m.t)}
              {#if mx > 2 && mx < w - 24 && (x(ahora) < 0 || x(ahora) > w || Math.abs(mx - Math.min(w - 22, Math.max(22, x(ahora)))) > 46)}
                <span class="m-txt" class:fuerte={m.fuerte} style:transform="translateX({mx}px)">{m.texto}</span>
              {/if}
            {/each}
            {#if x(ahora) >= 0 && x(ahora) <= w}<span class="hoy-txt" style:transform="translateX({Math.min(w - 22, Math.max(22, x(ahora)))}px)">Hoy</span>{/if}
          </div>
          <div class="rotulos" aria-hidden="true">
            {#each franjas as f (f.p)}
              {@const a = Math.max(0, x(f.desde))}
              {@const b = Math.min(w, x(f.hasta))}
              {#if b - a > 56}
                <span class="f-txt" style:transform="translateX({a + 8}px)" style:max-width="{b - a - 14}px">{NOMBRE_FRANJA[f.p]} · {f.n}</span>
              {/if}
            {/each}
          </div>

          {#each grupos as g (g.xs[0].v.id)}
            {#if g.xs.length >= 3}
              {@const hs = new Set(g.xs.map((m) => hueco(m.v.copia)))}
              <span
                class="burbuja h{hs.size === 1 ? [...hs][0] : 3}"
                aria-hidden="true"
                data-g={Math.round(g.xs.reduce((s, m) => s + m.v.t, 0) / g.xs.length)}
                style:transform="translate({g.x}px, {yCarril(g.c)}px)"
                onpointerenter={() => (sobreGrupo = { x: g.x, y: yCarril(g.c), n: g.xs.length, de: g.xs[0].v.t, a: g.xs.at(-1)!.v.t })}
                onpointerleave={() => (sobreGrupo = null)}>{g.xs.length}</span
              >
            {/if}
          {/each}
        {/if}

        {#each pintadas as m (m.v.id)}
          {@const motivo = motivos?.get(m.v.id)}
          <div
            id="{id}-{m.v.id}"
            class="marca h{hueco(m.v.copia)}"
            class:quita={motivos && motivo === null}
            class:elegida={m.v.id === seleccion}
            class:activa={m.v.id === activa?.id}
            role="option"
            tabindex="-1"
            aria-selected={m.v.id === seleccion}
            aria-setsize={lista.length}
            aria-posinset={posicion.get(m.v.id)}
            aria-label={textos.get(m.v.id)}
            data-v={m.v.id}
            style:transform="translate({m.x}px, {yCarril(m.c)}px)"
            onpointerenter={() => (sobreId = m.v.id)}
            onpointerleave={() => (sobreId = null)}
          >
            <span class="forma"></span>
          </div>
        {/each}
      </div>

      <!-- La ficha: la versión activa (o la elegida, con sus acciones). Fuera del listbox. -->
      {#if w && sobreGrupo}
        <div class="graf-tip ficha" class:izq={sobreGrupo.x > w - 220} style:left="{sobreGrupo.x}px" aria-hidden="true">
          <strong>{plural(sobreGrupo.n, "versión", "versiones")}</strong>
          <span>{fmtDia.format(sobreGrupo.de)}{fmtDia.format(sobreGrupo.de) !== fmtDia.format(sobreGrupo.a) ? ` – ${fmtDia.format(sobreGrupo.a)}` : ""} · pulsa para acercar</span>
        </div>
      {:else if w && activa && x(activa.t) >= -1 && x(activa.t) <= w + 1}
        {@const m = motivoTexto(activa.id)}
        {@const conAcciones = !!acciones && activa.id === seleccion && !sobreId}
        <div class="graf-tip ficha" class:izq={x(activa.t) > w - 250} class:con-acciones={conAcciones} style:left="{x(activa.t)}px">
          <div class="ficha-datos" aria-hidden="true">
            <strong class="num">{fmtFicha.format(activa.t)}</strong>
            <span class="ficha-copia">{@render forma(hueco(activa.copia), !!motivos && motivos.get(activa.id) === null)}{nombreCopia(activa.copia)}</span>
            {#if activa.bytes != null || activa.anadido != null}
              <span class="num"
                >{#if activa.bytes != null}<b>{bytes(activa.bytes)}</b>{/if}{#if activa.bytes != null && activa.anadido != null}{" · "}{/if}{#if activa.anadido != null}+{bytes(activa.anadido)} nuevos{/if}</span
              >
            {/if}
            {#if m}<span>{m}</span>{/if}
          </div>
          {#if conAcciones}<div class="ficha-acc">{@render acciones!(activa.id)}</div>{/if}
        </div>
      {/if}
      {#if w && elegida && (x(elegida.t) < 0 || x(elegida.t) > w)}
        <button type="button" class="volver" class:der={x(elegida.t) > w} onclick={() => irA(limitar(elegida.t + anchoMs / 2))}>
          {#if x(elegida.t) < 0}<ChevronLeft size={13} />{/if}Elegida · {fmtDia.format(elegida.t)}{#if x(elegida.t) > w}<ChevronRight size={13} />{/if}
        </button>
      {/if}
    </div>

    <!-- Toda la historia: el río entero y la ventana que se ve (se arrastra). Para el teclado, Re Pág y Av Pág. -->
    <div
      class="resumen"
      bind:this={resumenEl}
      aria-hidden="true"
      onpointerdown={bajarTira}
      onpointermove={moverTira}
      onpointerup={() => (tira = null)}
      onpointercancel={() => (tira = null)}
    >
      {#if w}
        {@const a = xr(desde)}
        {@const b = xr(hasta)}
        <svg width={w} height={RES} viewBox="0 0 {w} {RES}">
          {#if rioResumen.area}<path class="res-area" d={rioResumen.area} /><path class="res-linea" d={rioResumen.linea} />{/if}
          <rect class="res-fuera" x="0" y="0" width={Math.max(0, a)} height={RES} />
          <rect class="res-fuera" x={Math.min(w, b)} y="0" width={Math.max(0, w - b)} height={RES} />
          <rect class="ventana" x={Math.max(0.5, a)} y="0.5" width={Math.max(6, Math.min(w - 0.5, b) - Math.max(0.5, a))} height={RES - 1} rx="5" />
          <line class="hoy" x1={xr(ahora)} x2={xr(ahora)} y1="3" y2={RES - 3} />
        </svg>
      {/if}
    </div>

    <!-- Fuera del listbox: lo que tiene el foco, leído. -->
    <span class="sr-only" aria-live="polite" aria-atomic="true">{activaId && lista.find((v) => v.id === activaId) ? describir(lista.find((v) => v.id === activaId)!) : ""}</span>

    <div class="leyenda">
      {#if leyenda.length > 1 || leyenda[0]?.hueco !== 3}
        {#each leyenda as l (l.hueco + l.texto)}<span class="l">{@render forma(l.hueco)}{l.texto}</span>{/each}
      {/if}
      <span class="l"><span class="sw-rio"></span>Versiones por {ZOOM[zoom].por}</span>
      {#if regla}
        <span class="l">{@render forma(3, true)}La próxima retención la quitaría · {quitaria}</span>
        {#if franjas.length}<span class="l"><span class="sw-franja"></span>Lo que guarda cada regla</span>{/if}
        <span class="regla">Simulado con la retención {quien ? `que aplica ${quien}` : "del repositorio"}: {textoRegla(regla)}.</span>
      {/if}
    </div>
  </div>
{/if}

<style>
  .linea {
    /* El lienzo: claro sobre la tarjeta (las marcas de color llegan a 3:1
     * sobre blanco) y, en oscuro, hundido y con brillo, como un gráfico en vivo. */
    --lt-fondo: var(--surface);
    --lt-franja: color-mix(in srgb, var(--text-3) 7%, transparent);
    --lt-rio-linea: color-mix(in srgb, var(--text-3) 55%, transparent);
    --lt-rio-alfa: calc(var(--graf-area-alfa) * 1.2);
    --lt-halo: calc(var(--graf-brillo-px) * 0.7px);
    --lt-cue: 0.32;
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
  }
  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-theme="light"])) .linea {
      --lt-fondo: var(--bg-subtle);
      --lt-franja: color-mix(in srgb, var(--text-3) 6%, transparent);
      --lt-rio-linea: var(--accent);
      --lt-cue: 0.5;
    }
  }
  :global(:root[data-theme="dark"]) .linea,
  :global(:root[data-theme="black"]) .linea {
    --lt-fondo: var(--bg-subtle);
    --lt-franja: color-mix(in srgb, var(--text-3) 6%, transparent);
    --lt-rio-linea: var(--accent);
    --lt-cue: 0.5;
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

  /* El lienzo: el fondo (SVG), las marcas encima y la ficha fuera de la pista. */
  .lienzo {
    position: relative;
    min-width: 0;
  }
  .pista {
    position: relative;
    height: var(--alto);
    overflow: hidden;
    background: var(--lt-fondo);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
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
  .fondo {
    position: absolute;
    inset: 0;
    display: block;
    pointer-events: none;
  }
  .franja {
    fill: transparent;
  }
  .franja.par {
    fill: var(--lt-franja);
  }
  .franja-borde {
    stroke: var(--graf-base);
    stroke-width: 1;
  }
  .rejilla {
    stroke: var(--graf-rejilla);
    stroke-width: 1;
    stroke-dasharray: 2 3;
  }
  .rejilla.fuerte {
    stroke: var(--graf-base);
  }
  .rio-alto {
    stop-color: var(--graf-area);
    stop-opacity: var(--lt-rio-alfa);
  }
  .rio-bajo {
    stop-color: var(--graf-area);
    stop-opacity: 0.02;
  }
  .rio-linea {
    fill: none;
    stroke: var(--lt-rio-linea);
    stroke-width: 1.5;
    stroke-linejoin: round;
    filter: var(--graf-brillo);
  }
  .base {
    stroke: var(--graf-base);
    stroke-width: 1;
  }
  .carril {
    stroke: var(--graf-rejilla);
    stroke-width: 1;
  }
  .cue {
    stroke: var(--c);
    stroke-width: 2;
    stroke-linecap: round;
    opacity: var(--lt-cue);
  }
  .cue.quita {
    opacity: calc(var(--lt-cue) / 2);
  }
  .guia {
    stroke: var(--graf-guia);
    stroke-width: 1;
    stroke-dasharray: 3 3;
  }
  .guia-elegida {
    stroke: var(--accent);
    stroke-width: 1.5;
    opacity: 0.7;
  }
  .hoy {
    stroke: var(--accent);
    stroke-width: 1.5;
    filter: drop-shadow(0 0 var(--lt-halo) color-mix(in srgb, var(--accent) 70%, transparent));
  }

  /* El eje: rótulos apagados y tabulares; «Hoy» en una píldora del acento. */
  .eje,
  .rotulos {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .m-txt,
  .hoy-txt,
  .f-txt {
    position: absolute;
    left: 0;
    font-size: 10.5px;
    line-height: 14px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .m-txt {
    bottom: 6px;
    color: var(--graf-eje);
    translate: -50% 0;
  }
  .m-txt.fuerte {
    color: var(--text-2);
    font-weight: 500;
  }
  .hoy-txt {
    bottom: 4px;
    padding: 1px 7px;
    font-weight: 600;
    color: var(--accent-text);
    background: var(--accent-soft);
    border-radius: 999px;
    translate: -50% 0;
  }
  .f-txt {
    top: 6px;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  /* Cada versión: una marca pequeña con la forma de su copia, sobre su
   * carril. El objetivo es mayor que la marca (18 × 24). */
  .marca {
    position: absolute;
    top: 0;
    left: 0;
    width: 18px;
    height: 24px;
    margin: -12px 0 0 -9px;
    cursor: pointer;
  }
  .forma {
    position: absolute;
    top: 7px;
    left: 4.5px;
    width: 9px;
    height: 9px;
    box-sizing: border-box;
    background: var(--c);
    border: 1.5px solid var(--c);
    border-radius: 999px;
    /* Un aro del fondo separa las que se tocan; en oscuro, un halo de su color. */
    box-shadow:
      0 0 0 1.5px var(--lt-fondo),
      0 0 var(--lt-halo) color-mix(in srgb, var(--c) 55%, transparent);
  }
  .h1 .forma {
    border-radius: 2px;
  }
  .h2 .forma {
    border-radius: 2px;
    rotate: 45deg;
    scale: 0.92;
  }
  .h3 .forma {
    top: 9.5px;
    left: 3.5px;
    width: 11px;
    height: 4px;
    border-radius: 999px;
  }
  .marca.quita:not(.h3) .forma {
    top: 8.5px;
    left: 6px;
    width: 7px;
    height: 7px;
  }
  .marca.quita .forma {
    background: var(--lt-fondo);
    opacity: 0.6;
    box-shadow: 0 0 0 1.5px var(--lt-fondo);
  }
  .marca:hover .forma,
  .marca.activa .forma {
    scale: 1.45;
  }
  .h2.marca:hover .forma,
  .h2.marca.activa .forma {
    scale: 1.3;
  }
  .marca.activa,
  .marca.elegida {
    z-index: 2;
  }
  .marca.elegida .forma {
    opacity: 1;
    box-shadow:
      0 0 0 2px var(--lt-fondo),
      0 0 0 3.5px var(--accent),
      0 0 calc(var(--lt-halo) * 1.5) color-mix(in srgb, var(--accent) 60%, transparent);
  }

  /* Varias juntas: una burbuja con su número (pulsar acerca la escala). */
  .burbuja {
    position: absolute;
    top: 0;
    left: 0;
    z-index: 1;
    min-width: 20px;
    height: 18px;
    padding: 0 5px;
    margin: -9px 0 0 -10px;
    box-sizing: border-box;
    font-size: 10.5px;
    font-weight: 600;
    line-height: 16px;
    font-variant-numeric: tabular-nums;
    text-align: center;
    color: var(--text-1);
    background: color-mix(in srgb, var(--c) 16%, var(--lt-fondo));
    border: 1px solid color-mix(in srgb, var(--c) 60%, transparent);
    border-radius: 999px;
    box-shadow: 0 0 var(--lt-halo) color-mix(in srgb, var(--c) 45%, transparent);
    cursor: zoom-in;
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

  /* La ficha (el globo de las gráficas, .graf-tip): arriba, junto a la guía. */
  .ficha {
    top: 8px;
    z-index: 4;
    gap: 6px;
    min-width: 168px;
    margin-left: 12px;
  }
  .ficha.izq {
    margin-left: -12px;
    translate: -100% 0;
  }
  .ficha.con-acciones {
    pointer-events: auto;
  }
  .ficha-datos {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .ficha-copia {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
  }
  .ficha-acc {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding-top: 6px;
    border-top: 1px solid var(--border);
  }
  .volver {
    position: absolute;
    top: 8px;
    left: 8px;
    z-index: 3;
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: 24px;
    padding: 0 8px;
    font: inherit;
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--accent-text);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    box-shadow: var(--shadow-sm);
    cursor: pointer;
  }
  .volver.der {
    right: 8px;
    left: auto;
  }
  .volver:hover {
    background: var(--surface-2);
  }

  /* La tira de toda la historia, con la ventana que se ve. */
  .resumen {
    height: 30px;
    margin-top: calc(var(--sp-1) * -1);
    overflow: hidden;
    background: var(--lt-fondo);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    touch-action: pan-y;
    user-select: none;
  }
  .resumen svg {
    display: block;
  }
  .res-area {
    fill: var(--graf-area);
    opacity: calc(var(--graf-area-alfa) * 1.4);
  }
  .res-linea {
    fill: none;
    stroke: var(--lt-rio-linea);
    stroke-width: 1;
  }
  .res-fuera {
    fill: color-mix(in srgb, var(--text-3) 10%, transparent);
  }
  .ventana {
    fill: color-mix(in srgb, var(--accent) 8%, transparent);
    stroke: color-mix(in srgb, var(--accent) 65%, transparent);
    stroke-width: 1;
    cursor: grab;
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
    flex: none;
    width: 12px;
    height: 12px;
  }
  .mini .forma {
    top: 1.5px;
    left: 1.5px;
    box-shadow: none;
  }
  .mini.h3 .forma {
    top: 4px;
    left: 0.5px;
  }
  .mini.quita .forma {
    top: 1.5px;
    left: 1.5px;
    width: 9px;
    height: 9px;
    background: transparent;
    border-color: var(--text-3);
    border-radius: 999px;
    opacity: 0.8;
  }
  .sw-rio {
    width: 16px;
    height: 10px;
    background: linear-gradient(to bottom, color-mix(in srgb, var(--graf-area) 45%, transparent), transparent);
    border-top: 1.5px solid var(--lt-rio-linea);
    border-radius: 2px 2px 0 0;
  }
  .sw-franja {
    width: 14px;
    height: 10px;
    background: color-mix(in srgb, var(--text-3) 14%, transparent);
    border-left: 1px solid var(--graf-base);
  }
  .regla {
    flex-basis: 100%;
    color: var(--text-3);
  }
  @media (prefers-reduced-motion: no-preference) {
    .marca .forma {
      transition:
        scale var(--dur-fast) var(--ease),
        box-shadow var(--dur-fast) var(--ease);
    }
  }
</style>
