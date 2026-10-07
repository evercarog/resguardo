<script lang="ts">
  // Cuándo empieza una copia (plan 0.7.26, bloque 3): «Con horario», «En
  // cadena» (solo si la anterior sale bien) o «Después de la anterior»
  // (siempre). En las dos últimas no hay horario: «Inmediatamente» o «Con
  // retraso de N min». La primera solo puede ir con horario.
  //
  // El mismo control en el modo guiado (con plantillas rápidas y
  // «Personalizar») y en el avanzado (el editor completo a la vista): los dos
  // cambian la copia con las funciones de lib/copiaGuiada.ts.
  import { untrack } from "svelte";
  import { CalendarClock, Link2, Repeat, Settings2, Timer } from "@lucide/svelte";
  import { tip } from "$lib/tooltip";
  import EditorHorario from "$lib/componentes/EditorHorario.svelte";
  import { aplicarPlantilla, AYUDA_INICIO, inicioDe, plantillaDe, ponerInicio, ponerRetraso, RETRASOS, retrasoEnFrase, TEXTO_INICIO, TEXTO_PLANTILLA, type ModoInicio, type PlantillaHorario } from "$lib/copiaGuiada";
  import { resumenHorario } from "$lib/formato";
  import type { CopiaConfig, Horario } from "$lib/tipos";

  let {
    copias,
    i,
    admite,
    version = null,
    equipo,
    guiado = false,
  }: {
    copias: CopiaConfig[];
    i: number;
    admite: { reglas: boolean; cadenas: boolean; despues: boolean };
    version?: string | null;
    /** El nombre del equipo (para «Actualiza el agente de …»). */
    equipo: string;
    /** Con plantillas rápidas y «Personalizar» (si no, el editor completo). */
    guiado?: boolean;
  } = $props();

  const k = $derived(copias[i]);
  const modo = $derived(inicioDe(k));
  // El horario que tenía antes de ir en cadena, para volver a él.
  let antes = $state<Horario | null>(untrack(() => (copias[i]?.tras ? null : JSON.parse(JSON.stringify(copias[i]?.horario ?? null)))));
  function elegir(m: ModoInicio) {
    if (m === modo) return;
    if (modo === "horario") antes = JSON.parse(JSON.stringify(k.horario)) as Horario;
    ponerInicio(copias, i, m, antes);
  }
  const motivo = (m: ModoInicio): string | null => {
    if (m === "horario") return null;
    if (i === 0) return "La primera empieza con su horario";
    if (!admite.cadenas) return `Actualiza el agente de ${equipo} para encadenar copias`;
    if (m === "despues" && !admite.despues) return `Actualiza el agente de ${equipo} para «Después de la anterior»`;
    return null;
  };
  const anterior = $derived(i > 0 ? copias[i - 1] : null);
  const otraAnterior = $derived(k.tras && k.tras !== anterior?.id ? copias.find((x) => x.id === k.tras) : undefined);

  // Plantillas rápidas (solo en el guiado).
  const pl = $derived(plantillaDe(k.horario));
  let personalizar = $state(untrack(() => plantillaDe(copias[i]?.horario).plantilla === "personalizado"));
  let hora = $state(untrack(() => {
    const p = plantillaDe(copias[i]?.horario);
    return "hora" in p && p.hora ? p.hora : "21:00";
  }));
  function plantilla(p: PlantillaHorario) {
    personalizar = false;
    aplicarPlantilla(k, p, hora, admite.reglas);
  }
  function cambiarHora(h: string) {
    hora = h;
    if (pl.plantilla === "cada_dia" || pl.plantilla === "laborables") aplicarPlantilla(k, pl.plantilla, h, admite.reglas);
  }
  const ICONO = { horario: CalendarClock, cadena: Link2, despues: Repeat } as const;
  let conRetraso = $state(untrack(() => (copias[i]?.retraso_min ?? 0) > 0));
</script>

