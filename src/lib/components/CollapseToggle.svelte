<script lang="ts">
  // Botón para plegar o desplegar una tarjeta larga. El estado lo guarda quien lo usa.
  import { ChevronDown } from "@lucide/svelte";

  interface Props {
    open: boolean;
    /** Nombre de la sección, para el lector de pantalla y el tooltip. */
    label: string;
    /** id del contenido que se pliega. */
    controls: string;
    /** No se puede plegar ahora (p. ej. hay una tarea en curso o se está editando). */
    locked?: boolean;
    ontoggle: () => void;
  }
  let { open, label, controls, locked = false, ontoggle }: Props = $props();
  const text = $derived(locked ? `${label}: no se puede plegar mientras haya algo en curso o sin guardar` : `${open ? "Plegar" : "Desplegar"} ${label}`);
</script>

<button class="icon-btn toggle" class:open aria-expanded={open} aria-controls={controls} aria-label={text} title={text} disabled={locked} onclick={ontoggle}>
  <ChevronDown size={17} />
</button>

<style>
  .toggle {
    flex: none;
  }
  .toggle :global(svg) {
    transition: transform 0.18s;
    transform: rotate(-90deg);
  }
  .toggle.open :global(svg) {
    transform: none;
  }
</style>
