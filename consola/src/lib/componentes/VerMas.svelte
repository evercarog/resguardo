<script lang="ts">
  // 16t: «Ver más» igual en todas las listas largas. Con lo que queda (lista ya cargada), dice
  // cuántas se añaden y cuántas quedan; sin saberlo (el servidor da páginas), «Ver más antiguas».
  import { numero } from "$lib/formato";
  import { TANDA, textoSeVen } from "$lib/tablas";

  let {
    vistas,
    total,
    tanda = TANDA,
    cargando = false,
    onclick,
    alVolver,
  }: {
    vistas: number;
    /** null: no se sabe (quedan más en el servidor). */
    total: number | null;
    tanda?: number;
    cargando?: boolean;
    onclick: () => void;
    /** «Ver solo las primeras» cuando ya se abrió más de una tanda. */
    alVolver?: () => void;
  } = $props();

  const id = $props.id();
  const quedan = $derived(total == null ? null : Math.max(0, total - vistas));
</script>

{#if quedan == null || quedan > 0}
  <div class="ver-mas">
    <button type="button" class="btn btn-sm" aria-describedby={quedan != null ? id : undefined} disabled={cargando} {onclick}>
      {#if cargando}Cargando…{:else if quedan == null}Ver más antiguas{:else}Ver más ({numero(Math.min(tanda, quedan))}){/if}
    </button>
    {#if quedan != null && total != null}<span class="faint num" {id}>{textoSeVen(vistas, total)}</span>{/if}
  </div>
{:else if alVolver && vistas > tanda}
  <div class="ver-mas"><button type="button" class="btn btn-sm btn-ghost" onclick={alVolver}>Ver solo las primeras</button></div>
{/if}

<style>
  .ver-mas {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3) 0;
    font-size: var(--fs-sm);
  }
</style>
