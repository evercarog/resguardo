<script lang="ts">
  // Cambiar el nombre de un destino (tarea 7a): solo el catálogo del cliente
  // en el servidor. No manda órdenes ni pide la clave de administración: no
  // cambia nada en los equipos, sus repositorios ni sus kits. «Volver al de
  // siempre» lo quita del catálogo. Un destino suelto (aún sin repositorios)
  // se puede quitar de la lista.
  import { Pencil, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import BotonCargando from "./BotonCargando.svelte";
  import Ayuda from "./Ayuda.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { guardarEnCatalogo, quitarDelCatalogo } from "$lib/catalogoDestinos.svelte";
  import { errorNombreDestino, type DestinoVista } from "$lib/destinos";
  import type { DestinoCatalogo } from "$lib/tipos";

  let { cliente, destino, onclose }: { cliente: string; destino: DestinoVista; onclose: () => void } = $props();

  // El nombre que se ve al abrir (después manda lo que se escriba).
  const inicial = () => destino.nombre;
  let nombre = $state(inicial());
  let ocupado = $state(false);
  let error = $state("");
  const errorNombre = $derived(errorNombreDestino(nombre));
  const suelto = $derived(destino.clase === "suelto");
  const tipo = $derived<DestinoCatalogo["tipo"]>(destino.clase === "zona" ? "zona" : destino.clase === "nube" ? "nube" : (destino.tipo as DestinoCatalogo["tipo"]));

  // Tarea 8: lo marcado para la regla 3-2-1 se queda al volver al nombre de siempre.
  const atributos = $derived(destino.catalogo?.atributos ?? null);
  // Un destino de red (B2, S3, rest, SFTP) lleva su dirección también al renombrarlo (el servidor la pide).
  const donde = $derived(suelto || (["rest", "s3", "b2", "sftp"].includes(tipo) && destino.donde) ? destino.donde : null);
  /** Vuelve al nombre de siempre: sin nada más, lo quita del catálogo; con atributos, los deja. */
  async function alDeSiempre() {
    if (atributos) await guardarEnCatalogo(cliente, destino.clave, { nombre: "", tipo, donde, atributos });
    else await quitarDelCatalogo(cliente, destino.clave);
  }

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    if (errorNombre) return;
    ocupado = true;
    error = "";
    try {
      if (!suelto && nombre.trim() === destino.nombrePorDefecto && destino.catalogo) await alDeSiempre();
      else await guardarEnCatalogo(cliente, destino.clave, { nombre, tipo, donde });
      avisar(`Ahora se llama «${nombre.trim()}».`);
      onclose();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }

  async function quitar() {
    ocupado = true;
    error = "";
    try {
      if (suelto) await quitarDelCatalogo(cliente, destino.clave);
      else await alDeSiempre();
      avisar(suelto ? `«${destino.nombre}» ya no está en la lista.` : `Vuelve a llamarse «${destino.nombrePorDefecto}».`);
      onclose();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }
</script>

<Modal labelledby="t-renombrar" {onclose} width={480}>
  <div class="dlg-title">
    <span class="ticon"><Pencil size={18} /></span>
    <div>
      <h2 id="t-renombrar">Nombre del destino</h2>
      <p>Solo cambia cómo lo ves aquí: los equipos, sus repositorios y sus kits siguen igual. <Ayuda id="catalogo-destinos" /></p>
    </div>
  </div>
  <form class="form" onsubmit={guardar}>
    <div class="field">
      <label class="field-label" for="rd-nombre">Nombre</label>
      <!-- svelte-ignore a11y_autofocus -->
      <input id="rd-nombre" class="input" bind:value={nombre} maxlength="80" autofocus placeholder={destino.nombrePorDefecto} />
      {#if nombre && errorNombre}<p class="error-campo">{errorNombre}</p>{:else if !suelto && destino.nombrePorDefecto !== nombre.trim()}<span class="field-hint">El de siempre: «{destino.nombrePorDefecto}».</span>{/if}
    </div>
    {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
    <footer>
      {#if destino.catalogo && (suelto || destino.renombrado)}
        <button type="button" class="btn btn-ghost izquierda" disabled={ocupado} onclick={quitar}>{suelto ? "Quitar de la lista" : "Volver al de siempre"}</button>
      {/if}
      <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
      <BotonCargando class="btn btn-primary" disabled={!!errorNombre || nombre.trim() === destino.nombre} cargando={ocupado} textoCargando="Guardando…">Guardar</BotonCargando>
    </footer>
  </form>
</Modal>

<style>
  .izquierda {
    margin-right: auto;
  }
</style>
