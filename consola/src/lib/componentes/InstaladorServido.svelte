<script lang="ts">
  // 15x: qué versión del agente reparte el servidor en «Instalar muchos equipos» (y en el
  // instalador listo) y, al propietario del servidor, «Cambiarlo por el de la X» cuando es más
  // viejo que la última versión firmada que tiene o se puso a mano sin versión conocida. El
  // servidor ya lo cambia solo al bajar una versión más nueva, salvo uno puesto a mano que no
  // es de ninguna versión conocida: ese no lo pisa sin preguntar.
  import { RefreshCw, TriangleAlert } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { app } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { describirInstalador, type InstaladorServido } from "$lib/actualizaciones";
  import BotonCargando from "./BotonCargando.svelte";

  interface Props {
    info: InstaladorServido | null | undefined;
    /** Tras cambiarlo (para volver a pedir la huella o el estado). */
    cambiado?: () => void;
    /** Solo el botón (la página ya dice qué versión reparte). */
    soloAccion?: boolean;
  }
  let { info, cambiado, soloAccion = false }: Props = $props();

  const d = $derived(describirInstalador(info));
  const propietario = $derived(!!app.cuenta?.superusuario);
  let poniendo = $state(false);
  let error = $state("");

  async function cambiar() {
    poniendo = true;
    error = "";
    try {
      const r = await api.ponerInstaladorAgente();
      avisar(r.version ? `Listo: ahora se reparte el agente ${r.version}, con su firma y su SHA-256 comprobados.` : "Instalador del agente cambiado.");
      cambiado?.();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      poniendo = false;
    }
  }
</script>

{#if info?.hay}
  <div class="servido" class:aviso={d.aviso}>
    {#if !soloAccion}
      <p>
        {#if d.aviso}<TriangleAlert size={14} aria-hidden="true" />{/if}
        <span>{d.texto}</span>
      </p>
    {/if}
    {#if d.cambiarPor}
      {#if propietario}
        <BotonCargando class="btn btn-sm" cargando={poniendo} textoCargando="Cambiándolo…" onclick={cambiar}><RefreshCw size={14} />Cambiarlo por el de la {d.cambiarPor}</BotonCargando>
      {:else}
        <p class="faint">Quien administra el servidor puede cambiarlo por el de la {d.cambiarPor} en esta misma página.</p>
      {/if}
    {/if}
    {#if error}<p class="error" role="alert"><TriangleAlert size={14} /> No se pudo: {error}</p>{/if}
  </div>
{/if}

<style>
  .servido {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .servido p {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 0;
  }
  .servido.aviso p:first-child {
    color: var(--warn);
  }
  .error {
    color: var(--bad);
  }
</style>
