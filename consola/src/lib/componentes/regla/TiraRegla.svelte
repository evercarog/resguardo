<script lang="ts">
  // La tira «3 · 2 · 1 · 1 · 0» de una copia (tarea 8c, docs/regla-3-2-1.md;
  // diseño en docs/diseno.md §4): cinco segmentos iguales, cada uno con su
  // cifra grande, su nombre corto debajo («copias», «soportes», «fuera»,
  // «inmutable», «errores») y su estado con icono, color y palabra («Cumple»,
  // «Atrasado», «Falta 1»); el globo dice qué pide, cómo está y qué hacer.
  // Debajo, una línea corta con lo que falta y adónde ir. Guía, nunca
  // obligación: no bloquea nada.
  // `compacta`: la misma tira en una píldora de cinco segmentos (listas,
  // «Cambiar las copias», Estado): cifra e icono de estado en cada uno.
  import { Check, CircleDashed, Clock, Info, ShieldCheck, TriangleAlert, X } from "@lucide/svelte";
  import { tip } from "$lib/tooltip";
  import Ayuda from "../Ayuda.svelte";
  import { relativo } from "$lib/formato";
  import { cifraParte, estadoParte, fraseRegla, globoParte, lineaFalta, pasoAlDia, PARTES, queHacer, TEXTO_AVISO, type PasoVista, type ReglaCopia } from "$lib/regla321";
  import { guardar, leer } from "$lib/recordar";
  import { corta, estadoConexion } from "$lib/tipoDestino";
  import TipoDestino from "../TipoDestino.svelte";

  let {
    rc,
    cliente,
    ahora,
    compacta = false,
    titulo = true,
    onmarcar,
    enlaceDestino,
  }: {
    rc: ReglaCopia;
    cliente: string;
    ahora: number;
    compacta?: boolean;
    titulo?: boolean;
    onmarcar?: (p: PasoVista) => void;
    /** La página de un destino de la lista «Dónde llegan los datos» (si la tiene). */
    enlaceDestino?: (p: PasoVista) => string | null;
  } = $props();

  const r = $derived(rc.regla);
  const estado = $derived(r.cumple ? { tono: "ok", texto: "Cumple" } : r.dejo_de_cumplir ? { tono: "warn", texto: "Dejó de cumplir" } : { tono: "neutral", texto: "No la cumple" });
  // Lo que falta, con su enlace (la línea corta de debajo); el texto largo, en el globo de cada parte.
  const pendientes = $derived(r.partes.filter((p) => !p.cumple).map((p) => ({ p, que: queHacer(p, rc, cliente, ahora) })));
  const falta = $derived(lineaFalta(r));
  // Un enlace por sitio (dos partes que se arreglan en el mismo sitio, una vez).
  const enlaces = $derived([...new Map(pendientes.flatMap(({ que }) => (que?.enlace ? [[que.enlace.href, que.enlace] as const] : []))).values()]);
  const hayLocal = $derived(rc.pasos.some((p) => p.inmutable === "instantaneas" || p.inmutable === "desconectado"));
  // El enlace discreto a la guía del almacén inmutable (8e), que se puede ocultar (en este navegador).
  let guiaOculta = $state(leer("regla321.guia", ["oculta", ""] as const, "") === "oculta");
  function ocultarGuia() {
    guiaOculta = true;
    guardar("regla321.guia", "oculta");
  }
</script>

