<script lang="ts">
  // «Ver qué archivos»: los que no se pudieron leer en la última copia
  // automática de una copia, con su ruta completa (registro detallado del
  // agente; solo administradores).
  import { onMount } from "svelte";
  import { CircleAlert, Copy, FileWarning, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { toast } from "$lib/toast.svelte";
  import Modal from "./Modal.svelte";

  interface Props {
    repoId: string;
    planId: string;
    /** Nombre de la copia, para el título. */
    name: string;
    onclose: () => void;
  }
  let { repoId, planId, name, onclose }: Props = $props();

  let files = $state<string[] | null>(null);
  let error = $state("");

  onMount(async () => {
    try {
      files = await api.agentFailedFiles(repoId, planId);
    } catch (e) {
      error = String(e);
    }
  });

  /** «C:\x\y.pst: en uso» → ruta y motivo. */
  function split(line: string) {
    const i = line.search(/: (?=[^\\/]*$)/);
    return i > 0 ? { path: line.slice(0, i), why: line.slice(i + 2) } : { path: line, why: "" };
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText((files ?? []).join("\n"));
      toast("Lista copiada al portapapeles.", "info");
    } catch {
      toast("No se pudo copiar la lista.", "error");
    }
  }
</script>

<Modal {onclose} labelledby="files-title" width={680}>
  <header class="dlg-head">
    <div class="dlg-title">
      <span class="ticon warn"><FileWarning size={18} /></span>
      <div>
        <h2 id="files-title">Archivos que no se copiaron</h2>
        <p class="faint">Última copia automática de «{name}». Las rutas completas solo las ven los administradores.</p>
      </div>
    </div>
    <button class="icon-btn" onclick={onclose} title="Cerrar" aria-label="Cerrar"><X size={16} /></button>
  </header>

  {#if error}
    <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>
  {:else if files === null}
    <p class="faint">Cargando…</p>
  {:else if files.length === 0}
    <p class="faint empty">No hay archivos anotados para la última copia. El detalle se guarda desde esta versión de Resguardo: aparecerá tras la próxima copia con avisos.</p>
  {:else}
    <ul class="list selectable">
      {#each files as f, i (i)}
        {@const s = split(f)}
        <li><span class="mono path">{s.path}</span>{#if s.why}<span class="faint why">{s.why}</span>{/if}</li>
      {/each}
    </ul>
    <p class="faint hint-line">
      Suelen ser archivos abiertos por otro programa (correo, bases de datos) o sin permiso de lectura para el sistema. Si no hacen falta, exclúyelos de la copia.
    </p>
  {/if}

  <footer>
    <button class="btn btn-ghost" onclick={copy} disabled={!files?.length}><Copy size={14} /> Copiar</button>
    <button class="btn btn-primary" onclick={onclose}>Listo</button>
  </footer>
</Modal>

<style>
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: min(50vh, 400px);
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .list li {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 12px;
  }
  .list li + li {
    border-top: 1px solid var(--border);
  }
  .path {
    font-size: var(--fs-sm);
    overflow-wrap: anywhere;
  }
  .why {
    font-size: var(--fs-xs);
  }
  .hint-line {
    margin: 10px 0 0;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .empty {
    padding: 16px 0;
    line-height: 1.5;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }
</style>
