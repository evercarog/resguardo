<script lang="ts">
  // Los pasos de un asistente (Restaurar, Añadir equipo): número o ✓, el
  // nombre y una línea entre ellos. Los ya hechos se pueden pulsar para
  // volver (si `alElegir` lo permite). En el móvil, «Paso 3 de 6 · Versión».
  import { Check } from "@lucide/svelte";

  let {
    pasos,
    actual,
    completo = false,
    alElegir,
  }: {
    pasos: { id: string; texto: string }[];
    actual: string;
    /** Todo hecho: el último paso también lleva ✓. */
    completo?: boolean;
    /** Volver a un paso ya hecho; si devuelve false (o no hay), no se puede. */
    alElegir?: (id: string) => boolean | void;
  } = $props();

  const i = $derived(Math.max(0, pasos.findIndex((p) => p.id === actual)));
</script>

<nav class="pasos-asistente" aria-label="Pasos">
  <p class="movil">Paso {i + 1} de {pasos.length} · <strong>{pasos[i]?.texto}</strong></p>
  <ol>
    {#each pasos as p, j (p.id)}
      {@const hecho = j < i || (completo && j === i)}
      <li class:on={j === i && !completo} class:hecho aria-current={j === i ? "step" : undefined}>
        {#if hecho && alElegir}
          <button type="button" onclick={() => alElegir(p.id)}>
            <span class="n"><Check size={12} strokeWidth={3} /></span><span class="t">{p.texto}</span><span class="sr-only">: hecho, volver a este paso</span>
          </button>
        {:else}
          <span class="paso">
            <span class="n">{#if hecho}<Check size={12} strokeWidth={3} />{:else}{j + 1}{/if}</span><span class="t">{p.texto}</span>{#if hecho}<span class="sr-only">: hecho</span>{/if}
          </span>
        {/if}
      </li>
    {/each}
  </ol>
</nav>

<style>
  .pasos-asistente {
    min-width: 0;
  }
  ol {
    display: flex;
    align-items: center;
    gap: 0;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  li {
    display: flex;
    flex: 1;
    align-items: center;
    min-width: 0;
  }
  li:last-child {
    flex: none;
  }
  /* La línea que une un paso con el siguiente: en acento si ya se pasó. */
  li:not(:last-child)::after {
    content: "";
    flex: 1;
    height: 1px;
    min-width: 12px;
    margin: 0 10px;
    background: var(--border-strong);
    transition: background var(--dur) var(--ease);
  }
  li.hecho::after {
    background: color-mix(in srgb, var(--accent) 55%, transparent);
  }
  .paso,
  button {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    padding: 2px 4px 2px 2px;
    font: inherit;
    color: inherit;
    white-space: nowrap;
    background: none;
    border: none;
    border-radius: 999px;
  }
  button {
    cursor: pointer;
  }
  button:hover .t {
    color: var(--text-1);
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .n {
    display: grid;
    flex: none;
    place-items: center;
    width: 22px;
    height: 22px;
    font-size: 11px;
    font-weight: 650;
    background: var(--surface-3);
    border-radius: 999px;
    transition:
      background var(--dur) var(--ease),
      color var(--dur) var(--ease);
  }
  .on {
    color: var(--text-1);
    font-weight: 550;
  }
  .on .n {
    color: var(--accent-contrast);
    background: var(--accent);
    box-shadow: 0 0 0 4px var(--accent-soft);
  }
  .hecho .n {
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .hecho {
    color: var(--text-2);
  }
  .movil {
    display: none;
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .movil strong {
    color: var(--text-1);
    font-weight: 550;
  }
  @media (max-width: 760px) {
    .t {
      display: none;
    }
    .on .t {
      display: inline;
    }
  }
  @media (max-width: 480px) {
    ol {
      display: none;
    }
    .movil {
      display: block;
    }
  }
</style>
