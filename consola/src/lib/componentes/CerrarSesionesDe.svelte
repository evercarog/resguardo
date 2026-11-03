<script lang="ts">
  // 16k: el propietario del servidor cierra todas las sesiones de otra persona (quien
  // perdió su único dispositivo y no puede esperar las 24 h de una sesión nueva). Con
  // su contraseña y su código; queda en la auditoría del servidor.
  import { LogOut } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import CampoClave from "./CampoClave.svelte";
  import CampoCodigo from "./CampoCodigo.svelte";
  import * as api from "$lib/api";
  import { avisar } from "$lib/avisos.svelte";

  let { cuenta, nombre, onclose }: { cuenta: string; nombre: string; onclose: () => void } = $props();
  let contrasena = $state("");
  let codigo = $state("");
  let ocupado = $state(false);
  let error = $state("");
  const listo = $derived(!!contrasena && codigo.replace(/\D/g, "").length === 6);

  async function enviar(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    ocupado = true;
    try {
      const r = await api.cerrarSesionesDeOtra(cuenta, contrasena, codigo.replace(/\D/g, ""));
      avisar(r.cerradas === 0 ? `${nombre} no tenía ninguna sesión abierta.` : `Cerradas las ${r.cerradas === 1 ? "sesión" : `${r.cerradas} sesiones`} de ${nombre}.`);
      onclose();
    } catch (err) {
      error = err instanceof api.ApiError && err.estado === 404 ? "Este servidor aún no lo admite: actualiza Resguardo Server." : (err as Error).message;
    } finally {
      ocupado = false;
      contrasena = codigo = "";
    }
  }
</script>

<Modal labelledby="t-cerrar-de" {onclose} width={460} dismissible={false} {ocupado}>
  <div class="dlg-title">
    <span class="ticon"><LogOut size={18} /></span>
    <div>
      <h2 id="t-cerrar-de">Cerrar todas las sesiones de {nombre}</h2>
      <p>Tendrá que volver a entrar en todos sus dispositivos. Úsalo si perdió el móvil o el ordenador donde tenía la sesión.</p>
    </div>
  </div>
  <form class="form" onsubmit={enviar}>
    <CampoClave requerido id="cerrar-de-contrasena" etiqueta="Tu contraseña" bind:value={contrasena} autocomplete="current-password" />
    <CampoCodigo bind:value={codigo} />
    <p class="faint pequeno">El código de tu aplicación de verificación. Queda en la actividad del servidor.</p>
    {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
    <footer>
      <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
      <button class="btn btn-danger" disabled={ocupado || !listo}>{ocupado ? "Cerrando…" : "Cerrar sus sesiones"}</button>
    </footer>
  </form>
</Modal>

<style>
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    flex-wrap: wrap;
  }
  .pequeno {
    font-size: var(--fs-sm);
    margin: 0;
  }
</style>
