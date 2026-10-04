<script lang="ts">
  // Qué versiones guarda un repositorio (lib/retencion.ts): reglas de un clic
  // («Programas contables: horarias 15 días, diarias 1 año, mensuales siempre»…) o a
  // medida, con plazos («una por día durante 1 año») y cantidades («las
  // últimas 7 diarias», o «siempre»). Debajo, en palabras y cuántas versiones
  // quedan más o menos con el horario de sus copias.
  //
  // Con un agente sin `admite: ["retencion_plazos"]` solo se ofrecen diarias,
  // semanales, mensuales y anuales (lo que entiende); lo demás, «Actualiza el agente».
  import { untrack } from "svelte";
  import { Info } from "@lucide/svelte";
  import { numero } from "$lib/formato";
  import { copiaRegla, errorRegla, estimarVersiones, leerPlazo, PERIODOS, plazoEnPalabras, presetDe, PRESETS, resumenRegla, SIEMPRE, type Periodo } from "$lib/retencion";
  import type { Regla } from "$lib/tipos";

  let {
    id,
    regla = $bindable(),
    admite,
    horasDelDia,
    copias,
  }: {
    id: string;
    regla: Regla;
    /** ¿Entiende el agente horarias, plazos y «siempre»? (v1.28) */
    admite: boolean;
    /** Las horas de copia de un día (su horario), para estimar; sin él, una cada hora. */
    horasDelDia?: (fecha: Date) => string[];
    /** «unas 24 copias al día» (para la estimación). */
    copias?: string;
  } = $props();

  const NOMBRE: Record<Periodo, string> = { horarias: "Horarias", diarias: "Diarias", semanales: "Semanales", mensuales: "Mensuales", anuales: "Anuales" };
  const UNIDADES = [
    ["h", "horas"],
    ["d", "días"],
    ["m", "meses"],
    ["y", "años"],
  ] as const;
  /** Unidad propuesta al escribir un plazo nuevo en cada fila. */
  const UNIDAD_INICIAL: Record<Periodo, string> = { horarias: "d", diarias: "d", semanales: "m", mensuales: "y", anuales: "y" };

  let personalizada = $state(untrack(() => presetDe(regla) === "personalizada"));
  const elegido = $derived(personalizada ? "personalizada" : presetDe(regla));
  const filas = $derived(PERIODOS.filter((k) => admite || k !== "horarias"));
  const error = $derived(errorRegla(regla, admite));
  const estimacion = $derived(error ? null : estimarVersiones(regla, horasDelDia));

  function elegir(p: string) {
    if (p === "personalizada") {
      personalizada = true;
      return;
    }
    const preset = PRESETS.find((x) => x.id === p);
    if (!preset) return;
    personalizada = false;
    regla = copiaRegla(preset.regla);
  }

  const cantidad = (k: Periodo) => (k === "horarias" ? (regla.horarias ?? 0) : regla[k]);
  function ponerCantidad(k: Periodo, n: number) {
    const v = Number.isFinite(n) ? Math.trunc(n) : 0;
    if (k === "horarias") regla.horarias = v;
    else regla[k] = v;
  }

  /** El plazo de una fila como número y unidad («15d» → 15, «d»); uno compuesto («1y6m») se enseña aparte. */
  function plazo(k: Periodo): { n: number | ""; u: string; compuesto: string | null } {
    const p = regla.plazos?.[k] ?? "";
    const m = /^(\d+)([hdmy])$/.exec(p);
    if (m) return { n: Number(m[1]), u: m[2], compuesto: null };
    return { n: "", u: UNIDAD_INICIAL[k], compuesto: p && leerPlazo(p) ? p : null };
  }
  function ponerPlazo(k: Periodo, n: number | string, u: string) {
    const v = Math.trunc(Number(n));
    regla.plazos = { ...(regla.plazos ?? {}), [k]: v > 0 ? `${v}${u}` : null };
  }
</script>

