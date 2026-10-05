<script lang="ts">
  // Campo «Observaciones» de los diálogos de crear o cambiar (equipo,
  // repositorio, copia, destino, cliente). Texto libre con Markdown ligero,
  // hasta 2000 caracteres; con «Vista previa». Quien abre el diálogo lo guarda
  // con `guardarObservacion` (lib/notas.svelte.ts) al guardar lo suyo.
  import { untrack } from "svelte";
  import { AVISO_SECRETOS, errorTextoNota, MAX_TEXTO } from "$lib/notas.svelte";
  import TextoNota from "./TextoNota.svelte";

  let {
    id,
    valor = $bindable(""),
    filas = 3,
    etiqueta = "Observaciones",
  }: { id: string; valor: string; filas?: number; etiqueta?: string } = $props();

  let vista = $state(false);
  const n = $derived([...valor.trim()].length);
  const error = $derived(errorTextoNota(valor));
  // Crece con lo que se escribe (hasta 12 filas), sin barra de desplazamiento al principio.
  const alto = $derived(Math.min(12, Math.max(untrack(() => filas), valor.split("\n").length + 1)));
</script>

<div class="field campo-obs">
  <div class="cab">
    <label class="field-label" for={id}>{etiqueta} <span class="faint opcional">(opcional)</span></label>
    {#if valor.trim()}
      <button type="button" class="btn btn-sm btn-ghost" aria-pressed={vista} onclick={() => (vista = !vista)}>{vista ? "Escribir" : "Vista previa"}</button>
    {/if}
  </div>
  {#if vista && valor.trim()}
    <div class="vista" aria-label="Vista previa"><TextoNota texto={valor} /></div>
  {:else}
    <textarea
      {id}
      class="input"
      rows={alto}
      bind:value={valor}
      maxlength={MAX_TEXTO + 200}
      placeholder="Lo que conviene saber: «el disco se cambió el 3/10», «si falla, llamar a…»"
      aria-invalid={!!error}
      aria-describedby="{id}-ayuda"
    ></textarea>
  {/if}
  <p class="pie faint" id="{id}-ayuda">
    <span>{AVISO_SECRETOS} Admite **negrita**, *cursiva*, listas con «-» y enlaces.</span>
    <span class="cuenta" class:mal={!!error}>{n}/{MAX_TEXTO}</span>
  </p>
  {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
</div>

<style>
  .cab {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .opcional {
    font-weight: 400;
  }
  textarea {
    resize: vertical;
    min-height: 4.5em;
    line-height: 1.45;
  }
  .vista {
    padding: var(--sp-2) var(--sp-3);
    border: 1px dashed var(--border);
    border-radius: var(--radius-sm);
    min-height: 4.5em;
  }
  .pie {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    margin: 4px 0 0;
    font-size: var(--fs-xs);
  }
  .cuenta {
    flex: none;
    font-variant-numeric: tabular-nums;
  }
  .cuenta.mal {
    color: var(--bad);
  }
</style>
