<script lang="ts">
  // La tira «3 · 2 · 1 · 1 · 0» de una copia (tarea 8c, docs/regla-3-2-1.md):
  // cada parte de la regla 3-2-1-1-0, cumplida o no, con qué hacer para
  // cumplirla y adónde ir. Guía, nunca obligación: no bloquea nada.
  // `compacta`: solo las cinco cifras (listas, «Cambiar las copias»).
  import { Check, CircleDashed, Clock, Info, ShieldCheck, TriangleAlert, X } from "@lucide/svelte";
  import { tip } from "$lib/tooltip";
  import Ayuda from "../Ayuda.svelte";
  import { relativo } from "$lib/formato";
  import { cifraParte, fraseParte, fraseRegla, pasoAlDia, PARTES, queHacer, TEXTO_AVISO, TEXTO_INMUTABLE, TEXTO_LUGAR, type PasoVista, type ReglaCopia } from "$lib/regla321";
  import { guardar, leer } from "$lib/recordar";

  let {
    rc,
    cliente,
    ahora,
    compacta = false,
    titulo = true,
    onmarcar,
  }: { rc: ReglaCopia; cliente: string; ahora: number; compacta?: boolean; titulo?: boolean; onmarcar?: (p: PasoVista) => void } = $props();

  const r = $derived(rc.regla);
  const tono = (cumple: boolean, config: boolean) => (cumple ? "ok" : config ? "warn" : "neutral");
  const estado = $derived(r.cumple ? { tono: "ok", texto: "Cumple" } : r.dejo_de_cumplir ? { tono: "warn", texto: "Dejó de cumplir" } : { tono: "neutral", texto: "No la cumple" });
  const pendientes = $derived(r.partes.filter((p) => !p.cumple).map((p) => ({ p, que: queHacer(p, rc, cliente, ahora) })));
  const hayLocal = $derived(rc.pasos.some((p) => p.inmutable === "instantaneas" || p.inmutable === "desconectado"));
  // El enlace discreto a la guía del almacén inmutable (8e), que se puede ocultar (en este navegador).
  let guiaOculta = $state(leer("regla321.guia", ["oculta", ""] as const, "") === "oculta");
  function ocultarGuia() {
    guiaOculta = true;
    guardar("regla321.guia", "oculta");
  }
</script>

{#if compacta}
  <span class="tira-mini" role="img" aria-label="Regla 3-2-1-1-0: {fraseRegla(r)}" use:tip={fraseRegla(r)}>
    {#each r.partes as p (p.id)}
      <span class="mini tone-{tono(p.cumple, p.cumple_config)}" class:cumple={p.cumple}>{PARTES[p.id].cifra}</span>
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
        {@const t = tono(p.cumple, p.cumple_config)}
        <li class="parte tone-{t}" class:cumple={p.cumple}>
          <span class="num-parte">{PARTES[p.id].cifra}</span>
          <span class="nombre">{PARTES[p.id].titulo.replace(/^\d+ /, "")}</span>
          <span class="valor">
            {#if p.cumple}<Check size={12} aria-hidden="true" />{:else if p.cumple_config}<Clock size={12} aria-hidden="true" />{:else}<X size={12} aria-hidden="true" />{/if}
            {cifraParte(p)}<span class="sr-only">{p.cumple ? ": cumple" : p.cumple_config ? ": no está al día" : ": falta"}</span>
          </span>
          <span class="frase">{fraseParte(p, rc, ahora)}</span>
        </li>
      {/each}
    </ol>

    {#if pendientes.length}
      <ul class="hacer">
        {#each pendientes as { p, que } (p.id)}
          {#if que}
            <li>
              <span class="qh-parte tone-{tono(p.cumple, p.cumple_config)}">{PARTES[p.id].titulo}</span>
              <span>{que.texto}{#if que.enlace}{" "}<a class="link" href={que.enlace.href}>{que.enlace.texto} →</a>{/if}</span>
            </li>
          {/if}
        {/each}
      </ul>
      <p class="faint guia">Es una guía, no una obligación: la copia funciona igual aunque no la cumpla.</p>
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
            <li class="paso">
              <span class="p-nombre">{p.nombre}</span>
              <span class="faint">{TEXTO_LUGAR[p.lugar]} · {TEXTO_INMUTABLE[p.inmutable].replace(/ \(.*\)$/, "")}{#if p.marcado}{" "}<span class="badge badge-sm tone-neutral" use:tip={"Lo marcó una persona en el destino (no se deduce del tipo)."}>Marcado</span>{/if}{#if p.sistemaArchivos}{" · "}<span class="pastilla mono">{p.sistemaArchivos}</span>{/if}</span>
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
  .tira {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: var(--sp-2);
  }
  .parte {
    display: grid;
    gap: 2px;
    align-content: start;
    padding: var(--sp-2) var(--sp-3);
    border: 1px solid var(--border);
    border-top: 3px solid var(--tone);
    border-radius: var(--radius-sm);
    background: var(--surface);
    min-width: 0;
  }
  .parte.cumple {
    background: color-mix(in oklab, var(--ok) 6%, var(--surface));
  }
  .num-parte {
    font-family: var(--font-display);
    font-size: 1.75rem;
    line-height: 1;
    font-weight: 650;
    color: var(--tone);
    font-variant-numeric: tabular-nums;
  }
  .nombre {
    font-weight: 600;
    font-size: var(--fs-sm);
    color: var(--text-1);
  }
  .valor {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .valor :global(svg) {
    color: var(--tone);
  }
  .frase {
    font-size: var(--fs-xs);
    color: var(--text-3);
    overflow-wrap: anywhere;
  }
  .hacer {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  .hacer li {
    display: flex;
    gap: var(--sp-2);
    align-items: baseline;
    font-size: var(--fs-sm);
  }
  .qh-parte {
    flex: none;
    font-weight: 600;
    color: var(--tone);
    min-width: 9.5em;
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
  .tira-mini {
    display: inline-flex;
    gap: 2px;
    vertical-align: middle;
  }
  .mini {
    display: inline-grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    color: var(--tone);
    border: 1px solid color-mix(in oklab, var(--tone) 45%, transparent);
    background: transparent;
  }
  .mini.cumple {
    color: var(--accent-contrast, #fff);
    background: var(--tone);
    border-color: var(--tone);
  }
  @media (max-width: 720px) {
    .tira {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }
  @media (max-width: 480px) {
    /* En el móvil, una fila por parte: la cifra a la izquierda y lo demás al lado. */
    .tira {
      grid-template-columns: 1fr;
      gap: 6px;
    }
    .parte {
      grid-template-columns: 2.2rem minmax(0, 1fr);
      column-gap: var(--sp-2);
      border-top-width: 1px;
      border-left: 3px solid var(--tone);
    }
    .num-parte {
      grid-row: span 3;
      align-self: center;
      font-size: 1.5rem;
    }
    .hacer li {
      flex-direction: column;
      gap: 2px;
    }
    .qh-parte {
      min-width: 0;
    }
  }
</style>
