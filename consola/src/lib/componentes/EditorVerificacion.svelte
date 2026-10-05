<script lang="ts">
  // La verificación automática de un repositorio (v1.28): cada N días o (v1.40,
  // agente con `admite: "verificacion_horario"`) con un horario de reglas, el
  // mismo editor que el de las copias; y qué parte de los datos lee cada vez
  // (rotando). Con un horario, `cada_dias` se queda como estaba: es lo que usa
  // un agente anterior.
  import { untrack } from "svelte";
  import type { Horario, VerificacionAuto } from "$lib/tipos";
  import { conHorario, errorVerificacion, fraseVerificacion, HORARIO_VERIFICACION, PORCENTAJES } from "$lib/verificacion";
  import EditorHorario from "./EditorHorario.svelte";

  let {
    id,
    nombre,
    valor,
    admiteHorario,
    onchange,
  }: { id: string; nombre: string; valor: VerificacionAuto; admiteHorario: boolean; onchange: (v: VerificacionAuto) => void } = $props();

  const modo = $derived(conHorario(valor) ? "horario" : "dias");
  // El horario se edita aquí y se devuelve al cambiar (EditorHorario lo rehace de sus reglas).
  let horario = $state<Horario>(untrack(() => (valor.horario && conHorario(valor) ? $state.snapshot(valor.horario) : { ...HORARIO_VERIFICACION })));
  let emitido = untrack(() => JSON.stringify(horario));
  $effect(() => {
    const h = JSON.stringify(horario);
    if (h === emitido || modo !== "horario") return;
    emitido = h;
    untrack(() => onchange({ ...valor, horario: JSON.parse(h) as Horario }));
  });

  function ponerModo(m: string) {
    if (m === "horario") onchange({ ...valor, horario: $state.snapshot(horario) });
    else {
      const { horario: _, ...resto } = valor;
      onchange(resto);
    }
  }
  const porcentajes = $derived(PORCENTAJES.includes(valor.porcentaje) ? PORCENTAJES : [...PORCENTAJES, valor.porcentaje].sort((a, b) => a - b));
  const error = $derived(errorVerificacion(valor, admiteHorario));
</script>

<div class="editor-verif">
  <span class="verif-campos">
    {#if admiteHorario}
      <select class="input" value={modo} aria-label="Cuándo verificar «{nombre}»" onchange={(e) => ponerModo(e.currentTarget.value)}>
        <option value="dias">Cada N días</option>
        <option value="horario">Con un horario</option>
      </select>
    {/if}
    {#if modo === "dias"}
      <label>cada <input class="input num" type="number" min="1" max="31" value={valor.cada_dias} aria-label="Cada cuántos días verificar «{nombre}»" oninput={(e) => onchange({ ...valor, cada_dias: Math.trunc(Number(e.currentTarget.value)) })} /> días,</label>
    {/if}
    <select class="input" value={valor.porcentaje} aria-label="Qué parte de los datos lee cada vez" onchange={(e) => onchange({ ...valor, porcentaje: Number(e.currentTarget.value) })}>
      {#each porcentajes as p (p)}<option value={p}>{p === 0 ? "solo la estructura" : p === 100 ? "todos los datos" : `el ${p} % de los datos`}</option>{/each}
    </select>
  </span>
  {#if modo === "horario"}
    <div class="horario-verif"><EditorHorario id="verif-{id}" bind:horario admiteReglas para="verificacion" /></div>
  {/if}
  <span class="faint frase-verif" class:error-campo={!!error}>{error ?? fraseVerificacion(valor)}</span>
</div>

<style>
  .editor-verif {
    flex: 1 1 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .verif-campos {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    font-size: var(--fs-sm);
  }
  .verif-campos label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .verif-campos .input.num {
    width: 4.5em;
  }
  .verif-campos select {
    width: auto;
  }
  .frase-verif {
    font-size: var(--fs-sm);
  }
</style>
