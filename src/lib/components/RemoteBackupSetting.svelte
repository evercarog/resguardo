<script lang="ts">
  // «Copias a distancia»: permitir que se pida «Copiar ahora» de este equipo
  // desde la web o desde Resguardo en otro equipo de la cuenta. Solo copias:
  // nada se puede borrar, restaurar ni cambiar a distancia.
  import { CircleAlert } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { agent } from "$lib/agent.svelte";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { relaunchToSettings } from "$lib/settingsNav.svelte";
  import { toast } from "$lib/toast.svelte";
  import SettingRow from "./SettingRow.svelte";

  /** El equipo está vinculado con Resguardo Web (sin vínculo no llegan peticiones). */
  let { linked }: { linked: boolean } = $props();

  let error = $state("");
  const info = $derived(agent.info);
  const enabled = $derived(!!info?.remote_backup);
  /** Un destino del agente: su contraseña confirma el cambio. */
  const anchor = $derived(info?.repos[0] ?? null);
  const why = $derived(!anchor ? "Primero programa las copias automáticas de algún repositorio." : !linked && !enabled ? "Primero conecta este equipo con Resguardo Web." : "");

  async function toggle(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const want = input.checked;
    input.checked = enabled; // no cambia hasta que se confirme
    error = "";
    if (!info?.elevated) {
      relaunchToSettings("equipo").catch((err) => (error = String(err)));
      return;
    }
    if (!anchor) return;
    const done = await withPassword({
      title: want ? "Permitir copias a distancia" : "Dejar de permitir copias a distancia",
      message: want
        ? "Desde la web o desde Resguardo en otro equipo de tu cuenta se podrá pedir «Copiar ahora» de las copias de este equipo. Nada se puede borrar, restaurar ni cambiar a distancia."
        : "Ya no se podrá pedir «Copiar ahora» de este equipo desde la web ni desde otros equipos.",
      repoName: anchor.name,
      confirmLabel: want ? "Permitir" : "Dejar de permitir",
      action: async (password) => {
        agent.info = await api.agentSetRemoteBackup(anchor.id, want, password);
      },
    });
    if (done) toast(want ? "Copias a distancia permitidas en este equipo" : "Copias a distancia desactivadas en este equipo");
  }
</script>

<SettingRow
  label="Copias a distancia"
  description="Permitir que pidas «Copiar ahora» de este equipo desde la web o desde Resguardo en otro equipo. Solo copias: nada se puede borrar ni restaurar a distancia."
  needs={["admin", "password"]}
  for="remote-backup"
>
  <input id="remote-backup" type="checkbox" class="switch" checked={enabled} onchange={toggle} disabled={!info || !!why} title={why || undefined} />
  {#snippet status()}
    {#if why}<span class="faint">{why}</span>{/if}
  {/snippet}
  {#snippet below()}
    {#if error}<p class="err" role="alert"><CircleAlert size={14} /> {error}</p>{/if}
  {/snippet}
</SettingRow>

<style>
  .err {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--bad);
  }
</style>
