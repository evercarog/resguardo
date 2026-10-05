<script lang="ts">
  // Aviso «copias en el mismo equipo» (lib/dondeGuarda.ts): qué pasa, por qué
  // importa y las dos salidas: moverlas a un almacén de otro equipo o añadir
  // una copia externa. Color de aviso, con icono y texto.
  import { ArrowRightLeft, CloudUpload, TriangleAlert } from "@lucide/svelte";
  import type { RiesgoMismoEquipo } from "$lib/dondeGuarda";

  let {
    riesgo,
    onmover,
    hrefExterna,
    compacto = false,
  }: {
    riesgo: RiesgoMismoEquipo;
    /** «Mover a un almacén…» (sin permiso para cambiarlo, no se ofrece). */
    onmover?: () => void;
    /** «Añadir copia externa…»: a la ficha del equipo, con el diálogo abierto. */
    hrefExterna?: string;
    compacto?: boolean;
  } = $props();
</script>

<div class="notice notice-warn aviso" class:compacto role="note" aria-label={riesgo.titulo}>
  <TriangleAlert size={16} />
  <div class="cuerpo">
    <p><strong>{riesgo.titulo}.</strong> {riesgo.texto}</p>
    {#if riesgo.nota}<p class="nota">{riesgo.nota}</p>{/if}
    {#if onmover || hrefExterna}
      <div class="acciones">
        {#if onmover}<button type="button" class="btn btn-sm" onclick={onmover}><ArrowRightLeft size={14} />Mover a un almacén…</button>{/if}
        {#if hrefExterna}<a class="btn btn-sm btn-ghost" href={hrefExterna}><CloudUpload size={14} />Añadir copia externa…</a>{/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .aviso {
    align-items: flex-start;
  }
  .cuerpo {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .nota {
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 2px;
  }
  .compacto .acciones {
    margin-top: 0;
  }
</style>
