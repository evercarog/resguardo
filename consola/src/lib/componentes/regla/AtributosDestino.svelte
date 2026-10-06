<script lang="ts">
  // Lo que sabe la regla 3-2-1-1-0 de un destino (tarea 8a y 8e,
  // docs/regla-3-2-1.md): dónde está, si es inmutable y qué soporte es.
  // Por defecto se deduce de su tipo; aquí se cambia. Solo va al catálogo del
  // cliente (en claro, sin secretos): no manda órdenes ni toca ningún equipo.
  // El sistema de archivos y el entorno del almacén se enseñan solo como dato.
  import { ShieldCheck, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import BotonCargando from "../BotonCargando.svelte";
  import Ayuda from "../Ayuda.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { guardarEnCatalogo, quitarDelCatalogo } from "$lib/catalogoDestinos.svelte";
  import { TEXTO_INMUTABLE, TEXTO_LUGAR, textoEntorno, type MarcarDestino } from "$lib/regla321";
  import type { AtributosDestino, InmutableDestino, LugarDestino } from "$lib/tipos";

  let { cliente, destino, onclose }: { cliente: string; destino: MarcarDestino; onclose: () => void } = $props();

  const LUGARES: LugarDestino[] = ["este_equipo", "oficina", "otra_sede", "nube"];
  const INMUTABLES: InmutableDestino[] = ["solo_anadir", "object_lock", "instantaneas", "desconectado", "no"];
  // Lo que hay al abrir (lo marcado o lo deducido); después manda lo que se elija.
  const inicial = () => destino.catalogo?.atributos ?? {};
  const lugarInicial = () => inicial().lugar ?? destino.porDefecto.lugar;
  const inmutableInicial = () => inicial().inmutable ?? destino.porDefecto.inmutable;
  let lugar = $state<LugarDestino>(lugarInicial());
  let inmutable = $state<InmutableDestino>(inmutableInicial());
  let soporte = $state(inicial().soporte ?? "");
  let ocupado = $state(false);
  let error = $state("");

  /** Solo lo que se aparta de lo deducido. */
  const atributos = $derived<AtributosDestino>({
    ...(lugar !== destino.porDefecto.lugar ? { lugar } : {}),
    ...(inmutable !== destino.porDefecto.inmutable ? { inmutable } : {}),
    ...(soporte.trim() ? { soporte: soporte.trim() } : {}),
  });
  const marcado = $derived(Object.keys(atributos).length > 0);
  const errorSoporte = $derived(soporte.trim().length > 60 ? "Hasta 60 caracteres." : null);
  const local = $derived(inmutable === "instantaneas" || inmutable === "desconectado");
  const entorno = $derived(destino.equipo ? textoEntorno(destino.equipo) : null);
  const nombrePropio = $derived(destino.catalogo?.nombre.trim() ?? "");

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    if (errorSoporte) return;
    ocupado = true;
    error = "";
    try {
      if (marcado) await guardarEnCatalogo(cliente, destino.clave, { nombre: nombrePropio, tipo: destino.tipo, donde: destino.donde, atributos });
      else if (nombrePropio) await guardarEnCatalogo(cliente, destino.clave, { nombre: nombrePropio, tipo: destino.tipo, donde: destino.donde, atributos: null });
      else if (destino.catalogo) await quitarDelCatalogo(cliente, destino.clave);
      avisar(marcado ? `Guardado: la regla 3-2-1 cuenta «${destino.nombre}» como lo has marcado.` : `«${destino.nombre}» vuelve a lo deducido de su tipo.`);
      onclose();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }

  function volver() {
    lugar = destino.porDefecto.lugar;
    inmutable = destino.porDefecto.inmutable;
    soporte = "";
  }
</script>

<Modal labelledby="t-atributos" {onclose} width={540}>
  <div class="dlg-title">
    <span class="ticon"><ShieldCheck size={18} /></span>
    <div>
      <h2 id="t-atributos">«{destino.nombre}» en la regla 3-2-1</h2>
      <p>Se deduce de su tipo; cámbialo si no acierta. Solo cambia cómo lo cuenta la consola: los equipos siguen igual. <Ayuda id="regla-321" /></p>
    </div>
  </div>
  <form class="form" onsubmit={guardar}>
    <fieldset class="field">
      <legend class="field-label">Dónde está</legend>
      <div class="opciones">
        {#each LUGARES as l (l)}
          <label class="opcion"><input type="radio" name="at-lugar" value={l} bind:group={lugar} /><span>{TEXTO_LUGAR[l]}{#if l === destino.porDefecto.lugar}<span class="faint"> · lo deducido</span>{/if}</span></label>
        {/each}
      </div>
    </fieldset>
    <div class="field">
      <label class="field-label" for="at-inmutable">¿Se puede borrar desde los equipos? <Ayuda id="instantaneas" /></label>
      <select id="at-inmutable" class="input" bind:value={inmutable}>
        {#each INMUTABLES as i (i)}<option value={i}>{TEXTO_INMUTABLE[i]}{i === destino.porDefecto.inmutable ? " (lo deducido)" : ""}</option>{/each}
      </select>
      {#if local}
        <span class="field-hint">Lo dices tú: el almacén no puede comprobarlo. Cuenta como inmutable, pero es local: no protege de un incendio o un robo de la oficina.</span>
      {:else if inmutable === "solo_anadir" && destino.porDefecto.inmutable !== "solo_anadir"}
        <span class="field-hint">Solo si su servidor es de verdad de solo añadir: la salud de la protección sigue diciendo lo que comprueba el equipo.</span>
      {/if}
    </div>
    <div class="field">
      <label class="field-label" for="at-soporte">Soporte <span class="faint">(opcional)</span></label>
      <input id="at-soporte" class="input" bind:value={soporte} maxlength="60" placeholder="Por ejemplo: Discos USB rotados" aria-invalid={!!errorSoporte} />
      <span class="field-hint">{errorSoporte ?? "Dos destinos con el mismo nombre de soporte cuentan como uno (los mismos discos que se rotan, la misma nube)."}</span>
    </div>
    {#if destino.sistemaArchivos || entorno}
      <p class="dato faint">
        {#if destino.sistemaArchivos}Sistema de archivos: <span class="pastilla mono">{destino.sistemaArchivos}</span>{/if}
        {#if destino.sistemaArchivos && entorno}{" · "}{/if}
        {#if entorno}{entorno}{/if}
        <span class="solo-dato">(solo un dato: no cuenta en la regla)</span>
      </p>
    {/if}
    {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
    <footer>
      {#if marcado}<button type="button" class="btn btn-ghost izquierda" disabled={ocupado} onclick={volver}>Volver a lo deducido</button>{/if}
      <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
      <BotonCargando class="btn btn-primary" disabled={!!errorSoporte} cargando={ocupado} textoCargando="Guardando…">Guardar</BotonCargando>
    </footer>
  </form>
</Modal>

<style>
  fieldset {
    border: 0;
    padding: 0;
    margin: 0;
    min-width: 0;
  }
  .opciones {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }
  .opcion {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm, 8px);
    cursor: pointer;
    font-size: 0.875rem;
  }
  .opcion:has(input:checked) {
    border-color: var(--accent);
    background: var(--accent-soft, transparent);
  }
  .opcion input {
    margin-top: 3px;
  }
  .dato {
    font-size: 0.8125rem;
    margin: 0;
  }
  .solo-dato {
    display: block;
    font-size: 0.75rem;
  }
  .izquierda {
    margin-right: auto;
  }
  @media (max-width: 480px) {
    .opciones {
      grid-template-columns: 1fr;
    }
  }
</style>
