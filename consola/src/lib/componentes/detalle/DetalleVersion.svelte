<script lang="ts">
  import EtiquetaVersion from "$lib/componentes/EtiquetaVersion.svelte";
  // Una versión en detalle: cuándo, de qué copia, cuánto duró y añadió, los
  // archivos nuevos, cambiados y sin cambios (cada recuento abre «Qué cambió»
  // con ese filtro), las vueltas que la dejaron (con reintentos y errores), los
  // pasos «Antes de copiar» y lo que se puede hacer con ella.
  import { ArrowLeft, ArrowRight, Check, Copy, FolderSearch, GitCompareArrows, History, PieChart, TriangleAlert } from "@lucide/svelte";
  import type { CopiaResumen, EjecucionInforme, EntradaHistorial, VersionInforme } from "$lib/tipos";
  import { anteriorDeLaCopia, ganchosDeVuelta, vueltasDeVersion } from "$lib/detalle";
  import { anadidoDe, duracion, TEXTO_RESULTADO, TONO_RESULTADO } from "$lib/repo";
  import { explicarError } from "$lib/copia";
  import { NOMBRE_GANCHO } from "$lib/ganchos";
  import { bytes, fechaLarga, hora, numero, plural } from "$lib/formato";
  import { tip } from "$lib/tooltip";
  import Chip from "../Chip.svelte";
  import { abrirVersion, abrirVuelta, ir } from "./navegar";
  import "./pulsable.css";

  let {
    version: v,
    versiones,
    ejecuciones,
    historial,
    copias,
    repo,
    enlaceCopia,
    enlaceRestaurar,
    puedeRestaurar,
  }: {
    version: VersionInforme;
    versiones: VersionInforme[];
    ejecuciones: EjecucionInforme[];
    historial: EntradaHistorial[];
    copias: CopiaResumen[];
    repo: string;
    enlaceCopia: (copia: string) => string;
    enlaceRestaurar: (v: VersionInforme, todo: boolean) => string;
    puedeRestaurar: boolean;
  } = $props();

  const copia = $derived(copias.find((k) => k.id === v.copia) ?? null);
  const anterior = $derived(anteriorDeLaCopia(versiones, v));
  const siguiente = $derived(versiones.filter((x) => x.copia === v.copia && Date.parse(x.hora) > Date.parse(v.hora)).sort((a, b) => Date.parse(a.hora) - Date.parse(b.hora))[0] ?? null);
  const vueltas = $derived(vueltasDeVersion(ejecuciones, versiones, v));
  const propia = $derived(vueltas.find((e) => e.resultado === "ok" || e.resultado === "aviso") ?? vueltas[0] ?? null);
  const ganchos = $derived(ganchosDeVuelta(historial, propia, repo));
  const delta = $derived(v.total_bytes != null && anterior?.total_bytes != null ? v.total_bytes - anterior.total_bytes : null);
  const errores = $derived(vueltas.filter((e) => e.resultado === "fallo" || e.resultado === "aviso"));

  let copiado = $state(false);
  async function copiarId() {
    try {
      await navigator.clipboard.writeText(v.id);
      copiado = true;
      setTimeout(() => (copiado = false), 1400);
    } catch {
      /* sin portapapeles */
    }
  }
  const fmtDelta = (d: number) => (Math.abs(d) < 1 ? "igual que la anterior" : `${d > 0 ? "+" : "−"}${bytes(Math.abs(d))} que la anterior`);
</script>

