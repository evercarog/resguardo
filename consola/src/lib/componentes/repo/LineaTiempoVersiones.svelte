<script lang="ts">
  // Las versiones guardadas en el tiempo («máquina del tiempo», docs/diseno.md
  // §4): arriba, el calendario de calor (los últimos 7, 30, 60 días o un año;
  // en un año, un cuadro por día) para ver de un vistazo cuándo hay versiones
  // y cuáles quitará la retención; debajo, la bitácora (la lista por días) de
  // lo que se ha pulsado en el calendario (una hora, un día) o, sin filtro, de
  // los últimos días. Pulsar una versión la elige: con `acciones` (la página
  // del repositorio) se abre debajo con sus datos y acciones; sin ellas
  // (Restaurar), se elige. El día puede venir de fuera (`dia`/`alDia`, en la
  // URL). En el móvil (≤ 640 px) la bitácora manda y el calendario es una
  // tira de días que se desliza. Se actualiza en vivo: lo que llega se
  // ilumina un momento y se anuncia.
  import { tick, type Snippet } from "svelte";
  import { X } from "@lucide/svelte";
  import type { Regla } from "$lib/tipos";
  import { numero, plural } from "$lib/formato";
  import { calendario, claveDia, huecosDeCopia, inicioDia, nombreDia, porDias, RANGOS, rangoInicial, retencionDe, type Celda, type Rango, type VersionLinea } from "$lib/lineaTiempo";
  import { textoRegla } from "$lib/retencion";
  import CalendarioCalor from "./CalendarioCalor.svelte";
  import Bitacora from "./Bitacora.svelte";
  import FormaCopia from "./FormaCopia.svelte";

  interface Props {
    versiones: VersionLinea[];
    copias?: { id: string; nombre: string }[];
    regla?: Regla | null;
    /** Quién aplica la retención (un almacén), si no es el equipo. */
    quien?: string | null;
    ahora: number;
    seleccion?: string | null;
    alElegir: (id: string) => void;
    etiqueta: string;
    /** Lo que sale debajo de la versión elegida (sus datos y acciones: «Detalle», «Explorar», «Restaurar»). */
    acciones?: Snippet<[string]>;
    /** El calendario encima (sin él, solo la bitácora). */
    conCalendario?: boolean;
    /** El día elegido («2026-09-29»), si lo lleva la página (en la URL); si no, se guarda aquí. */
    dia?: string | null;
    alDia?: (k: string | null) => void;
  }
  let { versiones, copias = [], regla = null, quien = null, ahora, seleccion = null, alElegir, etiqueta, acciones, conCalendario = true, dia = null, alDia }: Props = $props();

  const id = $props.id();
  // El reloj, al minuto: el calendario no se rehace cada 15 s.
  const minuto = $derived(Math.floor(ahora / 60_000) * 60_000);
  const lista = $derived([...versiones].map((v) => ({ ...v, t: Date.parse(v.hora) })).sort((a, b) => b.t - a.t));
  const huecos = $derived(huecosDeCopia(copias, lista.map((v) => v.copia)));
  const nombreCopia = (c: string | null | undefined) => copias.find((k) => k.id === c)?.nombre ?? (c ? c : "Sin copia");
  const hueco = (c: string | null | undefined) => (c ? (huecos.get(c) ?? 3) : 3);
  const leyenda = $derived.by(() => {
    const out: { hueco: 0 | 1 | 2 | 3; texto: string }[] = [];
    for (const [c, h] of huecos) if (h < 3) out.push({ hueco: h, texto: nombreCopia(c) });
    if ([...huecos.values()].includes(3) || lista.some((v) => !v.copia)) out.push({ hueco: 3, texto: "Otras" });
    return out;
  });
  const motivos = $derived(retencionDe(lista, regla));
  const quitaria = $derived(motivos ? [...motivos.values()].filter((m) => m === null).length : 0);

  // El rango (se elige una vez, con las primeras versiones) y el ancho (el móvil).
  let rango = $state<Rango>(30);
  let iniciado = false;
  $effect(() => {
    if (iniciado || !lista.length) return;
    iniciado = true;
    rango = rangoInicial(lista.map((v) => v.t), ahora);
    // Si el día de la URL queda fuera, un rango que lo enseñe.
    if (dia) {
      const t = Date.parse(`${dia}T12:00:00`);
      const r = RANGOS.find((x) => t >= inicioDia(ahora, 1 - x.dias));
      if (r && r.dias > rango) rango = r.dias;
    }
  });
  let ancho = $state(0);
  const movil = $derived(ancho > 0 && ancho <= 640);
  const cal = $derived(calendario(lista, motivos, minuto, rango, movil));
  const inicioRango = $derived(inicioDia(minuto, 1 - rango));

  // El filtro: un día (de fuera, en la URL, o de aquí) o, dentro de él, una hora.
  let diaLocal = $state<string | null>(null);
  const diaFiltro = $derived(alDia ? dia : diaLocal);
  let hora = $state<{ desde: number; hasta: number; texto: string } | null>(null);
  const filtro = $derived.by(() => {
    if (!diaFiltro) return null;
    const desde = Date.parse(`${diaFiltro}T00:00:00`);
    if (!Number.isFinite(desde)) return null;
    if (hora && claveDia(hora.desde) === diaFiltro) return hora;
    return { desde, hasta: inicioDia(desde, 1), texto: nombreDia(desde, minuto) };
  });
  function ponerDia(k: string | null) {
    if (alDia) alDia(k);
    else diaLocal = k;
  }
  let todos = $state(false);
  const POR_PAGINA = 7;
  function filtrar(c: Celda, texto: string) {
    todos = false;
    // Pulsar otra vez lo mismo quita el filtro.
    if (filtro && filtro.desde === c.desde && filtro.hasta === c.hasta) {
      hora = null;
      ponerDia(null);
      return;
    }
    const diaEntero = inicioDia(c.desde) === c.desde && inicioDia(c.desde, 1) === c.hasta;
    hora = diaEntero ? null : { desde: c.desde, hasta: c.hasta, texto };
    ponerDia(claveDia(c.desde));
  }
  function quitarFiltro() {
    hora = null;
    ponerDia(null);
  }
  function cambiarRango(r: Rango) {
    rango = r;
    todos = false;
    if (filtro && filtro.desde < inicioDia(minuto, 1 - r)) quitarFiltro();
  }
  const enVista = $derived(filtro ? lista.filter((v) => v.t >= filtro.desde && v.t < filtro.hasta) : conCalendario ? lista.filter((v) => v.t >= inicioRango) : lista);
  const dias = $derived(porDias(enVista));
  const diasVisibles = $derived(filtro || todos ? dias : dias.slice(0, POR_PAGINA));
  const elegida = $derived(seleccion ? (lista.find((v) => v.id === seleccion) ?? null) : null);
  // Los días con versiones, para elegir uno con un control de tamaño cómodo (WCAG 2.5.8).
  const diasConVersiones = $derived(porDias(conCalendario ? lista.filter((v) => v.t >= inicioRango) : lista).map((d) => ({ clave: claveDia(d.dia), texto: `${nombreDia(d.dia, minuto)} · ${d.vs.length}` })));

  // La elegida (de fuera, p. ej. al abrir su detalle), a la vista.
  $effect(() => {
    const sel = elegida;
    if (!sel) return;
    if (!diasVisibles.some((d) => d.vs.some((v) => v.id === sel.id)) && enVista.some((v) => v.id === sel.id)) todos = true;
    void tick().then(() => document.getElementById(`version-${sel.id}`)?.scrollIntoView({ block: "nearest" }));
  });

  // En vivo: lo que llega después de abrir la página se ilumina y se anuncia.
  let vistas: Set<string> | null = null;
  let nuevas = $state(new Set<string>());
  let anuncio = $state("");
  const fmtHora = new Intl.DateTimeFormat("es", { hour: "2-digit", minute: "2-digit" });
  $effect(() => {
    const ids = lista.map((v) => v.id);
    if (!vistas) {
      if (ids.length) vistas = new Set(ids);
      return;
    }
    const llegan = lista.filter((v) => !vistas!.has(v.id));
    if (!llegan.length) return;
    for (const v of llegan) vistas.add(v.id);
    nuevas = new Set([...nuevas, ...llegan.map((v) => v.id)]);
    anuncio = llegan.length === 1 ? `Nueva versión de las ${fmtHora.format(llegan[0].t)}.` : `${llegan.length} versiones nuevas.`;
  });

  const nombreRango = $derived(rango === 365 ? "el último año" : `los últimos ${rango} días`);
  const resumen = $derived(
    cal.total
      ? `${plural(cal.total, "versión", "versiones")} en ${nombreRango}, ${plural(cal.diasCon, "día", "días")} con alguna${motivos ? `; la próxima retención quitaría ${numero(cal.quitan)}` : ""}.`
      : `Ninguna versión en ${nombreRango}.`,
  );
