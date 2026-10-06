<script lang="ts">
  // Una etiqueta de una versión («diaria», «semanal»…; no las de los equipos):
  // con un punto del color de su nombre (la misma paleta, siempre igual para
  // la misma etiqueta) y el texto, que es lo que se lee.
  import { colorPorNombre } from "$lib/etiquetasGrupos";

  let { nombre, oculta = false }: { nombre: string; oculta?: boolean } = $props();
</script>

<span class="badge badge-sm etv" style:--et="var(--et-{colorPorNombre(nombre)})" aria-hidden={oculta || undefined}><span class="punto" aria-hidden="true"></span>{nombre}</span>

<style>
  .etv {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--border);
  }
  .punto {
    flex: none;
    width: 6px;
    height: 6px;
    background: var(--et);
    border-radius: 999px;
  }
</style>
