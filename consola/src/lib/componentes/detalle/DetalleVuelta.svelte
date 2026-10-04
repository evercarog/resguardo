<script lang="ts">
  // Una vuelta de una copia (también las que fallaron y no dejaron versión):
  // resultado, qué dijo el equipo y qué significa, cuánto duró y añadió, la
  // versión que dejó y los demás problemas de esa copia ese día.
  import { ArrowRight, CircleHelp } from "@lucide/svelte";
  import type { CopiaResumen, EjecucionInforme, EntradaHistorial, VersionInforme } from "$lib/tipos";
  import { ganchosDeVuelta } from "$lib/detalle";
  import { anadidoDe, claveDia, duracion, TEXTO_RESULTADO, TONO_RESULTADO, versionDeVuelta } from "$lib/repo";
  import { explicarError } from "$lib/copia";
  import { NOMBRE_GANCHO } from "$lib/ganchos";
  import { bytes, fechaLarga, hora, numero } from "$lib/formato";
  import { tip } from "$lib/tooltip";
  import Chip from "../Chip.svelte";
  import { abrirVersion, abrirVuelta } from "./navegar";
  import "./pulsable.css";

  let {
    vuelta: e,
    versiones,
    ejecuciones,
    historial,
    copias,
    repo,
    enlaceCopia,
  }: {
    vuelta: EjecucionInforme;
    versiones: VersionInforme[];
    ejecuciones: EjecucionInforme[];
    historial: EntradaHistorial[];
    copias: CopiaResumen[];
    repo: string;
    enlaceCopia: (copia: string) => string;
  } = $props();

  const copia = $derived(copias.find((k) => k.id === e.copia) ?? null);
  const v = $derived(versionDeVuelta(versiones, e));
  const problema = $derived(e.resultado === "fallo" || e.resultado === "aviso");
  const exp = $derived(problema ? explicarError(e.mensaje_corto) : null);
  const ganchos = $derived(ganchosDeVuelta(historial, e, repo));
  const mismoDia = $derived(ejecuciones.filter((x) => x !== e && x.copia === e.copia && claveDia(new Date(x.hora)) === claveDia(new Date(e.hora))).sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora)));
  const nuevos = $derived(e.archivos_nuevos ?? v?.archivos_nuevos ?? null);
  const cambiados = $derived(e.archivos_cambiados ?? v?.archivos_cambiados ?? null);
</script>

<div class="vuelta">
  <p class="sub">
    <Chip tono={TONO_RESULTADO[e.resultado]} texto={TEXTO_RESULTADO[e.resultado]} />
    {#if e.reintento}<span class="badge badge-sm tone-neutral">reintento</span>{/if}
    <span>{fechaLarga(e.hora)}</span>
    {#if copia}· <a class="link" href={enlaceCopia(copia.id)}>{copia.nombre}</a>{/if}
  </p>

  {#if exp}
    <div class="explicacion tone-{e.resultado === 'fallo' ? 'bad' : 'warn'}">
      <strong>{exp.titulo}</strong>
      <p>{exp.texto}</p>
      {#if e.mensaje_corto}<p class="faint">El equipo dijo: «{e.mensaje_corto.replace(/\.$/, "")}»</p>{/if}
      <a class="btn btn-sm btn-ghost" href="/ayuda#{exp.ayuda}"><CircleHelp size={14} />¿Qué hago?</a>
    </div>
  {:else if e.resultado === "sin_cambios"}
    <p class="faint">Revisó las carpetas y nada había cambiado: no hizo falta guardar una versión nueva.</p>
  {/if}

  <dl class="datos">
    <div><dt>Duración</dt><dd class="num">{duracion(e.duracion_s ?? v?.duracion_s)}</dd></div>
    <div><dt>Añadió</dt><dd class="num">{e.resultado === "fallo" ? "—" : bytes(v ? anadidoDe(v) : (e.anadido ?? null))}</dd></div>
    <div>
      <dt>Archivos</dt>
      <dd class="num">
        {#if nuevos == null && cambiados == null}—
        {:else if v}<button class="pulsable" use:tip={"Ver cuáles"} onclick={() => abrirVersion(v!.id, "nuevos")}>{numero(nuevos ?? 0)} nuevos</button> · <button class="pulsable" use:tip={"Ver cuáles"} onclick={() => abrirVersion(v!.id, "cambiados")}>{numero(cambiados ?? 0)} cambiados</button>
        {:else}{numero(nuevos ?? 0)} nuevos · {numero(cambiados ?? 0)} cambiados{/if}
      </dd>
    </div>
  </dl>

  {#if v}
    <button class="btn" onclick={() => abrirVersion(v!.id)}>Ver la versión que dejó <code>{v.id}</code><ArrowRight size={14} /></button>
  {/if}

  {#if ganchos.length}
    <section>
      <h3>Antes de copiar</h3>
      <ul class="ganchos">
        {#each ganchos as g, i (i)}
          <li><Chip pequeno tono={g.estado === "ok" ? "ok" : g.estado === "aviso" ? "warn" : "bad"} texto={NOMBRE_GANCHO[g.tipo] ?? g.tipo} />{#if g.mensaje}<span class="faint">{g.mensaje}</span>{/if}</li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if mismoDia.length}
    <section>
      <h3>Ese día, la misma copia</h3>
      <ul class="otras">
        {#each mismoDia as x (x.hora)}
          <li>
            <button class="pulsable-bloque otra" use:tip={"Ver detalle"} onclick={() => abrirVuelta(x.hora)}>
              <span class="num">{hora(x.hora)}</span>
              <Chip pequeno tono={TONO_RESULTADO[x.resultado]} texto={TEXTO_RESULTADO[x.resultado]} />
              {#if x.mensaje_corto && x.resultado !== "ok"}<span class="faint msg">{x.mensaje_corto}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    </section>
  {/if}
</div>

<style>
  .vuelta {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-4);
  }
  .sub {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .explicacion {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    width: 100%;
    padding: var(--sp-3) var(--sp-4);
    border-left: 3px solid var(--c, var(--warn));
    background: color-mix(in srgb, var(--c, var(--warn)) var(--soft), transparent);
    border-radius: var(--radius);
  }
  .explicacion.tone-bad {
    --c: var(--bad);
  }
  .explicacion.tone-warn {
    --c: var(--warn);
  }
  .explicacion p {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .datos {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-3) var(--sp-6);
    margin: 0;
  }
  dt {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  dd {
    margin: 0;
    font-size: var(--fs-body);
    font-weight: 500;
  }
  section {
    width: 100%;
  }
  h3 {
    margin: 0 0 6px;
    font-size: var(--fs-sm);
    font-weight: 600;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .ganchos li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 4px 0;
    font-size: var(--fs-xs);
  }
  .otra {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 6px 4px;
    border-top: 1px solid var(--border);
    font-size: var(--fs-sm);
  }
  .otra:hover {
    background: var(--bg-subtle);
  }
  .msg {
    font-size: var(--fs-xs);
  }
</style>
