<script lang="ts">
  // Servidor → «Notificaciones» (propietario del servidor; api-servidor.md §13):
  // la dirección pública de la consola (para los enlaces), el tope por hora, la
  // hora de los resúmenes, los canales del servidor y el registro de envíos.
  import { onMount } from "svelte";
  import { BellRing, Check, Info } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { DIAS_SEMANA } from "$lib/notificaciones";
  import type { AjustesNotif } from "$lib/tipos";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import BotonCargando from "$lib/componentes/BotonCargando.svelte";
  import CanalesNotificacion from "$lib/componentes/CanalesNotificacion.svelte";
  import RegistroEnvios from "$lib/componentes/RegistroEnvios.svelte";

  /** null: servidor anterior (sin notificaciones); undefined: cargando. */
  let a = $state<AjustesNotif | null | undefined>(undefined);
  let url = $state("");
  let max = $state(10);
  let hora = $state("08:00");
  let dia = $state(1);
  let guardando = $state(false);

  onMount(async () => {
    try {
      a = await api.notifServidor();
      if (a) {
        url = a.url_consola ?? "";
        max = a.max_por_hora;
        hora = a.hora_resumen;
        dia = a.dia_semanal;
      }
    } catch {
      a = null;
    }
  });

  const cambiado = $derived(!!a && ((a.url_consola ?? "") !== url.trim() || a.max_por_hora !== max || a.hora_resumen !== hora || a.dia_semanal !== dia));
  /** La que se ve ahora en el navegador, para proponerla. */
  const propuesta = typeof location !== "undefined" ? location.origin : "";

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    guardando = true;
    try {
      const canales = a?.canales ?? [];
      a = { ...(await api.cambiarNotifServidor({ url_consola: url.trim(), max_por_hora: max, hora_resumen: hora, dia_semanal: dia })), canales };
      url = a.url_consola ?? "";
      avisar("Ajustes de las notificaciones guardados.");
    } catch (err) {
      fallo(err);
    } finally {
      guardando = false;
    }
  }
</script>

{#if a}
  <section class="card p" id="notificaciones" aria-labelledby="t-notificaciones">
    <div class="cab">
      <span class="card-icon"><BellRing size={18} /></span>
      <div class="cab-texto">
        <h2 id="t-notificaciones">Notificaciones <Ayuda id="notificaciones" /></h2>
        <p class="faint">Que los problemas lleguen a quien no abre la consola: por correo, webhook, ntfy o Telegram. Solo cuentan el estado de las copias, nunca contraseñas ni nombres de archivos.</p>
      </div>
    </div>

    <form class="ajustes" onsubmit={guardar}>
      <div class="field ancho">
        <label class="field-label" for="nt-url">Dirección pública de la consola</label>
        <div class="con-boton">
          <input id="nt-url" class="input mono" bind:value={url} placeholder={propuesta || "https://copias.empresa.com:8443"} spellcheck="false" />
          {#if !url && propuesta}<button type="button" class="btn btn-sm" onclick={() => (url = propuesta)}>Usar esta</button>{/if}
        </div>
        <span class="field-hint">Para los enlaces «Ver en la consola» de los mensajes. Sin ella, los mensajes no llevan enlaces.</span>
      </div>
      <div class="field">
        <label class="field-label" for="nt-max">Tope por hora</label>
        <input id="nt-max" class="input" type="number" min="1" max="120" bind:value={max} />
        <span class="field-hint">Mensajes por canal y destinatario; lo demás sale junto.</span>
      </div>
      <div class="field">
        <label class="field-label" for="nt-hora">Resúmenes a las</label>
        <input id="nt-hora" class="input" type="time" bind:value={hora} required />
      </div>
      <div class="field">
        <label class="field-label" for="nt-dia">El semanal, los</label>
        <select id="nt-dia" class="input" bind:value={dia}>
          {#each DIAS_SEMANA as d, i (d)}<option value={i + 1}>{d}</option>{/each}
        </select>
      </div>
      <div class="guardar">
        <BotonCargando class="btn btn-sm btn-primary" type="submit" cargando={guardando} disabled={!cambiado}><Check size={14} />Guardar</BotonCargando>
      </div>
    </form>

    <h3 class="sub">Canales del servidor</h3>
    <p class="faint pequeno">Valen para todos los clientes (o los que elijas). Cada cliente puede tener además los suyos, en «Personas y ajustes». Qué recibe cada persona por correo se elige en Personas.</p>
    <CanalesNotificacion ambito={{ servidor: true }} bind:canales={a.canales} vacio="Aún no hay canales: empieza por el correo." />

    {#if !a.canales.some((c) => c.tipo === "correo")}
      <div class="notice notice-info">
        <Info size={16} />
        <p>Sin un canal de correo, las personas no reciben avisos ni el «Resumen semanal de copias» en su buzón.</p>
      </div>
    {/if}

    <h3 class="sub">Registro de envíos</h3>
    <RegistroEnvios ambito={{ servidor: true }} />
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
  .ajustes {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-3);
    margin: var(--sp-4) 0;
    align-items: start;
  }
  .ancho {
    grid-column: 1 / -1;
  }
  .con-boton {
    display: flex;
    gap: var(--sp-2);
  }
  .con-boton .input {
    flex: 1;
  }
  .mono {
    font-family: var(--mono);
  }
  .guardar {
    grid-column: 1 / -1;
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
  @media (max-width: 640px) {
    .ajustes {
      grid-template-columns: 1fr;
    }
  }
</style>
