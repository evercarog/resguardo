<script lang="ts">
  // El camino de una copia a la vista (plan 0.8.0, C1; lib/camino.ts): cada
  // eslabón con el icono del tipo de su destino, su nombre (con enlace) y su
  // estado con icono y palabra (nunca solo color).
  // - «cabecera» (bajo el título de la copia): en una fila que se parte, con
  //   flechas y «después» donde toca.
  // - «lineas» (página «Copias», tarjeta del repositorio): una línea por paso,
  //   «→ Backblaze · al día, hace 2 h».
  // - `copiaEn` (tarjeta del repositorio, 14h): cada espejo como «su copia en
  //   <destino>», con lo que ocupa allí y su última comprobación.
  import { CircleAlert, CircleCheck, CircleDashed, CirclePause, LoaderCircle, TriangleAlert } from "@lucide/svelte";
  import { caminoEnTexto, textoEslabon, type Eslabon } from "$lib/camino";
  import { bytes, fechaLarga, relativo } from "$lib/formato";
  import { tip } from "$lib/tooltip";
  import TipoDestino from "./TipoDestino.svelte";

  let {
    eslabones,
    ahora,
    forma = "cabecera",
    etiqueta = "Camino de la copia",
    copiaEn = false,
  }: {
    eslabones: Eslabon[];
    ahora: number;
    forma?: "cabecera" | "lineas";
    /** El nombre de la lista para lectores de pantalla. */
    etiqueta?: string;
    /** 14h: los espejos como «su copia en <destino>», con su tamaño allí y su comprobación. */
    copiaEn?: boolean;
  } = $props();

  /** 14h: lo que se sabe de la copia en un espejo: «12 GB allí · comprobada hace 3 días». */
  const datosEspejo = (x: Eslabon) =>
    [
      x.ocupa != null ? `${bytes(x.ocupa)} allí` : null,
      x.verificacion ? (x.verificacion.mal ? `${x.verificacion.mal} archivos mal al comprobarla` : `comprobada ${relativo(x.verificacion.ultima, ahora)}`) : null,
    ].filter(Boolean);

  const ICONO = { ok: CircleCheck, warn: TriangleAlert, bad: CircleAlert, info: LoaderCircle, paused: CirclePause, neutral: CircleDashed };
  const QUE = { copia: "", espejo: "espejo", derivada: "copia derivada" } as const;
  const globo = (x: Eslabon) =>
    [x.clase === "espejo" ? `Espejo${x.quien ? ` (lo hace ${x.quien})` : ""}: el mismo repositorio en otro destino.` : x.clase === "derivada" ? "Copia derivada: sus versiones en otro repositorio." : "El repositorio donde guarda la copia.", x.despues ? "Empieza después de cada copia." : "", x.cuando ? fechaLarga(x.cuando) : ""].filter(Boolean).join(" ");
</script>

{#snippet estado(x: Eslabon)}
  {@const Icono = ICONO[x.tono]}
  <span class="est tone-{x.tono}"><Icono size={12} class={x.vivo ? "spin" : ""} aria-hidden="true" />{textoEslabon(x, ahora)}</span>
{/snippet}

{#snippet nombre(x: Eslabon)}
  {#if copiaEn && x.clase === "espejo"}
    <span class="nom"><TipoDestino {...x.tipo} soloIcono /><span>su copia en {#if x.href}<a class="link-suave" href={x.href}>{x.nombre}</a>{:else}{x.nombre}{/if}</span></span>
  {:else}
    <span class="nom"><TipoDestino {...x.tipo} soloIcono />{#if x.href}<a class="link-suave" href={x.href}>{x.nombre}</a>{:else}{x.nombre}{/if}{#if QUE[x.clase]}<span class="faint que">{QUE[x.clase]}</span>{/if}</span>
  {/if}
{/snippet}

{#if eslabones.length}
  {#if forma === "cabecera"}
    <ol class="camino-cab" aria-label="{etiqueta}: {caminoEnTexto(eslabones, ahora)}">
      {#each eslabones as x, i (x.id)}
        <li class="esl" use:tip={globo(x)}>
          {#if i}<span class="flecha faint" aria-hidden="true">{x.despues ? "→ después →" : "→"}</span>{/if}
          {@render nombre(x)}
          {@render estado(x)}
        </li>
      {/each}
    </ol>
  {:else}
    <ul class="camino-lin" aria-label={etiqueta}>
      {#each eslabones as x (x.id)}
        <li use:tip={globo(x)}>
          <span class="flecha faint" aria-hidden="true">→</span>
          <span class="cuerpo">{@render nombre(x)}<span class="faint sep" aria-hidden="true">{" · "}</span>{@render estado(x)}{#if copiaEn && x.clase === "espejo" && datosEspejo(x).length}<span class="faint datos">{" · "}{datosEspejo(x).join(" · ")}</span>{/if}</span>
        </li>
      {/each}
    </ul>
  {/if}
{/if}

<style>
  .camino-cab,
  .camino-lin {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .camino-cab {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
  }
  .camino-cab .esl {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    min-width: 0;
  }
  .camino-lin {
    display: grid;
    gap: 2px;
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
  }
  /* Una línea por paso: la flecha a la izquierda y el resto como texto que se parte (también en el móvil). */
  .camino-lin li {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 6px;
    align-items: baseline;
    min-width: 0;
  }
  .camino-lin .cuerpo {
    min-width: 0;
  }
  .camino-lin .nom {
    display: inline;
  }
  .camino-lin .nom :global(.td) {
    margin-right: 4px;
  }
  .camino-lin .que {
    margin-left: 5px;
  }
  .camino-lin .est {
    display: inline-flex;
    vertical-align: baseline;
  }
  .nom {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    overflow-wrap: anywhere;
    color: var(--text-1);
  }
  .que {
    font-size: var(--fs-xs);
    white-space: nowrap;
    overflow-wrap: normal;
  }
  .est {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--tone);
    font-size: var(--fs-xs);
    white-space: nowrap;
  }
  .est.tone-ok {
    color: var(--ok);
  }
  .est.tone-warn {
    color: var(--warn);
  }
  .est.tone-bad {
    color: var(--bad);
  }
  .est.tone-info {
    color: var(--info);
  }
  .est.tone-paused {
    color: var(--paused);
  }
  .est.tone-neutral {
    color: var(--text-3);
  }
  .flecha {
    white-space: nowrap;
  }
  .datos {
    font-size: var(--fs-xs);
  }
</style>
