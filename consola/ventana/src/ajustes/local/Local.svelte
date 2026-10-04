<script lang="ts">
  // Todo lo del equipo en modo local (sin consola): las copias, dónde se
  // guardan, restaurar y lo demás. Cada parte se carga al abrirla.
  import { onMount, type Component } from "svelte";
  import { esBloqueo, servicio } from "../../puente.svelte";
  import type { EstadoLocal, PropsParte } from "./comun";

  let { alBloquear }: { alBloquear: () => void } = $props();
  type Parte = "copias" | "donde" | "restaurar" | "mas";
  let parte = $state<Parte>("copias");
  let estado = $state<EstadoLocal | null>(null);
  let error = $state("");

  async function recargar() {
    try {
      estado = await servicio<EstadoLocal>("estado_local");
      error = "";
    } catch (e) {
      if (esBloqueo(e)) return alBloquear();
      error = (e as Error).message;
    }
  }
  onMount(recargar);

  const PARTES: [Parte, string][] = [
    ["copias", "Copias"],
    ["donde", "Dónde"],
    ["restaurar", "Restaurar"],
    ["mas", "Más"],
  ];
  const cargar: Record<Parte, () => Promise<{ default: Component<PropsParte> }>> = {
    copias: () => import("./CopiasLocal.svelte"),
    donde: () => import("./Donde.svelte"),
    restaurar: () => import("./Restaurar.svelte"),
    mas: () => import("./Mas.svelte"),
  };
</script>

<section class="v-pila" aria-label="Este equipo, sin consola">
  <nav class="segmented" aria-label="Partes">
    {#each PARTES as [p, t] (p)}<button class:on={parte === p} aria-current={parte === p ? "page" : undefined} onclick={() => (parte = p)}>{t}</button>{/each}
  </nav>
  {#if error}<p class="v-error" role="alert">{error}</p>{/if}
  {#if estado}
    {#await cargar[parte]()}
      <p class="v-sub">Cargando…</p>
    {:then m}
      <m.default {estado} {recargar} {alBloquear} />
    {/await}
  {:else if !error}
    <p class="v-sub">Leyendo la configuración del equipo…</p>
  {/if}
</section>