{#snippet icono(i: "cumple" | "atrasado" | "falta", t: number)}
  {#if i === "cumple"}<Check size={t} aria-hidden="true" />{:else if i === "atrasado"}<Clock size={t} aria-hidden="true" />{:else}<X size={t} aria-hidden="true" />{/if}
{/snippet}

{#if compacta}
  <span class="tira-mini" role="img" aria-label="Regla 3-2-1-1-0: {fraseRegla(r)}" use:tip={fraseRegla(r)}>
    {#each r.partes as p (p.id)}
      {@const e = estadoParte(p)}
      <span class="mini tone-{e.tono} e-{e.icono}"><span class="m-cifra">{PARTES[p.id].cifra}</span>{@render icono(e.icono, 10)}</span>
    {/each}
  </span>
{:else}
  <section class="card p regla" aria-labelledby="t-regla-{rc.copia.id}">
    {#if titulo}
      <div class="cab">
        <h2 class="section-title" id="t-regla-{rc.copia.id}"><ShieldCheck size={16} />Regla 3-2-1-1-0 <Ayuda id="regla-321" /></h2>
        <span class="badge badge-sm tone-{estado.tono}">{#if r.cumple}<Check size={11} />{:else if r.dejo_de_cumplir}<Clock size={11} />{:else}<CircleDashed size={11} />{/if}{estado.texto}</span>
      </div>
    {/if}
    <ol class="tira" aria-label="Las cinco partes de la regla">
      {#each r.partes as p (p.id)}
        {@const e = estadoParte(p)}
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <li class="parte tone-{e.tono} e-{e.icono}" tabindex="0" use:tip={globoParte(p, rc, cliente, ahora)}>
          <span class="r-cifra" aria-hidden="true">{PARTES[p.id].cifra}</span>
          <span class="nombre">{PARTES[p.id].corto}</span>
          <span class="estado">{@render icono(e.icono, 12)}<span>{e.texto}</span></span>
          <span class="sr-only">: {PARTES[p.id].titulo}, {cifraParte(p)}.</span>
        </li>
      {/each}
    </ol>

    {#if falta}
      <p class="falta">
        <span>{falta}</span>
        {#each enlaces as e (e.href)}
          <a class="link" href={e.href}>{e.texto} →</a>
        {/each}
      </p>
      <p class="faint guia">Es una guía, no una obligación. Pasa el ratón o toca cada parte para ver qué pide y qué hacer.</p>
    {/if}
{#each r.avisos as a (a)}
      <p class="aviso"><TriangleAlert size={14} />{TEXTO_AVISO[a] ?? a}</p>
    {/each}

    {#if rc.pasos.length}
      <details class="pasos">
        <summary>Dónde llegan los datos · {rc.pasos.length + 1} copias contando los originales</summary>
        <ul>
          <li class="paso"><span class="p-nombre">Los originales</span><span class="faint">en {rc.equipo.nombre}</span></li>
          {#each rc.pasos as p (p.id)}
            {@const al = pasoAlDia(p, ahora)}
            {@const href = enlaceDestino?.(p)}
            <li class="paso">
              {#if href}<a class="p-nombre link-suave" {href}>{p.nombre}</a>{:else}<span class="p-nombre">{p.nombre}</span>{/if}
              <span class="faint p-tipo"><TipoDestino {...corta(p.clasificacion)} />{#if p.clasificacion.tipoPorPersona || p.clasificacion.marcasPorPersona}<span class="badge badge-sm tone-neutral" use:tip={"Lo marcó una persona en el destino (no se deduce)."}>Marcado por una persona</span>{/if}{#if p.clasificacion.aislado}{@const cx = estadoConexion(p.conexion, p.clasificacion.aisladoDias, ahora)}<span class:tarde={cx.tarde}>{cx.texto}</span>{/if}{#if p.sistemaArchivos}<span class="pastilla mono">{p.sistemaArchivos}</span>{/if}</span>
              <span class="badge badge-sm tone-{al ? 'ok' : 'warn'}">{al ? "Al día" : p.ultima_ok ? `Atrasado · ${relativo(p.ultima_ok, ahora)}` : "No está al día"}</span>
              {#if onmarcar && p.clave}<button class="btn btn-sm btn-ghost" onclick={() => onmarcar(p)}>Cambiar</button>{/if}
            </li>
          {/each}
        </ul>
      </details>
    {/if}

    {#if hayLocal || !guiaOculta}
      <p class="faint guia-almacen">
        <Info size={13} />
        <a class="link-suave" href="/ayuda#guia-almacen-inmutable">Cómo añadir instantáneas que el almacén no pueda borrar</a>
        {#if !guiaOculta}<button class="btn btn-sm btn-ghost ocultar" onclick={ocultarGuia} aria-label="Ocultar el enlace a la guía">Ocultar</button>{/if}
      </p>
    {/if}
  </section>
{/if}

<style>
  .regla {
    display: grid;
    gap: var(--sp-3);
  }
  .cab {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex-wrap: wrap;
    justify-content: space-between;
  }
  .cab h2 {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
  }
  /* Cinco segmentos iguales en una sola pieza: la cifra grande, el nombre corto y el estado. */
  .tira {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    overflow: hidden;
    background: var(--surface);
  }
  .parte {
    position: relative;
    display: grid;
    justify-items: center;
    align-content: center;
    gap: 2px;
    min-width: 0;
    padding: var(--sp-3) var(--sp-1) 14px;
    text-align: center;
    cursor: default;
  }
  .parte + .parte {
    border-left: 1px solid var(--border);
  }
  /* El estado, en una raya abajo (además del icono y la palabra). */
  .parte::after {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 3px;
    background: var(--tone);
    opacity: 0.85;
  }
  .parte.e-falta::after {
    background: repeating-linear-gradient(90deg, var(--border-strong) 0 6px, transparent 6px 10px);
    opacity: 1;
  }
  .parte.e-cumple {
    background: color-mix(in srgb, var(--ok) var(--soft), var(--surface));
  }
  .parte.e-atrasado {
    background: color-mix(in srgb, var(--warn) var(--soft), var(--surface));
  }
  .parte:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .r-cifra {
    font-size: var(--fs-display);
    line-height: 1;
    font-weight: 650;
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
    color: var(--tone);
  }
  .e-falta .r-cifra {
    color: var(--text-3);
  }
  .nombre {
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--text-1);
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .estado {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--tone);
    white-space: nowrap;
  }
  .e-falta .estado {
    color: var(--text-2);
  }
  .falta {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px var(--sp-3);
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .guia,
  .guia-almacen {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .guia-almacen {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .ocultar {
    padding: 0 6px;
  }
  .aviso {
    display: flex;
    gap: 6px;
    align-items: flex-start;
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .aviso :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--warn);
  }
  .pasos summary {
    cursor: pointer;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .pasos ul {
    list-style: none;
    margin: var(--sp-2) 0 0;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  .paso {
    display: flex;
    flex-wrap: wrap;
    gap: 4px var(--sp-2);
    align-items: center;
    font-size: var(--fs-sm);
    padding: 4px 0;
    border-bottom: 1px solid var(--border);
  }
  .p-nombre {
    font-weight: 600;
  }
  .p-tipo {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
  }
  .p-tipo .tarde {
    color: var(--warn);
  }

  /* La compacta: una píldora de cinco segmentos iguales, cifra e icono de estado. */
  .tira-mini {
    display: inline-grid;
    grid-template-columns: repeat(5, 26px);
    height: 22px;
    flex: none;
    vertical-align: middle;
    border: 1px solid var(--border);
    border-radius: 999px;
    overflow: hidden;
    background: var(--surface);
  }
  .mini {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 1px;
    font-size: 11.5px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
    color: var(--tone);
  }
  .mini + .mini {
    border-left: 1px solid var(--border);
  }
  .mini.e-cumple {
    background: color-mix(in srgb, var(--ok) var(--soft), transparent);
  }
  .mini.e-atrasado {
    background: color-mix(in srgb, var(--warn) var(--soft), transparent);
  }
  .mini.e-falta {
    color: var(--text-3);
  }
  .mini :global(svg) {
    flex: none;
  }

  @media (max-width: 480px) {
    /* En el móvil, las cinco siguen en fila (alineadas): cifra algo menor y sin aire de más. */
    .parte {
      padding: var(--sp-2) 2px 12px;
    }
    .r-cifra {
      font-size: var(--fs-title);
    }
    .nombre {
      font-size: 10.5px;
      letter-spacing: -0.01em;
    }
    /* El icono encima de la palabra: «Atrasado» cabe en un quinto de la pantalla. */
    .estado {
      flex-direction: column;
      gap: 1px;
      font-size: 11px;
    }
  }
</style>
