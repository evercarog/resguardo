<script lang="ts">
  // 0.8.1 (H1, 15mm): «Recuperar en el equipo» de una copia: como el cliente, sí o no.
  // Con la clave de administración (la orden `recuperar_en_equipo`); solo administradores.
  // La consola solo lo ofrece si el agente lo admite.
  import { ArchiveRestore } from "@lucide/svelte";
  import type { Cliente, Equipo } from "$lib/tipos";
  import { cargarCliente, puede } from "$lib/estado.svelte";
  import OrdenDialog from "./OrdenDialog.svelte";
  import Ayuda from "./Ayuda.svelte";

  let { cliente, equipo, copia }: { cliente: Cliente; equipo: Equipo; copia: { id: string; repo: string; nombre: string } } = $props();

  const admite = $derived(!!equipo.resumen?.admite?.includes("recuperar_en_equipo"));
  const r = $derived(equipo.resumen?.recuperar ?? null);
  const propio = $derived<boolean | null>(r?.copias?.[`${copia.repo}#${copia.id}`] ?? null);
  const efectivo = $derived(propio ?? r?.cliente ?? false);
  const texto = $derived(
    propio === null ? `Como el cliente: ${r?.cliente ? "activado" : "desactivado"}` : propio ? "Activado en esta copia" : "Desactivado en esta copia",
  );
  let abierto = $state(false);
  let eleccion = $state<"cliente" | "si" | "no">("cliente");
  function abrir() {
    eleccion = propio === null ? "cliente" : propio ? "si" : "no";
    abierto = true;
  }
  const cuerpo = $derived({ repo: copia.repo, copia: copia.id, activo: eleccion === "cliente" ? null : eleccion === "si" });
</script>

{#if admite}
  <div class="card p recuperar">
    <ArchiveRestore size={16} aria-hidden="true" />
    <div class="texto">
      <strong>Recuperar en el equipo</strong>
      <Ayuda id="recuperar-en-el-equipo" />
      <p class="faint">
        {texto}{#if efectivo} · cada persona de {equipo.nombre} puede recuperar sus propios archivos de esta copia (hasta {r?.max_mb ?? 1024} MB cada vez).{:else}.{/if}
      </p>
    </div>
    {#if puede.administrar(cliente.rol) && equipo.modo !== "trasladado"}<button class="btn btn-sm" onclick={abrir}>Cambiar</button>{/if}
  </div>
{/if}

{#if abierto}
  <OrdenDialog
    {cliente}
    {equipo}
    tipo="recuperar_en_equipo"
    titulo="Recuperar en el equipo"
    {cuerpo}
    descripcion={`«${copia.nombre}» en ${equipo.nombre}: ${eleccion === "cliente" ? "como lo diga el cliente" : eleccion === "si" ? "cada persona puede recuperar sus propios archivos" : "nadie recupera desde el equipo"}.`}
    accion="Guardar"
    onclose={() => {
      abierto = false;
      void cargarCliente(cliente.id, { silencioso: true });
    }}
  >
    {#snippet campos()}
      <div class="opciones" role="radiogroup" aria-label="Recuperar en el equipo">
        <label><input type="radio" bind:group={eleccion} value="cliente" />Como el cliente ({r?.cliente ? "activado" : "desactivado"})</label>
        <label><input type="radio" bind:group={eleccion} value="si" />Sí, en esta copia</label>
        <label><input type="radio" bind:group={eleccion} value="no" />No, en esta copia</label>
      </div>
      <p class="faint pequeno">Solo lo que cada cuenta puede abrir, nunca encima de nada y sin ver contraseñas. Cada recuperación sale en «Restauraciones».</p>
    {/snippet}
  </OrdenDialog>
{/if}

<style>
  .recuperar {
    display: flex;
    gap: var(--sp-3);
    align-items: flex-start;
  }
  .recuperar > :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--text-3);
  }
  .texto {
    flex: 1;
    min-width: 0;
  }
  .texto p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .opciones {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .opciones label {
    display: flex;
    gap: 8px;
    align-items: center;
  }
</style>
