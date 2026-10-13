<script lang="ts">
  // Bajo «No copiar» (14q): las reglas que seguramente no hacen lo que se
  // quiere («.crdownload» sin «*» solo excluye un archivo llamado exactamente
  // así), cada una con un botón que la cambia por la que se quería. No impide
  // guardar: la persona puede querer justo eso.
  import { TriangleAlert, Wand } from "@lucide/svelte";
  import { corregirExclusion, exclusionesDudosas } from "$lib/copiaGuiada";
  import type { CopiaConfig } from "$lib/tipos";

  let { k }: { k: Pick<CopiaConfig, "exclusiones"> } = $props();
  const dudosas = $derived(exclusionesDudosas(k.exclusiones));
</script>

{#if dudosas.length}
  <div class="dudosas" role="status">
    <ul>
      {#each dudosas as d (d.regla)}
        <li>
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{d.motivo}</span>
          <button type="button" class="btn btn-sm" onclick={() => corregirExclusion(k, d.regla, d.propuesta)}><Wand size={13} />Cambiar por <span class="mono">{d.propuesta}</span></button>
        </li>
      {/each}
    </ul>
  </div>
{/if}

<style>
  .dudosas ul {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .dudosas li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 8px;
    padding: 6px 8px 6px 10px;
    font-size: var(--fs-sm);
    color: var(--text-2);
    background: var(--warn-soft, color-mix(in srgb, var(--warn) 9%, transparent));
    border: 1px solid color-mix(in srgb, var(--warn) 30%, var(--border));
    border-radius: var(--radius);
  }
  .dudosas li > :global(svg) {
    flex: none;
    color: var(--warn);
  }
  .dudosas li > span {
    flex: 1 1 240px;
    min-width: 0;
  }
  .mono {
    font-family: var(--mono);
    font-size: 12px;
  }
</style>
