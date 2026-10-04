<script lang="ts">
  // «Marca del cliente» (v1.32): su logo y un acento de los de la consola.
  // Lo cambian propietarios y administradores; queda en la actividad. El
  // logo se convierte a PNG en este navegador antes de mandarlo (lib/marca.ts).
  import { ImagePlus, Palette, Trash2 } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { actual, app } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { logoAPng, TIPOS_LOGO } from "$lib/marca";
  import type { MarcaCliente as Marca } from "$lib/tipos";
  import MarcaCliente from "./MarcaCliente.svelte";
  import BotonCargando from "./BotonCargando.svelte";
  import { tip } from "$lib/tooltip";

  let { cliente, nombre, marca, onclose }: { cliente: string; nombre: string; marca?: Marca | null; onclose: () => void } = $props();

  const ACENTOS: [NonNullable<Marca["acento"]>, string][] = [
    ["teal", "Verde azulado"],
    ["blue", "Azul"],
    ["indigo", "Índigo"],
    ["violet", "Violeta"],
    ["rose", "Rosa"],
    ["amber", "Ámbar"],
    ["graphite", "Grafito"],
  ];

  // svelte-ignore state_referenced_locally
  let acento = $state<Marca["acento"]>(marca?.acento ?? null);
  /** PNG nuevo (base64) y su vista previa; `quitar`: quitar el que hay. */
  let nuevo = $state<{ base64: string; vista: string } | null>(null);
  let quitar = $state(false);
  let error = $state("");
  let preparando = $state(false);
  let guardando = $state(false);
  let entrada = $state<HTMLInputElement>();

  const logoVista = $derived(nuevo?.vista ?? (quitar ? null : (marca?.logo ?? null)));
  const vista = $derived<Marca>({ acento, logo: logoVista });
  const cambiado = $derived(!!nuevo || quitar || acento !== (marca?.acento ?? null));

  async function elegir(e: Event) {
    const f = (e.currentTarget as HTMLInputElement).files?.[0];
    (e.currentTarget as HTMLInputElement).value = "";
    if (!f) return;
    error = "";
    preparando = true;
    try {
      const r = await logoAPng(f);
      nuevo = { base64: r.base64, vista: r.vista };
      quitar = false;
    } catch (x) {
      error = (x as Error).message;
    } finally {
      preparando = false;
    }
  }

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    guardando = true;
    error = "";
    try {
      const m = await api.cambiarMarca(cliente, { acento, ...(nuevo ? { logo: nuevo.base64 } : {}), ...(quitar ? { quitar_logo: true } : {}) });
      if (actual.cliente && actual.id === cliente) actual.cliente.marca = m;
      const x = app.clientes.find((k) => k.id === cliente);
      if (x) x.marca = m;
      avisar("Marca guardada. La verán todas las personas del cliente.");
      onclose();
    } catch (x) {
      error = (x as Error).message;
    } finally {
      guardando = false;
    }
  }
</script>

<Modal labelledby="t-marca" {onclose} width={520} dismissible={!cambiado}>
  <form class="form" onsubmit={guardar}>
    <div class="dlg-title">
      <span class="ticon"><Palette size={18} /></span>
      <div>
        <h2 id="t-marca">Marca de {nombre}</h2>
        <p>Su logo y su color salen en el selector de clientes, en su cabecera y en la portada de sus informes. Los ven todas las personas del cliente.</p>
      </div>
    </div>

    <div class="vista" aria-hidden="true">
      <MarcaCliente {nombre} marca={vista} tam={48} />
      <span class="vista-texto"><strong>{nombre}</strong><span class="faint">Así se verá</span></span>
    </div>

    <div class="field">
      <span class="field-label" id="t-logo">Logo</span>
      <div class="fila-logo">
        <button type="button" class="btn" aria-describedby="ayuda-logo" disabled={preparando} onclick={() => entrada?.click()}><ImagePlus size={16} />{preparando ? "Preparando…" : logoVista ? "Cambiar el logo" : "Elegir un logo"}</button>
        {#if logoVista}<button type="button" class="btn btn-ghost" onclick={() => ((nuevo = null), (quitar = true))}><Trash2 size={15} />Quitarlo</button>{/if}
      </div>
      <span class="field-hint" id="ayuda-logo">PNG, JPG, WebP o SVG. Se convierte a PNG en este navegador (hasta 512 px y 200 KB), así que un SVG nunca se guarda tal cual.</span>
      <input bind:this={entrada} type="file" accept={TIPOS_LOGO} hidden onchange={elegir} aria-labelledby="t-logo" />
    </div>

    <fieldset class="field acentos">
      <legend class="field-label">Color</legend>
      <div class="colores">
        <button type="button" class="color ninguno" class:on={acento === null} aria-pressed={acento === null} onclick={() => (acento = null)}>El de cada persona</button>
        {#each ACENTOS as [a, txt] (a)}
          <button type="button" class="color marca-cliente" data-acento={a} class:on={acento === a} aria-pressed={acento === a} aria-label={txt} use:tip={txt} onclick={() => (acento = a)}></button>
        {/each}
      </div>
      <span class="field-hint">Los mismos colores de la consola, con el contraste comprobado en claro y en oscuro.</span>
    </fieldset>

    {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
    <footer>
      <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
      <BotonCargando class="btn btn-primary" type="submit" cargando={guardando} disabled={!cambiado || preparando}>Guardar</BotonCargando>
    </footer>
  </form>
</Modal>

<style>
  .vista {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-4);
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .vista-texto {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .fila-logo {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }
  fieldset {
    margin: 0;
    padding: 0;
    border: none;
  }
  legend {
    padding: 0;
    margin-bottom: 6px;
  }
  .colores {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .color {
    width: 28px;
    height: 28px;
    background: var(--marca);
    border: 2px solid transparent;
    border-radius: 999px;
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.08);
    cursor: pointer;
  }
  .color.ninguno {
    width: auto;
    padding: 0 10px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-2);
    background: var(--surface-2);
    border-color: var(--border-strong);
  }
  .color.on {
    border-color: var(--text-1);
    box-shadow:
      inset 0 0 0 2px var(--surface),
      inset 0 0 0 3px rgb(0 0 0 / 0.08);
  }
  .color.ninguno.on {
    color: var(--text-1);
  }
</style>