<div class="editor-ret">
  <fieldset class="presets">
    <legend class="field-label">Qué versiones guarda</legend>
    {#each PRESETS as p (p.id)}
      <label class="preset" class:on={elegido === p.id} class:off={p.plazos && !admite}>
        <input type="radio" name="{id}-preset" value={p.id} checked={elegido === p.id} disabled={p.plazos && !admite} onchange={() => elegir(p.id)} />
        <span>{p.texto}{#if p.plazos && !admite}<span class="faint"> · Actualiza el agente para usar esto</span>{/if}</span>
      </label>
    {/each}
    <label class="preset" class:on={elegido === "personalizada"}>
      <input type="radio" name="{id}-preset" value="personalizada" checked={elegido === "personalizada"} onchange={() => elegir("personalizada")} />
      <span>A medida</span>
    </label>
  </fieldset>

  {#if elegido === "personalizada"}
    <div class="tabla" role="group" aria-label="Reglas a medida">
      <span class="cab"></span>
      {#if admite}<span class="cab">Una por periodo durante</span>{/if}
      <span class="cab">{admite ? "Y además, las últimas" : "Las últimas"}</span>
      {#each filas as k (k)}
        {@const pl = plazo(k)}
        {@const n = cantidad(k)}
        <span class="nombre">{NOMBRE[k]}</span>
        {#if admite}
          <span class="plazo">
            <input
              class="input num"
              type="number"
              min="0"
              max="9999"
              placeholder="—"
              value={pl.n}
              aria-label="{NOMBRE[k]}: durante cuánto tiempo"
              oninput={(e) => ponerPlazo(k, e.currentTarget.value, pl.u)}
            />
            <select class="input" value={pl.u} aria-label="{NOMBRE[k]}: unidad del plazo" onchange={(e) => ponerPlazo(k, pl.n || 1, e.currentTarget.value)}>
              {#each UNIDADES as [u, t] (u)}<option value={u}>{t}</option>{/each}
            </select>
            {#if pl.compuesto}<span class="faint">Ahora: {plazoEnPalabras(pl.compuesto)}</span>{/if}
          </span>
        {/if}
        <span class="cuantas">
          <input
            class="input num"
            type="number"
            min="0"
            max="1000"
            value={n === SIEMPRE ? "" : n}
            disabled={n === SIEMPRE}
            aria-label="{NOMBRE[k]}: cuántas"
            oninput={(e) => ponerCantidad(k, Number(e.currentTarget.value))}
          />
          {#if admite}
            <label class="siempre"><input type="checkbox" checked={n === SIEMPRE} onchange={(e) => ponerCantidad(k, e.currentTarget.checked ? SIEMPRE : 0)} />siempre</label>
          {/if}
        </span>
      {/each}
    </div>
  {/if}

  {#if error}
    <p class="err" role="alert">{error}</p>
  {:else}
    <p class="resumen">{resumenRegla(regla)}</p>
    {#if estimacion}
      <p class="faint estimacion">
        <Info size={14} />
        <span>
          Con {copias ?? "una copia cada hora"}: unas <strong>{numero(estimacion.versiones)} versiones</strong> a la vez{#if estimacion.porAno}, y unas {numero(estimacion.porAno)} más cada año (las de «siempre»){/if}.
          Solo se guarda versión si algo cambió, así que suelen ser menos.
        </span>
      </p>
    {/if}
  {/if}
  {#if !admite}
    <p class="faint nota">Horarias, plazos («una por día durante 1 año») y «siempre» necesitan un agente más nuevo: actualiza el agente para usar «Programas contables».</p>
  {/if}
</div>

<style>
  .editor-ret {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  fieldset {
    margin: 0;
    padding: 0;
    border: none;
  }
  .presets {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .presets legend {
    margin-bottom: 6px;
  }
  .preset {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  .preset.on {
    border-color: var(--accent, var(--text-1));
    background: var(--surface-2, transparent);
  }
  .preset.off {
    cursor: not-allowed;
    color: var(--text-3, var(--text-2));
  }
  .preset input {
    margin-top: 3px;
  }
  .tabla {
    display: grid;
    grid-template-columns: max-content 1fr 1fr;
    gap: 6px var(--sp-3);
    align-items: center;
  }
  .tabla:not(:has(.plazo)) {
    grid-template-columns: max-content 1fr;
  }
  .cab {
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .nombre {
    font-size: var(--fs-sm);
    font-weight: 600;
  }
  .plazo,
  .cuantas {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .plazo .input.num,
  .cuantas .input.num {
    width: 5.5em;
  }
  .plazo select {
    width: auto;
  }
  .siempre {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--fs-sm);
  }
  .resumen,
  .estimacion,
  .nota,
  .err {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .estimacion {
    display: flex;
    gap: 6px;
    align-items: flex-start;
  }
  .estimacion :global(svg) {
    flex: none;
    margin-top: 2px;
  }
  .err {
    color: var(--bad);
  }
  @media (max-width: 520px) {
    .tabla {
      grid-template-columns: 1fr;
    }
    .cab {
      display: none;
    }
  }
</style>
