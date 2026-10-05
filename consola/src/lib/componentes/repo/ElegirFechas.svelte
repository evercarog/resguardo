<script lang="ts">
  // Elegir una fecha o un intervalo en «Historial y versiones» (docs/diseno.md
  // §4): un botón con lo elegido que abre un globo con atajos (Hoy, Ayer,
  // Últimos 7 días, Este mes, Mes pasado, Personalizado) y un mes en
  // rejilla. Pulsar un día y luego otro elige el intervalo; «Aplicar» con uno
  // solo elige ese día. Con el teclado: flechas (un día, una semana), Inicio y
  // Fin (la semana), Re Pág y Av Pág (un mes; con Mayús, un año), Intro o
  // espacio eligen, Esc cierra y devuelve el foco al botón. Los días con algo
  // llevan un punto; los que aún no han llegado no se pueden elegir.
  import { tick } from "svelte";
  import { CalendarDays, ChevronLeft, ChevronRight, X } from "@lucide/svelte";
  import { claveDia, inicioDia, nombreIntervalo } from "$lib/lineaTiempo";

  interface Props {
    /** Lo elegido (inicio de cada día, los dos incluidos), o nada. */
    desde: number | null;
    hasta: number | null;
    ahora: number;
    /** Los días con algo («2026-09-29»): llevan un punto. */
    diasCon?: Set<string>;
    alElegir: (desde: number | null, hasta: number | null) => void;
  }
  let { desde, hasta, ahora, diasCon = new Set(), alElegir }: Props = $props();

  const id = $props.id();
  const hoy = $derived(inicioDia(ahora));
  let abierto = $state(false);
  let boton = $state<HTMLButtonElement | null>(null);
  let globo = $state<HTMLDivElement | null>(null);
  let rejilla = $state<HTMLTableElement | null>(null);

  // Lo que se va eligiendo dentro (no cambia nada fuera hasta «Aplicar» o un atajo).
  let ini = $state<number | null>(null);
  let fin = $state<number | null>(null);
  let sobre = $state<number | null>(null);
  /** El primer día del mes que se ve. */
  let mes = $state(0);
  /** El día con el foco (roving tabindex). */
  let foco = $state(0);

  const primeroDe = (t: number) => {
    const d = new Date(t);
    return new Date(d.getFullYear(), d.getMonth(), 1).getTime();
  };
  const masMeses = (t: number, n: number) => {
    const d = new Date(t);
    return new Date(d.getFullYear(), d.getMonth() + n, 1).getTime();
  };
  const ultimoDe = (t: number) => inicioDia(masMeses(t, 1), -1);

  async function abrir() {
    ini = desde;
    fin = hasta;
    sobre = null;
    foco = desde ?? hoy;
    mes = primeroDe(foco);
    abierto = true;
    await tick();
    enfocarDia();
  }
  function cerrar(volver = true) {
    abierto = false;
    if (volver) boton?.focus();
  }
  function aplicar(a: number | null, b: number | null) {
    alElegir(a, b);
    cerrar();
  }

  // El mes en semanas de lunes a domingo (con los días de los meses de al lado, tenues).
  const semanas = $derived.by(() => {
    const lunes = inicioDia(mes, -((new Date(mes).getDay() + 6) % 7));
    const out: number[][] = [];
    for (let s = 0; s < 6; s++) {
      const fila = Array.from({ length: 7 }, (_, i) => inicioDia(lunes, s * 7 + i));
      if (s > 3 && new Date(fila[0]).getMonth() !== new Date(mes).getMonth()) break;
      out.push(fila);
    }
    return out;
  });
  const fmtMes = new Intl.DateTimeFormat("es", { month: "long", year: "numeric" });
  const fmtLargo = new Intl.DateTimeFormat("es", { weekday: "long", day: "numeric", month: "long", year: "numeric" });
  const DIAS = [
    ["L", "lunes"],
    ["M", "martes"],
    ["X", "miércoles"],
    ["J", "jueves"],
    ["V", "viernes"],
    ["S", "sábado"],
    ["D", "domingo"],
  ];

  // Lo que se ve marcado: lo elegido o, con un inicio y el ratón encima, lo que se elegiría.
  const tramo = $derived.by(() => {
    if (ini == null) return null;
    const b = fin ?? (sobre != null ? sobre : ini);
    return { a: Math.min(ini, b), b: Math.max(ini, b) };
  });
  const enTramo = (d: number) => !!tramo && d >= tramo.a && d <= tramo.b;

  function pulsarDia(d: number) {
    if (d > hoy) return;
    foco = d;
    if (ini == null || fin != null) {
      ini = d;
      fin = null;
    } else if (d < ini) {
      fin = ini;
      ini = d;
    } else fin = d;
  }

  async function enfocarDia() {
    await tick();
    rejilla?.querySelector<HTMLButtonElement>(`[data-dia="${foco}"]`)?.focus();
  }
  function moverA(d: number) {
    foco = Math.min(d, hoy);
    if (primeroDe(foco) !== mes) mes = primeroDe(foco);
    void enfocarDia();
  }
  function tecla(ev: KeyboardEvent) {
    const d = foco;
    const k = ev.key;
    const dow = (new Date(d).getDay() + 6) % 7;
    if (k === "ArrowLeft") moverA(inicioDia(d, -1));
    else if (k === "ArrowRight") moverA(inicioDia(d, 1));
    else if (k === "ArrowUp") moverA(inicioDia(d, -7));
    else if (k === "ArrowDown") moverA(inicioDia(d, 7));
    else if (k === "Home") moverA(inicioDia(d, -dow));
    else if (k === "End") moverA(inicioDia(d, 6 - dow));
    else if (k === "PageUp" || k === "PageDown") {
      const n = (k === "PageUp" ? -1 : 1) * (ev.shiftKey ? 12 : 1);
      const dest = masMeses(d, n);
      moverA(Math.min(inicioDia(dest, new Date(d).getDate() - 1), ultimoDe(dest)));
    } else if (k === "Enter" || k === " ") pulsarDia(d);
    else return;
    ev.preventDefault();
  }
  function cambiarMes(n: number) {
    mes = masMeses(mes, n);
    foco = Math.min(mes, hoy);
  }

  // Atajos: se aplican al momento.
  const atajos = $derived.by(() => {
    const primero = primeroDe(hoy);
    const antes = masMeses(hoy, -1);
    return [
      { id: "hoy", texto: "Hoy", a: hoy, b: hoy },
      { id: "ayer", texto: "Ayer", a: inicioDia(hoy, -1), b: inicioDia(hoy, -1) },
      { id: "7", texto: "Últimos 7 días", a: inicioDia(hoy, -6), b: hoy },
      { id: "mes", texto: "Este mes", a: primero, b: hoy },
      { id: "pasado", texto: "Mes pasado", a: antes, b: ultimoDe(antes) },
    ];
  });
  const atajoActual = $derived(desde == null ? null : (atajos.find((x) => x.a === desde && x.b === hasta)?.id ?? "personalizado"));
  async function personalizado() {
    ini = fin = null;
    foco = desde ?? hoy;
    mes = primeroDe(foco);
    await enfocarDia();
  }

  const textoBoton = $derived(desde != null && hasta != null ? nombreIntervalo(desde, hasta, ahora) : "Fechas");
  const textoEleccion = $derived(
    ini == null ? "Elige el primer día." : fin == null ? `${nombreIntervalo(ini, ini, ahora)}: elige el último día o pulsa «Aplicar» para ese día solo.` : `${nombreIntervalo(ini, fin, ahora)}.`,
  );

  // Fuera del globo (ratón o foco), se cierra sin cambiar nada.
  function fuera(ev: PointerEvent) {
    const t = ev.target as Node;
    if (abierto && !globo?.contains(t) && !boton?.contains(t)) cerrar(false);
  }
  function salirFoco(ev: FocusEvent) {
    const a = ev.relatedTarget as Node | null;
    if (a && !globo?.contains(a) && !boton?.contains(a)) cerrar(false);
  }
