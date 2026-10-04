<script lang="ts">
  import { tip } from "$lib/tooltip";
  // El horario de una copia como reglas que se suman (lib/horario.ts): «a estas
  // horas», «cada cierto tiempo» (minutos u horas, en una franja), «cada N días»
  // y «un día de cada mes». Debajo, el resumen en frase y cuántas copias salen.
  //
  // Con un agente anterior a la 0.7.9 solo se ofrece lo que se puede guardar
  // como lista de horas (horas e intervalos de horas, con los mismos días).
  import { untrack } from "svelte";
  import { CalendarClock, Plus, X } from "@lucide/svelte";
  import { DIAS, DIAS_CORTOS, resumenReglas } from "$lib/formato";
  import {
    ACTUALIZA,
    copiasAlDia,
    DIAS_ATAJOS,
    errorRegla,
    expresable,
    horarioParaEnviar,
    horaValida,
    INTERVALOS,
    MAX_HORAS,
    MAX_REGLAS,
    MAX_CADA_DIAS,
    MINUTOS,
    reglaNuevaDe,
    reglasDe,
    VERSION_REGLAS,
  } from "$lib/horario";
  import type { Horario, ReglaHorario } from "$lib/tipos";

  let {
    id,
    horario = $bindable(),
    admiteReglas,
    version = null,
  }: { id: string; horario: Horario; admiteReglas: boolean; version?: string | null } = $props();

  // Lo que se edita son las reglas; el horario de la copia se rehace de ellas
  // al cambiar algo. Si el horario cambia desde fuera (una plantilla, «Cancelar
  // cambios»), se vuelven a leer.
  let reglas = $state<ReglaHorario[]>(untrack(() => reglasDe(horario, admiteReglas)));
  let base = untrack(() => JSON.stringify(reglas));
  let emitido = untrack(() => JSON.stringify(horario));

  $effect(() => {
    const h = JSON.stringify(horario);
    if (h === emitido) return;
    emitido = h;
    reglas = reglasDe(horario, admiteReglas);
    base = JSON.stringify(reglas);
  });
  $effect(() => {
    const s = JSON.stringify(reglas);
    if (s === base) return;
    base = s;
    const h = horarioParaEnviar(JSON.parse(s) as ReglaHorario[], admiteReglas);
    emitido = JSON.stringify(h);
    horario = h;
  });

  const TIPOS: { tipo: ReglaHorario["tipo"]; texto: string; nuevo: boolean }[] = [
    { tipo: "horas", texto: "A estas horas", nuevo: false },
    { tipo: "intervalo", texto: "Cada cierto tiempo", nuevo: false },
    { tipo: "cada_dias", texto: "Cada N días", nuevo: true },
    { tipo: "mensual", texto: "Un día de cada mes", nuevo: true },
  ];
  const DIAS_MES = Array.from({ length: 28 }, (_, i) => i + 1);

  function cambiarTipo(i: number, tipo: ReglaHorario["tipo"]) {
    const antes = reglas[i];
    const nueva = reglaNuevaDe(tipo, "dias" in antes && antes.dias.length ? antes.dias : undefined);
    // Se conserva la hora que ya tenía, si la tenía.
    const hora = antes.tipo === "horas" ? antes.horas[0] : antes.tipo === "intervalo" ? antes.desde : antes.hora;
    if (hora && horaValida(hora)) {
      if (nueva.tipo === "cada_dias" || nueva.tipo === "mensual") nueva.hora = hora;
      if (nueva.tipo === "horas") nueva.horas = [hora];
    }
    reglas[i] = nueva;
  }
  function anadir() {
    const conDias = [...reglas].reverse().find((r) => "dias" in r) as Extract<ReglaHorario, { dias: number[] }> | undefined;
    reglas.push({ tipo: "horas", dias: [...(conDias?.dias ?? [1, 2, 3, 4, 5])], horas: ["19:00"] });
  }
  function ponerDias(r: Extract<ReglaHorario, { dias: number[] }>, d: number) {
    r.dias = r.dias.includes(d) ? r.dias.filter((x) => x !== d) : [...r.dias, d].sort((a, b) => a - b);
  }

  const muchas = $derived((copiasAlDia(reglas)?.n ?? 0) >= 100);
  /** Con un agente anterior: reglas con días distintos no se pueden guardar como una lista. */
  const sinCombinar = $derived(!admiteReglas && reglas.length > 1 && reglas.every((r) => !errorRegla(r, false)) && !expresable(reglas));
