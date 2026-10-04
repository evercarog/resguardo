<script lang="ts">
  // Notificaciones de un cliente (sus propietarios; api-servidor.md §13): por
  // dónde salen sus avisos (el correo y los canales del servidor que lo
  // incluyen), sus canales propios (un correo propio sustituye al del
  // servidor; los demás se suman) y su registro de envíos.
  import { BellRing, Info, Mail } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { TIPO_CANAL, textoSeveridades } from "$lib/notificaciones";
  import type { NotifCliente } from "$lib/tipos";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import CanalesNotificacion from "$lib/componentes/CanalesNotificacion.svelte";
  import RegistroEnvios from "$lib/componentes/RegistroEnvios.svelte";

  let { cliente }: { cliente: string } = $props();

  /** null: servidor anterior (sin notificaciones); undefined: cargando. */
  let n = $state<NotifCliente | null | undefined>(undefined);
  async function cargar() {
    try {
      n = await api.notifCliente(cliente);
    } catch {
      n = null;
    }
  }
  $effect(() => {
    void cliente;
    void cargar();
  });
</script>

{#if n}
  <section class="card p" id="sec-notificaciones" aria-labelledby="t-notif-cliente">
    <div class="cab">
      <span class="card-icon"><BellRing size={18} /></span>
      <div class="cab-texto">
        <h2 id="t-notif-cliente">Notificaciones <Ayuda id="notificaciones" /></h2>
        <p class="faint">Por dónde salen los avisos de este cliente. Qué recibe cada persona por correo se elige arriba, en su fila (el botón de la campana).</p>
      </div>
    </div>

    <ul class="por-donde">
      <li>
        <Mail size={15} />
        {#if n.correo}
          <span>Correo: <strong>{n.correo.nombre}</strong> {n.correo.de === "servidor" ? "(el del servidor)" : "(propio de este cliente)"}.</span>
        {:else}
          <span>Sin correo: el propietario del servidor aún no lo ha puesto (o añade uno propio abajo).</span>
        {/if}
      </li>
      {#each n.servidor.canales as k (k.nombre + k.tipo)}
        <li><BellRing size={15} /><span>{TIPO_CANAL[k.tipo].texto} del servidor: <strong>{k.nombre}</strong> · {textoSeveridades(k.severidades).toLowerCase()}.</span></li>
      {/each}
    </ul>
    {#if !n.servidor.url_consola}
      <div class="notice notice-info"><Info size={16} /><p>Los mensajes aún no llevan enlaces a la consola: falta que el propietario del servidor ponga su dirección pública.</p></div>
    {/if}

    <h3 class="sub">Canales propios</h3>
    <p class="faint pequeno">Por ejemplo, el Telegram o el webhook del equipo de soporte de este cliente. Un correo propio sustituye al del servidor para sus personas.</p>
    <!-- Al cambiar los canales propios, el correo que vale puede ser otro: se vuelve a leer. -->
    <CanalesNotificacion ambito={{ cliente }} bind:canales={n.canales} vacio="Ninguno: salen por los del servidor." alCambiar={cargar} />

    <h3 class="sub">Registro de envíos</h3>
    <RegistroEnvios ambito={{ cliente }} />
  </section>
{/if}

<style>
  .cab {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
  }
  .cab-texto {
    flex: 1;
    min-width: 0;
  }
  .cab h2 {
    display: flex;
    align-items: center;
    margin: 0;
    font-size: var(--fs-md, 15px);
  }
  .cab p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .por-donde {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: var(--sp-4) 0 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-sm);
  }
  .por-donde li {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }
  .por-donde :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--text-3);
  }
  .sub {
    margin: var(--sp-5) 0 var(--sp-1);
    font-size: var(--fs-sm);
    font-weight: 600;
  }
  .pequeno {
    margin: 0 0 var(--sp-3);
    font-size: var(--fs-xs);
  }
  .card > .notice {
    margin-top: var(--sp-3);
  }
</style>
