<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Historial de una copia: cada vuelta de los últimos 60 días, agrupada por
  // día, con su resultado, lo que duró, lo que añadió y, si fue mal, el mensaje
  // del equipo. Los pasos «Antes de copiar» de la última vuelta van con ella.
  import { CircleAlert, CircleCheck, CircleDashed, Database, History, RefreshCw, RotateCcw, TriangleAlert } from "@lucide/svelte";
  import type { EjecucionInforme, ResultadoGancho, VersionInforme } from "$lib/tipos";
  import { NOMBRE_GANCHO } from "$lib/ganchos";
  import { bytes, dia, fechaLarga, hora, numero } from "$lib/formato";
  import { anadidoDe, duracion, TEXTO_RESULTADO, TONO_RESULTADO, versionDeVuelta } from "$lib/repo";
  import type { Tono } from "$lib/salud";
  import { claveDia } from "$lib/repo";
  import "../detalle/pulsable.css";

  let {
    ejecuciones,
    versiones,
    ganchos = [],
    cuandoGanchos = null,
    alAbrirVersion,
    alAbrirVuelta,
    dia: diaElegido = null,
    alDia,
  }: {
    ejecuciones: EjecucionInforme[];
    versiones: VersionInforme[];
    /** Resultado de los pasos «Antes de copiar» de la última vuelta (v1.10). */
    ganchos?: ResultadoGancho[];
    cuandoGanchos?: string | null;
    /** Abrir el detalle de la versión de una vuelta (con filtro: sus archivos nuevos o cambiados). */
    alAbrirVersion?: (id: string, filtro?: "nuevos" | "cambiados") => void;
    /** Abrir una vuelta sin versión (fallida, sin cambios). */
    alAbrirVuelta?: (hora: string) => void;
    /** Solo las vueltas de este día (`AAAA-MM-DD`). */
    dia?: string | null;
    alDia?: (k: string | null) => void;
  } = $props();

  interface Item {
    clave: string;
    hora: string;
    tono: Tono;
    icono: typeof History;
    titulo: string;
    chip: string;
    meta: string | null;
    detalle: string | null;
    version: string | null;
    vuelta: string | null;
    nuevos: number | null;
    cambiados: number | null;
  }

  const items = $derived.by(() => {
    // Lo de cada vuelta: de la propia vuelta (v1.12) o de su versión (misma hora).
    const out: Item[] = ejecuciones.map((e) => {
      const v = versionDeVuelta(versiones, e);
      const dur = e.duracion_s ?? v?.duracion_s ?? null;
      const anadido = v ? anadidoDe(v) : (e.anadido ?? null);
      const nuevos = e.archivos_nuevos ?? v?.archivos_nuevos ?? null;
      const cambiados = e.archivos_cambiados ?? v?.archivos_cambiados ?? null;
      const meta = [
        dur != null ? duracion(dur) : null,
        anadido != null && e.resultado !== "sin_cambios" && e.resultado !== "fallo" ? `+${bytes(anadido)}` : null,
        nuevos != null && cambiados != null && (nuevos || cambiados) ? `${numero(nuevos)} nuevos · ${numero(cambiados)} cambiados` : null,
        e.reintento ? "reintento" : null,
      ].filter(Boolean);
      return {
        clave: `e|${e.hora}`,
        hora: e.hora,
        tono: TONO_RESULTADO[e.resultado],
        icono: e.reintento ? RotateCcw : RefreshCw,
        titulo: e.resultado === "sin_cambios" ? "Sin cambios: no hizo falta una versión nueva" : e.resultado === "fallo" ? "La copia falló" : v ? `Versión ${v.id}` : "Copia hecha",
        chip: TEXTO_RESULTADO[e.resultado],
        meta: meta.join(" · ") || null,
        detalle: e.resultado === "ok" || e.resultado === "sin_cambios" ? null : (e.mensaje_corto ?? null),
        version: v?.id ?? null,
        vuelta: e.hora,
        nuevos,
        cambiados,
      };
    });
    if (cuandoGanchos)
      for (const [i, g] of ganchos.entries())
        out.push({
          clave: `g|${i}|${cuandoGanchos}`,
          hora: cuandoGanchos,
          tono: g.estado === "ok" ? "ok" : g.estado === "aviso" ? "warn" : "bad",
          icono: Database,
          titulo: NOMBRE_GANCHO[g.tipo] ?? g.tipo,
          chip: g.estado === "ok" ? "Correcto" : g.estado === "aviso" ? "Con avisos" : "Falló",
          meta: "antes de copiar",
          detalle: g.mensaje,
          version: null,
          vuelta: null,
          nuevos: null,
          cambiados: null,
        });
    return out.sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora));
  });

  let soloProblemas = $state(false);
  const PASO = 25;
  let limite = $state(PASO);
  const delDia = $derived(diaElegido ? items.filter((i) => claveDia(new Date(i.hora)) === diaElegido) : items);
  const vistos = $derived(soloProblemas ? delDia.filter((i) => i.tono === "bad" || i.tono === "warn") : delDia);
  const abrir = (i: Item) => (i.version && alAbrirVersion ? alAbrirVersion(i.version) : i.vuelta && alAbrirVuelta ? alAbrirVuelta(i.vuelta) : undefined);
  const fmtDia = new Intl.DateTimeFormat("es", { weekday: "long", day: "numeric", month: "long" });
  const grupos = $derived.by(() => {
    const out: { dia: string; items: Item[] }[] = [];
    for (const i of vistos.slice(0, limite)) {
      const d = dia(i.hora);
      if (out.at(-1)?.dia === d) out.at(-1)!.items.push(i);
      else out.push({ dia: d, items: [i] });
    }
    return out;
  });
  const ICONO_TONO = { ok: CircleCheck, warn: TriangleAlert, bad: CircleAlert, info: CircleCheck, paused: CircleDashed, neutral: CircleDashed };
