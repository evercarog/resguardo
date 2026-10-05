<script lang="ts">
  // Observaciones y comentarios de algo que no tiene página propia (un destino):
  // los mismos componentes, en un diálogo.
  import { NotebookPen } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import type { TipoNota } from "$lib/tipos";
  import Observaciones from "./Observaciones.svelte";
  import Comentarios from "./Comentarios.svelte";

  let { tipo, objeto, nombre, onclose }: { tipo: TipoNota; objeto: string; nombre: string; onclose: () => void } = $props();
</script>

<Modal labelledby="t-notas-dlg" {onclose} width={560}>
  <div class="form">
    <div class="dlg-title">
      <span class="ticon"><NotebookPen size={18} /></span>
      <div>
        <h2 id="t-notas-dlg">Notas de «{nombre}»</h2>
        <p>Observaciones y comentarios para quien trabaje con este cliente.</p>
      </div>
    </div>
    <Observaciones {tipo} {objeto} />
    <Comentarios {tipo} {objeto} sinTarjeta />
    <div class="dlg-actions">
      <button type="button" class="btn" onclick={onclose}>Cerrar</button>
    </div>
  </div>
</Modal>
