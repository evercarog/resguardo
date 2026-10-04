<script lang="ts">
  // Poner o quitar etiquetas a un equipo (técnicos o más; v1.18). Propone las
  // que ya usa el cliente para que no salgan «Contabilidad» y «contabilidad».
  import { Tag } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { actual, cargarCliente } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { etiquetasDe, limpiarEtiqueta, MAX_ETIQUETAS, MAX_LARGO } from "$lib/etiquetas.svelte";
  import type { Equipo } from "$lib/tipos";
  import EtiquetaChip from "./EtiquetaChip.svelte";
  import BotonCargando from "./BotonCargando.svelte";

  let { equipo, onclose, alGuardar }: { equipo: Equipo; onclose: () => void; alGuardar?: (e: Equipo) => void } = $props();

  // Copia para editar (lo del equipo no cambia hasta guardar).
  const inicio = () => [...(equipo.etiquetas ?? [])];
  let lista = $state<string[]>(inicio());
  let texto = $state("");
  let error = $state("");
  let guardando = $state(false);
  const existentes = $derived(etiquetasDe(actual.equipos).filter((t) => !lista.some((x) => x.toLowerCase() === t.nombre.toLowerCase())));
  const sugeridas = $derived(texto.trim() ? existentes.filter((t) => t.nombre.toLowerCase().includes(texto.trim().toLowerCase())) : existentes);

  function anadir(t: string) {
    error = "";
    const x = limpiarEtiqueta(t);
    if (!x) {
      if (t.trim()) error = `Hasta ${MAX_LARGO} caracteres, sin comas.`;
      return;
    }
    // Si ya existe en el cliente con otras mayúsculas, se usa esa.
    const igual = etiquetasDe(actual.equipos).find((e) => e.nombre.toLowerCase() === x.toLowerCase());
    const nombre = igual?.nombre ?? x;
    if (lista.some((e) => e.toLowerCase() === nombre.toLowerCase())) return void (texto = "");
    if (lista.length >= MAX_ETIQUETAS) return void (error = `Como mucho ${MAX_ETIQUETAS} etiquetas por equipo.`);
    lista = [...lista, nombre];
    texto = "";
  }
  function teclas(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      anadir(texto);
    } else if (e.key === "Backspace" && !texto && lista.length) lista = lista.slice(0, -1);
  }
  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    if (texto.trim()) anadir(texto);
    if (error) return;
    guardando = true;
    try {
      const nuevo = await api.ponerEtiquetas(actual.id, equipo.id, lista);
      avisar(lista.length ? "Etiquetas guardadas." : "Etiquetas quitadas.");
      alGuardar?.(nuevo);
      void cargarCliente(actual.id, { silencioso: true });
      onclose();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      guardando = false;
    }
  }
  const cambiado = $derived(JSON.stringify(lista) !== JSON.stringify(equipo.etiquetas ?? []) || !!texto.trim());
</script>

<Modal labelledby="t-etiquetas" {onclose} width={480}>
  <form class="form" onsubmit={guardar}>
    <div class="dlg-title">
      <span class="ticon"><Tag size={18} /></span>
      <div>
        <h2 id="t-etiquetas">Etiquetas de {equipo.nombre}</h2>
        <p>Para agrupar y filtrar equipos: «Contabilidad», «Servidores», «Sede norte»…</p>
      </div>
    </div>
    <div class="field">
      <label class="field-label" for="et-nueva">Etiquetas</label>
      <div class="caja">
        {#each lista as t (t)}<EtiquetaChip nombre={t} onquitar={() => (lista = lista.filter((x) => x !== t))} />{/each}
        <!-- svelte-ignore a11y_autofocus -->
        <input id="et-nueva" class="entrada" bind:value={texto} onkeydown={teclas} placeholder={lista.length ? "Añadir otra…" : "Escribe y pulsa Intro"} maxlength={MAX_LARGO} autocomplete="off" autofocus aria-invalid={!!error} aria-describedby="et-nueva-ayuda" />
      </div>
      {#if error}<p class="error-campo" id="et-nueva-ayuda" role="alert">{error}</p>{:else}<span class="field-hint" id="et-nueva-ayuda">Intro o coma para añadir. Hasta {MAX_ETIQUETAS} por equipo.</span>{/if}
    </div>
    {#if sugeridas.length}
      <div class="sugeridas">
        <span class="faint">Ya en uso:</span>
        {#each sugeridas.slice(0, 12) as t (t.nombre)}<EtiquetaChip nombre={t.nombre} n={t.n} onclick={() => anadir(t.nombre)} />{/each}
      </div>
    {/if}
    <p class="faint nota">Se guardan en el servidor sin cifrar, como el nombre del equipo: úsalas para organizar, no para datos privados.</p>
    <footer>
      <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
      <BotonCargando class="btn btn-primary" disabled={!cambiado} cargando={guardando} textoCargando="Guardando…">Guardar</BotonCargando>
    </footer>
  </form>
</Modal>

<style>
  .caja {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    min-height: 40px;
    padding: 6px 8px;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
  }
  .caja:focus-within {
    border-color: var(--accent);
    box-shadow: var(--focus);
  }
  .entrada {
    flex: 1;
    min-width: 120px;
    height: 26px;
    padding: 0 4px;
    font: inherit;
    color: var(--text-1);
    background: none;
    border: none;
    outline: none;
  }
  .sugeridas {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .nota {
    margin: 0;
    font-size: var(--fs-xs);
  }
</style>
