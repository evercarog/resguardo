<script lang="ts">
  // La «bitácora» (docs/diseno.md §4, «Historial y versiones»): una lista
  // vertical por días con un riel a la izquierda. Cada versión es una fila:
  // su marca (color y forma de su copia, hueca si la próxima retención la
  // quitaría), la hora, lo nuevo en una píldora mono («+154 MB»), la copia y
  // si se conserva. Los demás sucesos (una copia que falló o no guardó nada,
  // una comprobación, una subida a la nube…) van en el mismo riel como filas
  // compactas: el icono del estado en el riel, la hora, qué fue y su estado
  // en palabra (en chip si fue mal), y debajo el motivo. Con `acciones` (el
  // repositorio), pulsar una versión la abre y debajo salen sus acciones; sin
  // ellas (Restaurar), pulsar es elegirla. Un suceso con su vuelta en el
  // informe abre su detalle. Las filas que se pulsan son botones de verdad
  // (Tab) y las flechas ↑↓ van de una a otra.
  import type { Snippet } from "svelte";
  import { fade } from "svelte/transition";
  import { ArchiveRestore, ArrowRightLeft, ChevronRight, CircleAlert, CircleCheck, CircleDashed, CloudUpload, Copy, HardDrive, Info, RefreshCw, ShieldCheck, TriangleAlert, Wrench } from "@lucide/svelte";
  import { dur } from "$ui/movimiento";
  import { bytes, plural } from "$lib/formato";
  import { NOMBRE_MOTIVO, nombreDia, type VersionLinea } from "$lib/lineaTiempo";
  import type { NotaVersion, Suceso, TipoSuceso } from "$lib/historial";
  import { NOMBRE_GANCHO } from "$lib/ganchos";
  import type { Periodo } from "$lib/retencion";
  import type { Tono } from "$lib/salud";
  import FormaCopia from "./FormaCopia.svelte";

  type V = VersionLinea & { t: number };
  export type FilaBitacora = { k: "v"; t: number; v: V } | { k: "s"; t: number; s: Suceso };
  interface Props {
    dias: { dia: number; filas: FilaBitacora[] }[];
    ahora: number;
    motivos: Map<string, Periodo | null> | null;
    hueco: (copia: string | null | undefined) => 0 | 1 | 2 | 3;
    nombreCopia: (copia: string | null | undefined, repo?: string | null) => string;
    seleccion: string | null;
    alElegir: (id: string) => void;
    acciones?: Snippet<[string]>;
    /** La más reciente de todas (lleva su distintivo). */
    reciente?: string | null;
    /** Las que han llegado mientras se miraba (ids de versión o claves de suceso; se iluminan un momento). */
    nuevas: Set<string>;
    etiqueta: string;
    /** Lo que se dice de cada versión aparte (avisos de su vuelta, pasos previos). */
    notas?: Map<string, NotaVersion>;
    /** Abrir el detalle de la vuelta de un suceso. */
    alAbrirSuceso?: (s: Suceso) => void;
  }
  let { dias, ahora, motivos, hueco, nombreCopia, seleccion, alElegir, acciones, reciente = null, nuevas, etiqueta, notas, alAbrirSuceso }: Props = $props();

  const fmtHora = new Intl.DateTimeFormat("es", { hour: "2-digit", minute: "2-digit" });
  const fmtLargo = new Intl.DateTimeFormat("es", { weekday: "long", day: "numeric", month: "long", hour: "2-digit", minute: "2-digit" });
  const quita = (id: string) => !!motivos && motivos.get(id) === null;
  function estado(id: string): string | null {
    if (!motivos) return null;
    const m = motivos.get(id);
    return m ? `se conserva · ${NOMBRE_MOTIVO[m]}` : "la quitará la retención";
  }
  /** Lo que se avisa de una versión: su vuelta con avisos o un paso previo que no fue bien. */
  function avisoDe(id: string): { tono: Tono; texto: string; motivo: string | null } | null {
    const n = notas?.get(id);
    if (!n) return null;
    const mal = n.ganchos.find((g) => g.estado !== "ok");
    if (mal) return { tono: mal.estado === "fallo" ? "bad" : "warn", texto: `${NOMBRE_GANCHO[mal.tipo] ?? mal.tipo}: ${mal.estado === "fallo" ? "falló" : "con avisos"}`, motivo: mal.mensaje };
    if (n.tono) return { tono: n.tono, texto: "Con avisos", motivo: n.texto };
    return null;
  }
  function describir(v: V): string {
    const a = avisoDe(v.id);
    return [
      fmtLargo.format(v.t),
      v.anadido != null ? `${bytes(v.anadido)} nuevos` : v.bytes != null ? bytes(v.bytes) : null,
      v.archivos != null ? plural(v.archivos, "archivo", "archivos") : null,
      `copia «${nombreCopia(v.copia, v.repo)}»`,
      a ? `${a.texto}${a.motivo ? `: ${a.motivo}` : ""}` : null,
      estado(v.id),
      v.id === reciente ? "la más reciente" : null,
      ...(v.etiquetas ?? []),
    ]
      .filter(Boolean)
      .join(", ");
  }
  const describirSuceso = (s: Suceso) => [fmtLargo.format(s.t), s.titulo, s.chip, s.meta, s.detalle, alAbrirSuceso && s.vuelta ? "ver el detalle" : null].filter(Boolean).join(", ");

  const ICONO_TIPO: Record<TipoSuceso, typeof RefreshCw> = {
    copia: RefreshCw,
    sin_cambios: RefreshCw,
    fallo: RefreshCw,
    resumen: Copy,
    gancho: Wrench,
    verificacion: ShieldCheck,
    prueba: ArchiveRestore,
    externa: CloudUpload,
    espejo: HardDrive,
    aviso: Info,
    historial: ArrowRightLeft,
  };
  const ICONO_TONO: Record<Tono, typeof RefreshCw> = { ok: CircleCheck, warn: TriangleAlert, bad: CircleAlert, info: Info, paused: CircleDashed, neutral: CircleDashed };
  /** Lo que fue bien va en tinta tranquila (icono del estado y palabra); lo que no, en chip. */
  const enChip = (t: Tono) => t === "bad" || t === "warn";

  function cuentaDia(filas: FilaBitacora[]): string {
    const vs = filas.filter((f) => f.k === "v") as { v: V }[];
    const ss = filas.filter((f) => f.k === "s") as { s: Suceso }[];
    const mal = ss.filter((f) => f.s.tono === "bad").length;
    const quitan = vs.filter((f) => quita(f.v.id)).length;
    const otros = ss.length - mal;
    return [
      vs.length ? plural(vs.length, "versión", "versiones") : null,
      mal ? (mal === 1 ? "1 fallo" : `${mal} fallos`) : null,
      otros && !vs.length && !mal ? plural(otros, "suceso", "sucesos") : null,
      quitan ? `la retención quitará ${quitan}` : null,
    ]
      .filter(Boolean)
      .join(" · ");
  }

  let lista = $state<HTMLOListElement | null>(null);
  function flechas(ev: KeyboardEvent) {
    if (ev.key !== "ArrowDown" && ev.key !== "ArrowUp") return;
    const filas = [...(lista?.querySelectorAll<HTMLElement>(".fv") ?? [])];
    const i = filas.indexOf(ev.target as HTMLElement);
    if (i < 0) return;
    ev.preventDefault();
    filas[Math.max(0, Math.min(filas.length - 1, i + (ev.key === "ArrowDown" ? 1 : -1)))]?.focus();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<ol class="bitacora" aria-label={etiqueta} bind:this={lista} onkeydown={flechas}>
  {#each dias as d (d.dia)}
    <li class="dia" in:fade={{ duration: dur(140) }}>
      <h3 class="dia-cab">
        <span class="dia-nombre">{nombreDia(d.dia, ahora)}</span>
        <span class="dia-cuenta num">{cuentaDia(d.filas)}</span>
      </h3>
      <ol class="riel">
        {#each d.filas as f (f.k === "v" ? `v|${f.v.repo ?? ""}|${f.v.id}` : f.s.clave)}
          {#if f.k === "v"}
            {@const v = f.v}
            {@const q = quita(v.id)}
            {@const elegida = v.id === seleccion}
            {@const a = avisoDe(v.id)}
            <li class="v" id="version-{v.id}" class:elegida class:quita={q} class:nueva={nuevas.has(v.id)}>
              <span class="nodo"><FormaCopia hueco={hueco(v.copia)} quita={q} tamano={10} /></span>
              <button
                type="button"
                class="fv"
                aria-label={describir(v)}
                aria-expanded={acciones ? elegida : undefined}
                aria-current={elegida ? "true" : undefined}
                onclick={() => alElegir(v.id)}
              >
                <span class="hora" aria-hidden="true">{fmtHora.format(v.t)}</span>
                {#if v.anadido != null}<span class="pildora" aria-hidden="true">+{bytes(v.anadido)}</span>{:else if v.bytes != null}<span class="pildora" aria-hidden="true">{bytes(v.bytes)}</span>{/if}
                <span class="copia" aria-hidden="true">{nombreCopia(v.copia, v.repo)}</span>
                {#if a}
                  {@const IconoA = ICONO_TONO[a.tono]}
                  <span class="badge badge-sm tone-{a.tono}" aria-hidden="true"><IconoA size={12} />{a.texto}</span>
                {/if}
                {#if v.id === reciente}<span class="badge badge-sm tone-accent" aria-hidden="true">La más reciente</span>{/if}
                {#each v.etiquetas ?? [] as t (t)}<span class="badge badge-sm tone-info" aria-hidden="true">{t}</span>{/each}
                <span class="hueco"></span>
                {#if v.archivos != null}<span class="dato num" aria-hidden="true">{plural(v.archivos, "archivo", "archivos")}</span>{/if}
                {#if estado(v.id)}<span class="estado" aria-hidden="true">{estado(v.id)}</span>{/if}
                <ChevronRight size={15} class="flecha" aria-hidden="true" />
              </button>
              {#if acciones && elegida}
                <div class="acciones">{@render acciones(v.id)}</div>
              {/if}
            </li>
          {:else}
            {@const s = f.s}
            {@const IconoT = ICONO_TIPO[s.tipo]}
            {@const IconoE = ICONO_TONO[s.tono]}
            {@const abre = !!(alAbrirSuceso && s.vuelta)}
            <li class="s tone-{s.tono}" class:nueva={nuevas.has(s.clave)}>
              <span class="nodo nodo-s" aria-hidden="true"><span class="marca-s"><IconoE size={12} /></span></span>
              {#snippet contenido()}
                <span class="hora">{fmtHora.format(s.t)}</span>
                <span class="tipo" aria-hidden="true"><IconoT size={14} /></span>
                <span class="titulo-s">{s.titulo}</span>
                {#if enChip(s.tono)}<span class="badge badge-sm tone-{s.tono}"><IconoE size={12} />{s.chip}</span>{:else}<span class="palabra">{s.chip}</span>{/if}
                <span class="hueco"></span>
                {#if s.meta}<span class="meta-s num">{s.meta}</span>{/if}
                {#if abre}<ChevronRight size={15} class="flecha" aria-hidden="true" />{/if}
                {#if s.detalle}<span class="motivo">{s.detalle}</span>{/if}
              {/snippet}
              {#if abre}
                <button type="button" class="fs fv" aria-label={describirSuceso(s)} onclick={() => alAbrirSuceso!(s)}>{@render contenido()}</button>
              {:else}
                <div class="fs">{@render contenido()}</div>
              {/if}
            </li>
          {/if}
        {/each}
      </ol>
    </li>
  {/each}
</ol>

<style>
  .bitacora,
  .riel {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .bitacora {
    /* Las filas se adaptan al ancho de la bitácora (no al de la ventana). */
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .dia-cab {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 10px;
    margin: 0 0 6px;
    font-size: var(--fs-sm);
    font-weight: 600;
    letter-spacing: -0.005em;
    color: var(--text-1);
  }
  .dia-nombre::first-letter {
    text-transform: uppercase;
  }
  .dia-cuenta {
    font-size: var(--fs-xs);
    font-weight: 400;
    color: var(--text-3);
  }
  /* El riel: una raya fina a la izquierda que se desvanece al final del día; cada fila, su marca encima. */
  .riel {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding-left: 24px;
  }
  .riel::before {
    content: "";
    position: absolute;
    top: 8px;
    bottom: 8px;
    left: 8px;
    width: 2px;
    border-radius: 2px;
    background: linear-gradient(to bottom, var(--border-strong), var(--border) 70%, color-mix(in srgb, var(--border) 40%, transparent));
  }
  .v,
  .s {
    position: relative;
  }
  .nodo {
    position: absolute;
    top: 0;
    left: -24px;
    display: grid;
    place-items: center;
    width: 18px;
    height: 36px;
    pointer-events: none;
  }
  /* Un aro del fondo separa la marca del riel. */
  .nodo :global(.forma) {
    box-shadow: 0 0 0 2.5px var(--surface);
  }
  .marca-s {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    border-radius: 999px;
    background: var(--surface);
    box-shadow: 0 0 0 2px var(--surface);
  }
  .tone-ok .marca-s {
    color: var(--ok);
  }
  .tone-warn .marca-s {
    color: var(--warn);
  }
  .tone-bad .marca-s {
    color: var(--bad);
  }
  .tone-neutral .marca-s,
  .tone-info .marca-s,
  .tone-paused .marca-s {
    color: var(--text-3);
  }
  .fv,
  .fs {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    column-gap: 10px;
    width: 100%;
    min-height: 36px;
    padding: 4px 10px;
    font: inherit;
    font-size: var(--fs-sm);
    text-align: left;
    color: var(--text-1);
    background: none;
    border: 1px solid transparent;
    border-radius: var(--radius);
  }
  .fv {
    cursor: pointer;
  }
  .fv:hover {
    background: var(--surface-2);
  }
  .fv:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 0;
  }
  .elegida > .fv {
    background: var(--accent-soft);
    border-color: color-mix(in srgb, var(--accent) 35%, transparent);
  }
  /* La hora: una columna fija, monoespaciada y tabular. */
  .hora {
    flex: none;
    width: 44px;
    font-family: var(--mono);
    font-size: 12.5px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.01em;
  }
  .quita .hora,
  .s .hora {
    font-weight: 500;
    color: var(--text-2);
  }
  .pildora {
    flex: none;
    padding: 0 7px;
    font-family: var(--mono);
    font-size: 11.5px;
    line-height: 18px;
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
    white-space: nowrap;
  }
  .copia,
  .titulo-s {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-2);
  }
  .tone-bad .titulo-s,
  .tone-warn .titulo-s {
    color: var(--text-1);
    font-weight: 500;
  }
  .tipo {
    display: grid;
    flex: none;
    color: var(--text-3);
  }
  .palabra {
    flex: none;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .hueco {
    flex: 1;
  }
  .dato,
  .estado,
  .meta-s {
    flex: none;
    font-size: var(--fs-xs);
    color: var(--text-3);
    white-space: nowrap;
  }
  /* El motivo de lo que fue mal: debajo, a todo lo ancho, alineado con el texto. */
  .motivo {
    flex-basis: 100%;
    padding: 0 0 2px 78px;
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    color: var(--text-2);
    overflow-wrap: anywhere;
  }
  .tone-bad .motivo {
    color: var(--bad);
  }
  .fv :global(.flecha) {
    flex: none;
    color: var(--text-3);
    opacity: 0;
  }
  .fv:hover :global(.flecha),
  .fv:focus-visible :global(.flecha) {
    opacity: 1;
  }
  .elegida > .fv :global(.flecha) {
    opacity: 1;
    rotate: 90deg;
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 8px 10px 6px;
  }
  @container (max-width: 600px) {
    .fv,
    .fs {
      row-gap: 2px;
      min-height: 44px;
    }
    .estado,
    .meta-s {
      flex-basis: 100%;
      order: 9;
      padding-left: 54px;
      white-space: normal;
    }
    .s .meta-s,
    .motivo {
      padding-left: 78px;
    }
    .motivo {
      order: 10;
    }
    /* Un suceso: el título puede ocupar dos líneas antes que irse solo a la suya. */
    .titulo-s {
      flex: 1 1 0;
      white-space: normal;
    }
    .s .hueco {
      display: none;
    }
    .dato {
      display: none;
    }
  }
  @media (prefers-reduced-motion: no-preference) {
    .nueva > .fv,
    .nueva > .fs {
      animation: llega 1.6s var(--ease) both;
    }
    .fv,
    .fv :global(.flecha) {
      transition:
        background-color var(--dur-fast) var(--ease),
        opacity var(--dur-fast) var(--ease),
        rotate var(--dur-fast) var(--ease);
    }
  }
  @keyframes llega {
    from {
      background: color-mix(in srgb, var(--accent) 22%, transparent);
    }
  }
</style>
