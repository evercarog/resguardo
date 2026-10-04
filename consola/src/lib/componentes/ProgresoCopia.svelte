<script lang="ts">
  import { tip } from "$lib/tooltip";
  // El progreso de algo en marcha en un equipo (v1.25): una copia o una tarea
  // larga (verificar, copia externa…). Barra con el porcentaje, qué está
  // haciendo, «X de Y archivos · A de B · velocidad · quedan ~N min» y cuánto
  // lleva. Entre dos noticias del equipo (cada 3–5 s) la barra avanza sola al
  // ritmo que lleva, sin pasarse; con «reducir movimiento», sin animaciones.
  // `compacto`: solo un chip «Copiando… 42 %».
  import { LoaderCircle } from "@lucide/svelte";
  import Anuncio from "./Anuncio.svelte";
  import type { TareaEnMarcha } from "$lib/tipos";
  import { pctPintado, pctVisible, pulso } from "$lib/progreso.svelte";
  import { cifrasTarea, porcentaje, textoCorto, textoDuracion, textoFase } from "$lib/textoProgreso";
  import { ritmoDe } from "$lib/progreso.svelte";
  import GraficaOndas from "$ui/componentes/GraficaOndas.svelte";
  import { porSegundoArchivos, serieArchivos, seriesBytes } from "$ui/ritmos";

  // `grafica`: las ondas en vivo (lectura y subida, y archivos por segundo) si el agente las manda (v1.36).
  let { tarea, equipo, compacto = false, titulo, grafica = true }: { tarea: TareaEnMarcha; equipo: string; compacto?: boolean; titulo?: string; grafica?: boolean } = $props();
  const ritmo = $derived(grafica && !compacto ? ritmoDe(equipo, tarea) : undefined);
  const ondas = $derived(seriesBytes(tarea.tipo, ritmo).filter((s) => s.puntos.length >= 2));
  const ondasArchivos = $derived(serieArchivos(ritmo).filter((s) => s.puntos.length >= 2));

  // El reloj y el avance entre noticias son los de progreso.svelte.ts: así este
  // número es el mismo que el de la barra lateral y el de las tarjetas.
  const ahora = $derived(pulso.ahora);
  const real = $derived(porcentaje(tarea));
  /** Lo que se pinta: el real más lo que habrá avanzado desde que llegó (como mucho 6 s y nunca el 100 %). */
  const pintado = $derived(pctPintado(equipo, tarea, ahora));
  const visible = $derived(pctVisible(equipo, tarea, ahora));
  const fase = $derived(textoFase(tarea));
  const cifras = $derived(cifrasTarea(tarea));
  const lleva = $derived(tarea.empezo ? textoDuracion((ahora - Date.parse(tarea.empezo)) / 1000) : null);
  /** Minutos sin noticias del equipo (si se quedó callado más de 2 min). */
  const callado = $derived.by(() => {
    const t = Date.parse(tarea.actualizado ?? "");
    if (!Number.isFinite(t) || tarea.fase === "antes_de_copiar") return null;
    const m = Math.floor((ahora - t) / 60_000);
    return m >= 2 ? m : null;
  });
  const nombre = $derived(titulo ?? tarea.nombre ?? "");
  /** Para lectores de pantalla, solo los hitos: cada fase nueva y cada cuarto (25, 50, 75 %), no cada noticia. */
  const hito = $derived([nombre, fase, real != null && real >= 25 ? `${Math.floor(real / 25) * 25} %` : null].filter(Boolean).join(": "));
  const texto = $derived(
    [fase, real != null ? `${real} %` : null, ...cifras].filter(Boolean).join(", "),
  );
</script>

{#if compacto}
  <!-- En pantallas estrechas, solo el porcentaje (el nombre del equipo no se come). -->
  <span class="badge badge-sm tone-info chip" use:tip={[nombre, fase, ...cifras].filter(Boolean).join(" · ")}>
    <LoaderCircle size={12} class="spin" aria-hidden="true" /><span class="largo">{textoCorto(tarea, visible)}</span><span class="corto" aria-hidden="true">{visible != null ? `${visible} %` : "…"}</span>
  </span>
{:else}
  <div class="progreso">
    <Anuncio texto={hito} />
    <div class="cab">
      <LoaderCircle size={14} class="spin" aria-hidden="true" />
      <span class="fase">{fase}{#if nombre && titulo}<span class="faint"> · {nombre}</span>{/if}</span>
      {#if pintado != null}<strong class="pct num" aria-hidden="true">{visible} %</strong>{/if}
    </div>
    <div
      class="barra"
      class:indeterminada={pintado == null}
      role="progressbar"
      aria-label={nombre ? `Progreso de «${nombre}»` : "Progreso"}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={real ?? undefined}
      aria-valuetext={texto}
    >
      <span style:width={pintado == null ? undefined : `${pintado}%`}></span>
    </div>
    {#if cifras.length || lleva}
      <p class="meta num">
        {#if cifras.length}<span>{cifras.join(" · ")}</span>{/if}
        {#if lleva}<span class="faint">lleva {lleva}</span>{/if}
      </p>
    {/if}
    {#if ondas.length}
      <GraficaOndas series={ondas} titulo={`Ritmo de «${nombre || fase}»`} alto={72} retraso={6000} />
      {#if ondasArchivos.length}<GraficaOndas series={ondasArchivos} titulo="Archivos por segundo" alto={36} retraso={6000} formato={porSegundoArchivos} />{/if}
    {/if}
    {#if callado}<p class="callado">Sin noticias del equipo desde hace {callado} min: puede estar esperando al almacén o haberse apagado.</p>{/if}
  </div>
{/if}

<style>
  .chip .corto {
    display: none;
  }
  @media (max-width: 640px) {
    .chip .largo {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }
    .chip .corto {
      display: inline;
    }
  }
  .progreso {
    display: grid;
    gap: 6px;
    min-width: 0;
  }
  .cab {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    font-size: var(--fs-sm);
    color: var(--text-1);
  }
  .cab :global(svg) {
    flex: none;
    color: var(--info);
  }
  .fase {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 550;
  }
  .pct {
    margin-left: auto;
    font-weight: 650;
  }
  .barra {
    position: relative;
    height: 8px;
    overflow: hidden;
    background: var(--surface-3);
    border-radius: 999px;
  }
  .barra span {
    position: absolute;
    inset: 0 auto 0 0;
    width: 0;
    background: var(--info);
    border-radius: inherit;
    transition: width 0.9s linear;
  }
  /* Sin porcentaje todavía: una franja que va y viene (quieta con «reducir movimiento»). */
  .barra.indeterminada span {
    width: 35%;
    animation: vaiven 1.6s ease-in-out infinite;
  }
  @keyframes vaiven {
    from {
      left: -35%;
    }
    to {
      left: 100%;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .barra span {
      transition: none;
    }
    .barra.indeterminada span {
      left: 0;
      width: 100%;
      opacity: 0.35;
      animation: none;
    }
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: 2px 12px;
    margin: 0;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .callado {
    margin: 0;
    font-size: var(--fs-xs);
    color: var(--warn);
  }
</style>
