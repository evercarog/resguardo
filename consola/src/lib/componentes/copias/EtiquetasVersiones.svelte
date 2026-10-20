<script lang="ts">
  // 0.8.1 (15v): «Etiquetas de sus versiones» en el editor de una copia (`config.copias[].etiquetas`,
  // agente con `admite: "etiquetas_copia"`). Cada versión nueva de la copia las lleva; sin
  // ninguna, el agente pone `resguardo-agente`. Sirven para filtrar el historial y en los
  // filtros de las copias derivadas. Cambiarlas solo afecta a las versiones nuevas.
  import { Plus, Tag, X } from "@lucide/svelte";
  import { tip } from "$lib/tooltip";
  import EtiquetaVersion from "../EtiquetaVersion.svelte";
  import { limpiarEtiquetaVersion, MAX_ETIQUETAS_VERSION } from "$lib/copiaGuiada";

  let {
    id,
    etiquetas = $bindable([]),
    admite,
    version = null,
    sugeridas = [],
  }: {
    id: string;
    etiquetas?: string[];
    /** ¿Las entiende el agente? */
    admite: boolean;
    version?: string | null;
    /** Las que ya usan otras copias (para no escribir «Bases» y «bases»). */
    sugeridas?: string[];
  } = $props();

  let texto = $state("");
  let error = $state("");
  const libres = $derived(sugeridas.filter((s) => !etiquetas.includes(s)).slice(0, 8));

  function anadir(t: string) {
    error = "";
    const r = limpiarEtiquetaVersion(t);
    if ("error" in r) return void (error = r.error);
    if (etiquetas.includes(r.etiqueta)) return void (texto = "");
    if (etiquetas.length >= MAX_ETIQUETAS_VERSION) return void (error = `Como mucho ${MAX_ETIQUETAS_VERSION} etiquetas.`);
    etiquetas = [...etiquetas, r.etiqueta];
    texto = "";
  }
  function tecla(e: KeyboardEvent) {
    if ((e.key === "Enter" || e.key === "," || e.key === " ") && texto.trim()) {
      e.preventDefault();
      anadir(texto);
    }
  }
</script>

<div class="field et-versiones">
  <label class="field-label" for="{id}-et"><Tag size={14} />Etiquetas de sus versiones</label>
  {#if !admite}
    <p class="faint">Actualiza el agente{version ? ` (tiene la ${version})` : ""} para elegirlas. Mientras, sus versiones llevan «resguardo-agente».</p>
  {:else}
    {#if etiquetas.length}
      <ul class="puestas" aria-label="Etiquetas puestas">
        {#each etiquetas as t (t)}
          <li><EtiquetaVersion nombre={t} /><button type="button" class="icon-btn quitar" aria-label="Quitar la etiqueta {t}" use:tip={"Quitar"} onclick={() => (etiquetas = etiquetas.filter((x) => x !== t))}><X size={13} /></button></li>
        {/each}
      </ul>
    {/if}
    <div class="anadir">
      <input id="{id}-et" class="input mono" bind:value={texto} onkeydown={tecla} placeholder="Por ejemplo: bases" maxlength="40" spellcheck="false" autocomplete="off" aria-describedby="{id}-et-ayuda" aria-invalid={!!error} />
      <button type="button" class="btn btn-sm btn-ghost" disabled={!texto.trim()} onclick={() => anadir(texto)}><Plus size={14} />Añadir</button>
    </div>
    {#if libres.length}
      <div class="sugeridas">
        <span class="faint">Ya en uso:</span>
        {#each libres as s (s)}<button type="button" class="chip-sug" onclick={() => anadir(s)}><Plus size={11} />{s}</button>{/each}
      </div>
    {/if}
    {#if error}<span class="error-campo" role="alert">{error}</span>{/if}
    <span class="field-hint" id="{id}-et-ayuda">
      Cada versión nueva de esta copia las llevará: sirven para filtrar el historial y para elegir qué versiones van a una copia derivada. {etiquetas.length ? "" : "Sin ninguna, llevan «resguardo-agente». "}Cambiarlas solo afecta a las versiones nuevas: las que ya están se quedan como estaban.
    </span>
  {/if}
</div>

<style>
  .et-versiones .field-label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .puestas {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0 0 6px;
    padding: 0;
    list-style: none;
  }
  .puestas li {
    display: inline-flex;
    align-items: center;
    gap: 0;
  }
  .quitar {
    width: 24px;
    height: 24px;
  }
  .anadir {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .anadir .input {
    flex: 1 1 180px;
    max-width: 280px;
    height: 28px;
    font-family: var(--mono);
    font-size: 12.5px;
  }
  .sugeridas {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
    font-size: var(--fs-xs);
  }
  .chip-sug {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: 24px;
    padding: 0 8px;
    font: inherit;
    font-size: var(--fs-xs);
    color: var(--text-2);
    background: var(--surface);
    border: 1px dashed var(--border-strong);
    border-radius: 999px;
    cursor: pointer;
  }
  .chip-sug:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
