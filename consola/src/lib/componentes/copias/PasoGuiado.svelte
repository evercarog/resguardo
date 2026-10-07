<script lang="ts">
  // Un paso de un editor guiado (plan 0.7.26: «guiado y avanzado»): número o ✓,
  // el título y, si ya está hecho, su resumen en una línea que se toca para
  // volver a abrirlo. Solo un paso abierto a la vez (lo decide quien lo usa).
  // El contenido del paso abierto va dentro (`children`).
  import type { Snippet } from "svelte";
  import { Check, ChevronDown } from "@lucide/svelte";

  let {
    n,
    titulo,
    abierto,
    hecho = false,
    resumen = "",
    deshabilitado = false,
    onabrir,
    ayuda,
    children,
  }: {
    n: number;
    titulo: string;
    abierto: boolean;
    /** Ya tiene lo necesario: enseña ✓ y su resumen. */
    hecho?: boolean;
    /** Lo elegido, en una línea («Cada hora», «2 carpetas»…). */
    resumen?: string;
    /** Aún no se puede abrir (falta un paso de antes). */
    deshabilitado?: boolean;
    onabrir: () => void;
    /** El «?» del paso (fuera del botón). */
    ayuda?: Snippet;
    children: Snippet;
  } = $props();
  const uid = $props.id();
</script>

<li class="paso-g" class:abierto class:hecho aria-current={abierto ? "step" : undefined}>
  <div class="cab">
    <button type="button" class="linea" aria-expanded={abierto} aria-controls="pg-{uid}" disabled={deshabilitado && !abierto} onclick={onabrir}>
      <span class="n" aria-hidden="true">{#if hecho && !abierto}<Check size={12} strokeWidth={3} />{:else}{n}{/if}</span>
      <span class="t">{titulo}</span>
      {#if !abierto && resumen}<span class="r">{resumen}</span>{/if}
      {#if !abierto && hecho}<span class="sr-only">: hecho, tócalo para cambiarlo</span>{/if}
      {#if !abierto && hecho && !deshabilitado}<span class="cambiar" aria-hidden="true">Cambiar</span>{:else if !abierto && !deshabilitado}<ChevronDown size={15} class="chev" aria-hidden="true" />{/if}
    </button>
    {#if ayuda}<span class="ayuda">{@render ayuda()}</span>{/if}
  </div>
  {#if abierto}
    <div class="cuerpo" id="pg-{uid}">
      {@render children()}
    </div>
  {/if}
</li>

<style>
  .paso-g {
    position: relative;
    list-style: none;
    padding-left: 34px;
  }
  /* La línea fina que une los pasos. */
  .paso-g:not(:last-child)::before {
    content: "";
    position: absolute;
    left: 11px;
    top: 30px;
    bottom: -6px;
    width: 1px;
    background: var(--border-strong);
  }
  .paso-g.hecho:not(:last-child)::before {
    background: var(--accent);
    opacity: 0.5;
  }
  .cab {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 36px;
  }
  .linea {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    min-width: 0;
    min-height: 36px;
    padding: 4px 8px 4px 0;
    margin-left: -34px;
    font: inherit;
    text-align: left;
    color: var(--text-1);
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .linea:disabled {
    cursor: default;
    color: var(--text-3);
  }
  .linea:not(:disabled):hover .r,
  .linea:not(:disabled):hover .t {
    color: var(--text-1);
  }
  .linea:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .n {
    display: grid;
    flex: none;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 999px;
    font-size: var(--fs-xs);
    font-weight: 650;
    font-variant-numeric: tabular-nums;
    color: var(--text-2);
    background: var(--surface-3);
  }
  .abierto .n {
    color: var(--accent-contrast);
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .hecho:not(.abierto) .n {
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .t {
    flex: none;
    font-weight: 600;
    font-size: var(--fs-body);
  }
  .hecho:not(.abierto) .t {
    color: var(--text-2);
    font-weight: 550;
  }
  .r {
    min-width: 0;
    overflow: hidden;
    font-size: var(--fs-sm);
    color: var(--text-1);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .linea :global(.chev) {
    flex: none;
    margin-left: auto;
    color: var(--text-3);
  }
  .cambiar {
    flex: none;
    margin-left: auto;
    font-size: var(--fs-xs);
    font-weight: 550;
    color: var(--accent-text);
  }
  .ayuda {
    flex: none;
  }
  .cuerpo {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-2) 0 var(--sp-4);
    animation: rise var(--dur) var(--ease-out) backwards;
  }
  @media (prefers-reduced-motion: reduce) {
    .cuerpo {
      animation: none;
    }
  }
</style>
