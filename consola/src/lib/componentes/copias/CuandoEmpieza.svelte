<script lang="ts">
  // Cuándo empieza una copia (plan 0.7.26, bloque 3): «Con horario», «En
  // cadena» (solo si la anterior sale bien) o «Después de la anterior»
  // (siempre). En las dos últimas no hay horario: «Inmediatamente» o «Con
  // retraso de N min». La primera solo puede ir con horario.
  //
  // Plan 0.8.0, F1 (14pp): también «Solo a mano» (sin horario, solo «Copiar
  // ahora») y «Una sola vez» (ahora o en una fecha y hora), y para cualquier
  // horario «Termina: nunca / después de N copias / el día …». Al terminar, la
  // copia queda completada. Lo cuenta el agente.
  //
  // El mismo control en el modo guiado (con plantillas rápidas y
  // «Personalizar») y en el avanzado (el editor completo a la vista): los dos
  // cambian la copia con las funciones de lib/copiaGuiada.ts.
  import { untrack } from "svelte";
  import { CalendarCheck, CalendarClock, Hand, Lightbulb, Link2, Repeat, Settings2, Timer } from "@lucide/svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import { tip } from "$lib/tooltip";
  import EditorHorario from "$lib/componentes/EditorHorario.svelte";
  import {
    aplicarPlantilla,
    AYUDA_INICIO,
    esAhora,
    fechaHoraDe,
    inicioDe,
    MAX_VECES,
    plantillaDe,
    ponerInicio,
    ponerRetraso,
    ponerTermina,
    ponerUnaVez,
    RETRASOS,
    retrasoEnFrase,
    terminaDe,
    TEXTO_INICIO,
    TEXTO_PLANTILLA,
    TEXTO_TERMINA,
    type ModoInicio,
    type ModoTermina,
    type PlantillaHorario,
  } from "$lib/copiaGuiada";
  import { resumenHorario } from "$lib/formato";
  import type { CopiaConfig, Horario } from "$lib/tipos";

  let {
    copias,
    i,
    admite,
    version = null,
    equipo,
    guiado = false,
    hechas = null,
  }: {
    copias: CopiaConfig[];
    i: number;
    admite: { reglas: boolean; cadenas: boolean; despues: boolean; termina?: boolean };
    version?: string | null;
    /** El nombre del equipo (para «Actualiza el agente de …»). */
    equipo: string;
    /** Con plantillas rápidas y «Personalizar» (si no, el editor completo). */
    guiado?: boolean;
    /** F1: copias correctas hechas desde que se puso el fin (lo dice el equipo). */
    hechas?: number | null;
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
    // F1: no dependen de la anterior; solo de que el agente las entienda.
    if (m === "a_mano" || m === "una_vez") return admite.termina ? null : `Actualiza el agente de ${equipo} para «${TEXTO_INICIO[m]}»`;
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
  const ICONO = { horario: CalendarClock, cadena: Link2, despues: Repeat, a_mano: Hand, una_vez: CalendarCheck } as const;
  let conRetraso = $state(untrack(() => (copias[i]?.retraso_min ?? 0) > 0));

  // F1: cuándo termina y «una sola vez».
  const fin = $derived(terminaDe(k));
  const MODOS_FIN: ModoTermina[] = ["nunca", "veces", "hasta"];
  let unaVezAhora = $state(untrack(() => esAhora(copias[i]?.termina?.una_vez)));
  /** La fecha y hora que se propone para «En una fecha y hora»: mañana a las 21:00. */
  const manana = () => `${fechaHoraDe(Date.now() + 86_400_000).slice(0, 10)}T21:00`;
</script>

<div class="cuando">
  <div class="segmented inline" role="radiogroup" aria-label="Cuándo empieza «{k.nombre}»">
    {#each ["horario", "cadena", "despues", "a_mano", "una_vez"] as const as m (m)}
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
    {#if admite.termina}
      <!-- F1 (14pp): como el «Finaliza» de un calendario. -->
      <div class="termina">
        <span class="etq">Termina</span>
        <div class="segmented inline" role="radiogroup" aria-label="Cuándo termina «{k.nombre}»">
          {#each MODOS_FIN as t (t)}
            <button type="button" role="radio" aria-checked={fin === t} class:on={fin === t} onclick={() => fin !== t && ponerTermina(k, t)}>{TEXTO_TERMINA[t]}</button>
          {/each}
        </div>
        {#if fin === "veces"}
          <label class="min">
            Después de
            <input class="input num" type="number" min="1" max={MAX_VECES} value={k.termina?.veces ?? 10} aria-label="Copias antes de terminar" onchange={(e) => ponerTermina(k, "veces", Number(e.currentTarget.value))} />
            copias correctas
          </label>
        {:else if fin === "hasta"}
          <label class="min">
            El
            <input class="input" type="date" value={k.termina?.hasta ?? ""} aria-label="Último día" onchange={(e) => ponerTermina(k, "hasta", e.currentTarget.value)} />
            (incluido)
          </label>
        {/if}
      </div>
      {#if fin !== "nunca"}
        <p class="faint linea">Al terminar queda completada: no avisa ni cuenta como atrasada, y sus versiones se quedan. Podrás copiar a mano o reanudarla.{#if fin === "veces" && hechas != null}{" "}Van {hechas}.{/if}</p>
      {/if}
    {/if}
    <!-- 14qq: el truco de «cada 5 minutos + solo si hay cambios», en vez de «cuando algo cambie». -->
    <p class="faint linea consejo"><Lightbulb size={14} aria-hidden="true" /><span>Para copiar enseguida lo que cambia poco: cada 5 minutos con «Solo guardar si hay cambios». Si no cambió nada, no guarda nada ni ocupa espacio. <Ayuda id="copiar-enseguida" /></span></p>
  {:else if modo === "a_mano"}
    <p class="faint linea">Sin horario: solo se copia cuando pulses «Copiar ahora». No cuenta como atrasada ni avisa por no copiar.</p>
  {:else if modo === "una_vez"}
    <div class="retraso">
      <div class="segmented inline" role="radiogroup" aria-label="Cuándo se hace «{k.nombre}»">
        <button type="button" role="radio" aria-checked={unaVezAhora} class:on={unaVezAhora} onclick={() => ((unaVezAhora = true), ponerUnaVez(k, "ahora"))}>Ahora</button>
        <button type="button" role="radio" aria-checked={!unaVezAhora} class:on={!unaVezAhora} onclick={() => ((unaVezAhora = false), ponerUnaVez(k, manana()))}>En una fecha y hora</button>
      </div>
      {#if !unaVezAhora}
        <label class="min">
          <span class="sr-only">Fecha y hora</span>
          <input class="input" type="datetime-local" value={k.termina?.una_vez ?? ""} onchange={(e) => e.currentTarget.value && ponerUnaVez(k, e.currentTarget.value.slice(0, 16))} />
        </label>
      {/if}
    </div>
    <p class="faint linea">{unaVezAhora ? "En cuanto el equipo reciba los cambios." : "A esa hora del equipo (si está apagado, al encenderlo)."} Cuando salga bien queda completada, con sus versiones.</p>
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
  .termina {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .termina .etq {
    font-size: var(--fs-sm);
    font-weight: 550;
    color: var(--text-2);
  }
  .min {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .min .num {
    width: 6em;
  }
  .consejo {
    display: flex;
    align-items: flex-start;
    gap: 6px;
  }
  .consejo > :global(svg) {
    flex: none;
    margin-top: 2px;
  }
</style>
