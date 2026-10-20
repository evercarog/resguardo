<script lang="ts">
  // «Copiar ahora» de una copia, con 0.8.1 (D8, 15kk) «Al terminar: nada / apagar /
  // hibernar» para esa vez (agente con `admite: "apagar_al_terminar"`). Apagar pide la
  // clave de administración; en un servidor o un almacén, además, confirmarlo expresamente.
  import OrdenDialog from "../OrdenDialog.svelte";
  import { esServidor } from "$lib/apagar";
  import type { Cliente, Equipo } from "$lib/tipos";

  let { cliente, equipo, repo, copia, nombre, onclose }: { cliente: Cliente; equipo: Equipo; repo: string; copia: string; nombre: string; onclose: () => void } = $props();

  const admite = $derived(!!equipo.resumen?.admite?.includes("apagar_al_terminar"));
  const servidor = $derived(esServidor(equipo));
  let alTerminar = $state<"" | "apagar" | "hibernar">("");
  let confirmado = $state(false);
  const cuerpo = $derived({ repo, copia, ...(alTerminar ? { al_terminar: alTerminar, ...(servidor ? { confirmar_servidor: true } : {}) } : {}) });
  const OPCIONES = [
    ["", "Nada"],
    ["apagar", "Apagar"],
    ["hibernar", "Hibernar"],
  ] as const;
</script>

<OrdenDialog
  {cliente}
  {equipo}
  tipo="copiar_ahora"
  {cuerpo}
  descripcion={`Se hará ahora la copia «${nombre}» de ${equipo.nombre}, sin esperar a su hora. No borra nada.`}
  accion="Copiar ahora"
  valido={!alTerminar || !servidor || confirmado}
  {onclose}
>
  {#snippet campos()}
    {#if admite}
      <div class="field">
        <span class="field-label" id="ca-fin">Al terminar</span>
        <div class="segmented inline" role="radiogroup" aria-labelledby="ca-fin">
          {#each OPCIONES as [v, t] (v)}<button type="button" role="radio" aria-checked={alTerminar === v} class:on={alTerminar === v} onclick={() => (alTerminar = v)}>{t}</button>{/each}
        </div>
        {#if alTerminar}
          <span class="field-hint">Solo si termina bien (si falla, no: para poder mirar) y cuando no quede nada más por hacer. Quien use {equipo.nombre} tendrá 2 minutos para pararlo con «No apagar» en el icono de Resguardo. Pide la clave de administración.</span>
          {#if servidor}
            <label class="check-row confirmar"><input type="checkbox" bind:checked={confirmado} /><span>{equipo.nombre} es un servidor{equipo.resumen?.guarda_copias?.activo ? " y guarda copias de otros equipos" : ""}: sí, quiero {alTerminar === "hibernar" ? "hibernarlo" : "apagarlo"} al terminar.</span></label>
          {/if}
        {/if}
      </div>
    {/if}
  {/snippet}
</OrdenDialog>

<style>
  .confirmar {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin-top: 6px;
    font-size: var(--fs-sm);
  }
</style>
