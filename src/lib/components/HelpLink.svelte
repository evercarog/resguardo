<script lang="ts">
  import { CircleHelp } from "@lucide/svelte";
  import { openHelp } from "$lib/help.svelte";

  // Pequeño «?» junto a un campo que abre la ayuda en el apartado indicado.
  interface Props {
    /** Id del apartado (ver src/lib/helpContent.ts). */
    topic: string;
    /** Para el lector de pantalla y el título: «Ayuda sobre …». */
    label: string;
  }
  let { topic, label }: Props = $props();
</script>

<button
  type="button"
  class="help-link"
  title="Ayuda: {label}"
  aria-label="Ayuda sobre {label}"
  onclick={(e) => {
    e.preventDefault();
    e.stopPropagation();
    openHelp(topic);
  }}
>
  <CircleHelp size={14} />
</button>

<style>
  .help-link {
    display: inline-grid;
    place-items: center;
    width: 20px;
    height: 20px;
    margin: -3px 0 -3px 2px;
    padding: 0;
    vertical-align: middle;
    color: var(--text-3);
    background: none;
    border: none;
    border-radius: 50%;
    cursor: pointer;
    transition:
      color 0.12s,
      background 0.12s;
  }
  .help-link:hover {
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .help-link:focus-visible {
    outline: none;
    box-shadow: var(--focus);
  }
</style>