</script>

{#if lista.length}
  <div class="linea" class:movil bind:clientWidth={ancho}>
    {#if conCalendario}
      <div class="herr">
        <div class="segmented inline" role="group" aria-label="Periodo">
          {#each RANGOS as r (r.dias)}
            <button type="button" class:on={rango === r.dias} aria-pressed={rango === r.dias} onclick={() => cambiarRango(r.dias)}>{r.texto}</button>
          {/each}
        </div>
        <p class="resumen num" id="{id}-resumen">{resumen}</p>
      </div>

      <div class="calor">
        <CalendarioCalor
          {cal}
          {filtro}
          elegida={elegida?.t ?? null}
          alFiltrar={filtrar}
          descrito="{id}-resumen"
          etiqueta="{etiqueta}: calendario de {nombreRango}{cal.modo === 'horas' ? ', un día por columna y las horas en filas' : cal.modo === 'dias' ? ', una semana por columna' : ', un cuadro por día'}. Flechas para moverse; Intro muestra sus versiones abajo."
        />
        <div class="pie-cal">
          <div class="leyenda" aria-hidden="true">
            <span class="l escala">Menos<span class="sw n0"></span><span class="sw n1"></span><span class="sw n2"></span><span class="sw n3"></span><span class="sw n4"></span>Más</span>
            {#if motivos}<span class="l"><span class="sw raya"></span>Todas las quitaría la retención</span>{/if}
            <span class="l"><span class="sw ahora"></span>Ahora</span>
          </div>
          <!-- Lo mismo que las casillas, con un control de tamaño cómodo (WCAG 2.5.8). -->
          <label class="elegir-dia">
            <span class="sr-only">Ver las versiones de un día</span>
            <select class="input" value={diaFiltro ?? ""} onchange={(e) => ((hora = null), ponerDia(e.currentTarget.value || null))}>
              <option value="">Todos los días</option>
              {#if diaFiltro && !diasConVersiones.some((d) => d.clave === diaFiltro)}<option value={diaFiltro}>{filtro?.texto ?? diaFiltro}</option>{/if}
              {#each diasConVersiones as d (d.clave)}<option value={d.clave}>{d.texto}</option>{/each}
            </select>
          </label>
        </div>
      </div>
    {/if}

    <section class="bit" class:sin-cal={!conCalendario} aria-labelledby="{id}-bit">
      <div class="bit-cab">
        <h3 class="bit-titulo" id="{id}-bit">
          {#if filtro}<span class="first">{filtro.texto}</span>{:else}Las últimas{/if}
          <span class="faint num">· {plural(enVista.length, "versión", "versiones")}</span>
        </h3>
        {#if filtro}
          <button type="button" class="btn btn-sm btn-ghost" onclick={quitarFiltro}><X size={14} />Ver todas</button>
        {/if}
      </div>
      {#if enVista.length}
        <Bitacora
          dias={diasVisibles}
          ahora={minuto}
          {motivos}
          {hueco}
          {nombreCopia}
          {seleccion}
          {alElegir}
          {acciones}
          reciente={acciones ? null : (lista[0]?.id ?? null)}
          {nuevas}
          etiqueta={filtro ? `Versiones de ${filtro.texto}` : `Versiones de ${nombreRango}`}
        />
        {#if diasVisibles.length < dias.length}
          <button type="button" class="btn btn-sm btn-ghost mas" onclick={() => (todos = true)}>Ver {plural(dias.length - diasVisibles.length, "día más", "días más")}</button>
        {/if}
      {:else}
        <p class="faint vacio">{filtro ? "Ninguna versión en ese momento. Prueba con otra casilla o con el día entero." : `Ninguna versión en ${nombreRango}: elige un periodo más largo.`}</p>
      {/if}
    </section>

    <span class="sr-only" aria-live="polite" aria-atomic="true">{anuncio}</span>

    <div class="leyenda pie">
      {#if leyenda.length > 1 || leyenda[0]?.hueco !== 3}
        {#each leyenda as l (l.hueco + l.texto)}<span class="l"><FormaCopia hueco={l.hueco} />{l.texto}</span>{/each}
      {/if}
      {#if regla}
        <span class="l"><FormaCopia hueco={leyenda[0]?.hueco ?? 0} quita tamano={10} />La quitará la próxima retención · {numero(quitaria)}</span>
        <span class="regla">Simulado con la retención {quien ? `que aplica ${quien}` : "del repositorio"}: {textoRegla(regla)}.</span>
      {/if}
    </div>
  </div>
{/if}

<style>
  .linea {
    /* La escala de calor del calendario: un solo tono (el azul secuencial
     * validado de dataviz), de claro a oscuro; en oscuro, al revés (lo de
     * más, lo más claro). Las vacías, la pista `--surface-3`. Contraste en
     * docs/diseno.md §7. */
    --calor-0: var(--surface-3);
    --calor-1: #9ec5f4;
    --calor-2: #5598e7;
    --calor-3: #256abf;
    --calor-4: #0d366b;
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    min-width: 0;
  }
  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-theme="light"])) .linea {
      --calor-1: #184f95;
      --calor-2: #2a78d6;
      --calor-3: #6da7ec;
      --calor-4: #b7d3f6;
    }
  }
  :global(:root[data-theme="dark"]) .linea,
  :global(:root[data-theme="black"]) .linea {
    --calor-1: #184f95;
    --calor-2: #2a78d6;
    --calor-3: #6da7ec;
    --calor-4: #b7d3f6;
  }
  .herr {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2) var(--sp-4);
  }
  .resumen {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .calor {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }
  .bit {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .bit-cab {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
  }
  .bit-titulo {
    margin: 0;
    font-size: var(--fs-h2);
    font-weight: 600;
  }
  .bit-titulo .first {
    display: inline-block;
  }
  .bit-titulo .first::first-letter {
    text-transform: uppercase;
  }
  .bit-titulo .faint {
    font-weight: 400;
    font-size: var(--fs-sm);
  }
  .bit.sin-cal {
    padding-top: 0;
    border-top: 0;
  }
  .pie-cal {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 6px var(--sp-3);
  }
  .elegir-dia .input {
    width: auto;
    max-width: min(100%, 16rem);
    height: 28px;
    padding: 0 8px;
    font-size: var(--fs-xs);
  }
  .mas {
    align-self: flex-start;
  }
  .vacio {
    margin: 0;
    font-size: var(--fs-sm);
  }
  /* En el móvil, la bitácora primero; el calendario, una tira que se desliza debajo del periodo. */
  .movil .herr {
    order: 0;
  }
  .movil .calor {
    order: 1;
  }
  .movil .bit {
    order: 2;
    padding-top: 0;
    border-top: 0;
  }
  .movil .pie {
    order: 3;
  }
  .movil .calor .leyenda {
    display: none;
  }
  .movil .elegir-dia,
  .movil .elegir-dia .input {
    width: 100%;
    max-width: none;
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
  .escala {
    gap: 3px;
    color: var(--text-3);
  }
  .escala .sw:first-of-type {
    margin-left: 4px;
  }
  .escala .sw:last-of-type {
    margin-right: 4px;
  }
  .sw {
    display: inline-block;
    width: 11px;
    height: 11px;
    border-radius: 3px;
  }
  .sw.n0 {
    background: var(--calor-0);
  }
  .sw.n1 {
    background: var(--calor-1);
  }
  .sw.n2 {
    background: var(--calor-2);
  }
  .sw.n3 {
    background: var(--calor-3);
  }
  .sw.n4 {
    background: var(--calor-4);
  }
  .sw.raya {
    background: repeating-linear-gradient(135deg, color-mix(in srgb, var(--text-3) 55%, transparent) 0 1.5px, var(--calor-0) 1.5px 4px);
  }
  .sw.ahora {
    background: var(--calor-0);
    box-shadow: inset 0 0 0 1.5px var(--text-2);
  }
  .regla {
    flex-basis: 100%;
    color: var(--text-3);
  }
</style>
