<script lang="ts">
  // «Mapa de la protección» (docs/diseno.md §4): de izquierda a derecha, los
  // equipos, sus repositorios (píldoras), el almacén o destino y lo que sale
  // de ahí (espejo, copia externa). Lienzo con rejilla de puntos, tarjetas de
  // verdad (enlaces: Tab, Intro y flechas) y, detrás, los trazos en curva y
  // discontinuos: en tinta tenue si todo va bien, del color del estado (con
  // su icono y texto en la tarjeta y en el rótulo) si algo falla, y moviéndose
  // mientras algo está en marcha (quietos con movimiento reducido). En
  // estrecho (o con «Ver como lista»), un árbol en vertical con lo mismo.
  //
  // Dónde va cada tarjeta y por dónde pasa cada trazo lo calcula
  // lib/mapaGeometria.ts (por capas, sin pasar nunca por detrás de una
  // tarjeta). Aquí se mide el alto de cada tarjeta y se mueve el plano:
  // arrastrar, Ctrl/⌘ + rueda o pellizcar, doble clic, + − 0 y las flechas,
  // los botones de abajo y pantalla completa. Lo que se acerca y se mueve se
  // recuerda en esta pestaña, por mapa.
  import { tick, untrack, type Snippet } from "svelte";
  import { ChevronDown, ChevronRight, ChevronsUpDown, CircleAlert, CircleCheck, CircleDashed, CirclePause, Cloud, Database, HardDrive, Layers, Link2, List, LoaderCircle, Maximize, Minimize, Monitor, Scan, Server, TriangleAlert, Waypoints, ZoomIn, ZoomOut } from "@lucide/svelte";
  import type { Equipo, Informe } from "$lib/tipos";
  import { construirMapa, raices, type IconoNodo, type Mapa, type NodoMapa, type Perspectiva } from "$lib/mapa";
  import { disponer } from "$lib/mapaGeometria";
  import MarcaCliente from "../MarcaCliente.svelte";
  import TipoDestino from "../TipoDestino.svelte";
  import { catalogoDe } from "$lib/catalogoDestinos.svelte";
  import { pctVisible, tareasDe } from "$lib/progreso.svelte";
  import { fechaLarga, relativo } from "$lib/formato";
  import { tip } from "$lib/tooltip";
  import type { Tono } from "$lib/salud";

  interface Props {
    equipos: Equipo[];
    /** Todos los del cliente (si `equipos` viene filtrado), para encontrar los almacenes. */
    todos?: Equipo[];
    informes: Record<string, Informe | null | undefined>;
    cliente: string;
    ahora: number;
    /** En la página de un equipo: solo lo suyo, sin pestañas ni selector. */
    equipo?: string;
    titulo?: string;
    /** Un mapa ya hecho (el de todos los clientes, lib/global.ts): sin pestañas ni selector; van `herramientas`. */
    dado?: Mapa;
    herramientas?: Snippet;
    /** Plegar o desplegar un cliente (tarjetas `cliente`). */
    alPlegar?: (cliente: string) => void;
    /** Por debajo de este ancho, en lista. */
    listaDesde?: number;
    /** Lo que se dice si no hay nada que dibujar. */
    vacio?: string;
    /**
     * v1.56: «Conectar también…» en una tarjeta de un paso que depende de un equipo que no
     * está en esta consola (p. ej. el almacén, gestionado desde otra). Sin ella, solo el aviso.
     */
    alConectarFuera?: (n: NodoMapa) => void;
  }
  let {
    equipos,
    todos,
    informes,
    cliente,
    ahora,
    equipo,
    titulo = "Mapa de la protección",
    dado,
    herramientas,
    alPlegar,
    listaDesde = 640,
    vacio = "Todavía no hay copias que dibujar: cuando un equipo tenga un repositorio, aparecerá aquí con su camino.",
    alConectarFuera,
  }: Props = $props();

  // La perspectiva y la raíz elegidas se recuerdan por cliente (en este navegador).
  const CLAVE = $derived(`resguardo.mapa.${cliente}`);
  let perspectiva = $state<Perspectiva>("equipos");
  let raiz = $state("");
  /** «auto»: en lista si es estrecho; si no, lo que la persona eligió. */
  let modo = $state<"auto" | "mapa" | "lista">("auto");
  $effect(() => {
    if (equipo) return;
    try {
      const x = JSON.parse(localStorage.getItem(CLAVE) ?? "null") as { p?: Perspectiva; r?: string } | null;
      if (x?.p) perspectiva = x.p;
      raiz = x?.r ?? "";
    } catch {
      /* sin almacenamiento: lo de siempre */
    }
  });
  function recordar() {
    try {
      localStorage.setItem(CLAVE, JSON.stringify({ p: perspectiva, r: raiz }));
    } catch {
      /* sin almacenamiento */
    }
  }
  const opciones = $derived(equipo || dado ? [] : raices(equipos, perspectiva, todos));
  // Una raíz que ya no existe (otro cliente, un equipo que se fue): todos.
  const raizValida = $derived(opciones.some((o) => o.id === raiz) ? raiz : "");

  function enVivo(e: string, r: string, tipo: "copia" | "copia_externa"): string | null {
    const t = tareasDe(e, { repo: r, tipos: [tipo] })[0];
    if (!t) return null;
    const p = pctVisible(e, t);
    return `${tipo === "copia" ? "Copiando" : "Subiendo"}${p != null ? ` ${p} %` : "…"}`;
  }
  const mapa = $derived(
    dado ??
    construirMapa(equipo ? (todos ?? equipos) : equipos, informes, { cliente, ahora, enVivo, todos, catalogo: catalogoDe(cliente), raiz: equipo ? { perspectiva: "equipos", id: equipo } : { perspectiva, id: raizValida } }),
  );
  const porId = $derived(new Map(mapa.nodos.map((n) => [n.id, n])));
  const aristaPorId = $derived(new Map(mapa.aristas.map((a) => [a.id, a])));

  // --- Geometría: el ancho de cada columna sale del sitio que hay; el alto de cada tarjeta, de medirla.
  /** Aire alrededor del dibujo, dentro del plano. */
  const MARGEN = 28;
  let lienzo = $state<HTMLDivElement | null>(null);
  let plano = $state<HTMLDivElement | null>(null);
  let ancho = $state(0);
  let altoCompleto = $state(0);
  let altoVentana = $state(900);
  let altos = $state<Record<string, number>>({});
  let listo = $state(false);
  const estrecho = $derived(ancho > 0 && ancho < listaDesde);
  const enLista = $derived(modo === "lista" || (modo === "auto" && estrecho));
  const cols = $derived([...new Set(mapa.nodos.map((n) => n.col))].sort((a, b) => a - b));
  const tipoDeCol = (c: number) => mapa.nodos.find((n) => n.col === c)?.tipo;
  const sepCol = $derived(cols.length >= 5 ? 44 : 60);
  const anchos = $derived.by(() => {
    const natural = cols.map((c) => (tipoDeCol(c) === "cliente" ? 200 : tipoDeCol(c) === "repo" ? 224 : 204));
    const suma = natural.reduce((s, x) => s + x, 0);
    const libre = ancho - 2 * MARGEN - sepCol * (cols.length - 1);
    // Llena el ancho (hasta un tope); en poco sitio, algo más estrechas y el resto lo hace la escala.
    const f = ancho && suma ? Math.min(1.45, Math.max(0.85, libre / suma)) : 1;
    return Object.fromEntries(cols.map((c, i) => [c, Math.round(natural[i] * f)])) as Record<number, number>;
  });
  const disp = $derived(
    disponer({
      nodos: mapa.nodos.map((n) => ({ id: n.id, col: n.col })),
      aristas: mapa.aristas.map((a) => ({ id: a.id, de: a.de, a: a.a, marca: a.tono === "bad" || a.tono === "warn" })),
      altos,
      anchos,
      altoPorDefecto: 64,
      separacionCol: sepCol,
      separacionFila: Object.fromEntries(cols.map((c) => [c, tipoDeCol(c) === "repo" ? 10 : 14])),
    }),
  );
  const anchoPlano = $derived(disp.ancho + 2 * MARGEN);
  const altoPlano = $derived(disp.alto + 2 * MARGEN);
  const conSalida = $derived(new Set(mapa.aristas.map((a) => a.de)));

  function medir() {
    if (!plano) return;
    const nuevo: Record<string, number> = {};
    for (const el of plano.querySelectorAll<HTMLElement>("[data-caja]")) {
      nuevo[el.dataset.caja!] = el.offsetHeight;
      vigiaCajas?.observe(el);
    }
    const viejo = untrack(() => altos);
    const ks = Object.keys(nuevo);
    if (ks.length !== Object.keys(viejo).length || ks.some((k) => viejo[k] !== nuevo[k])) altos = nuevo;
    listo = true;
  }
  // Una tarjeta que cambia de alto (algo en marcha, la letra que acaba de cargar): se vuelve a medir.
  let vigiaCajas: ResizeObserver | null = null;
  $effect(() => {
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => medir());
    vigiaCajas = ro;
    return () => {
      ro.disconnect();
      vigiaCajas = null;
    };
  });
  $effect(() => {
    if (!lienzo) return;
    const ro = new ResizeObserver(() => {
      ancho = lienzo?.clientWidth ?? 0;
      altoCompleto = lienzo?.clientHeight ?? 0;
    });
    ro.observe(lienzo);
    return () => ro.disconnect();
  });
  $effect(() => {
    void mapa;
    void anchos;
    void enLista;
    void tick().then(medir);
  });

  // --- Acercar y mover: el plano se transforma (translate + scale) dentro del lienzo, que no se mueve.
  interface Vista {
    s: number;
    x: number;
    y: number;
  }
  const MIN = 0.3;
  const MAX = 2.5;
  let vista = $state<Vista>({ s: 1, x: 0, y: 0 });
  /** La persona ya acercó o movió el mapa (o venía de antes): no se reajusta solo. */
  let tocado = $state(false);
  let animar = $state(false);
  let pantallaCompleta = $state(false);
  /** Pantalla completa sin la API del navegador (p. ej. Safari en el iPhone): el mapa ocupa la ventana. */
  let ampliado = $state(false);
  let seccion = $state<HTMLElement | null>(null);
  const limitarA = (v: number, a: number, b: number) => Math.min(b, Math.max(a, v));
  const escalaAncho = $derived(ancho ? Math.min(1, ancho / anchoPlano) : 1);
  /** Sitio de más abajo para los botones (que no tapen la última tarjeta al abrir). */
  const reserva = $derived(Math.round(52 - MARGEN * escalaAncho));
  // Fuera de pantalla completa, el lienzo mide lo que el dibujo a lo ancho (con un mínimo y un tope).
  const altoLienzo = $derived(Math.round(limitarA(altoPlano * escalaAncho + reserva, equipo ? 170 : 220, Math.max(2400, altoVentana))));
  const altoVisible = $derived(pantallaCompleta ? altoCompleto || altoVentana : altoLienzo);
  const sinMovimiento = () => typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;

  /** Todo el dibujo a la vista (sin pasar del tamaño real), centrado. */
  function ajuste(): Vista {
    const s = limitarA(Math.min(1, ancho / anchoPlano, (altoVisible - reserva) / altoPlano), MIN, 1);
    return { s, x: (ancho - anchoPlano * s) / 2, y: Math.max(0, (altoVisible - reserva - altoPlano * s) / 2) };
  }
  /** Al abrir: a lo ancho y desde arriba (casi siempre, lo mismo que ajustar). */
  function inicial(): Vista {
    const s = escalaAncho;
    return { s, x: (ancho - anchoPlano * s) / 2, y: Math.max(0, (altoVisible - reserva - altoPlano * s) / 2) };
  }
  /** Que siempre quede algo del dibujo a la vista. */
  function limitar(v: Vista): Vista {
    const m = 64;
    const w = anchoPlano * v.s;
    const h = altoPlano * v.s;
    return { s: v.s, x: limitarA(v.x, Math.min(m - w, 0), Math.max(ancho - m, 0)), y: limitarA(v.y, Math.min(m - h, 0), Math.max(altoVisible - m, 0)) };
  }
  let guardando: ReturnType<typeof setTimeout> | undefined;
  let finAnimacion: ReturnType<typeof setTimeout> | undefined;
  function poner(v: Vista, conAnimacion = false) {
    animar = conAnimacion && !sinMovimiento();
    clearTimeout(finAnimacion);
    if (animar) finAnimacion = setTimeout(() => (animar = false), 220);
    vista = limitar(v);
    tocado = true;
    clearTimeout(guardando);
    guardando = setTimeout(guardarVista, 250);
  }
  function zoomEn(f: number, px: number, py: number, conAnimacion = false) {
    const s = limitarA(vista.s * f, MIN, MAX);
    const k = s / vista.s;
    poner({ s, x: px - (px - vista.x) * k, y: py - (py - vista.y) * k }, conAnimacion);
  }
  const zoomCentro = (f: number) => zoomEn(f, ancho / 2, altoVisible / 2, true);
  const tamanoReal = () => zoomEn(1 / vista.s, ancho / 2, altoVisible / 2, true);
  const ajustar = () => poner(ajuste(), true);

  // Recordar la vista de cada mapa en esta pestaña (sesión), no en el navegador.
  const claveVista = $derived(`resguardo.mapa.vista:${cliente}:${equipo ? `equipo:${equipo}` : dado ? "todos" : `${perspectiva}:${raizValida}`}`);
  function guardarVista() {
    try {
      sessionStorage.setItem(claveVista, JSON.stringify({ s: vista.s, x: vista.x, y: vista.y, a: ancho }));
    } catch {
      /* sin almacenamiento: no se recuerda */
    }
  }
  $effect(() => {
    const k = claveVista;
    let v: (Vista & { a?: number }) | null = null;
    try {
      v = JSON.parse(sessionStorage.getItem(k) ?? "null");
    } catch {
      v = null;
    }
    untrack(() => {
      const bien = !!v && [v.s, v.x, v.y].every(Number.isFinite) && v.s >= MIN && v.s <= MAX;
      // Con otro ancho de ventana, lo guardado ya no cuadra: se empieza ajustado.
      if (v && bien && (!v.a || !ancho || Math.abs(v.a - ancho) < 2)) {
        vista = { s: v.s, x: v.x, y: v.y };
        tocado = true;
      } else tocado = false;
    });
  });
  // Mientras nadie lo toque, el dibujo se ajusta solo (al medirse, al cambiar de ancho o de mapa).
  $effect(() => {
    const v = inicial();
    if (!listo || tocado || !ancho) return;
    vista = v;
  });

  // Ratón, dedo y lápiz: arrastrar mueve; dos dedos acercan. Un clic sin arrastrar sigue siendo un clic.
  const punteros = new Map<number, { x: number; y: number }>();
  let arrastre: { id: number; x0: number; y0: number; v0: Vista; movido: boolean } | null = null;
  let pellizco: { d0: number; cx: number; cy: number; v0: Vista } | null = null;
  let recienArrastrado = false;
  let arrastrando = $state(false);
  function local(ev: { clientX: number; clientY: number }) {
    const r = lienzo!.getBoundingClientRect();
    return { x: ev.clientX - r.left, y: ev.clientY - r.top };
  }
  const distancia = (a: { x: number; y: number }, b: { x: number; y: number }) => Math.hypot(a.x - b.x, a.y - b.y) || 1;
  function alBajar(ev: PointerEvent) {
    if (enLista || (ev.pointerType === "mouse" && ev.button !== 0)) return;
    if ((ev.target as Element).closest("button, select, input, textarea, .m-ctrl")) return;
    const p = local(ev);
    punteros.set(ev.pointerId, p);
    if (punteros.size === 1) arrastre = { id: ev.pointerId, x0: p.x, y0: p.y, v0: { ...vista }, movido: false };
    else if (punteros.size === 2) {
      const [a, b] = [...punteros.values()];
      pellizco = { d0: distancia(a, b), cx: (a.x + b.x) / 2, cy: (a.y + b.y) / 2, v0: { ...vista } };
      if (arrastre?.movido) recienArrastrado = true;
      arrastre = null;
      for (const id of punteros.keys()) {
        try {
          lienzo?.setPointerCapture(id);
        } catch {
          /* ya se soltó */
        }
      }
    }
  }
  function alMover(ev: PointerEvent) {
    if (!punteros.has(ev.pointerId)) return;
    const p = local(ev);
    punteros.set(ev.pointerId, p);
    if (pellizco && punteros.size >= 2) {
      const [a, b] = [...punteros.values()];
      const s = limitarA((pellizco.v0.s * distancia(a, b)) / pellizco.d0, MIN, MAX);
      const k = s / pellizco.v0.s;
      // Lo que estaba bajo los dedos sigue bajo los dedos.
      poner({ s, x: (a.x + b.x) / 2 - (pellizco.cx - pellizco.v0.x) * k, y: (a.y + b.y) / 2 - (pellizco.cy - pellizco.v0.y) * k });
      return;
    }
    if (!arrastre || arrastre.id !== ev.pointerId) return;
    const dx = p.x - arrastre.x0;
    const dy = p.y - arrastre.y0;
    if (!arrastre.movido) {
      if (Math.hypot(dx, dy) < 5) return;
      arrastre.movido = true;
      arrastrando = true;
      try {
        lienzo?.setPointerCapture(ev.pointerId);
      } catch {
        /* ya se soltó */
      }
    }
    poner({ s: arrastre.v0.s, x: arrastre.v0.x + dx, y: arrastre.v0.y + dy });
  }
  function alSoltar(ev: PointerEvent) {
    punteros.delete(ev.pointerId);
    if (arrastre?.id === ev.pointerId) {
      if (arrastre.movido) recienArrastrado = true;
      arrastre = null;
      arrastrando = false;
    }
    if (punteros.size < 2) pellizco = null;
    if (recienArrastrado) setTimeout(() => (recienArrastrado = false), 0);
  }
  /** Lo que se arrastró no abre la tarjeta donde empezó. */
  function alClic(ev: MouseEvent) {
    if (!recienArrastrado) return;
    ev.preventDefault();
    ev.stopPropagation();
    recienArrastrado = false;
  }
  function alDobleClic(ev: MouseEvent) {
    if (enLista || (ev.target as Element).closest("[data-caja], .m-ctrl")) return;
    const p = local(ev);
    zoomEn(ev.shiftKey ? 1 / 1.6 : 1.6, p.x, p.y, true);
  }
  // La rueda: con Ctrl o ⌘ (o pellizcando en el panel táctil) acerca; sola, mueve la página
  // (en pantalla completa, el mapa). Sin la tecla, una pista de cómo acercar.
  let pista = $state(false);
  let finPista: ReturnType<typeof setTimeout> | undefined;
  const esMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
  function rueda(ev: WheelEvent) {
    if (enLista) return;
    const d = ev.deltaMode === 1 ? ev.deltaY * 16 : ev.deltaMode === 2 ? ev.deltaY * 400 : ev.deltaY;
    if (ev.ctrlKey || ev.metaKey) {
      ev.preventDefault();
      const p = local(ev);
      zoomEn(2 ** (-limitarA(d, -50, 50) * 0.006), p.x, p.y);
    } else if (pantallaCompleta) {
      ev.preventDefault();
      const dx = ev.deltaMode === 1 ? ev.deltaX * 16 : ev.deltaX;
      poner({ ...vista, x: vista.x - (ev.shiftKey && !dx ? d : dx), y: vista.y - (ev.shiftKey && !dx ? 0 : d) });
    } else {
      pista = true;
      clearTimeout(finPista);
      finPista = setTimeout(() => (pista = false), 1600);
    }
  }
  $effect(() => {
    const el = lienzo;
    if (!el) return;
    // No pasiva: con Ctrl, la rueda acerca el mapa y no la página.
    el.addEventListener("wheel", rueda, { passive: false });
    return () => el.removeEventListener("wheel", rueda);
  });
  // Al enfocar una tarjeta con el teclado, el plano se mueve para que se vea entera.
  function asegurarVisible(id: string) {
    const c = disp.cajas[id];
    if (!c) return;
    const s = vista.s;
    const m = 16;
    const x0 = vista.x + (MARGEN + c.x) * s;
    const y0 = vista.y + (MARGEN + c.y) * s;
    const x1 = x0 + c.w * s;
    const y1 = y0 + c.h * s;
    const dx = x0 < m ? m - x0 : x1 > ancho - m ? Math.max(m - x0, ancho - m - x1) : 0;
    const dy = y0 < m ? m - y0 : y1 > altoVisible - m ? Math.max(m - y0, altoVisible - m - y1) : 0;
    if (dx || dy) poner({ s, x: vista.x + dx, y: vista.y + dy }, true);
  }
  function alEnfocar(ev: FocusEvent) {
    const el = ev.target as HTMLElement;
    const caja = el.closest<HTMLElement>("[data-caja]");
    if (caja && el.matches(":focus-visible")) asegurarVisible(caja.dataset.caja!);
  }
  /** El navegador desplaza lo que tiene `overflow: hidden` al enfocar algo de dentro: aquí se mueve el plano. */
  function sinDesplazar() {
    if (lienzo && (lienzo.scrollTop || lienzo.scrollLeft)) {
      lienzo.scrollTop = 0;
      lienzo.scrollLeft = 0;
    }
  }

  // Pantalla completa (la del navegador, o el mapa sobre toda la ventana si no la hay).
  async function alternarPantalla() {
    if (pantallaCompleta) {
      if (document.fullscreenElement) await document.exitFullscreen().catch(() => {});
      ampliado = false;
      pantallaCompleta = false;
      tocado = false;
      return;
    }
    tocado = false;
    if (seccion?.requestFullscreen && document.fullscreenEnabled) {
      try {
        await seccion.requestFullscreen();
        return;
      } catch {
        /* sin permiso: la de la ventana */
      }
    }
    ampliado = true;
    pantallaCompleta = true;
  }
  $effect(() => {
    const cambio = () => {
      if (ampliado) return;
      pantallaCompleta = !!seccion && document.fullscreenElement === seccion;
      tocado = false;
    };
    document.addEventListener("fullscreenchange", cambio);
    return () => document.removeEventListener("fullscreenchange", cambio);
  });
  function teclaVentana(ev: KeyboardEvent) {
    if (ampliado && ev.key === "Escape") {
      ampliado = false;
      pantallaCompleta = false;
      tocado = false;
    }
  }

  interface Marca {
    id: string;
    de: string;
    a: string;
    tono: Tono;
    x: number;
    y: number;
  }
  /** Marcas sobre los trazos que no van bien: un círculo con el icono del estado (la palabra va en la tarjeta a la que llega). */
  const marcas = $derived(
    disp.rutas
      .filter((r) => r.marca)
      .map((r): Marca => ({ id: r.id, de: r.de, a: r.a, tono: aristaPorId.get(r.id)?.tono ?? "neutral", x: r.marca!.x, y: r.marca!.y })),
  );

  // Al pasar por una tarjeta (o enfocarla), su cadena entera resalta y lo demás se apaga.
  let foco = $state<string | null>(null);
  const cadena = $derived.by(() => {
    if (!foco) return null;
    const s = new Set<string>([foco]);
    const ir = (id: string, dir: "de" | "a") => {
      for (const x of mapa.aristas) {
        const [desde, hasta] = dir === "de" ? [x.de, x.a] : [x.a, x.de];
        if (desde === id && !s.has(hasta)) {
          s.add(hasta);
          ir(hasta, dir);
        }
      }
    };
    ir(foco, "de");
    ir(foco, "a");
    return s;
  });

  // Teclado: ↑↓ dentro de la columna, → al primero que recibe de esta tarjeta, ← al primero que le manda;
  // + − y 0 acercan, alejan y ajustan; con el lienzo enfocado, las flechas mueven el plano.
  function tecla(ev: KeyboardEvent) {
    if (enLista) return;
    const sinMod = !ev.ctrlKey && !ev.metaKey && !ev.altKey;
    if (sinMod && (ev.key === "+" || ev.key === "=")) return (ev.preventDefault(), zoomCentro(1.25));
    if (sinMod && (ev.key === "-" || ev.key === "_")) return (ev.preventDefault(), zoomCentro(1 / 1.25));
    if (sinMod && ev.key === "0") return (ev.preventDefault(), ajustar());
    if (ev.target === lienzo && sinMod && ev.key.startsWith("Arrow")) {
      ev.preventDefault();
      const paso = ev.shiftKey ? 200 : 60;
      const [dx, dy] = ev.key === "ArrowLeft" ? [paso, 0] : ev.key === "ArrowRight" ? [-paso, 0] : ev.key === "ArrowUp" ? [0, paso] : [0, -paso];
      poner({ ...vista, x: vista.x + dx, y: vista.y + dy }, true);
      return;
    }
    const el = (ev.target as HTMLElement).closest<HTMLElement>("[data-nodo]");
    if (!el || !["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End"].includes(ev.key)) return;
    const id = el.dataset.nodo!;
    const col = disp.capas.find((c) => c.includes(id));
    if (!col) return;
    const i = col.indexOf(id);
    const arriba = (ids: string[]) => ids.sort((a, b) => (disp.cajas[a]?.y ?? 0) - (disp.cajas[b]?.y ?? 0))[0];
    let destino: string | undefined;
    if (ev.key === "ArrowDown") destino = col[i + 1];
    else if (ev.key === "ArrowUp") destino = col[i - 1];
    else if (ev.key === "Home") destino = col[0];
    else if (ev.key === "End") destino = col.at(-1);
    else if (ev.key === "ArrowRight") destino = arriba(mapa.aristas.filter((x) => x.de === id).map((x) => x.a));
    else destino = arriba(mapa.aristas.filter((x) => x.a === id).map((x) => x.de));
    if (!destino) return;
    ev.preventDefault();
    lienzo?.querySelector<HTMLElement>(`[data-nodo="${CSS.escape(destino)}"]`)?.focus();
  }

  const ICONO: Record<IconoNodo, typeof Monitor> = { cliente: Layers, equipo: Monitor, grupo: Layers, almacen: Server, disco: HardDrive, nube: Cloud, dropbox: Cloud, servidor: Server, repo: Database, otra_consola: Link2 };
  const ESTADO = { ok: CircleCheck, warn: TriangleAlert, bad: CircleAlert, info: LoaderCircle, paused: CirclePause, neutral: CircleDashed };
  const tonoTrazo = (t: Tono) => (t === "bad" || t === "warn" || t === "info" ? t : "calma");
  const cuando = (n: NodoMapa) => (n.ultima ? relativo(n.ultima, ahora) : null);
  const hijos = (id: string) => mapa.aristas.filter((x) => x.de === id && porId.has(x.a)).map((x) => ({ a: x, n: porId.get(x.a)! }));
  const idCliente = (n: NodoMapa) => n.id.replace(/^cl:/, "");
  const PERSPECTIVAS: { id: Perspectiva; texto: string }[] = [
    { id: "equipos", texto: "Equipos" },
    { id: "repositorios", texto: "Repositorios" },
    { id: "destinos", texto: "Destinos" },
  ];
  const TODOS: Record<Perspectiva, string> = { equipos: "Todos los equipos", repositorios: "Todos los repositorios", destinos: "Todos los destinos" };
  const idMapa = $derived(`mapa-${equipo ?? (dado ? "todos" : "cliente")}`);