</script>

<div class="editor-horario">
  <ol class="reglas" aria-label="Reglas del horario">
    {#each reglas as r, i (i)}
      {@const err = errorRegla(r, admiteReglas)}
      <li class="regla" class:con-error={!!err}>
        <div class="regla-cab">
          {#if reglas.length > 1}<span class="num" aria-hidden="true">{i + 1}</span>{/if}
          <label class="sr-only" for="tipo-{id}-{i}">Tipo de la regla {i + 1}</label>
          <select id="tipo-{id}-{i}" class="input tipo" value={r.tipo} onchange={(e) => cambiarTipo(i, e.currentTarget.value as ReglaHorario["tipo"])}>
            {#each TIPOS as t (t.tipo)}
              {#if admiteReglas || !t.nuevo || r.tipo === t.tipo}<option value={t.tipo}>{t.texto}</option>{/if}
            {/each}
          </select>
          {#if reglas.length > 1}
            <button type="button" class="icon-btn quitar" aria-label="Quitar la regla {i + 1}" use:tip={"Quitar esta regla"} onclick={() => reglas.splice(i, 1)}><X size={15} /></button>
          {/if}
        </div>

        {#if r.tipo === "horas" || r.tipo === "intervalo"}
          <div class="dias-fila">
            <div class="dias" role="group" aria-label="Días de la regla {i + 1}">
              {#each DIAS_CORTOS as d, j (j)}
                <button type="button" class="dia" class:on={r.dias.includes(j + 1)} aria-pressed={r.dias.includes(j + 1)} aria-label={DIAS[j]} use:tip={DIAS[j]} onclick={() => ponerDias(r, j + 1)}>{d}</button>
              {/each}
            </div>
            <div class="atajos" role="group" aria-label="Atajos de días">
              {#each DIAS_ATAJOS as a (a.texto)}
                {@const on = [...r.dias].sort((x, y) => x - y).join() === a.dias.join()}
                <button type="button" class="chip-dia" class:on aria-pressed={on} onclick={() => (r.dias = [...a.dias])}>{a.texto}</button>
              {/each}
            </div>
          </div>
        {/if}

        {#if r.tipo === "horas"}
          <div class="horas">
            {#each r.horas as _, h (h)}
              <span class="hora">
                <input class="input num" type="time" bind:value={r.horas[h]} aria-label="Hora {h + 1}" />
                {#if r.horas.length > 1}<button type="button" class="icon-btn" aria-label="Quitar esta hora" onclick={() => r.horas.splice(h, 1)}><X size={14} /></button>{/if}
              </span>
            {/each}
            <button type="button" class="btn btn-sm btn-ghost" disabled={r.horas.length >= MAX_HORAS} onclick={() => r.horas.push("19:00")}><Plus size={14} />Hora</button>
          </div>
        {:else if r.tipo === "intervalo"}
          <div class="linea">
            <label class="trozo"
              >Cada
              <select class="input" bind:value={r.cada_min} aria-label="Cada cuánto">
                {#if admiteReglas || r.cada_min % 60}
                  <optgroup label="Minutos">
                    {#each MINUTOS as n (n)}{#if admiteReglas || r.cada_min === n}<option value={n}>{n} minutos</option>{/if}{/each}
                  </optgroup>
                {/if}
                <optgroup label="Horas">
                  {#each INTERVALOS as n (n)}<option value={n * 60}>{n === 1 ? "1 hora" : `${n} horas`}</option>{/each}
                </optgroup>
              </select></label
            >
            <label class="trozo">de <input class="input num" type="time" bind:value={r.desde} aria-label="Desde" /></label>
            <label class="trozo">a <input class="input num" type="time" bind:value={r.hasta} aria-label="Hasta" /></label>
          </div>
        {:else if r.tipo === "cada_dias"}
          <div class="linea">
            <label class="trozo">Cada <input class="input dias-n" type="number" min="1" max={MAX_CADA_DIAS} step="1" bind:value={r.cada} aria-label="Cada cuántos días" /> {r.cada === 1 ? "día" : "días"}</label>
            <label class="trozo">a las <input class="input num" type="time" bind:value={r.hora} aria-label="Hora" /></label>
            <label class="trozo">desde el <input class="input fecha" type="date" bind:value={r.inicio} aria-label="Desde qué día" /></label>
          </div>
          <p class="faint pista">La primera, el día que eliges; después, cada {r.cada === 1 ? "día" : `${r.cada || "N"} días`} contando desde ese.</p>
        {:else if r.tipo === "mensual"}
          <div class="linea">
            <label class="trozo"
              >El
              <select class="input" bind:value={r.dia} aria-label="Qué día del mes">
                {#each DIAS_MES as d (d)}<option value={d}>día {d}</option>{/each}
                <option value={-1}>último día</option>
              </select>
              de cada mes</label
            >
            <label class="trozo">a las <input class="input num" type="time" bind:value={r.hora} aria-label="Hora" /></label>
          </div>
          {#if r.dia === -1}<p class="faint pista">El 31, el 30 o el 28 (29 en años bisiestos), según el mes.</p>{/if}
        {/if}
        {#if err}<p class="error-campo" role="alert">{err}</p>{/if}
      </li>
    {/each}
  </ol>

  <div class="pie">
    <button type="button" class="btn btn-sm" disabled={reglas.length >= MAX_REGLAS} onclick={anadir}><Plus size={14} />Añadir otra regla</button>
    {#if !admiteReglas}
      <span class="faint aviso-version">Cada pocos minutos, cada N días o un día de cada mes: {ACTUALIZA.toLowerCase()} ({version ? `tiene la ${version}; ` : ""}hace falta la {VERSION_REGLAS}).</span>
    {/if}
  </div>
  {#if sinCombinar}<p class="error-campo" role="alert">{ACTUALIZA}: este agente solo guarda reglas con los mismos días y hasta {MAX_HORAS} horas al día.</p>{/if}

  <p class="resumen-horario" aria-live="polite"><CalendarClock size={14} /><span>{resumenReglas(reglas)}</span></p>
  {#if muchas}<p class="faint pista">Muchas vueltas al día: con «Solo guardar si hay cambios», solo se guarda una versión cuando algo cambió.</p>{/if}
</div>

<style>
  .editor-horario {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  .reglas {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .regla {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: var(--sp-3);
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    min-width: 0;
  }
  .regla.con-error {
    border-color: color-mix(in srgb, var(--bad) 45%, var(--border));
  }
  .regla-cab {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .num {
    display: inline-grid;
    place-items: center;
    flex: none;
    width: 22px;
    height: 22px;
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--accent-text);
    background: var(--accent-soft);
    border-radius: 999px;
  }
  .tipo {
    width: auto;
    min-width: 0;
    max-width: 100%;
    font-weight: 500;
  }
  .quitar {
    margin-left: auto;
  }
  .dias-fila {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 12px;
  }
  .dias {
    display: flex;
    gap: 4px;
  }
  .dia {
    width: 34px;
    height: 34px;
    font: inherit;
    font-weight: 600;
    color: var(--text-2);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    cursor: pointer;
  }
  .dia.on {
    color: var(--accent-contrast);
    background: var(--accent);
    border-color: var(--accent);
  }
  .atajos {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .chip-dia {
    height: 26px;
    padding: 0 10px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--text-2);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    cursor: pointer;
  }
  .chip-dia.on {
    color: var(--accent-text);
    background: var(--accent-soft);
    border-color: color-mix(in srgb, var(--accent) 40%, transparent);
  }
  .horas,
  .linea {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
  }
  .horas {
    gap: 6px;
  }
  .hora {
    display: inline-flex;
    align-items: center;
  }
  /* Ancho para «01:00 p. m.» con su icono, en cualquier idioma del navegador. */
  input[type="time"] {
    width: 9.5rem;
    min-width: 9.5rem;
  }
  .dias-n {
    width: 5.5rem;
  }
  .fecha {
    width: auto;
    min-width: 10.5rem;
  }
  .trozo {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .trozo select {
    width: auto;
  }
  .pista {
    margin: -4px 0 0;
    font-size: var(--fs-xs);
  }
  .pie {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 12px;
  }
  .aviso-version {
    font-size: var(--fs-xs);
  }
  .resumen-horario {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
    font-weight: 500;
    color: var(--text-1);
  }
  .resumen-horario :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--accent-text);
  }
  .error-campo {
    margin: 0;
  }
  @media (max-width: 480px) {
    .dia {
      width: 32px;
      height: 32px;
    }
    .linea {
      flex-direction: column;
      align-items: flex-start;
    }
  }
</style>
