<script lang="ts">
  // Ajustes «Bandeja y avisos»: seguir en la bandeja al cerrar, iniciar con
  // Windows y avisos de Windows. Son preferencias de este usuario (como el
  // aspecto): no piden contraseña ni administrador.
  import { onMount } from "svelte";
  import { CircleAlert } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { TraySettings } from "$lib/api";
  import SettingRow from "./SettingRow.svelte";

  let s = $state<TraySettings | null>(null);
  let busy = $state(false);
  let error = $state("");

  onMount(() => {
    api.traySettings().then(
      (v) => (s = v),
      (e) => (error = String(e)),
    );
  });

  async function change(key: keyof TraySettings, e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    if (!s || busy) return;
    const want = { ...s, [key]: input.checked };
    input.checked = s[key]; // no cambia hasta que se guarde
    busy = true;
    error = "";
    try {
      s = await api.traySet(want);
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }
</script>

{#if !s}
  <SettingRow label="Al cerrar, seguir en la bandeja" description={error || "Consultando…"} />
{:else}
  <SettingRow
    label="Al cerrar, seguir en la bandeja"
    description="La ventana se oculta y Resguardo sigue junto al reloj: ves el estado de un vistazo. Para salir del todo, usa «Salir» en su menú."
    for="tray-close"
  >
    <input id="tray-close" type="checkbox" class="switch" checked={s.close_to_tray} onchange={(e) => change("close_to_tray", e)} disabled={busy} />
  </SettingRow>
  <SettingRow
    label="Iniciar con Windows (en la bandeja)"
    description="Solo para tu usuario. Las copias automáticas no dependen de esto: las hace el agente aunque Resguardo esté cerrado."
    for="tray-autostart"
  >
    <input id="tray-autostart" type="checkbox" class="switch" checked={s.autostart} onchange={(e) => change("autostart", e)} disabled={busy} />
  </SettingRow>
  <SettingRow
    label="Avisos de Windows"
    description="Si falla una copia, si se frena una subida por un cambio inusual, cuando esa subida termina y, como mucho una vez por semana, si falta el kit. Respetan «No molestar»."
    for="tray-notify"
  >
    <input id="tray-notify" type="checkbox" class="switch" checked={s.notifications} onchange={(e) => change("notifications", e)} disabled={busy} />
    {#snippet below()}
      {#if error}<p class="note" role="alert"><CircleAlert size={14} /> {error}</p>{/if}
    {/snippet}
  </SettingRow>
{/if}

<style>
  .note {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--bad);
  }
</style>
