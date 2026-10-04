<script lang="ts">
  import { RotateCcw, X } from "@lucide/svelte";
  import type { Repo, Snapshot } from "$lib/api";
  import { formatDate } from "$lib/format";
  import { clearRestore, restores } from "$lib/restores.svelte";
  import Modal from "./Modal.svelte";
  import RestoreForm from "./RestoreForm.svelte";

  interface Props {
    repo: Repo;
    snapshot: Snapshot;
    dir: string;
    /** Nombres dentro de `dir`; vacío = todo el contenido. */
    names: string[];
    /** Descripción de lo que se restaura, para el título. */
    label: string;
    onclose: () => void;
    ondone: () => void;
  }
  let { repo, snapshot, dir, names, label, onclose, ondone }: Props = $props();

  let running = $state(false);

  function close() {
    if (running) return;
    // Resultado ya visto: se olvida (si sigue en marcha en segundo plano, no se toca).
    if (!restores[repo.id]?.running) clearRestore(repo.id);
    onclose();
  }
</script>

<Modal onclose={close} labelledby="restore-title" width={560}>
  <header class="dlg-head">
    <div class="dlg-title">
      <span class="ticon"><RotateCcw size={19} /></span>
      <div>
        <h2 id="restore-title">Restaurar {label}</h2>
        <p class="faint">Versión del {formatDate(snapshot.time)} · <span class="mono">{snapshot.short_id}</span></p>
      </div>
    </div>
    <button class="icon-btn" title="Cerrar" aria-label="Cerrar" onclick={close} disabled={running}><X size={17} /></button>
  </header>

  <RestoreForm {repo} {snapshot} {dir} {names} {label} bind:running onbackground={onclose} onclose={close} {ondone} />
</Modal>

<style>
</style>
