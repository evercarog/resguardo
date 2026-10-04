<script lang="ts">
  // Ajuste «Ver versiones en Resguardo» en el menú del Explorador (clic
  // derecho en un archivo o carpeta). Solo para este usuario; sin contraseña.
  import { onMount } from "svelte";
  import { CircleAlert } from "@lucide/svelte";
  import * as api from "$lib/api";
  import SettingRow from "./SettingRow.svelte";

  let enabled = $state<boolean | null>(null);
  let busy = $state(false);
  let error = $state("");

  onMount(() => {
    api.shellMenuStatus().then(
      (v) => (enabled = v),
      (e) => (error = String(e)),
    );
  });

  async function change(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const want = input.checked;
    input.checked = !!enabled;
    if (busy) return;
    busy = true;
    error = "";
    try {
      enabled = await api.shellMenuSet(want);
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }
</script>

<SettingRow
  label="«Ver versiones en Resguardo» al hacer clic derecho"
  description={enabled === null ? error || "Consultando…" : "En un archivo o carpeta del Explorador: ves sus versiones y restauras la que quieras con otro nombre, sin reemplazar nada. En Windows 11, en «Mostrar más opciones»."}
  for="shell-menu"
>
  {#if enabled !== null}
    <input id="shell-menu" type="checkbox" class="switch" checked={enabled} onchange={change} disabled={busy} />
  {/if}
  {#snippet below()}
    {#if error && enabled !== null}<p class="note" role="alert"><CircleAlert size={14} /> {error}</p>{/if}
  {/snippet}
</SettingRow>

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
