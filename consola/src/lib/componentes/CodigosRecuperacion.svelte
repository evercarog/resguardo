<script lang="ts">
  // Códigos de recuperación: se muestran una sola vez. Se pueden copiar,
  // descargar o imprimir, y hay que confirmar que se guardaron.
  import { Copy, Download, Printer } from "@lucide/svelte";
  import { avisar } from "$lib/avisos.svelte";

  let { codigos, seguir }: { codigos: string[]; seguir: () => void } = $props();
  let guardados = $state(false);
  const texto = $derived(`Códigos de recuperación de Resguardo Server\nCada uno sirve una sola vez si pierdes tu aplicación de verificación.\n\n${codigos.join("\n")}\n`);

  function descargar() {
    const url = URL.createObjectURL(new Blob([texto], { type: "text/plain;charset=utf-8" }));
    const a = Object.assign(document.createElement("a"), { href: url, download: "resguardo-codigos-de-recuperacion.txt" });
    a.click();
    URL.revokeObjectURL(url);
  }
</script>

<div class="cod">
  <div class="notice notice-warn"><p>Guárdalos ahora: no se vuelven a mostrar. Cada uno sirve una vez si pierdes el móvil.</p></div>
  <ul class="rejilla-cod selectable">
    {#each codigos as c (c)}<li><code>{c}</code></li>{/each}
  </ul>
  <div class="acciones">
    <button
      type="button"
      class="btn btn-sm"
      onclick={async () => {
        await navigator.clipboard.writeText(codigos.join("\n"));
        avisar("Códigos copiados.");
      }}><Copy size={14} />Copiar</button
    >
    <button type="button" class="btn btn-sm" onclick={descargar}><Download size={14} />Descargar</button>
    <button type="button" class="btn btn-sm" onclick={() => window.print()}><Printer size={14} />Imprimir</button>
  </div>
  <label class="switch-row"><input type="checkbox" bind:checked={guardados} /><span>Los he guardado en un sitio seguro</span></label>
  <button class="btn btn-primary btn-lg" disabled={!guardados} onclick={seguir}>Seguir</button>
</div>

<style>
  .cod {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .rejilla-cod {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 6px 16px;
    margin: 0;
    padding: var(--sp-4);
    list-style: none;
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .rejilla-cod code {
    font-size: 15px;
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
</style>