</script>

<svelte:window bind:innerHeight={altoVentana} onkeydown={teclaVentana} />

{#snippet estado(n: NodoMapa, conCuando = true)}
  {@const Ic = ESTADO[n.tono]}
  <span class="st">
    <span class="st-ic tone-{n.tono}" aria-hidden="true"><Ic size={12} class={n.tono === "info" ? "spin" : ""} /></span>
    <span>{n.estado}{#if conCuando && cuando(n) && !n.vivo}<span class="cuando">{" · "}{cuando(n)}</span>{/if}</span>
  </span>
{/snippet}

{#snippet tarjeta(n: NodoMapa)}
  {@const Ic = ICONO[n.icono]}
  {#if n.tipo === "repo"}
    <a
      class="pildora"
      class:apagado={cadena && !cadena.has(n.id)}
      class:no-ok={n.tono !== "ok"}
      href={n.href}
      draggable="false"
      data-nodo={n.id}
      onmouseenter={() => (foco = n.id)}
      onmouseleave={() => (foco = null)}
      onfocus={() => (foco = n.id)}
      onblur={() => (foco = null)}
      use:tip={[n.sub, n.ultima ? `Última versión: ${fechaLarga(n.ultima)}` : null].filter(Boolean).join(" · ")}
    >
      {#if n.tono === "ok"}
        <span class="punto" aria-hidden="true"></span>
      {:else}
        {@const St = ESTADO[n.tono]}
        <span class="st-ic tone-{n.tono}" aria-hidden="true"><St size={12} class={n.tono === "info" ? "spin" : ""} /></span>
      {/if}
      <span class="p-txt">
        <span class="p-nombre">{n.nombre}</span>
        <span class="p-sub">{n.tono === "ok" ? (cuando(n) ? `última ${cuando(n)}` : n.sub) : n.estado}</span>
        {#if n.aviso}<span class="aviso-nodo"><TriangleAlert size={11} aria-hidden="true" />{n.aviso}</span>{/if}
      </span>
      {#if n.cifra}<span class="nodo-cifra num">{n.cifra}</span>{/if}
      <span class="sr-only">{n.tono === "ok" ? `, ${n.estado}` : ""}, {n.sub}</span>
    </a>
  {:else if n.tipo === "fuera"}
    <!-- v1.56: lo que hace un equipo que no está en esta consola (su espejo) no se ve aquí. -->
    <div
      class="nodo nodo-fuera"
      class:apagado={cadena && !cadena.has(n.id)}
      data-nodo={n.id}
      role="group"
      aria-label={n.nombre}
      onmouseenter={() => (foco = n.id)}
      onmouseleave={() => (foco = null)}
    >
      <span class="n-ic" aria-hidden="true"><Ic size={16} /></span>
      <span class="n-txt">
        <span class="n-nombre">{n.nombre}</span>
        <span class="n-sub">{n.sub}</span>
        <span class="st"><span class="st-ic tone-neutral" aria-hidden="true"><CircleDashed size={12} /></span><span>{n.estado}</span></span>
        {#if alConectarFuera}<button type="button" class="btn btn-sm conectar-fuera" onclick={() => alConectarFuera(n)}><Link2 size={13} />Conectar también…</button>{/if}
      </span>
    </div>
  {:else if n.tipo === "cliente"}
    <!-- Un cliente (mapa de todos): su marca, su estado y, al lado, plegar o desplegar sus equipos. -->
    <div class="cli" class:apagado={cadena && !cadena.has(n.id)}>
      <a
        class="nodo nodo-cli"
        href={n.href}
        draggable="false"
        data-nodo={n.id}
        onmouseenter={() => (foco = n.id)}
        onmouseleave={() => (foco = null)}
        onfocus={() => (foco = n.id)}
        onblur={() => (foco = null)}
      >
        <span class="n-txt">
          <span class="n-nombre cli-nombre"><MarcaCliente nombre={n.nombre} marca={n.marca} tam={20} />{n.nombre}</span>
          <span class="n-sub">{n.sub}</span>
          {@render estado(n, !n.vivo)}
        </span>
      </a>
      {#if alPlegar}
        <button
          type="button"
          class="icon-btn plegar"
          aria-expanded={!n.plegado}
          aria-label={n.plegado ? `Desplegar ${n.nombre}` : `Plegar ${n.nombre}`}
          use:tip={n.plegado ? "Ver sus equipos" : "Plegar"}
          onclick={() => alPlegar(idCliente(n))}
        >
          {#if n.plegado}<ChevronRight size={15} />{:else}<ChevronDown size={15} />{/if}
        </button>
      {/if}
    </div>
  {:else}
    <a
      class="nodo"
      class:apagado={cadena && !cadena.has(n.id)}
      href={n.href}
      draggable="false"
      data-nodo={n.id}
      onmouseenter={() => (foco = n.id)}
      onmouseleave={() => (foco = null)}
      onfocus={() => (foco = n.id)}
      onblur={() => (foco = null)}
    >
      <span class="n-ic" aria-hidden="true"><Ic size={16} /></span>
      <span class="n-txt">
        <span class="n-nombre">{n.nombre}</span>
        <span class="n-sub">{n.sub}</span>
        {#if n.tipoDestino}<span class="n-tipo"><TipoDestino {...n.tipoDestino} /></span>{/if}
        {@render estado(n, n.icono !== "almacen")}
        {#if n.aviso}<span class="aviso-nodo"><TriangleAlert size={12} aria-hidden="true" />{n.aviso}</span>{/if}
      </span>
    </a>
  {/if}
{/snippet}

<!-- Una rama del árbol (en lista): la tarjeta y lo que le llega; lo que sale de un repositorio, en líneas. -->
{#snippet rama(n: NodoMapa)}
  {@render tarjeta(n)}
  {#if n.tipo === "repo"}
    <ul class="hojas">
      {#each hijos(n.id) as d (d.n.id)}
        {@const Ic = ICONO[d.n.icono]}
        <li class="hoja">
          <span class="flecha" aria-hidden="true">→</span>
          <a href={d.n.href}><Ic size={14} aria-hidden="true" />{d.a.tipo === "externa" && d.n.nombre !== "Copia externa" ? "Copia externa a " : ""}{d.n.nombre}</a>
          {@render estado(d.n, d.n.icono !== "almacen")}
          {#if d.n.aviso}<span class="aviso-nodo"><TriangleAlert size={12} aria-hidden="true" />{d.n.aviso}</span>{/if}
        </li>
      {/each}
    </ul>
  {:else if hijos(n.id).length}
    <ul>
      {#each hijos(n.id) as h (h.n.id)}
        <li>{@render rama(h.n)}</li>
      {/each}
    </ul>
  {/if}
{/snippet}

<section class="card mapa" class:compacto={!!equipo} class:completa={pantallaCompleta} class:fija={ampliado} bind:this={seccion} aria-labelledby="t-mapa-{equipo ?? (dado ? 'todos' : 'cliente')}">
  <header class="m-cab">
    <h2 class="section-title" id="t-mapa-{equipo ?? (dado ? 'todos' : 'cliente')}"><Waypoints size={16} />{titulo}</h2>
    <div class="m-herr">
      {#if herramientas}
        {@render herramientas()}
      {:else if !equipo}
        <div class="segmentos" role="group" aria-label="Ver por">
          {#each PERSPECTIVAS as p (p.id)}
            <button type="button" aria-pressed={perspectiva === p.id} onclick={() => ((perspectiva = p.id), (raiz = ""), recordar())}>{p.texto}</button>
          {/each}
        </div>
        <label class="raiz">
          <span class="sr-only">Mostrar</span>
          <select bind:value={raiz} onchange={recordar}>
            <option value="">{TODOS[perspectiva]}</option>
            {#each opciones as o (o.id)}<option value={o.id}>{o.texto}</option>{/each}
          </select>
          <ChevronsUpDown size={14} aria-hidden="true" />
        </label>
      {/if}
      {#if mapa.nodos.length}
        <!-- En estrecho empieza en lista, pero el mapa (con dedo y pellizco) también se puede ver. -->
        <button type="button" class="btn btn-sm btn-ghost" aria-pressed={enLista} onclick={() => (modo = enLista ? "mapa" : "lista")}>
          {#if enLista}<Waypoints size={14} />Ver el mapa{:else}<List size={14} />Ver como lista{/if}
        </button>
      {/if}
    </div>
  </header>

  {#if !mapa.nodos.length}
    <p class="faint vacio">{vacio}</p>
  {:else}
    <!-- La alternativa en texto (siempre): lo mismo en frases. -->
    <div class="sr-only">
      <p>{mapa.frases.join(" ")}</p>
    </div>

    <p class="sr-only" id="{idMapa}-ayuda">
      Arrastra para moverte por el mapa. Para acercar o alejar: {esMac ? "⌘" : "Ctrl"} y la rueda, pellizcar, doble clic o las teclas + y −; 0 lo ajusta. Con el mapa enfocado, las flechas lo mueven; en una tarjeta, recorren el mapa.
    </p>
    <!-- svelte-ignore a11y_no_static_element_interactions, a11y_no_noninteractive_tabindex -->
    <div
      class="lienzo"
      class:lista={enLista}
      class:movil={!enLista}
      class:arrastrando
      bind:this={lienzo}
      role={enLista ? undefined : "group"}
      aria-label={enLista ? undefined : `${titulo}: el dibujo`}
      aria-describedby={enLista ? undefined : `${idMapa}-ayuda`}
      tabindex={enLista ? undefined : 0}
      style:height={enLista || pantallaCompleta ? undefined : `${altoLienzo}px`}
      style:background-position={enLista ? undefined : `${vista.x}px ${vista.y}px`}
      style:background-size={enLista ? undefined : `${18 * vista.s}px ${18 * vista.s}px`}
      onkeydown={tecla}
      onpointerdown={alBajar}
      onpointermove={alMover}
      onpointerup={alSoltar}
      onpointercancel={alSoltar}
      onclickcapture={alClic}
      ondblclick={alDobleClic}
      ondragstart={(ev) => !enLista && ev.preventDefault()}
      onfocusin={alEnfocar}
      onscroll={sinDesplazar}
    >
      {#if enLista}
        <ul class="arbol">
          {#each mapa.nodos.filter((n) => n.col === 0) as n (n.id)}
            <li>{@render rama(n)}</li>
          {/each}
          {#each mapa.nodos.filter((n) => n.tipo === "destino" && hijos(n.id).length) as n (n.id)}
            <li>
              {@render tarjeta(n)}
              <ul class="hojas">
                {#each hijos(n.id) as d (d.n.id)}
                  {@const Ic = ICONO[d.n.icono]}
                  <li class="hoja">
                    <span class="flecha" aria-hidden="true">→</span>
                    {#if d.n.tipo === "fuera"}
                      <!-- v1.56: el almacén de otra consola: lo suyo no se ve aquí. -->
                      <span class="hoja-fuera"><Ic size={14} aria-hidden="true" />{d.n.nombre}: {d.n.sub.toLowerCase()}</span>
                      {@render estado(d.n)}
                      {#if alConectarFuera}<button type="button" class="btn btn-sm conectar-fuera" onclick={() => alConectarFuera(d.n)}><Link2 size={13} />Conectar también…</button>{/if}
                    {:else}
                      <a href={d.n.href}><Ic size={14} aria-hidden="true" />Espejo en {d.n.nombre}</a>
                      {@render estado(d.n)}
                    {/if}
                  </li>
                {/each}
              </ul>
            </li>
          {/each}
        </ul>
      {:else}
        <!-- El plano: el dibujo entero, que se mueve y se acerca (translate + scale). -->
        <div
          class="plano"
          class:listo
          class:animar
          bind:this={plano}
          style:width="{anchoPlano}px"
          style:height="{altoPlano}px"
          style:transform="translate({vista.x}px, {vista.y}px) scale({vista.s})"
        >
          <svg class="trazos" width={anchoPlano} height={altoPlano} aria-hidden="true">
            <g transform="translate({MARGEN} {MARGEN})">
              {#each disp.rutas as r (r.id)}
                {@const a = aristaPorId.get(r.id)}
                {#if a}<path class="trazo t-{tonoTrazo(a.tono)}" class:vivo={a.vivo} class:apagado={cadena && !(cadena.has(a.de) && cadena.has(a.a))} d={r.d} />{/if}
              {/each}
              {#each Object.entries(disp.cajas) as [id, c] (id)}
                {#if conSalida.has(id)}<circle class="puerto" cx={c.x + c.w} cy={c.y + c.h / 2} r="3" />{/if}
              {/each}
            </g>
          </svg>
          {#each disp.capas as capa, i (i)}
            {#each capa as id (id)}
              {@const n = porId.get(id)}
              {@const c = disp.cajas[id]}
              {#if n && c}
                <div class="caja" data-caja={id} style:left="{MARGEN + c.x}px" style:top="{MARGEN + c.y}px" style:width="{c.w}px">{@render tarjeta(n)}</div>
              {/if}
            {/each}
          {/each}
          {#each marcas as r (r.id)}
            {@const St = ESTADO[r.tono]}
            <span class="marca tone-{r.tono}" class:apagado={cadena && !(cadena.has(r.de) && cadena.has(r.a))} style:left="{MARGEN + r.x}px" style:top="{MARGEN + r.y}px" aria-hidden="true"><St size={11} /></span>
          {/each}
        </div>
        <div class="m-pista" class:visible={pista} aria-hidden="true">{esMac ? "⌘" : "Ctrl"} + rueda para acercar · arrastra para moverte</div>
        <div class="m-ctrl" class:chico={ancho < 420} role="group" aria-label="Acercar y mover el mapa">
          <button type="button" class="icon-btn" aria-label="Alejar" use:tip={"Alejar (−)"} disabled={vista.s <= MIN + 0.001} onclick={() => zoomCentro(1 / 1.25)}><ZoomOut size={15} /></button>
          <button type="button" class="m-pct num" aria-label="Tamaño real (ahora al {Math.round(vista.s * 100)} %)" use:tip={"Tamaño real"} onclick={tamanoReal}>{Math.round(vista.s * 100)} %</button>
          <button type="button" class="icon-btn" aria-label="Acercar" use:tip={"Acercar (+)"} disabled={vista.s >= MAX - 0.001} onclick={() => zoomCentro(1.25)}><ZoomIn size={15} /></button>
          <span class="m-sep" aria-hidden="true"></span>
          <button type="button" class="m-txt" aria-label="Ajustar el mapa a la vista" use:tip={"Todo a la vista (0)"} onclick={ajustar}><Scan size={14} aria-hidden="true" />Ajustar</button>
          <button type="button" class="icon-btn" aria-label={pantallaCompleta ? "Salir de pantalla completa" : "Pantalla completa"} use:tip={pantallaCompleta ? "Salir (Esc)" : "Pantalla completa"} onclick={alternarPantalla}>
            {#if pantallaCompleta}<Minimize size={15} />{:else}<Maximize size={15} />{/if}
          </button>
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .mapa {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-5);
    min-width: 0;
  }
  .m-cab {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
  }
  .m-cab h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
  }
  .m-cab h2 :global(svg) {
    color: var(--text-3);
  }
  .m-herr {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  /* Pestañas segmentadas: «Equipos | Repositorios | Destinos». */
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
  .raiz {
    position: relative;
    display: inline-flex;
    align-items: center;
  }
  .raiz select {
    min-height: 30px;
    max-width: 240px;
    padding: 0 28px 0 10px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-1);
    background: var(--surface);
    border: 1px solid var(--border-input);
    border-radius: var(--radius);
    appearance: none;
    cursor: pointer;
  }
  .raiz :global(svg) {
    position: absolute;
    right: 8px;
    color: var(--text-3);
    pointer-events: none;
  }

  /* El lienzo: fondo hundido con una rejilla de puntos muy tenue. */
  .lienzo {
    position: relative;
    margin: 0 calc(-1 * var(--sp-5)) calc(-1 * var(--sp-5));
    padding: var(--sp-6) var(--sp-5);
    background-color: var(--bg-subtle);
    background-image: radial-gradient(circle at 1px 1px, color-mix(in srgb, var(--text-3) 26%, transparent) 1px, transparent 1.4px);
    background-size: 18px 18px;
    border-top: 1px solid var(--border);
    border-radius: 0 0 var(--radius-lg) var(--radius-lg);
    overflow: hidden;
    --brillo: none;
  }
  :global(:root[data-theme="dark"]) .lienzo,
  :global(:root[data-theme="black"]) .lienzo {
    --brillo: drop-shadow(0 0 3px color-mix(in srgb, var(--info) 60%, transparent));
  }
  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-theme="light"])) .lienzo {
      --brillo: drop-shadow(0 0 3px color-mix(in srgb, var(--info) 60%, transparent));
    }
  }
  /* El mapa que se mueve: sin padding (el aire va dentro del plano), cursor de mano y sin seleccionar texto al arrastrar. */
  .lienzo.movil {
    padding: 0;
    cursor: grab;
    user-select: none;
    -webkit-user-select: none;
    /* Un dedo en vertical mueve la página; en horizontal o con dos dedos, el mapa. */
    touch-action: pan-y;
  }
  .lienzo.movil:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .lienzo.arrastrando,
  .lienzo.arrastrando :global(a) {
    cursor: grabbing;
  }
  .plano {
    position: absolute;
    top: 0;
    left: 0;
    transform-origin: 0 0;
    visibility: hidden;
  }
  .plano.listo {
    visibility: visible;
  }
  .plano.animar {
    transition: transform 0.2s var(--ease);
  }
  @media (prefers-reduced-motion: reduce) {
    .plano.animar {
      transition: none;
    }
  }
  .caja {
    position: absolute;
    z-index: 1;
    min-width: 0;
  }
  .trazos {
    position: absolute;
    inset: 0;
    z-index: 0;
    overflow: visible;
    pointer-events: none;
  }

  /* Los botones de abajo a la derecha: alejar, el tamaño, acercar, ajustar y pantalla completa. */
  .m-ctrl {
    position: absolute;
    right: 12px;
    bottom: 12px;
    z-index: 3;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 3px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow-sm);
    cursor: default;
  }
  .m-ctrl .icon-btn {
    width: 30px;
    height: 30px;
    color: var(--text-2);
  }
  .m-txt {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 30px;
    padding: 0 8px;
    font: inherit;
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--text-2);
    background: none;
    border: 0;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .m-txt:hover {
    color: var(--text-1);
    background: var(--surface-2);
  }
  .m-pct {
    min-width: 48px;
    height: 30px;
    padding: 0 4px;
    font: inherit;
    font-size: var(--fs-xs);
    color: var(--text-2);
    background: none;
    border: 0;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .m-pct:hover {
    color: var(--text-1);
    background: var(--surface-2);
  }
  .m-sep {
    width: 1px;
    height: 18px;
    margin: 0 3px;
    background: var(--border);
  }
  /* En un mapa estrecho, sin el tamaño en cifras (los botones siguen). */
  .m-ctrl.chico {
    right: 8px;
    bottom: 8px;
  }
  .m-ctrl.chico .m-pct,
  .m-ctrl.chico .m-sep {
    display: none;
  }
  /* La pista de la rueda: aparece un momento en el centro, sin tapar nada que se pueda pulsar. */
  .m-pista {
    position: absolute;
    top: 50%;
    left: 50%;
    z-index: 4;
    padding: 8px 14px;
    font-size: var(--fs-sm);
    color: var(--surface);
    background: color-mix(in srgb, var(--text-1) 86%, transparent);
    border-radius: 999px;
    opacity: 0;
    translate: -50% -50%;
    pointer-events: none;
    transition: opacity var(--dur) var(--ease);
  }
  .m-pista.visible {
    opacity: 1;
  }
  @media (prefers-reduced-motion: reduce) {
    .m-pista {
      transition: none;
    }
  }

  /* Pantalla completa: la tarjeta entera (cabecera y mapa) ocupa la pantalla. */
  .mapa.completa {
    width: 100%;
    height: 100%;
    max-width: none;
    border-radius: 0;
  }
  .mapa.fija {
    position: fixed;
    inset: 0;
    z-index: 1000;
    height: 100dvh;
  }
  .mapa.completa .lienzo {
    flex: 1;
    min-height: 0;
    border-radius: 0;
    touch-action: none;
  }

  /* Tarjetas: icono en su caja, nombre, una línea y el estado (icono + palabra). */
  .nodo {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 12px;
    min-width: 0;
    color: var(--text-1);
    text-decoration: none;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    box-shadow: var(--shadow-sm);
    transition:
      opacity var(--dur) var(--ease),
      border-color var(--dur-fast) var(--ease);
  }
  .nodo:hover {
    border-color: var(--border-input);
  }
  /* v1.56: un paso de otra consola: borde discontinuo y sin sombra (no es un sitio de aquí). */
  .nodo-fuera {
    background: var(--surface-2, var(--surface));
    border-style: dashed;
    box-shadow: none;
  }
  .nodo-fuera .n-sub {
    white-space: normal;
  }
  .conectar-fuera {
    align-self: flex-start;
    max-width: 100%;
    height: auto;
    min-height: 28px;
    margin-top: 6px;
    white-space: normal;
    text-align: left;
  }
  /* La caja del icono: no se llama `.tile` (la tarjeta tranquila global, con su
   * padding de 16, empujaba el icono fuera de la caja). */
  .n-ic {
    box-sizing: border-box;
    display: grid;
    flex: none;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
    line-height: 0;
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  .n-txt {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .n-nombre {
    font-size: var(--fs-sm);
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  /* v1.41: «En el mismo equipo que protege» (color de aviso, con icono y texto). */
  .aviso-nodo {
    display: inline-flex;
    align-items: flex-start;
    gap: 4px;
    margin-top: 2px;
    padding: 1px 6px;
    width: fit-content;
    max-width: 100%;
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    font-weight: 500;
    color: var(--warn);
    background: var(--warn-soft);
    border-radius: 6px;
  }
  .aviso-nodo :global(svg) {
    flex: none;
    margin-top: 2px;
  }
  .n-tipo {
    display: flex;
    margin-top: 2px;
  }
  .n-sub {
    overflow: hidden;
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    color: var(--text-3);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .st {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    margin-top: 3px;
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    color: var(--text-2);
  }
  .st-ic {
    display: inline-grid;
    flex: none;
    color: var(--tone);
  }
  .cuando {
    color: var(--text-3);
  }

  /* Un cliente (mapa de todos): su marca en la caja del icono y el botón de plegar al lado. */
  .cli {
    position: relative;
    display: flex;
    min-width: 0;
    transition: opacity var(--dur) var(--ease);
  }
  .cli .nodo-cli {
    flex: 1;
    padding-right: 34px;
  }
  .cli-nombre {
    display: flex;
    align-items: flex-start;
    gap: 7px;
    overflow-wrap: break-word;
  }
  .cli-nombre :global(.mc) {
    margin-top: -1px;
  }
  .plegar {
    position: absolute;
    top: 6px;
    right: 4px;
    width: 28px;
    height: 28px;
  }
  .arbol .cli {
    max-width: 420px;
  }

  /* Píldoras (los repositorios): el enlace entre el equipo y su destino. */
  .pildora {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 36px;
    padding: 4px 6px 4px 10px;
    min-width: 0;
    color: var(--text-1);
    text-decoration: none;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    transition: opacity var(--dur) var(--ease);
  }
  .pildora:hover {
    border-color: var(--border-input);
  }
  .punto {
    flex: none;
    width: 7px;
    height: 7px;
    background: var(--text-3);
    border-radius: 999px;
  }
  .p-txt {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
    line-height: 1.2;
  }
  .p-nombre {
    overflow: hidden;
    font-size: var(--fs-xs);
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .p-sub {
    overflow: hidden;
    font-size: 11px;
    color: var(--text-3);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pildora.no-ok .p-sub {
    color: var(--text-2);
  }
  .nodo-cifra {
    flex: none;
    padding: 1px 6px;
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
  }
  .apagado {
    opacity: 0.35;
  }

  /* Trazos: discontinuos y en curva. Tinta tenue si van bien; del estado si no; en marcha, se mueven. */
  .trazo {
    fill: none;
    stroke-width: 1.5;
    stroke-dasharray: 5 5;
    stroke-linecap: round;
    transition: opacity var(--dur) var(--ease);
  }
  .t-calma {
    stroke: color-mix(in srgb, var(--text-3) 70%, transparent);
  }
  .t-warn {
    stroke: var(--warn);
  }
  .t-bad {
    stroke: var(--bad);
    stroke-width: 1.75;
  }
  .t-info {
    stroke: var(--info);
    stroke-width: 1.75;
  }
  .trazo.vivo {
    filter: var(--brillo);
    animation: flujo 0.9s linear infinite;
  }
  @keyframes flujo {
    to {
      stroke-dashoffset: -10;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .trazo.vivo {
      animation: none;
    }
  }
  .puerto {
    fill: var(--surface);
    stroke: var(--text-3);
    stroke-width: 1.25;
  }
  .marca {
    position: absolute;
    z-index: 2;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    color: var(--tone);
    background: var(--surface);
    border: 1px solid color-mix(in srgb, var(--tone) 45%, transparent);
    border-radius: 999px;
    translate: -50% -50%;
    pointer-events: none;
    transition: opacity var(--dur) var(--ease);
  }

  /* En lista (estrecho o pedido): un árbol en vertical con lo mismo. */
  .lienzo.lista {
    padding: var(--sp-4);
  }
  .arbol,
  .arbol ul {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .arbol > li + li {
    margin-top: 6px;
  }
  .arbol ul {
    margin: 8px 0 0 15px;
    padding-left: 14px;
    border-left: 1px dashed var(--border-input);
  }
  .arbol .nodo {
    max-width: 420px;
  }
  .arbol .pildora {
    max-width: 380px;
  }
  .hoja {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px 8px;
    font-size: var(--fs-sm);
  }
  .hoja a {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-height: 24px;
    color: var(--text-1);
  }
  .hoja a :global(svg) {
    color: var(--text-3);
  }
  .hoja .st {
    margin: 0;
  }
  .hoja-fuera {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--text-2);
  }
  .hoja-fuera :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .flecha {
    color: var(--text-3);
  }
  .vacio {
    margin: 0;
  }
</style>
