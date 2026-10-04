<script lang="ts">
  import { onMount } from "svelte";
  import { CircleAlert, Copy, RefreshCw, ScrollText, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { agent } from "$lib/agent.svelte";
  import { toast } from "$lib/toast.svelte";
  import Modal from "./Modal.svelte";

  // Registro del agente: qué hizo cada vez que se despertó (copias, errores,
  // informes a la web). Sirve para saber por qué una copia no se hizo.
  let { onclose }: { onclose: () => void } = $props();

  let lines = $state<string[] | null>(null);
  let error = $state("");
  let loading = $state(false);
  let box = $state<HTMLPreElement>();
  /** Registro detallado (rutas completas): solo con la app abierta como administrador. */
  let detail = $state(false);
  const elevated = $derived(!!agent.info?.elevated);

  async function load() {
    loading = true;
    error = "";
    try {
      lines = detail ? await api.agentLogDetail() : await api.agentLog();
      // Lo más reciente está al final: se muestra ya desplazado hasta ahí.
      queueMicrotask(() => box && (box.scrollTop = box.scrollHeight));
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  /** Las líneas con errores o avisos se resaltan. */
  function tone(line: string) {
    const l = line.toLowerCase();
    if (l.includes("error") || l.includes("no se pudo") || l.includes("falt")) return "bad";
    if (l.includes("aviso") || l.includes("warning") || l.includes("algunos archivos")) return "warn";
    if (l.includes("completada") || l.includes(": ok ")) return "ok";
    return "";
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText((lines ?? []).join("\n"));
      toast("Registro copiado al portapapeles.", "info");
    } catch {
      toast("No se pudo copiar el registro.", "error");
    }
  }
</script>

<Modal {onclose} labelledby="log-title" width={720}>
  <header class="dlg-head">
    <div class="dlg-title">
      <span class="ticon"><ScrollText size={18} /></span>
      <div>
        <h2 id="log-title">Registro del agente</h2>
        <p class="faint">
          Lo que hizo el agente de copias automáticas en este equipo, lo más reciente al final.{detail
            ? " Con las rutas completas: solo lo ven los administradores."
            : " Sin rutas de archivos: lo pueden ver todos los usuarios del equipo."}
        </p>
      </div>
    </div>
    <button class="icon-btn" onclick={onclose} title="Cerrar" aria-label="Cerrar"><X size={16} /></button>
  </header>

  {#if error}
    <div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>
  {:else if lines && lines.length === 0}
    <p class="faint empty">El registro está vacío: el agente todavía no ha hecho nada en este equipo.</p>
  {:else}
    <pre class="log mono selectable" bind:this={box} aria-busy={loading}>{#if lines}{#each lines as line, i (i)}<span class="line {tone(line)}">{line}</span>
{/each}{:else}Cargando…{/if}</pre>
  {/if}

  <footer>
    {#if elevated}
      <label class="detail">
        <input type="checkbox" class="switch" bind:checked={detail} onchange={load} disabled={loading} />
        Ver detalle (rutas completas)
      </label>
    {/if}
    <span class="spacer"></span>
    <button class="btn btn-ghost" onclick={copy} disabled={!lines?.length}><Copy size={14} /> Copiar</button>
    <button class="btn" onclick={load} disabled={loading}><RefreshCw size={14} /> Actualizar</button>
    <button class="btn btn-primary" onclick={onclose}>Listo</button>
  </footer>
</Modal>

<style>
  .log {
    margin: 0;
    height: min(52vh, 420px);
    overflow: auto;
    padding: 12px 14px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: var(--surface-2);
    font-size: var(--fs-xs);
    line-height: 1.6;
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
  }
  .line.bad {
    color: var(--bad);
  }
  .line.warn {
    color: var(--warn);
  }
  .line.ok {
    color: var(--ok);
  }
  .empty {
    padding: 24px 0;
    text-align: center;
  }
  footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-top: 16px;
  }
  .detail {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: var(--fs-sm);
    cursor: pointer;
  }
</style>
