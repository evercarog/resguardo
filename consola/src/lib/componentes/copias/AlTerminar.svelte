<script lang="ts">
  // 0.8.1 (D8, 15kk): «Al terminar: nada / apagar / hibernar» como opción de la copia
  // (`config.copias[].al_terminar`, agente con `admite: "apagar_al_terminar"`). En un
  // servidor o un almacén, solo confirmándolo expresamente.
  import { Power } from "@lucide/svelte";
  import { esServidor } from "$lib/apagar";
  import type { CopiaConfig, Equipo } from "$lib/tipos";

  let { k, equipo, admite, version = null }: { k: CopiaConfig; equipo: Equipo; admite: boolean; version?: string | null } = $props();
  const servidor = $derived(esServidor(equipo));
  const OPCIONES = [
    ["", "Nada"],
    ["apagar", "Apagar"],
    ["hibernar", "Hibernar"],
  ] as const;
  function poner(v: "" | "apagar" | "hibernar") {
    k.al_terminar = v || null;
    if (!v) k.al_terminar_servidor = false;
  }
</script>

<div class="field al-terminar">
  <span class="field-label" id="at-{k.id}"><Power size={14} />Al terminar</span>
  {#if !admite}
    <p class="faint">Actualiza el agente{version ? ` (tiene la ${version})` : ""} para apagar el equipo al terminar.</p>
  {:else}
    <div class="segmented inline" role="radiogroup" aria-labelledby="at-{k.id}">
      {#each OPCIONES as [v, t] (v)}<button type="button" role="radio" aria-checked={(k.al_terminar ?? "") === v} class:on={(k.al_terminar ?? "") === v} onclick={() => poner(v)}>{t}</button>{/each}
    </div>
    {#if k.al_terminar}
      <span class="field-hint">Cada vez que termine bien (si falla, no: para poder mirar) y no quede nada más por hacer. Quien use el equipo tendrá 2 minutos para pararlo con «No apagar». Para primeras copias largas, mejor solo esa vez, desde «Copiar ahora».</span>
      {#if servidor}
        <label class="check-row confirmar"
          ><input type="checkbox" checked={!!k.al_terminar_servidor} onchange={(e) => (k.al_terminar_servidor = e.currentTarget.checked)} /><span
            >{equipo.nombre} es un servidor{equipo.resumen?.guarda_copias?.activo ? " y guarda copias de otros equipos" : ""}: sí, quiero {k.al_terminar === "hibernar" ? "hibernarlo" : "apagarlo"} al terminar.</span
          ></label
        >
      {/if}
    {/if}
  {/if}
</div>

<style>
  .al-terminar .field-label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .confirmar {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: var(--fs-sm);
  }
</style>