<div class="cuando">
  <div class="segmented inline" role="radiogroup" aria-label="Cuándo empieza «{k.nombre}»">
    {#each ["horario", "cadena", "despues"] as const as m (m)}
      {@const no = motivo(m)}
      {@const Icono = ICONO[m]}
      <button type="button" role="radio" aria-checked={modo === m} class:on={modo === m} disabled={!!no && modo !== m} use:tip={no ?? AYUDA_INICIO[m]} onclick={() => elegir(m)}><Icono size={14} />{TEXTO_INICIO[m]}</button>
    {/each}
  </div>

  {#if modo === "horario"}
    {#if guiado}
      <div class="plantillas" role="radiogroup" aria-label="Horario de «{k.nombre}»">
        {#each ["cada_hora", "cada_dia", "laborables"] as const as p (p)}
          {@const on = !personalizar && pl.plantilla === p}
          <div class="pl" class:on>
            <button type="button" role="radio" aria-checked={on} class="pl-b" onclick={() => plantilla(p)}>{TEXTO_PLANTILLA[p].replace("…", "")}</button>
            {#if p !== "cada_hora"}
              <input class="input hora" type="time" value={hora} aria-label="Hora de «{TEXTO_PLANTILLA[p]}»" onfocus={() => !on && plantilla(p)} onchange={(e) => cambiarHora(e.currentTarget.value)} />
            {/if}
          </div>
        {/each}
        <button type="button" role="radio" aria-checked={personalizar || pl.plantilla === "personalizado"} class="pl-b personalizar" class:on={personalizar || pl.plantilla === "personalizado"} onclick={() => (personalizar = true)}><Settings2 size={14} />Personalizar</button>
      </div>
      {#if personalizar || pl.plantilla === "personalizado"}
        <EditorHorario id="g-{k.id}" bind:horario={k.horario} admiteReglas={admite.reglas} {version} />
      {:else}
        <p class="faint linea">{resumenHorario(k.horario)}</p>
      {/if}
    {:else}
      <EditorHorario id={k.id} bind:horario={k.horario} admiteReglas={admite.reglas} {version} />
    {/if}
  {:else}
    {#if otraAnterior}<p class="faint linea">Va después de «{otraAnterior.nombre}», que no es la de encima.</p>{/if}
    <div class="retraso">
      <div class="segmented inline" role="radiogroup" aria-label="Cuánto espera tras «{anterior?.nombre ?? "la anterior"}»">
        <button type="button" role="radio" aria-checked={!conRetraso} class:on={!conRetraso} onclick={() => ((conRetraso = false), ponerRetraso(k, 0))}>Inmediatamente</button>
        <button
          type="button"
          role="radio"
          aria-checked={conRetraso}
          class:on={conRetraso}
          disabled={!admite.despues && !conRetraso}
          use:tip={!admite.despues ? `Actualiza el agente de ${equipo} para esperar` : "Espera unos minutos tras la anterior"}
          onclick={() => ((conRetraso = true), ponerRetraso(k, k.retraso_min || 15))}><Timer size={14} />Con retraso</button
        >
      </div>
      {#if conRetraso}
        <label class="min">
          <span class="sr-only">Minutos de retraso</span>
          <select class="input" value={k.retraso_min ?? 15} onchange={(e) => ponerRetraso(k, Number(e.currentTarget.value))}>
            {#each [...new Set([...RETRASOS, k.retraso_min ?? 15])].sort((a, b) => a - b) as m (m)}<option value={m}>{retrasoEnFrase(m).replace(/^Con /, "").replace(/ de retraso$/, "")}</option>{/each}
          </select>
        </label>
      {/if}
    </div>
    <p class="faint linea">{modo === "cadena" ? `Si «${anterior?.nombre ?? "la anterior"}» falla, esta no se hace y se avisa.` : `Tanto si «${anterior?.nombre ?? "la anterior"}» sale bien como si falla.`}</p>
  {/if}
</div>

<style>
  .cuando {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
  }
  /* Las opciones bajan de línea si no caben (en el móvil), sin cortarse. */
  .cuando .segmented {
    display: flex;
    flex-wrap: wrap;
    max-width: 100%;
  }
  .cuando [role="radio"]:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .plantillas {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .pl {
    display: inline-flex;
    max-width: 100%;
    align-items: center;
    gap: 2px;
    padding: 3px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    background: var(--surface);
  }
  .pl.on,
  .personalizar.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .pl-b {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 10px;
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 550;
    color: var(--text-1);
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    white-space: nowrap;
    cursor: pointer;
  }
  .on .pl-b,
  .personalizar.on {
    color: var(--accent-text);
  }
  .personalizar {
    height: 36px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    background: var(--surface);
  }
  .pl-b:focus-visible,
  .personalizar:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .hora {
    flex: 0 1 auto;
    min-width: 0;
    width: 9.5em;
    height: 28px;
    padding: 0 6px;
  }
  .retraso {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .min .input {
    width: auto;
    height: 34px;
  }
  .linea {
    margin: 0;
    font-size: var(--fs-sm);
  }
</style>
