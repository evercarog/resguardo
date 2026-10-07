<script lang="ts">
  // Tipo y marcas de un destino (0.7.26, bloque 2; antes «dónde está» e
  // «inmutable», tarea 8a y 8e; docs/regla-3-2-1.md). Tipo (uno): Local, Fuera
  // del sitio o Nube. Marcas: Inmutable (con los días del bloqueo de objetos) y
  // Aislado (con los días tras los que avisa). Se deducen del destino; aquí se
  // cambian. Solo va al catálogo del cliente (en claro, sin secretos): no manda
  // órdenes ni toca ningún equipo. El sistema de archivos, el entorno y la
  // última conexión que vio el agente se enseñan solo como dato.
  import { Building2, Cloud, HardDrive, Lock, ShieldCheck, TriangleAlert, Unplug } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import BotonCargando from "../BotonCargando.svelte";
  import Ayuda from "../Ayuda.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { guardarEnCatalogo, quitarDelCatalogo } from "$lib/catalogoDestinos.svelte";
  import { textoEntorno, type MarcarDestino } from "$lib/regla321";
  import { AYUDA_TIPO, atributosDe, clasificar, COMO_INMUTABLE, DIAS_AISLADO, estadoConexion, TEXTO_COMO_INMUTABLE, TEXTO_TIPO, TIPOS_DESTINO, tipoDeLugar } from "$lib/tipoDestino";
  import type { InmutableDestino, TipoDestino } from "$lib/tipos";

  let { cliente, destino, onclose }: { cliente: string; destino: MarcarDestino; onclose: () => void } = $props();

  const ICONO = { local: HardDrive, fuera: Building2, nube: Cloud } as const;
  // Lo que hay al abrir (lo marcado o lo deducido); después manda lo que se elija.
  const inicial = () => clasificar(destino.porDefecto, destino.catalogo?.atributos);
  const deducido = () => clasificar(destino.porDefecto, null);
  let tipo = $state<TipoDestino>(inicial().tipo);
  let inmutable = $state(inicial().inmutable);
  let como = $state<InmutableDestino>(inicial().inmutable ? inicial().como : deducido().inmutable ? deducido().como : "instantaneas");
  let bloqueo = $state<number | null>(inicial().bloqueoDias);
  let aislado = $state(inicial().aislado);
  let aisladoDias = $state<number>(inicial().aisladoDias);
  const soporteInicial = () => destino.catalogo?.atributos?.soporte ?? "";
  let soporte = $state(soporteInicial());
  let ocupado = $state(false);
  let error = $state("");

  /** Solo lo que se aparta de lo deducido (con lo que entiende una consola anterior). */
  const atributos = $derived(atributosDe(destino.porDefecto, { tipo, inmutable, como, aislado, bloqueoDias: bloqueo, aisladoDias, soporte }));
  const marcado = $derived(Object.keys(atributos).length > 0);
  const errorSoporte = $derived(soporte.trim().length > 60 ? "Hasta 60 caracteres." : null);
  const errorBloqueo = $derived(inmutable && como === "object_lock" && bloqueo != null && !(Number.isInteger(bloqueo) && bloqueo >= 1 && bloqueo <= 36500) ? "De 1 a 36 500 días." : null);
  const errorDias = $derived(aislado && !(Number.isInteger(aisladoDias) && aisladoDias >= 1 && aisladoDias <= 365) ? "De 1 a 365 días." : null);
  const invalido = $derived(!!(errorSoporte || errorBloqueo || errorDias));
  const local = $derived(tipo === "local" && (aislado || (inmutable && como === "instantaneas")));
  const entorno = $derived(destino.equipo ? textoEntorno(destino.equipo) : null);
  const nombrePropio = $derived(destino.catalogo?.nombre.trim() ?? "");
  const tipoDeducido = $derived(tipoDeLugar(destino.porDefecto.lugar));
  const conexion = $derived(aislado ? estadoConexion(destino.conexion, aisladoDias, Date.now()) : null);

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    if (invalido) return;
    ocupado = true;
    error = "";
    try {
      if (marcado) await guardarEnCatalogo(cliente, destino.clave, { nombre: nombrePropio, tipo: destino.tipo, donde: destino.donde, atributos });
      else if (nombrePropio) await guardarEnCatalogo(cliente, destino.clave, { nombre: nombrePropio, tipo: destino.tipo, donde: destino.donde, atributos: null });
      else if (destino.catalogo) await quitarDelCatalogo(cliente, destino.clave);
      avisar(marcado ? `Guardado: «${destino.nombre}» queda como lo has marcado.` : `«${destino.nombre}» vuelve a lo deducido.`);
      onclose();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }

  function volver() {
    const d = deducido();
    tipo = d.tipo;
    inmutable = d.inmutable;
    if (d.inmutable) como = d.como;
    bloqueo = d.bloqueoDias;
    aislado = d.aislado;
    aisladoDias = DIAS_AISLADO;
    soporte = "";
  }
</script>

