<script lang="ts">
  // «¿Salir sin enviar?» al ir a otra página de la consola con trabajo sin
  // enviar (lib/salir.svelte.ts). Va una vez, en el marco de la consola.
  import { LogOut } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { quedarse, salida, salirIgual } from "$lib/salir.svelte";
</script>

{#if salida.abierta}
  <Modal labelledby="t-salir" onclose={quedarse} width={440}>
    <div class="dlg-title">
      <span class="ticon warn"><LogOut size={18} /></span>
      <div>
        <h2 id="t-salir">¿Salir sin enviar?</h2>
        <p>{salida.texto}</p>
      </div>
    </div>
    <footer>
      <button type="button" class="btn btn-ghost" onclick={salirIgual}>Salir</button>
      <!-- svelte-ignore a11y_autofocus -->
      <button type="button" class="btn btn-primary" autofocus onclick={quedarse}>Seguir aquí</button>
    </footer>
  </Modal>
{/if}
