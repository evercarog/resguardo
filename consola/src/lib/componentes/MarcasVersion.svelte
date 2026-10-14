<script lang="ts">
  // Plan 0.8.0, F2: junto a una versión (en las listas y en Restaurar), si se
  // conserva para siempre (con su motivo) y cuántos comentarios tiene (con el
  // primero en el tooltip). Lo dice el equipo en su resumen.
  import { MessageSquare, Pin } from "@lucide/svelte";
  import { tip } from "$lib/tooltip";
  import { comentariosDe, conservadaDe } from "$lib/versionesNotas";
  import type { Equipo } from "$lib/tipos";

  let { equipo, repo, version }: { equipo: Equipo | null | undefined; repo: string; version: string } = $props();
  const conservada = $derived(conservadaDe(equipo, repo, version));
  const comentarios = $derived(comentariosDe(equipo, repo, version));
  const textoComentarios = $derived(
    comentarios.length ? `${comentarios.length === 1 ? "1 comentario" : `${comentarios.length} comentarios`}: «${comentarios[0].texto.length > 90 ? `${comentarios[0].texto.slice(0, 90)}…` : comentarios[0].texto}»${comentarios[0].por ? ` (${comentarios[0].por})` : ""}` : "",
  );
</script>

{#if conservada}
  <span class="marca conservada" role="img" aria-label="Se conserva para siempre: {conservada.motivo}" use:tip={`Se conserva para siempre: «${conservada.motivo}»`}><Pin size={13} /></span>
{/if}
{#if comentarios.length}
  <span class="marca" role="img" aria-label={textoComentarios} use:tip={textoComentarios}><MessageSquare size={13} />{#if comentarios.length > 1}<span class="n num">{comentarios.length}</span>{/if}</span>
{/if}

<style>
  .marca {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    color: var(--text-3);
    vertical-align: middle;
  }
  .conservada {
    color: var(--accent-text);
  }
  .n {
    font-size: var(--fs-xs);
  }
</style>
