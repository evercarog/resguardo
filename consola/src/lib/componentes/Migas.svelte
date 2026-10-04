<script lang="ts">
  // Migas de pan: «Equipos / CONTABILIDAD / Siigo y documentos». Los
  // anteriores son enlaces; el último es la página actual.
  import { ChevronRight } from "@lucide/svelte";

  let { items }: { items: { texto: string; href?: string }[] } = $props();
</script>

<nav aria-label="Dónde estás">
  <ol class="migas">
    {#each items as m, i (i)}
      <li>
        {#if i > 0}<ChevronRight size={13} class="sep" aria-hidden="true" />{/if}
        {#if m.href && i < items.length - 1}<a href={m.href}>{m.texto}</a>{:else}<span aria-current="page">{m.texto}</span>{/if}
      </li>
    {/each}
  </ol>
</nav>
