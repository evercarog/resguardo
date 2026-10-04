<script lang="ts">
  // De dónde sale el espacio: lo protegido (lo que ocupan los archivos), lo que
  // ocupa de verdad en disco (comprimido y sin duplicados) y qué versiones
  // añadieron más. Cada versión abre su detalle; «Lo que más ocupa» abre las
  // carpetas y archivos más grandes de la última.
  import { PieChart } from "@lucide/svelte";
  import type { RepoInforme, RepositorioResumen } from "$lib/tipos";
  import { anadidoDe, bytesRepo, ratioTexto, versionesDe } from "$lib/repo";
  import { bytes, fechaCorta, numero, relativo } from "$lib/formato";
  import { tip } from "$lib/tooltip";
  import { abrirVersion, ir } from "./navegar";
  import "./pulsable.css";

  let { repo, inf, ahora }: { repo: RepositorioResumen; inf: RepoInforme | null; ahora: number } = $props();

  const versiones = $derived(versionesDe(inf));
  const protegido = $derived(bytesRepo(repo, inf) ?? inf?.espacio?.sin_comprimir ?? null);
  const esp = $derived(inf?.espacio ?? null);
  const masAnadieron = $derived(
    versiones
      .filter((v) => anadidoDe(v) != null)
      .sort((a, b) => (anadidoDe(b) ?? 0) - (anadidoDe(a) ?? 0))
      .slice(0, 10),
  );
  const totalAnadido = $derived(versiones.reduce((n, v) => n + (anadidoDe(v) ?? 0), 0));
  const max = $derived(Math.max(1, ...masAnadieron.map((v) => anadidoDe(v) ?? 0)));
</script>

<div class="espacio">
  <dl class="cifras">
    <div>
      <dt>Protegido</dt>
      <dd class="num">{bytes(protegido)}</dd>
      <span class="faint">lo que ocupan tus archivos en la última versión</span>
    </div>
    <div>
      <dt>En disco</dt>
      <dd class="num">{bytes(esp?.en_disco_bytes)}</dd>
      <span class="faint">todas las versiones juntas, comprimidas y sin duplicados</span>
    </div>
    <div>
      <dt>Sin comprimir</dt>
      <dd class="num">{bytes(esp?.sin_comprimir)}</dd>
      <span class="faint">{esp?.ratio ? `${ratioTexto(esp.ratio)} menos gracias a la compresión` : "lo mismo, antes de comprimir"}</span>
    </div>
  </dl>
  {#if esp?.leido}<p class="faint pequeno">Espacio medido {relativo(esp.leido, ahora)} (el equipo lo mide una vez al día).</p>{/if}

  <section aria-labelledby="t-anadieron">
    <h3 id="t-anadieron">Las versiones que más añadieron <span class="faint">· 60 días, {bytes(totalAnadido)} en total</span></h3>
    {#if !masAnadieron.length}
      <p class="faint">El informe no trae lo que añadió cada versión.</p>
    {:else}
      <ol>
        {#each masAnadieron as v (v.id)}
          <li>
            <button class="pulsable-bloque fila" use:tip={"Ver detalle"} onclick={() => abrirVersion(v.id)}>
              <span class="num cuando">{fechaCorta(v.hora)}</span>
              <span class="barra" aria-hidden="true"><span style:width="{((anadidoDe(v) ?? 0) / max) * 100}%"></span></span>
              <span class="num tam">{bytes(anadidoDe(v))}</span>
              <span class="faint num arch">{v.archivos_nuevos != null ? `${numero(v.archivos_nuevos)} nuevos` : ""}</span>
            </button>
          </li>
        {/each}
      </ol>
    {/if}
  </section>

  {#if versiones[0]}
    <section>
      <h3>Qué ocupa dentro</h3>
      <p class="faint pequeno">Las carpetas y los archivos más grandes de la última versión ({fechaCorta(versiones[0].hora)}). Hace falta la contraseña del repositorio.</p>
      <button class="btn" onclick={() => ir({ vista: "ocupa", version: versiones[0].id })}><PieChart size={15} />Lo que más ocupa</button>
    </section>
  {/if}
</div>

<style>
  .espacio {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
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
  .cifras .faint,
  .pequeno {
    margin: 0;
    font-size: var(--fs-xs);
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
  h3 {
    margin: 0 0 var(--sp-2);
    font-size: var(--fs-sm);
    font-weight: 600;
  }
  h3 .faint {
    font-weight: 400;
  }
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .fila {
    display: grid;
    grid-template-columns: 9rem minmax(0, 1fr) 5.5rem 6rem;
    align-items: center;
    gap: var(--sp-2);
    min-height: 36px;
    padding: 4px;
    border-radius: var(--radius-sm);
    font-size: var(--fs-sm);
  }
  .fila:hover {
    background: var(--bg-subtle);
  }
  .barra {
    height: 6px;
    background: var(--border);
    border-radius: 3px;
    overflow: hidden;
  }
  .barra span {
    display: block;
    height: 100%;
    background: color-mix(in srgb, var(--text-3) 75%, transparent);
  }
  .fila:hover .barra span {
    background: var(--accent);
  }
  .tam {
    text-align: right;
  }
  .arch {
    font-size: var(--fs-xs);
  }
  section {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
  }
  section ol {
    width: 100%;
  }
  @media (max-width: 560px) {
    .cifras {
      grid-template-columns: minmax(0, 1fr);
    }
    .fila {
      grid-template-columns: minmax(0, 1fr) auto;
    }
    .barra,
    .arch {
      display: none;
    }
  }
</style>
