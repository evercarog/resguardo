<script lang="ts">
  // «?»: los atajos de teclado de la consola, en una tabla corta.
  import { Keyboard, X } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { atajos, IR_A, MOD } from "$lib/atajos.svelte";

  const cerrar = () => (atajos.ayuda = false);
</script>

{#if atajos.ayuda}
  <Modal labelledby="t-atajos" onclose={cerrar} width={440}>
    <div class="dlg-head">
      <div class="dlg-title">
        <span class="ticon neutral"><Keyboard size={18} /></span>
        <div>
          <h2 id="t-atajos">Atajos de teclado</h2>
          <p>Funcionan en cualquier pantalla, salvo mientras escribes.</p>
        </div>
      </div>
      <button class="icon-btn" aria-label="Cerrar" onclick={cerrar}><X size={16} /></button>
    </div>
    <dl>
      <div><dt>Ir a cualquier sitio</dt><dd><kbd>{MOD}</kbd><kbd>K</kbd></dd></div>
      <div><dt>Ver estos atajos</dt><dd><kbd>?</kbd></dd></div>
      {#each IR_A as a (a.tecla)}
        <div><dt>{a.texto}</dt><dd><kbd>G</kbd><span class="luego">luego</span><kbd>{a.tecla.toUpperCase()}</kbd></dd></div>
      {/each}
      <div><dt>Cerrar un diálogo o menú</dt><dd><kbd>Esc</kbd></dd></div>
    </dl>
    <footer><button class="btn btn-primary" onclick={cerrar}>Entendido</button></footer>
  </Modal>
{/if}

<style>
  dl {
    display: flex;
    flex-direction: column;
    margin: 0;
  }
  dl > div {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    min-height: 36px;
    border-top: 1px solid var(--border);
  }
  dl > div:first-child {
    border-top: none;
  }
  dt {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  dd {
    display: flex;
    align-items: center;
    gap: 4px;
    margin: 0;
  }
  .luego {
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
</style>
