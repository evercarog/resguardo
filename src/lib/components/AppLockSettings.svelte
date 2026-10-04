<script lang="ts">
  // Ajuste «Pedir Windows Hello al abrir Resguardo». Cualquier cambio pide pasar la comprobación.
  import { onMount } from "svelte";
  import { CircleAlert, Info, LoaderCircle } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { appLock, refreshLockStatus } from "$lib/lock.svelte";
  import { toast } from "$lib/toast.svelte";
  import SettingRow from "./SettingRow.svelte";

  const IDLE = [5, 10, 15, 30, 60];

  let busy = $state(false);
  let error = $state("");
  const s = $derived(appLock.status);
  const available = $derived(s?.availability === "available");

  onMount(() => {
    refreshLockStatus().catch(() => {});
  });

  async function apply(enabled: boolean, idle: number | null) {
    if (busy) return;
    busy = true;
    error = "";
    try {
      appLock.status = await api.appLockSet(enabled, idle);
      toast(
        !enabled
          ? "Resguardo ya no pedirá Windows Hello al abrirse"
          : idle
            ? `Resguardo pedirá Windows Hello al abrirse y tras ${idle} minutos sin usarlo`
            : "Resguardo pedirá Windows Hello al abrirse",
        "success",
      );
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function toggle(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const want = input.checked;
    // El interruptor no cambia hasta que Windows lo confirme.
    input.checked = !!s?.enabled;
    void apply(want, s?.idle_minutes ?? null);
  }

  function changeIdle(e: Event) {
    const v = (e.currentTarget as HTMLSelectElement).value;
    const idle = v ? Number(v) : null;
    (e.currentTarget as HTMLSelectElement).value = s?.idle_minutes ? String(s.idle_minutes) : "";
    void apply(true, idle);
  }
</script>

{#if !s}
  <SettingRow label="Pedir Windows Hello al abrir Resguardo" description="Consultando…" needs={["hello"]} />
{:else}
  <SettingRow
    label="Pedir Windows Hello al abrir Resguardo"
    description="Rostro, huella o PIN. Así, en un equipo compartido, nadie más puede ver ni restaurar tus archivos desde aquí. Las copias automáticas no se ven afectadas."
    needs={["hello"]}
    for="lock-enabled"
  >
    <input id="lock-enabled" type="checkbox" class="switch" checked={s.enabled} onchange={toggle} disabled={busy || (!s.enabled && !available)} title={!s.enabled && !available ? s.explain : undefined} />
    {#snippet below()}
      {#if busy}
        <p class="note"><span class="spin" style="display:grid"><LoaderCircle size={14} /></span> Confirma que eres tú en la ventana de Windows…</p>
      {:else if error}
        <p class="note err" role="alert"><CircleAlert size={14} /> {error}</p>
      {:else if !available && !s.enabled}
        <p class="note faint"><Info size={14} /> {s.explain}</p>
      {/if}
    {/snippet}
  </SettingRow>
  <SettingRow label="Volver a pedirla tras un rato sin usar la app" description="Además de al abrir, cuando lleves estos minutos sin tocar Resguardo." needs={["hello"]} for="lock-idle">
    <select id="lock-idle" class="input" value={s.idle_minutes ? String(s.idle_minutes) : ""} onchange={changeIdle} disabled={busy || !s.enabled}>
      <option value="">Solo al abrir</option>
      {#each IDLE as m (m)}<option value={String(m)}>Tras {m} minutos</option>{/each}
    </select>
  </SettingRow>
{/if}

<style>
  .note {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
    line-height: 1.45;
  }
  .note :global(svg) {
    flex: none;
  }
  .err {
    color: var(--bad);
  }
  select {
    width: auto;
    height: 32px;
  }
</style>
