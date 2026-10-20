<script lang="ts">
  // 0.8.1 (D7, 15ii): un equipo que entra y sale (portátil, equipo de campo), en «En el
  // equipo» del editor de copias (`config.movil`, agente con `admite: "equipos_moviles"`):
  // «Sale de la oficina» con su margen, la batería mínima y la conexión medida. Se manda con
  // las copias («Enviar al equipo»).
  import { Laptop } from "@lucide/svelte";
  import type { MovilEquipo } from "$lib/tipos";

  let { movil = $bindable(null), nombre }: { movil?: MovilEquipo | null; nombre: string } = $props();

  const m = $derived(movil ?? {});
  function poner(c: Partial<MovilEquipo>) {
    movil = { ...m, ...c };
  }
  const conBateria = $derived((m.bateria_min ?? 0) > 0);
</script>

<div class="movil">
  <span class="field-label"><Laptop size={14} />Si sale de la oficina</span>
  <label class="switch-row"
    ><input type="checkbox" class="switch" checked={!!m.sale_oficina} onchange={(e) => poner({ sale_oficina: e.currentTarget.checked })} /><span
      >Sale de la oficina<span class="faint">Un portátil o un equipo de campo: sin conexión con el destino no cuenta como fallida hasta pasar el margen; al volver, se pone al día.</span></span
    ></label
  >
  {#if m.sale_oficina}
    <label class="linea">
      Avisar si pasa más de
      <input class="input num" type="number" min="1" max="90" value={m.margen_dias ?? 7} oninput={(e) => poner({ margen_dias: Math.max(1, Math.min(90, Math.trunc(Number(e.currentTarget.value)) || 7)) })} aria-label="Días sin copia antes de avisar" />
      días sin copia
    </label>
    <label class="switch-row"
      ><input type="checkbox" class="switch" checked={!!m.permitir_medida} onchange={(e) => poner({ permitir_medida: e.currentTarget.checked })} /><span
        >Copiar también con datos del móvil<span class="faint">Sin esto, con una conexión medida (el móvil compartido, por ejemplo) las copias esperan.</span></span
      ></label
    >
  {/if}
  <label class="switch-row"
    ><input type="checkbox" class="switch" checked={conBateria} onchange={(e) => poner({ bateria_min: e.currentTarget.checked ? 30 : null })} /><span
      >Cuidar la batería<span class="faint">Sin enchufar y con poca batería, lo que {nombre} hace solo espera. «Copiar ahora» se hace igual.</span></span
    ></label
  >
  {#if conBateria}
    <label class="linea">
      No empezar por debajo del
      <input class="input num" type="number" min="5" max="95" value={m.bateria_min ?? 30} oninput={(e) => poner({ bateria_min: Math.max(5, Math.min(95, Math.trunc(Number(e.currentTarget.value)) || 30)) })} aria-label="Batería mínima en %" />
      %
    </label>
  {/if}
</div>

<style>
  .movil {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .movil .field-label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .linea {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
    margin-left: 46px;
  }
  .linea .num {
    width: 5em;
  }
  @media (max-width: 520px) {
    .linea {
      margin-left: 0;
    }
  }
</style>
