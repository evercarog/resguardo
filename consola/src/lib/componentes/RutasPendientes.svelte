<script lang="ts">
  // B1 (docs/nombres-legibles.md): un almacén movió una carpeta («Ordenar nombres de
  // carpeta») y un equipo sigue con la ruta de antes (p. ej. se cerró el navegador a
  // medias, o el equipo estaba apagado). Lo dice y ofrece «Poner la ruta nueva»
  // (`cambiar_destino`, con la clave de administración).
  import { TriangleAlert } from "@lucide/svelte";
  import type { Cliente, Equipo } from "$lib/tipos";
  import { rutasPendientes, type RutaPendiente } from "$lib/nombres";
  import OrdenDialog from "./OrdenDialog.svelte";

  let { cliente, equipos, equipo, puede }: { cliente: Cliente; equipos: Equipo[]; /** Solo las de este equipo (o, si es un almacén, también las de sus clientes). */ equipo: Equipo; puede: boolean } = $props();

  const filas = $derived(
    equipos
      .filter((a) => a.resumen?.guarda_copias?.activo && a.resumen.guarda_copias.renombres?.length)
      .flatMap((a) => rutasPendientes(a, equipos))
      .filter((p) => p.en.equipo.id === equipo.id || p.almacen.id === equipo.id),
  );
  let abierta = $state<RutaPendiente | null>(null);
  const que = (p: RutaPendiente) =>
    [p.movido.usuario ? "la carpeta del equipo" : "", ...p.movido.repos.map((r) => `«${p.en.repos.find((x) => x.carpeta === r.de)?.repo.nombre ?? r.de}»`)].filter(Boolean).join(" y ");
</script>

{#each filas as p (p.en.equipo.id + p.en.destino.id)}
  <div class="notice notice-warn" role="status">
    <TriangleAlert size={16} />
    <p>
      {p.almacen.nombre} cambió {que(p)} de {p.en.equipo.nombre}, pero {p.en.equipo.nombre} sigue con la ruta de antes: no puede copiar allí hasta que use la nueva.
      {#if puede}<button type="button" class="btn btn-sm" onclick={() => (abierta = p)}>Poner la ruta nueva</button>{/if}
    </p>
  </div>
{/each}

{#if abierta}
  <OrdenDialog
    {cliente}
    equipo={abierta.en.equipo}
    tipo="cambiar_destino"
    cuerpo={abierta.cuerpo}
    titulo="Poner la ruta nueva"
    descripcion={`${abierta.en.equipo.nombre} usará la carpeta nueva en ${abierta.almacen.nombre}. Antes de guardar, abre cada repositorio con ella: si alguno no abre, no cambia nada.`}
    accion="Poner la ruta nueva"
    onclose={() => (abierta = null)}
  />
{/if}

<style>
  .notice p {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px var(--sp-3);
  }
</style>
