<script lang="ts">
  // «Comprobar con un ancla» (plan-mejoras 9b; docs/plataforma.md §7.3.1): la línea
  // `resguardo-ancla:…` de un resumen por correo anterior. Este navegador baja toda
  // la actividad del cliente, la recalcula desde la primera entrada y mira si la
  // entrada del ancla sigue con la misma huella. Si el servidor rehízo la cadena
  // (aunque cuadre consigo misma), no cuadra. No se fía de lo que diga el servidor.
  import { Anchor, LoaderCircle, ShieldCheck, ShieldX, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { comprobarAncla, leerAncla, type Comprobacion } from "$lib/auditoria";
  import { fechaLarga, numero } from "$lib/formato";
  import type { EntradaAuditoria } from "$lib/tipos";

  let { cliente, nombre, onclose }: { cliente: string; nombre: string; onclose: () => void } = $props();

  let pegado = $state("");
  let cargadas = $state(0);
  let comprobando = $state(false);
  let error = $state("");
  let resultado = $state<Comprobacion | null>(null);
  let vuelta = 0;

  const ancla = $derived(leerAncla(pegado));
  const deOtro = $derived(!!ancla && ancla.cliente !== cliente);
  const fechaAncla = $derived(ancla ? fechaLarga(new Date(ancla.creado * 1000).toISOString()) : "");

  /** Toda la actividad, de la primera entrada a la última, por páginas de 1000. */
  async function todas(mia: number): Promise<EntradaAuditoria[] | null> {
    const out: EntradaAuditoria[] = [];
    for (let desde = 0; ; ) {
      const p = await api.auditoria(cliente, desde, 1000);
      if (mia !== vuelta) return null;
      out.push(...p);
      cargadas = out.length;
      if (p.length < 1000) return out;
      desde = p[p.length - 1].n;
    }
  }

  async function comprobar(e: SubmitEvent) {
    e.preventDefault();
    if (!ancla || deOtro) return;
    const mia = ++vuelta;
    const a = ancla;
    comprobando = true;
    error = "";
    resultado = null;
    cargadas = 0;
    try {
      const entradas = await todas(mia);
      if (entradas) resultado = comprobarAncla(entradas, a);
    } catch (err) {
      if (mia === vuelta) error = (err as Error).message;
    } finally {
      if (mia === vuelta) comprobando = false;
    }
  }

  /** Otro texto: lo que se estaba comprobando ya no vale. */
  function otroTexto() {
    vuelta++;
    resultado = null;
    error = "";
    comprobando = false;
  }

  function cerrar() {
    vuelta++;
    onclose();
  }
</script>

<Modal labelledby="t-ancla" onclose={cerrar} width={560}>
  <form class="form" onsubmit={comprobar}>
    <div class="dlg-title">
      <span class="ticon"><Anchor size={18} /></span>
      <div>
        <h2 id="t-ancla">Comprobar con un ancla</h2>
        <p>Pega la línea «resguardo-ancla:…» de un resumen por correo anterior (o el correo entero). Este navegador recalcula toda la actividad de {nombre} desde la primera entrada y mira si esa entrada sigue igual.</p>
      </div>
    </div>
    <div class="field">
      <label class="field-label" for="ancla-pegada">Ancla</label>
      <textarea id="ancla-pegada" class="input mono" rows="3" spellcheck="false" placeholder="resguardo-ancla:1:…" bind:value={pegado} oninput={otroTexto}></textarea>
      {#if pegado.trim() && !ancla}
        <p class="faint pequeno">No encuentro ninguna ancla en lo pegado. Copia la línea entera, desde «resguardo-ancla».</p>
      {:else if deOtro}
        <p class="faint pequeno">Es el ancla de otro cliente: ábrela en ese cliente.</p>
      {:else if ancla}
        <p class="faint pequeno">Entrada n.º {numero(ancla.n)}, del {fechaAncla}.</p>
      {/if}
    </div>

    {#if resultado?.estado === "bien"}
      <div class="notice notice-success" role="status">
        <ShieldCheck size={16} />
        <p>Cuadra. La actividad está entera ({numero(resultado.total)} entradas) y la entrada n.º {numero(ancla?.n ?? 0)} sigue igual que el {fechaAncla}: nadie ha cambiado ni quitado nada de lo anterior a esa fecha.</p>
      </div>
    {:else if resultado?.estado === "distinta"}
      <div class="notice notice-danger" role="alert">
        <ShieldX size={16} />
        <p>No cuadra: la entrada n.º {numero(resultado.ahora.n)} tiene hoy otra huella ({resultado.ahora.hash.slice(0, 12)}… en vez de {ancla?.hash.slice(0, 12)}…, la del {fechaAncla}). Alguien ha rehecho la actividad de este cliente. Avisa a quien administre el servidor y guarda el correo del ancla.</p>
      </div>
    {:else if resultado?.estado === "falta"}
      <div class="notice notice-danger" role="alert">
        <ShieldX size={16} />
        <p>No cuadra: la actividad de hoy tiene {numero(resultado.total)} entradas y el ancla es de la n.º {numero(ancla?.n ?? 0)}. Faltan entradas: se rehízo la actividad o se restauró una copia anterior de la consola. Si nadie restauró una copia, avisa a quien administre el servidor.</p>
      </div>
    {:else if resultado?.estado === "rota"}
      <div class="notice notice-danger" role="alert">
        <ShieldX size={16} />
        <p>La actividad de hoy está rota en la entrada n.º {numero(resultado.en)}: alguien ha cambiado o quitado entradas. Avisa a quien administre el servidor.</p>
      </div>
    {/if}
    {#if error}<div class="notice notice-warn" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}

    <footer>
      {#if comprobando}<span class="espera-txt" role="status"><LoaderCircle size={15} class="spin" />{numero(cargadas)} entradas leídas…</span>{/if}
      <button type="button" class="btn btn-ghost" onclick={cerrar}>Cerrar</button>
      <button class="btn btn-primary" disabled={!ancla || deOtro || comprobando}><ShieldCheck size={15} />Comprobar</button>
    </footer>
  </form>
</Modal>

<style>
  .pequeno {
    font-size: var(--fs-xs);
    margin: 4px 0 0;
  }
  .espera-txt {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
</style>
