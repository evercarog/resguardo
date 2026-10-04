<script lang="ts">
  // Exportar o importar el historial de un cliente como paquete cifrado
  // (.resguardo-cliente, api-servidor.md §11). Todo se cifra y se descifra
  // aquí con K_exp (de la clave de administración y la sal del cliente).
  //  - Exportar (en el antiguo): se descarga y, si se quiere, se guarda
  //    también cifrado en este servidor (PUT paquete).
  //  - Importar (en el nuevo, propietario): se elige el archivo (o la copia
  //    guardada aquí), se abre y se manda el historial; la actividad se
  //    acepta solo si su cadena está entera, y una sola vez.
  import { onDestroy } from "svelte";
  import { Archive, Download, FileUp, KeyRound, LoaderCircle, TriangleAlert, Upload } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { guardar } from "$lib/descarga";
  import { abrir, cifrar, importar, juntar, nombreArchivo, type ContenidoPaquete } from "$lib/exportar";
  import { bytes, fechaLarga, numero, plural } from "$lib/formato";
  import type { Cliente } from "$lib/tipos";
  import Ayuda from "./Ayuda.svelte";
  import CampoClave from "./CampoClave.svelte";

  let { cliente, modo, onclose, alTerminar }: { cliente: Cliente; modo: "exportar" | "importar"; onclose: () => void; alTerminar: (texto: string) => void } = $props();

  let claveAdmin = $state("");
  let guardarAqui = $state(true);
  let archivo = $state<File | null>(null);
  let origen = $state<"archivo" | "guardado">("archivo");
  let contenido = $state<ContenidoPaquete | null>(null);
  let ocupado = $state(false);
  let pasoTxt = $state("");
  let error = $state("");

  onDestroy(() => {
    claveAdmin = "";
    contenido = null;
  });

  async function exportar(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    ocupado = true;
    try {
      const c = await juntar(cliente, (t) => (pasoTxt = t));
      pasoTxt = "Cifrando en este navegador…";
      const p = await cifrar(claveAdmin, cliente, c);
      claveAdmin = "";
      if (guardarAqui) {
        pasoTxt = `Guardando la copia cifrada (${bytes(p.length)})…`;
        await api.subirPaquete(cliente.id, p);
      }
      await guardar(new Blob([new Uint8Array(p)], { type: "application/octet-stream" }), nombreArchivo(cliente));
      alTerminar(`Historial exportado: ${plural(c.auditoria.length, "entrada", "entradas")} de actividad, ${plural(c.informes.length, "informe", "informes")} y ${plural(c.avisos.length, "aviso", "avisos")}.`);
      onclose();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }

  async function abrirPaquete(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    ocupado = true;
    try {
      let p: Uint8Array | null;
      if (origen === "guardado") {
        pasoTxt = "Bajando la copia guardada…";
        p = await api.bajarPaquete(cliente.id);
        if (!p) throw new Error("No hay ninguna copia guardada en este servidor.");
      } else {
        if (!archivo) return;
        p = new Uint8Array(await archivo.arrayBuffer());
      }
      pasoTxt = "Abriendo en este navegador…";
      contenido = await abrir(claveAdmin, cliente, p);
      claveAdmin = "";
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }

  async function confirmarImportar() {
    if (!contenido) return;
    error = "";
    ocupado = true;
    pasoTxt = "Importando…";
    try {
      await importar(cliente.id, contenido);
      alTerminar(`Historial de ${contenido.origen} importado.`);
      contenido = null;
      onclose();
    } catch (err) {
      error = err instanceof api.ApiError && err.estado === 409 ? "Este cliente ya tiene un historial importado: solo se puede importar una vez." : (err as Error).message;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }
</script>

<Modal labelledby="t-historial" {onclose} width={540} dismissible={false}>
  <div class="dlg-title">
    <span class="ticon"><Archive size={18} /></span>
    <div>
      {#if modo === "exportar"}
        <h2 id="t-historial">Exportar el historial <Ayuda id="paquete" /></h2>
        <p>Equipos, configuraciones cifradas, informes, avisos y toda la actividad con su cadena de huellas, en un archivo que se cifra aquí. Ningún servidor puede leerlo.</p>
      {:else}
        <h2 id="t-historial">Importar el historial de otro servidor <Ayuda id="paquete" /></h2>
        <p>El paquete que se exportó en el servidor antiguo. Se abre en este navegador con la clave de administración del cliente.</p>
      {/if}
    </div>
  </div>

  {#if modo === "exportar"}
    <form class="form" onsubmit={exportar}>
      <label class="check"><input type="checkbox" bind:checked={guardarAqui} />Guardar también una copia cifrada en este servidor (hasta 64 MB)</label>
      <CampoClave requerido id="clave-admin" etiqueta="Clave de administración" bind:value={claveAdmin}>
        {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
      </CampoClave>
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
        <button class="btn btn-primary" disabled={ocupado || !claveAdmin}><Download size={15} />Cifrar y descargar</button>
      </footer>
    </form>
  {:else if !contenido}
    <form class="form" onsubmit={abrirPaquete}>
      <div class="opciones">
        <label class="check"><input type="radio" bind:group={origen} value="archivo" />Desde un archivo</label>
        <label class="check"><input type="radio" bind:group={origen} value="guardado" />La copia guardada en este servidor</label>
      </div>
      {#if origen === "archivo"}
        <label class="archivo">
          <FileUp size={16} />
          <span>{archivo ? `${archivo.name} · ${bytes(archivo.size)}` : "Elige el archivo .resguardo-cliente"}</span>
          <input type="file" accept=".resguardo-cliente,application/octet-stream" onchange={(e) => (archivo = e.currentTarget.files?.[0] ?? null)} />
        </label>
      {/if}
      <CampoClave requerido id="clave-admin" etiqueta="Clave de administración" bind:value={claveAdmin}>
        {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
      </CampoClave>
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
        <button class="btn btn-primary" disabled={ocupado || !claveAdmin || (origen === "archivo" && !archivo)}><KeyRound size={15} />Abrir el paquete</button>
      </footer>
    </form>
  {:else}
    <div class="form">
      <dl class="resumen">
        <div><dt>De</dt><dd>{contenido.origen}</dd></div>
        <div><dt>Exportado</dt><dd>{fechaLarga(contenido.exportado)}</dd></div>
        <div><dt>Equipos</dt><dd>{contenido.equipos.map((e) => e.nombre).join(", ") || "—"}</dd></div>
        <div><dt>Actividad</dt><dd>{plural(contenido.auditoria.length, "entrada", "entradas")}</dd></div>
        <div><dt>Informes y avisos</dt><dd>{numero(contenido.informes.length)} · {numero(contenido.avisos.length)}</dd></div>
      </dl>
      <div class="notice notice-info"><p>La actividad del servidor antiguo se guarda aparte, tal cual, y la de aquí la enlaza con su última huella. Solo se puede importar una vez.</p></div>
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
        <button class="btn btn-primary" disabled={ocupado} onclick={confirmarImportar}><Upload size={15} />Importar el historial</button>
      </footer>
    </div>
  {/if}
</Modal>

<style>
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .opciones {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-4);
  }
  .archivo {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 14px;
    font-size: var(--fs-sm);
    color: var(--text-2);
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
    cursor: pointer;
  }
  .archivo input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
  }
  .resumen {
    display: grid;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
  }
  .resumen div {
    display: grid;
    grid-template-columns: 140px 1fr;
    gap: 8px;
  }
  .resumen dt {
    color: var(--text-3);
  }
  .resumen dd {
    margin: 0;
    word-break: break-word;
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
</style>