<Modal labelledby="t-atributos" {onclose} width={540}>
  <div class="dlg-title">
    <span class="ticon"><ShieldCheck size={18} /></span>
    <div>
      <h2 id="t-atributos">Tipo y marcas de «{destino.nombre}»</h2>
      <p>Se deducen del destino; cámbialos si no aciertan. Solo cambia cómo lo cuenta la consola. <Ayuda id="tipo-destino" /></p>
    </div>
  </div>
  <form class="form" onsubmit={guardar}>
    <fieldset class="field">
      <legend class="field-label">Tipo</legend>
      <div class="opciones">
        {#each TIPOS_DESTINO as t (t)}
          {@const Icono = ICONO[t]}
          <label class="opcion" title={AYUDA_TIPO[t]}>
            <input type="radio" name="at-tipo" value={t} bind:group={tipo} />
            <span class="op-texto"><span class="op-nombre"><Icono size={15} aria-hidden="true" />{TEXTO_TIPO[t]}</span>{#if t === tipoDeducido}<span class="faint deducido">lo deducido</span>{/if}</span>
          </label>
        {/each}
      </div>
      <span class="field-hint">{AYUDA_TIPO[tipo]}</span>
    </fieldset>
    <fieldset class="field">
      <legend class="field-label">Marcas</legend>
      <label class="marca"><input type="checkbox" bind:checked={inmutable} /><Lock size={15} aria-hidden="true" /><span>Inmutable <span class="faint">· no se puede borrar desde los equipos</span></span></label>
      {#if inmutable}
        <div class="sub">
          <label class="sr-only" for="at-como">Cómo es inmutable</label>
          <select id="at-como" class="input" bind:value={como}>
            {#each COMO_INMUTABLE as c (c)}<option value={c}>{TEXTO_COMO_INMUTABLE[c]}{deducido().inmutable && c === deducido().como ? " (lo deducido)" : ""}</option>{/each}
          </select>
          {#if como === "object_lock"}
            <label class="dias"><span>Bloqueo de</span><input class="input" type="number" min="1" max="36500" step="1" bind:value={bloqueo} aria-invalid={!!errorBloqueo} placeholder="30" /><span>días</span></label>
            {#if errorBloqueo}<span class="field-hint error">{errorBloqueo}</span>{/if}
          {:else if como === "solo_anadir" && deducido().como !== "solo_anadir"}
            <span class="field-hint">Solo si su servidor es de verdad de solo añadir: la salud sigue diciendo lo que comprueba el equipo.</span>
          {:else if como === "instantaneas"}
            <span class="field-hint">Lo dices tú: el almacén no puede comprobarlas. <Ayuda id="instantaneas" /></span>
          {/if}
        </div>
      {/if}
      <label class="marca"><input type="checkbox" bind:checked={aislado} /><Unplug size={15} aria-hidden="true" /><span>Aislado <span class="faint">· se desconecta y se rota</span></span></label>
      {#if aislado}
        <div class="sub">
          <label class="dias"><span>Avisar si no se conecta en</span><input class="input" type="number" min="1" max="365" step="1" bind:value={aisladoDias} aria-invalid={!!errorDias} /><span>días</span></label>
          {#if errorDias}<span class="field-hint error">{errorDias}</span>{/if}
          {#if conexion}
            <p class="conexion" class:tarde={conexion.tarde}>
              {#if conexion.tarde}<TriangleAlert size={14} aria-hidden="true" />{/if}{conexion.texto}
            </p>
            {#if conexion.discos.length}
              <ul class="discos">{#each conexion.discos as d (d.id)}<li class:tarde={d.tarde}>{d.texto}</li>{/each}</ul>
            {/if}
          {/if}
        </div>
      {/if}
      {#if local}<span class="field-hint">Cuenta en la regla, pero es local: no protege de un incendio o un robo del sitio.</span>{/if}
    </fieldset>
    <div class="field">
      <label class="field-label" for="at-soporte">Soporte <span class="faint">(opcional)</span></label>
      <input id="at-soporte" class="input" bind:value={soporte} maxlength="60" placeholder="Por ejemplo: Discos USB rotados" aria-invalid={!!errorSoporte} />
      <span class="field-hint">{errorSoporte ?? "Dos destinos con el mismo soporte cuentan como uno."}</span>
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
      <BotonCargando class="btn btn-primary" disabled={invalido} cargando={ocupado} textoCargando="Guardando…">Guardar</BotonCargando>
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
    grid-template-columns: repeat(3, minmax(0, 1fr));
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
  .op-texto {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .op-nombre {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 500;
  }
  .deducido {
    font-size: 0.75rem;
  }
  .marca {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.875rem;
    padding: 4px 0;
    cursor: pointer;
  }
  .sub {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 2px 0 6px 26px;
  }
  .dias {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.8125rem;
    flex-wrap: wrap;
  }
  .dias .input {
    width: 96px;
  }
  .conexion {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: 0.8125rem;
    color: var(--text-2);
  }
  .conexion.tarde,
  .discos .tarde {
    color: var(--warn);
  }
  .discos {
    margin: 0;
    padding-left: 18px;
    font-size: 0.75rem;
    color: var(--text-2);
  }
  .error {
    color: var(--bad);
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
