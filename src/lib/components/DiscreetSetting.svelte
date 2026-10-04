<script lang="ts">
  // «Modo discreto»: mientras se trabaja (unos días y horas), el agente hace
  // copias, subidas, verificaciones y pruebas de restauración con prioridad
  // baja y, si se quiere, con la subida limitada. Cambia la configuración del
  // agente: pide administrador y la contraseña de uno de sus destinos.
  import { untrack } from "svelte";
  import { CircleAlert, Feather, Info, Lock, ShieldCheck } from "@lucide/svelte";
  import * as api from "$lib/api";
  import type { Discreet } from "$lib/api";
  import { agent } from "$lib/agent.svelte";
  import { DISCREET_DEFAULT, discreetActive, discreetSummary, uploadLabel } from "$lib/discreet";
  import { withPassword } from "$lib/passwordPrompt.svelte";
  import { toast } from "$lib/toast.svelte";
  import SettingRow from "./SettingRow.svelte";
  import Advanced from "./Advanced.svelte";
  import { relaunchToSettings } from "$lib/settingsNav.svelte";

  const DAY_LETTERS = ["L", "M", "X", "J", "V", "S", "D"];
  const DAY_NAMES = ["Lunes", "Martes", "Miércoles", "Jueves", "Viernes", "Sábado", "Domingo"];
  /** Límites de subida que se ofrecen (KiB/s). */
  const LIMITS = [512, 1024, 2048, 5120, 10240, 20480, 51200];

  const info = $derived(agent.info);
  const saved = $derived(info?.discreet ?? null);
  /** Un destino del agente: su contraseña confirma el cambio. */
  const anchor = $derived(info?.repos[0] ?? null);

  // Borrador: lo guardado o, si no hay, lunes a sábado de 7 a 19.
  const start = untrack(() => saved ?? DISCREET_DEFAULT);
  let enabled = $state(untrack(() => !!saved));
  let days = $state<number[]>([...start.days]);
  let from = $state(start.from);
  let to = $state(start.to);
  let limitOn = $state(!!start.upload_kib);
  let limit = $state(start.upload_kib ?? 2048);
  let error = $state("");

  const draft = $derived<Discreet | null>(enabled ? { days: [...days].sort(), from, to, upload_kib: limitOn ? limit : null } : null);
  const changed = $derived(JSON.stringify(draft) !== JSON.stringify(saved ? { ...saved, upload_kib: saved.upload_kib ?? null, days: [...saved.days].sort() } : null));
  const invalid = $derived(enabled && (!days.length || !from || !to || from === to));
  const activeNow = $derived(discreetActive(saved));

  function toggleDay(i: number) {
    days = days.includes(i) ? days.filter((d) => d !== i) : [...days, i];
  }

  async function save() {
    error = "";
    if (!info?.elevated) {
      relaunchToSettings("equipo").catch((e) => (error = String(e)));
      return;
    }
    if (!anchor || invalid) return;
    const value = draft;
    const done = await withPassword({
      title: value ? "Modo discreto" : "Quitar el modo discreto",
      message: value
        ? `Las copias automáticas, subidas, verificaciones y pruebas de restauración irán con prioridad baja ${discreetSummary(value)}${value.upload_kib ? `, con la subida a repositorios remotos limitada a ${uploadLabel(value.upload_kib)}` : ""}.`
        : "Las copias automáticas volverán a ir siempre con prioridad normal.",
      repoName: anchor.name,
      confirmLabel: "Guardar",
      action: async (password) => {
        agent.info = await api.agentSetDiscreet(anchor.id, value, password);
      },
    });
    if (done) toast(value ? "Modo discreto guardado" : "Modo discreto desactivado", "success");
  }
</script>

<SettingRow
  label="Modo discreto: mientras se trabaja, copiar con prioridad baja"
  description="En esos días y horas, las copias automáticas, subidas, verificaciones y pruebas de restauración ceden el equipo a lo que estés haciendo: tardan algo más, pero no se notan."
  needs={["admin", "password"]}
  for="discreet-on"
>
  <input id="discreet-on" type="checkbox" class="switch" bind:checked={enabled} disabled={!info || !anchor} title={!anchor ? "Primero programa las copias automáticas de algún repositorio" : undefined} />
  {#snippet status()}
    {#if activeNow && saved}
      <span class="now"><Feather size={13} /> Ahora en marcha: las copias van con prioridad baja ({discreetSummary(saved)}).</span>
    {:else if saved}
      <span>Activado: {discreetSummary(saved)}{saved.upload_kib ? `, subida limitada a ${uploadLabel(saved.upload_kib)}` : ""}.</span>
    {/if}
  {/snippet}
  {#snippet below()}
    {#if enabled}
      <div class="form">
        <div class="days" role="group" aria-label="Días">
          {#each DAY_LETTERS as letter, i (i)}
            <button type="button" class="day" class:on={days.includes(i)} aria-pressed={days.includes(i)} aria-label={DAY_NAMES[i]} title={DAY_NAMES[i]} onclick={() => toggleDay(i)}>
              {letter}
            </button>
          {/each}
        </div>
        <div class="hours">
          <span class="faint">De</span>
          <input class="input" type="time" bind:value={from} aria-label="Desde" />
          <span class="faint">a</span>
          <input class="input" type="time" bind:value={to} aria-label="Hasta" />
        </div>
        <Advanced id="discreto" hint={limitOn ? `Subida limitada a ${uploadLabel(limit)}` : "Sin límite de subida"} custom={limitOn}>
          <div class="hours">
            <label class="inline"><input type="checkbox" bind:checked={limitOn} /> Limitar la subida a repositorios remotos a</label>
            <select class="input" bind:value={limit} disabled={!limitOn} aria-label="Límite de subida">
              {#each LIMITS as k (k)}<option value={k}>{uploadLabel(k)}</option>{/each}
            </select>
          </div>
        </Advanced>
      </div>
    {/if}
    {#if !anchor && info}
      <p class="note faint"><Info size={14} /> Disponible cuando programes las copias automáticas de algún repositorio.</p>
    {/if}
    {#if invalid}<p class="note err"><CircleAlert size={14} /> Elige al menos un día y dos horas distintas.</p>{/if}
    {#if error}<p class="note err" role="alert"><CircleAlert size={14} /> {error}</p>{/if}
    {#if changed && anchor}
      <div class="actions">
        {#if info?.elevated}
          <button class="btn btn-primary btn-sm" onclick={save} disabled={invalid}><Lock size={12} /> Guardar</button>
        {:else}
          <button class="btn btn-primary btn-sm" onclick={save}><ShieldCheck size={13} /> Abrir como administrador para guardar</button>
        {/if}
      </div>
    {/if}
  {/snippet}
</SettingRow>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .days {
    display: flex;
    gap: 6px;
  }
  .day {
    width: 34px;
    height: 32px;
    font: inherit;
    font-weight: 650;
    color: var(--text-2);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .day.on {
    color: var(--accent-contrast);
    background: var(--accent);
    border-color: var(--accent);
  }
  .hours {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .hours .input {
    width: auto;
    height: 32px;
  }
  .inline {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }
  .note {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 8px 0 0;
    font-size: var(--fs-sm);
    line-height: 1.45;
  }
  .note :global(svg) {
    flex: none;
  }
  .now {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--info);
  }
  .err {
    color: var(--bad);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 10px;
  }
</style>
