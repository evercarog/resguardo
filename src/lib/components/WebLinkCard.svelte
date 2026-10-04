<script lang="ts">
  // Resguardo Web en «Estado»: solo el estado del vínculo y de las copias a
  // distancia. Se cambia en Ajustes → Este equipo.
  import { onMount } from "svelte";
  import { ArrowRight, CircleAlert, Globe } from "@lucide/svelte";
  import { agent } from "$lib/agent.svelte";
  import { openSettings } from "$lib/settingsNav.svelte";
  import { refreshWeb, web } from "$lib/web.svelte";
  import RelTime from "./RelTime.svelte";

  onMount(refreshWeb);

  const info = $derived(web.info);
  const link = $derived(info?.link ?? null);
  const linked = $derived(!!link && !link.revoked);
</script>

<section class="card web">
  <span class="card-icon" class:on={linked}><Globe size={18} /></span>
  <div class="text">
    <strong>Resguardo Web</strong>
    <span class="faint">
      {#if !info}
        {web.error || "Consultando…"}
      {:else if link?.revoked}
        La web desvinculó este equipo.
      {:else if link}
        Conectado como <strong>{link.device_name}</strong>
        {#if info.state.last_report}· último informe <RelTime iso={info.state.last_report} />{:else}· esperando el primer informe{/if}
        · copias a distancia {agent.info?.remote_backup ? "permitidas" : "no permitidas"}
      {:else}
        No conectado. Conéctalo para ver el estado de las copias de este equipo desde el navegador o el móvil.
      {/if}
    </span>
    {#if info?.state.last_error && linked}
      <span class="err"><CircleAlert size={13} /> Último envío fallido: {info.state.last_error}</span>
    {/if}
  </div>
  <button class="btn btn-ghost btn-sm" onclick={() => openSettings("equipo")}>Cambiar en Ajustes <ArrowRight size={13} /></button>
</section>

<style>
  .web {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 12px;
    padding: 14px 16px;
  }
  .text {
    display: flex;
    flex-direction: column;
    flex: 1 1 240px;
    min-width: 0;
  }
  .text .faint {
    font-size: var(--fs-sm);
  }
  .text .faint strong {
    color: var(--text-2);
  }
  .err {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: var(--fs-sm);
    color: var(--warn);
  }
</style>