</script>

<section class="card historia" aria-labelledby="t-historial-copia">
  <div class="cab">
    <h2 class="section-title" id="t-historial-copia">Historial <span class="count">· {diaElegido ? `${numero(delDia.length)} el ${fmtDia.format(new Date(`${diaElegido}T12:00:00`))}` : `${numero(ejecuciones.length)} vueltas en 60 días`}</span></h2>
    {#if diaElegido && alDia}<button class="btn btn-sm btn-ghost" onclick={() => alDia(null)}>Ver todos los días</button>{/if}
    <div class="segmented inline" role="group" aria-label="Qué mostrar">
      <button class:on={!soloProblemas} aria-pressed={!soloProblemas} onclick={() => ((soloProblemas = false), (limite = PASO))}>Todo</button>
      <button class:on={soloProblemas} aria-pressed={soloProblemas} onclick={() => ((soloProblemas = true), (limite = PASO))}>Problemas</button>
    </div>
  </div>

  {#if !vistos.length}
    <div class="empty-state">
      <History size={28} strokeWidth={1.5} />
      <p>{soloProblemas ? "Sin fallos ni avisos en estos 60 días." : "Todavía no hay vueltas que contar: aparecerán aquí en cuanto se haga la primera."}</p>
    </div>
  {:else}
    {#each grupos as g (g.dia)}
      <h3 class="overline dia">{g.dia}</h3>
      <ul class="lista">
        {#each g.items as i (i.clave)}
          {@const Icono = i.icono}
          {@const IconoTono = ICONO_TONO[i.tono]}
          <li class="fila-h tone-{i.tono}">
            <span class="ic" aria-hidden="true"><Icono size={16} /></span>
            <div class="principal">
              <div class="linea">
                {#if (i.version && alAbrirVersion) || (i.vuelta && alAbrirVuelta)}
                  <button class="pulsable titulo" use:tip={"Ver detalle"} onclick={() => abrir(i)}>{i.titulo}</button>
                {:else}<strong>{i.titulo}</strong>{/if}
                <span class="badge badge-sm tone-{i.tono}"><IconoTono size={12} />{i.chip}</span>
              </div>
              {#if i.meta}<span class="faint meta num">{i.meta}</span>{/if}
              {#if i.version && alAbrirVersion && (i.nuevos || i.cambiados)}
                <span class="faint meta num">
                  Ver los archivos:
                  {#if i.nuevos}<button class="pulsable" onclick={() => alAbrirVersion(i.version!, "nuevos")}>{numero(i.nuevos)} nuevos</button>{/if}
                  {#if i.nuevos && i.cambiados}·{/if}
                  {#if i.cambiados}<button class="pulsable" onclick={() => alAbrirVersion(i.version!, "cambiados")}>{numero(i.cambiados)} cambiados</button>{/if}
                </span>
              {/if}
              {#if i.detalle}<span class="detalle">{i.detalle}</span>{/if}
            </div>
            <span class="cuando num" use:tip={fechaLarga(i.hora)}>{hora(i.hora)}</span>
          </li>
        {/each}
      </ul>
    {/each}
    {#if vistos.length > limite}
      <button class="btn btn-ghost mas" onclick={() => (limite += PASO)}>Mostrar más ({numero(vistos.length - limite)} más)</button>
    {/if}
  {/if}
</section>

<style>
  .historia {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-5);
  }
  .cab {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
  }
  .count {
    font-weight: 400;
    color: var(--text-3);
  }
  .dia {
    margin: var(--sp-3) 0 var(--sp-1);
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .lista {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .fila-h {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr) auto;
    align-items: start;
    gap: var(--sp-3);
    min-height: 44px;
    padding: 10px 0;
    border-top: 1px solid var(--border);
  }
  .ic {
    display: grid;
    padding-top: 2px;
    color: var(--text-3);
  }
  .principal {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .linea {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  .linea strong,
  .linea .titulo {
    font-weight: 500;
  }
  .meta,
  .detalle {
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
    overflow-wrap: anywhere;
  }
  .detalle {
    color: var(--text-2);
  }
  .tone-bad .detalle {
    color: var(--bad);
  }
  .cuando {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .mas {
    align-self: center;
  }
</style>
