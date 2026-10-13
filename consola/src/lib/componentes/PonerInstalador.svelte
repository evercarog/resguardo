<script lang="ts">
  // 14dd: el servidor no tiene el instalador del agente de Windows. Al propietario del
  // servidor, «Ponerlo ahora»: el servidor toma el de la versión firmada que ya tiene (o la
  // baja antes de GitHub), con su SHA-256 y las comprobaciones de `poner-instalador-agente`.
  // Si no se puede, dice por qué. A los demás, cómo se pone y a quién pedírselo.
  import { Download, TriangleAlert } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { app } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import BotonCargando from "./BotonCargando.svelte";

  interface Props {
    /** Lo que se puede hacer mientras tanto (p. ej. «usa Instalador normal y un código»). */
    mientras?: string;
  }
  let { mientras = "" }: Props = $props();

  let poniendo = $state(false);
  let error = $state("");
  const propietario = $derived(!!app.cuenta?.superusuario);

  async function ponerAhora() {
    poniendo = true;
    error = "";
    try {
      const r = await api.ponerInstaladorAgente();
      avisar(r.version ? `Instalador del agente puesto: el de la ${r.version}, con su firma y su SHA-256 comprobados.` : "Instalador del agente puesto.");
      if (app.servidor) app.servidor = { ...app.servidor, instalador_agente: true };
    } catch (e) {
      error = (e as Error).message;
    } finally {
      poniendo = false;
    }
  }
</script>

<div class="notice notice-info poner">
  <div>
    <p>
      Este servidor aún no tiene el instalador del agente de Windows.
      {#if propietario}
        Puede tomarlo de la versión firmada del agente que baja para las actualizaciones (y, si aún no tiene ninguna, bajarla ahora de GitHub), comprobando su firma y su SHA-256.
      {:else}
        Pídeselo a quien administra el servidor: lo pone con un clic en esta misma página, o con <code>sudo resguardo-server poner-instalador-agente</code>.
      {/if}
      {mientras}
    </p>
    {#if propietario}
      <div class="acc">
        <BotonCargando class="btn btn-sm btn-primary" cargando={poniendo} textoCargando="Poniéndolo… (puede tardar si hay que bajarlo)" onclick={ponerAhora}><Download size={14} />Ponerlo ahora</BotonCargando>
      </div>
      {#if error}<p class="error" role="alert"><TriangleAlert size={14} /> No se pudo: {error}</p>{/if}
      <p class="faint pequeno">También a mano, en la máquina del servidor: <code>sudo resguardo-server poner-instalador-agente Resguardo-Agente-setup.exe</code>.</p>
    {/if}
  </div>
</div>

<style>
  .poner p {
    margin: 0;
  }
  .acc {
    margin-top: var(--sp-3);
  }
  .error {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin-top: var(--sp-2) !important;
    color: var(--bad);
    font-size: var(--fs-sm);
  }
  .pequeno {
    margin-top: var(--sp-2) !important;
    font-size: var(--fs-xs);
  }
</style>
