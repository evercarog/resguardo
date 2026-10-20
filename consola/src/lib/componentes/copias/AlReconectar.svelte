<script lang="ts">
  // 0.8.1 (D7, 15ii): «Además, al volver a conectarse», junto al horario de una copia
  // (`config.copias[].al_reconectar`). Se suma al horario: cuando el destino vuelve a
  // responder y la última copia buena es antigua, copia una vez tras unos minutos.
  import { Wifi } from "@lucide/svelte";

  let { activo = $bindable(false) }: { activo?: boolean } = $props();
</script>

<label class="switch-row al-reconectar"
  ><input type="checkbox" class="switch" bind:checked={activo} /><span
    ><span class="con-icono"><Wifi size={14} />Además, al volver a conectarse</span><span class="faint"
      >Para portátiles: si el equipo estuvo fuera y la última copia es más antigua que su horario, copia una vez a los pocos minutos de volver a ver el destino (la oficina o la VPN). Conectado siempre, no hace nada.</span
    ></span
  ></label
>

<style>
  .al-reconectar {
    margin-top: var(--sp-2);
  }
  .con-icono {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
</style>
