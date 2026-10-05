<script lang="ts">
  // «Mapa de la protección» (docs/diseno.md §4): de izquierda a derecha, los
  // equipos, sus repositorios (píldoras), el almacén o destino y lo que sale
  // de ahí (espejo, copia externa). Lienzo con rejilla de puntos, tarjetas de
  // verdad (enlaces: Tab, Intro y flechas) y, detrás, los trazos en curva y
  // discontinuos: en tinta tenue si todo va bien, del color del estado (con
  // su icono y texto en la tarjeta y en el rótulo) si algo falla, y moviéndose
  // mientras algo está en marcha (quietos con movimiento reducido). En
  // estrecho (o con «Ver como lista»), un árbol en vertical con lo mismo.
  import { tick, type Snippet } from "svelte";
  import { ChevronDown, ChevronRight, ChevronsUpDown, CircleAlert, CircleCheck, CircleDashed, CirclePause, Cloud, Database, HardDrive, Layers, List, LoaderCircle, Monitor, Server, TriangleAlert, Waypoints } from "@lucide/svelte";
  import type { Equipo, Informe } from "$lib/tipos";
  import { construirMapa, raices, type AristaMapa, type IconoNodo, type Mapa, type NodoMapa, type Perspectiva } from "$lib/mapa";
  import MarcaCliente from "../MarcaCliente.svelte";
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
  }: Props = $props();

  // La perspectiva y la raíz elegidas se recuerdan por cliente (en este navegador).
  const CLAVE = $derived(`resguardo.mapa.${cliente}`);
  let perspectiva = $state<Perspectiva>("equipos");
  let raiz = $state("");
  let comoLista = $state(false);
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
    construirMapa(equipo ? (todos ?? equipos) : equipos, informes, { cliente, ahora, enVivo, todos, raiz: equipo ? { perspectiva: "equipos", id: equipo } : { perspectiva, id: raizValida } }),
  );
  const columnas = $derived([0, 1, 2, 3, 4].map((c) => mapa.nodos.filter((n) => n.col === c)).filter((c) => c.length));
  const porId = $derived(new Map(mapa.nodos.map((n) => [n.id, n])));

  // Geometría: se mide dónde quedó cada tarjeta y se trazan las curvas detrás.
  let lienzo = $state<HTMLDivElement | null>(null);
  let ancho = $state(0);
  let alto = $state(0);
  let pos = $state<Record<string, { x: number; y: number; w: number; h: number }>>({});
  const estrecho = $derived(ancho > 0 && ancho < listaDesde);
  const enLista = $derived(comoLista || estrecho);
  function medir() {
    if (!lienzo) return;
    const base = lienzo.getBoundingClientRect();
    const nuevo: typeof pos = {};
    for (const el of lienzo.querySelectorAll<HTMLElement>("[data-nodo]")) {
      const r = el.getBoundingClientRect();
      nuevo[el.dataset.nodo!] = { x: r.left - base.left, y: r.top - base.top, w: r.width, h: r.height };
    }
    pos = nuevo;
    alto = lienzo.scrollHeight;
  }
  $effect(() => {
    if (!lienzo) return;
    const ro = new ResizeObserver(() => {
      ancho = lienzo?.clientWidth ?? 0;
      medir();
    });
    ro.observe(lienzo);
    return () => ro.disconnect();
  });
  $effect(() => {
    void mapa;
    void enLista;
    void tick().then(medir);
  });

  interface Trazo {
    a: AristaMapa;
    d: string;
    mx: number;
    my: number;
  }
  const trazos = $derived.by(() => {
    const out: Trazo[] = [];
    for (const a of mapa.aristas) {
      const p = pos[a.de];
      const q = pos[a.a];
      if (!p || !q) continue;
      const x1 = p.x + p.w;
      const y1 = p.y + p.h / 2;
      const x2 = q.x;
      const y2 = q.y + q.h / 2;
      const dx = Math.max(24, (x2 - x1) / 2);
      out.push({ a, d: `M${x1},${y1} C${x1 + dx},${y1} ${x2 - dx},${y2} ${x2},${y2}`, mx: (x1 + x2) / 2, my: (y1 + y2) / 2 });
    }
    return out;
  });
  /**
   * Marcas sobre los trazos que no van bien: un círculo con el icono del estado
   * en su mitad (la palabra va en la tarjeta a la que llega). Sin pisarse.
   */
  const marcas = $derived.by(() => {
    const xs = trazos
      .filter((t) => t.a.tono === "bad" || t.a.tono === "warn")
      .map((t) => ({ ...t, y: t.my }))
      .sort((a, b) => a.mx - b.mx || a.y - b.y);
    for (let i = 1; i < xs.length; i++) if (Math.abs(xs[i - 1].mx - xs[i].mx) < 24 && xs[i].y - xs[i - 1].y < 22) xs[i].y = xs[i - 1].y + 22;
    return xs;
  });

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

  // Teclado: ↑↓ dentro de la columna, → al primero que recibe de esta tarjeta, ← al primero que le manda.
  function tecla(ev: KeyboardEvent) {
    const el = (ev.target as HTMLElement).closest<HTMLElement>("[data-nodo]");
    if (!el || !["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End"].includes(ev.key)) return;
    const id = el.dataset.nodo!;
    const n = porId.get(id);
    if (!n) return;
    let destino: string | undefined;
    const col = mapa.nodos.filter((x) => x.col === n.col);
    const i = col.findIndex((x) => x.id === id);
    if (ev.key === "ArrowDown") destino = col[i + 1]?.id;
    else if (ev.key === "ArrowUp") destino = col[i - 1]?.id;
    else if (ev.key === "Home") destino = col[0]?.id;
    else if (ev.key === "End") destino = col.at(-1)?.id;
    else if (ev.key === "ArrowRight") destino = mapa.aristas.find((x) => x.de === id)?.a;
    else destino = mapa.aristas.find((x) => x.a === id)?.de;
    if (!destino) return;
    ev.preventDefault();
    lienzo?.querySelector<HTMLElement>(`[data-nodo="${CSS.escape(destino)}"]`)?.focus();
  }

  const ICONO: Record<IconoNodo, typeof Monitor> = { cliente: Layers, equipo: Monitor, grupo: Layers, almacen: Server, disco: HardDrive, nube: Cloud, dropbox: Cloud, servidor: Server, repo: Database };
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
</script>

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
      </span>
      {#if n.cifra}<span class="nodo-cifra num">{n.cifra}</span>{/if}
      <span class="sr-only">{n.tono === "ok" ? `, ${n.estado}` : ""}, {n.sub}</span>
    </a>
  {:else if n.tipo === "cliente"}
    <!-- Un cliente (mapa de todos): su marca, su estado y, al lado, plegar o desplegar sus equipos. -->
    <div class="cli" class:apagado={cadena && !cadena.has(n.id)}>
      <a
        class="nodo nodo-cli"
        href={n.href}
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
      data-nodo={n.id}
      onmouseenter={() => (foco = n.id)}
      onmouseleave={() => (foco = null)}
      onfocus={() => (foco = n.id)}
      onblur={() => (foco = null)}
    >
      <span class="tile" aria-hidden="true"><Ic size={16} /></span>
      <span class="n-txt">
        <span class="n-nombre">{n.nombre}</span>
        <span class="n-sub">{n.sub}</span>
        {@render estado(n, n.icono !== "almacen")}
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

<section class="card mapa" class:compacto={!!equipo} aria-labelledby="t-mapa-{equipo ?? (dado ? 'todos' : 'cliente')}">
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
      {#if !estrecho}
        <button type="button" class="btn btn-sm btn-ghost" aria-pressed={comoLista} onclick={() => (comoLista = !comoLista)}>
          {#if comoLista}<Waypoints size={14} />Ver el mapa{:else}<List size={14} />Ver como lista{/if}
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

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="lienzo" class:lista={enLista} bind:this={lienzo} onkeydown={tecla}>
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
                    <a href={d.n.href}><Ic size={14} aria-hidden="true" />Espejo en {d.n.nombre}</a>
                    {@render estado(d.n)}
                  </li>
                {/each}
              </ul>
            </li>
          {/each}
        </ul>
      {:else}
        <svg class="trazos" width={ancho} height={alto} aria-hidden="true">
          {#each trazos as t (t.a.id)}
            <path class="trazo t-{tonoTrazo(t.a.tono)}" class:vivo={t.a.vivo} class:apagado={cadena && !(cadena.has(t.a.de) && cadena.has(t.a.a))} d={t.d} />
          {/each}
          {#each mapa.nodos.filter((n) => mapa.aristas.some((a) => a.de === n.id)) as n (n.id)}
            {@const p = pos[n.id]}
            {#if p}<circle class="puerto" cx={p.x + p.w} cy={p.y + p.h / 2} r="3" />{/if}
          {/each}
        </svg>
        <div class="columnas" class:cinco={columnas.length >= 5} style:grid-template-columns={columnas.map((c) => (c[0].tipo === "repo" ? "minmax(0, 1.2fr)" : "minmax(0, 1fr)")).join(" ")}>
          {#each columnas as col, i (i)}
            <div class="col" class:pildoras={col[0].tipo === "repo"}>
              {#each col as n (n.id)}{@render tarjeta(n)}{/each}
            </div>
          {/each}
        </div>
        {#each marcas as r (r.a.id)}
          {@const St = ESTADO[r.a.tono]}
          <span class="marca tone-{r.a.tono}" class:apagado={cadena && !(cadena.has(r.a.de) && cadena.has(r.a.a))} style:left="{r.mx}px" style:top="{r.y}px" aria-hidden="true"><St size={11} /></span>
        {/each}
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
  .columnas {
    position: relative;
    z-index: 1;
    display: grid;
    column-gap: 56px;
    align-items: center;
  }
  .col {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 14px;
    min-width: 0;
  }
  .col.pildoras {
    gap: 10px;
  }
  .trazos {
    position: absolute;
    inset: 0;
    z-index: 0;
    overflow: visible;
    pointer-events: none;
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
  .tile {
    display: grid;
    flex: none;
    place-items: center;
    width: 30px;
    height: 30px;
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
  .flecha {
    color: var(--text-3);
  }
  .vacio {
    margin: 0;
  }
  @media (max-width: 1100px) {
    .columnas {
      column-gap: 40px;
    }
  }
  /* Con los clientes delante (cinco columnas), menos aire entre ellas. */
  .columnas.cinco {
    column-gap: 30px;
  }
</style>