</script>

<svelte:window onpointerdown={fuera} />

<div class="fechas">
  <button
    type="button"
    class="btn btn-sm disparador"
    class:on={desde != null}
    bind:this={boton}
    aria-haspopup="dialog"
    aria-expanded={abierto}
    aria-controls={abierto ? `${id}-globo` : undefined}
    onclick={() => (abierto ? cerrar() : abrir())}
  >
    <CalendarDays size={14} />{textoBoton}<span class="sr-only">{desde != null ? ": cambiar las fechas" : ": elegir una fecha o un intervalo"}</span>
  </button>
  {#if desde != null}
    <button type="button" class="icon-btn quitar" aria-label="Quitar las fechas ({textoBoton})" onclick={() => alElegir(null, null)}><X size={14} /></button>
  {/if}

  {#if abierto}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="globo"
      id="{id}-globo"
      role="dialog"
      aria-labelledby="{id}-titulo"
      tabindex="-1"
      bind:this={globo}
      onfocusout={salirFoco}
      onkeydown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          e.stopPropagation();
          cerrar();
        }
      }}
    >
      <h3 class="sr-only" id="{id}-titulo">Elegir fechas</h3>
      <div class="atajos" role="group" aria-label="Atajos">
        {#each atajos as x (x.id)}
          <button type="button" class="atajo" class:on={atajoActual === x.id} aria-pressed={atajoActual === x.id} onclick={() => aplicar(x.a, x.b)}>{x.texto}</button>
        {/each}
        <button type="button" class="atajo" class:on={atajoActual === "personalizado"} aria-pressed={atajoActual === "personalizado"} onclick={personalizado}>Personalizado</button>
      </div>

      <div class="mes">
        <div class="mes-cab">
          <button type="button" class="icon-btn" aria-label="Mes anterior" onclick={() => cambiarMes(-1)}><ChevronLeft size={15} /></button>
          <span class="mes-nombre" aria-live="polite">{fmtMes.format(mes)}</span>
          <button type="button" class="icon-btn" aria-label="Mes siguiente" disabled={masMeses(mes, 1) > hoy} onclick={() => cambiarMes(1)}><ChevronRight size={15} /></button>
        </div>
        <table class="dias" role="grid" aria-label={fmtMes.format(mes)} bind:this={rejilla} onkeydown={tecla} onpointerleave={() => (sobre = null)}>
          <thead>
            <tr>{#each DIAS as [c, l] (l)}<th scope="col" abbr={l}><span aria-hidden="true">{c}</span><span class="sr-only">{l}</span></th>{/each}</tr>
          </thead>
          <tbody>
            {#each semanas as s (s[0])}
              <tr>
                {#each s as d (d)}
                  {@const otro = new Date(d).getMonth() !== new Date(mes).getMonth()}
                  {@const futuro = d > hoy}
                  {@const elegido = enTramo(d)}
                  {@const extremo = !!tramo && (d === tramo.a || d === tramo.b)}
                  <td role="gridcell" aria-selected={elegido}>
                    <button
                      type="button"
                      class="dia"
                      class:otro
                      class:hoy={d === hoy}
                      class:tramo={elegido}
                      class:extremo
                      class:con={diasCon.has(claveDia(d))}
                      data-dia={d}
                      tabindex={d === foco ? 0 : -1}
                      aria-disabled={futuro || undefined}
                      aria-label="{fmtLargo.format(d)}{d === hoy ? ', hoy' : ''}{diasCon.has(claveDia(d)) ? ', con versiones o sucesos' : ''}"
                      onclick={() => pulsarDia(d)}
                      onpointerenter={() => (sobre = futuro ? null : d)}
                      onfocus={() => (foco = d)}>{new Date(d).getDate()}</button
                    >
                  </td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>

      <div class="pie">
        <p class="eleccion" role="status">{textoEleccion}</p>
        <div class="botones">
          <button type="button" class="btn btn-sm btn-ghost" onclick={() => cerrar()}>Cancelar</button>
          <button type="button" class="btn btn-sm btn-primary" disabled={ini == null} onclick={() => aplicar(ini, fin ?? ini)}>Aplicar</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .fechas {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }
  .disparador {
    height: 28px;
    font-size: var(--fs-xs);
    font-variant-numeric: tabular-nums;
  }
  .disparador.on {
    color: var(--accent-text);
    background: var(--accent-soft);
    border-color: color-mix(in srgb, var(--accent) 35%, transparent);
  }
  .quitar {
    width: 28px;
    height: 28px;
  }
  .globo {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 30;
    display: grid;
    grid-template-columns: auto auto;
    grid-template-areas: "atajos mes" "pie pie";
    gap: var(--sp-3) var(--sp-4);
    width: max-content;
    max-width: calc(100vw - 32px);
    padding: var(--sp-3);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-md);
  }
  .atajos {
    grid-area: atajos;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 132px;
    padding-right: var(--sp-3);
    border-right: 1px solid var(--border);
  }
  .atajo {
    height: 30px;
    padding: 0 10px;
    font: inherit;
    font-size: var(--fs-sm);
    text-align: left;
    color: var(--text-2);
    background: transparent;
    border: 0;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .atajo:hover {
    color: var(--text-1);
    background: var(--surface-2);
  }
  .atajo.on {
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .mes {
    grid-area: mes;
  }
  .mes-cab {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 4px;
  }
  .mes-nombre {
    font-size: var(--fs-sm);
    font-weight: 600;
  }
  .mes-nombre::first-letter {
    text-transform: uppercase;
  }
  .dias {
    border-collapse: collapse;
  }
  .dias th {
    height: 24px;
    font-size: 11px;
    font-weight: 500;
    color: var(--text-3);
  }
  .dias td {
    padding: 1px 0;
  }
  .dia {
    position: relative;
    display: grid;
    place-items: center;
    width: 34px;
    height: 32px;
    padding: 0;
    font: inherit;
    font-size: var(--fs-sm);
    font-variant-numeric: tabular-nums;
    color: var(--text-1);
    background: transparent;
    border: 0;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .dia:hover {
    background: var(--surface-2);
  }
  .dia.otro {
    color: var(--text-3);
  }
  .dia[aria-disabled="true"] {
    color: var(--text-3);
    opacity: 0.45;
    cursor: default;
  }
  .dia[aria-disabled="true"]:hover {
    background: transparent;
  }
  .dia.hoy {
    font-weight: 700;
    box-shadow: inset 0 0 0 1px var(--border-strong);
  }
  .dia.tramo {
    color: var(--accent-text);
    background: var(--accent-soft);
    border-radius: 0;
  }
  .dia.extremo {
    color: var(--accent-contrast);
    background: var(--accent);
    border-radius: var(--radius-sm);
  }
  /* Un día con algo: un punto debajo del número. */
  .dia.con::after {
    content: "";
    position: absolute;
    bottom: 3px;
    left: 50%;
    width: 4px;
    height: 4px;
    margin-left: -2px;
    border-radius: 999px;
    background: var(--text-3);
  }
  .dia.extremo.con::after {
    background: var(--accent-contrast);
  }
  .dia:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
    z-index: 1;
  }
  .pie {
    grid-area: pie;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 6px var(--sp-3);
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .eleccion {
    margin: 0;
    max-width: 30rem;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .botones {
    display: flex;
    gap: 6px;
    margin-left: auto;
  }
  /* En el móvil: los atajos en fila encima del mes, y el globo a lo ancho. */
  @media (max-width: 640px) {
    .fechas {
      position: static;
    }
    .globo {
      left: 0;
      right: 0;
      width: auto;
      max-width: none;
      grid-template-columns: 1fr;
      grid-template-areas: "atajos" "mes" "pie";
    }
    .atajos {
      flex-direction: row;
      flex-wrap: wrap;
      padding: 0 0 var(--sp-2);
      border-right: 0;
      border-bottom: 1px solid var(--border);
    }
    .atajo {
      height: 32px;
      background: var(--surface-2);
      border-radius: 999px;
    }
    .dias {
      width: 100%;
    }
    .dia {
      width: 100%;
      height: 36px;
    }
  }
</style>
