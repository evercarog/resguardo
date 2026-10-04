<script lang="ts">
  // Los diálogos que abre la paleta (lib/acciones.svelte.ts): los mismos que
  // los de Estado y Repositorios y destinos, con la misma orden.
  import { actual } from "$lib/estado.svelte";
  import { acciones, destinosDe } from "$lib/acciones.svelte";
  import OrdenDialog from "./OrdenDialog.svelte";
  import NuevoRepositorio from "./NuevoRepositorio.svelte";
</script>

{#if acciones.copiar && actual.cliente}
  {#key acciones.copiar}
    <OrdenDialog
      cliente={actual.cliente}
      equipo={acciones.copiar.equipo}
      tipo="copiar_ahora"
      cuerpo={{ repo: acciones.copiar.repo, copia: acciones.copiar.copia }}
      descripcion="Se hará ahora la copia «{acciones.copiar.nombre}», sin esperar a su hora. No borra nada."
      accion="Copiar ahora"
      onclose={() => (acciones.copiar = null)}
    />
  {/key}
{/if}

{#if acciones.nuevoRepo && actual.cliente}
  <NuevoRepositorio cliente={actual.cliente} equipos={actual.equipos} destinos={destinosDe(actual.equipos)} onclose={() => (acciones.nuevoRepo = false)} />
{/if}