<div class="detalle">
  <p class="sub">
    {#if copia}<a class="link" href={enlaceCopia(copia.id)}>{copia.nombre}</a>{:else}<span>Copia</span>{/if}
    · <span class="id-wrap"><code class="selectable">{v.id}</code><button class="icon-btn mini" use:tip={"Copiar el id"} aria-label="Copiar el id de la versión" onclick={copiarId}>{#if copiado}<Check size={12} />{:else}<Copy size={12} />{/if}</button></span>
    {#each v.etiquetas ?? [] as t (t)}<EtiquetaVersion nombre={t} />{/each}
  </p>

  <dl class="cifras">
    <div>
      <dt>Duración</dt>
      <dd class="num">{duracion(v.duracion_s)}</dd>
    </div>
    <div>
      <dt>Datos añadidos</dt>
      <dd class="num" use:tip={"Lo nuevo que ocupó en el repositorio (comprimido y sin duplicados)"}>{bytes(anadidoDe(v))}</dd>
      {#if v.anadido != null && v.anadido_empaquetado != null && v.anadido !== v.anadido_empaquetado}<span class="faint">{bytes(v.anadido)} sin comprimir</span>{/if}
    </div>
    <div>
      <dt>Tamaño de lo copiado</dt>
      <dd class="num">{bytes(v.total_bytes)}</dd>
      {#if delta != null}<span class="faint">{fmtDelta(delta)}</span>{/if}
    </div>
  </dl>

  <section aria-labelledby="t-archivos">
    <h3 id="t-archivos">Archivos</h3>
    {#if v.archivos_nuevos == null && v.archivos_cambiados == null}
      <p class="faint">El equipo no contó los archivos de esta versión (agente anterior).</p>
    {:else}
      <ul class="recuentos">
        <li><button class="pulsable" use:tip={"Ver qué archivos son nuevos"} onclick={() => abrirVersion(v.id, "nuevos")}><strong class="num">{numero(v.archivos_nuevos ?? 0)}</strong> nuevos</button></li>
        <li><button class="pulsable" use:tip={"Ver qué archivos cambiaron"} onclick={() => abrirVersion(v.id, "cambiados")}><strong class="num">{numero(v.archivos_cambiados ?? 0)}</strong> cambiados</button></li>
        <li><span><strong class="num">{numero(v.archivos_sin_cambios ?? 0)}</strong> sin cambios</span></li>
        <li><button class="pulsable" use:tip={"Ver qué se borró desde la versión anterior"} onclick={() => abrirVersion(v.id, "borrados")}>borrados</button></li>
      </ul>
    {/if}
    <div class="acciones">
      <button class="btn btn-primary" onclick={() => ir({ vista: "cambios", version: v.id, con: null, filtro: "todos" })}><GitCompareArrows size={15} />Qué cambió</button>
      <button class="btn" onclick={() => ir({ vista: "ocupa", version: v.id })}><PieChart size={15} />Lo que más ocupa</button>
      {#if puedeRestaurar}
        <a class="btn btn-ghost" href={enlaceRestaurar(v, false)}><FolderSearch size={15} />Explorar</a>
        <a class="btn btn-ghost" href={enlaceRestaurar(v, true)}><History size={15} />Restaurar entera</a>
      {/if}
    </div>
  </section>

  <section aria-labelledby="t-vueltas">
    <h3 id="t-vueltas">Cómo se hizo</h3>
    {#if !vueltas.length}
      <p class="faint">El informe no trae la copia que la guardó (puede ser anterior a estos 60 días o de un agente anterior).</p>
    {:else}
      {#if errores.length}<p class="aviso"><TriangleAlert size={14} />{plural(errores.length, "intento con problemas", "intentos con problemas")} antes de guardarla.</p>{/if}
      <ul class="vueltas">
        {#each vueltas as e (e.hora)}
          <li>
            <button class="pulsable-bloque fila-vuelta" use:tip={"Ver detalle"} onclick={() => abrirVuelta(e.hora)}>
              <span class="num hora">{hora(e.hora)}</span>
              <Chip pequeno tono={TONO_RESULTADO[e.resultado]} texto={TEXTO_RESULTADO[e.resultado]} />
              {#if e.reintento}<span class="badge badge-sm tone-neutral">reintento</span>{/if}
              <span class="faint meta num">{[e.duracion_s != null ? duracion(e.duracion_s) : null, e.anadido != null && e.resultado !== "fallo" ? `+${bytes(e.anadido)}` : null].filter(Boolean).join(" · ")}</span>
              {#if e.mensaje_corto && e.resultado !== "ok"}<span class="msg">{explicarError(e.mensaje_corto).titulo}: «{e.mensaje_corto}»</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
    {#if ganchos.length}
      <h4>Antes de copiar</h4>
      <ul class="ganchos">
        {#each ganchos as g, i (i)}
          <li>
            <Chip pequeno tono={g.estado === "ok" ? "ok" : g.estado === "aviso" ? "warn" : "bad"} texto={NOMBRE_GANCHO[g.tipo] ?? g.tipo} />
            {#if g.mensaje}<span class="faint">{g.mensaje}</span>{/if}
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <nav class="vecinas" aria-label="Otras versiones de esta copia">
    {#if anterior}<button class="btn btn-sm btn-ghost" onclick={() => abrirVersion(anterior!.id)}><ArrowLeft size={14} />Anterior <span class="faint">{fechaLarga(anterior.hora)}</span></button>{:else}<span></span>{/if}
    {#if siguiente}<button class="btn btn-sm btn-ghost" onclick={() => abrirVersion(siguiente!.id)}>Siguiente <span class="faint">{fechaLarga(siguiente.hora)}</span><ArrowRight size={14} /></button>{/if}
  </nav>
</div>

<style>
  .detalle {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
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
  .id-wrap {
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }
  .mini {
    width: 24px;
    height: 24px;
  }
  .cifras {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-3);
    margin: 0;
  }
  .cifras > div {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--sp-3);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }
  dt {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  dd {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
  }
  .cifras .faint {
    font-size: var(--fs-xs);
  }
  h3 {
    margin: 0 0 var(--sp-2);
    font-size: var(--fs-sm);
    font-weight: 600;
  }
  h4 {
    margin: var(--sp-3) 0 6px;
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-3);
  }
  .recuentos {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2) var(--sp-5);
    margin: 0 0 var(--sp-3);
    padding: 0;
    list-style: none;
    font-size: var(--fs-sm);
  }
  .recuentos strong {
    font-size: 16px;
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .aviso {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 0 6px;
    font-size: var(--fs-sm);
    color: var(--warn);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .vueltas li + li {
    border-top: 1px solid var(--border);
  }
  .fila-vuelta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
    padding: 8px 4px;
    border-radius: var(--radius-sm);
    font-size: var(--fs-sm);
  }
  .fila-vuelta:hover {
    background: var(--bg-subtle);
  }
  .hora {
    font-weight: 500;
  }
  .meta {
    font-size: var(--fs-xs);
  }
  .msg {
    flex-basis: 100%;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .ganchos li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 4px 0;
    font-size: var(--fs-xs);
  }
  .vecinas {
    display: flex;
    justify-content: space-between;
    gap: var(--sp-2);
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  @media (max-width: 560px) {
    .cifras {
      grid-template-columns: minmax(0, 1fr);
    }
    .vecinas .faint {
      display: none;
    }
  }
</style>
