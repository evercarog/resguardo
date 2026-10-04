<script lang="ts">
  // El kit de recuperación: lo necesario para abrir las copias si se pierde
  // este equipo. Las contraseñas no salen del servicio: la del repositorio
  // recién creado se imprime (la acaba de ver el administrador); las demás,
  // se escriben a mano en la hoja.
  import { onMount } from "svelte";
  import { servicio } from "../../puente.svelte";

  interface EntradaKit {
    id: string;
    nombre: string;
    destino?: string | null;
    tipo?: string | null;
    ubicacion?: string | null;
    id_restic?: string | null;
    usuario_servidor?: string | null;
  }
  let { nombreEquipo, recien = null }: { nombreEquipo: string; recien?: { nombre: string; contrasena: string; id: string } | null } = $props();
  let entradas = $state<EntradaKit[]>([]);
  let error = $state("");
  onMount(async () => {
    try {
      entradas = await servicio<EntradaKit[]>("kit");
      if (recien) entradas = entradas.filter((e) => e.id === recien.id);
    } catch (e) {
      error = (e as Error).message;
    }
  });
  const hoy = new Date().toLocaleDateString("es", { dateStyle: "long" });
</script>

<section class="v-tarjeta kit" aria-label="Kit de recuperación">
  <h3 class="v-titulo">Kit de recuperación · {nombreEquipo}</h3>
  <p class="v-mini">{hoy}. Guárdalo fuera de este equipo (impreso, en un cajón o una caja fuerte). Con él y restic se abren las copias desde otro equipo.</p>
  {#if error}<p class="v-error">{error}</p>{/if}
  {#each entradas as e (e.id)}
    <dl class="datos">
      <dt>Repositorio</dt><dd>{e.nombre}</dd>
      <dt>Dónde</dt><dd class="mono">{e.ubicacion ?? e.destino ?? "—"}</dd>
      {#if e.usuario_servidor}<dt>Usuario del servidor</dt><dd class="mono">{e.usuario_servidor} (contraseña: ________________)</dd>{/if}
      <dt>ID de restic</dt><dd class="mono">{e.id_restic ?? "—"}</dd>
      <dt>Contraseña</dt>
      <dd class="mono">{recien && recien.id === e.id ? recien.contrasena : "______________________________"}</dd>
    </dl>
  {:else}
    {#if !error}<p class="v-mini">Leyendo…</p>{/if}
  {/each}
  <p class="v-mini">Para abrirlas: <span class="mono">restic -r "{entradas[0]?.ubicacion ?? "<dónde>"}" snapshots</span> y escribe la contraseña.</p>
</section>

<style>
  .datos {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px var(--sp-3);
    margin: var(--sp-3) 0;
    font-size: var(--fs-sm);
  }
  .datos dt {
    color: var(--text-3);
  }
  .datos dd {
    margin: 0;
    word-break: break-all;
  }
  @media print {
    :global(body *) {
      visibility: hidden;
    }
    .kit,
    .kit :global(*) {
      visibility: visible;
      color: #000 !important;
    }
    .kit {
      position: absolute;
      inset: 0 auto auto 0;
      width: 100%;
      border: 0;
      background: #fff;
    }
  }
</style>
