<script lang="ts">
  // Un cliente en «Todos los clientes»: su marca (logo o inicial en su
  // acento, y una franja fina de su acento), su estado (icono y texto), la
  // última copia, cuántos equipos están al día, lo protegido, los cuadros de
  // 14 días de todos sus equipos y, si hay algo en marcha, su chip. Pulsar
  // lleva a su Estado.
  import { LoaderCircle } from "@lucide/svelte";
  import type { PanelCliente, SaludCliente } from "$lib/global";
  import { NOMBRE_ROL } from "$lib/estado.svelte";
  import { bytes, plural, relativo, fechaLarga } from "$lib/formato";
  import { tip } from "$lib/tooltip";
  import { todos } from "$lib/todos.svelte";
  import Chip from "../Chip.svelte";
  import MarcaCliente from "../MarcaCliente.svelte";
  import DiasCuadros from "../repo/DiasCuadros.svelte";

  let { cliente, salud, ahora }: { cliente: PanelCliente; salud: SaludCliente; ahora: number } = $props();
  const enMarcha = $derived(todos.progreso.filter((x) => x.cliente === cliente.id).flatMap((x) => x.tareas));
  const pct = $derived.by(() => {
    const xs = enMarcha.map((t) => t.porcentaje).filter((p): p is number => p != null);
    return xs.length ? Math.floor((xs.reduce((a, b) => a + b, 0) / xs.length) * 100) : null;
  });
  const conDatos = $derived(salud.dias.some((d) => d.estado !== "nada"));
</script>

<a class="card tile marca-cliente" data-acento={cliente.marca?.acento ?? undefined} href="/c/{cliente.id}">
  <span class="franja" aria-hidden="true"></span>
  <div class="cab">
    <MarcaCliente nombre={cliente.nombre} marca={cliente.marca} tam={36} />
    <span class="nombre">
      <strong>{cliente.nombre}</strong>
      <span class="sub">{NOMBRE_ROL[cliente.rol]} · {plural(salud.equipos, "equipo", "equipos")}</span>
    </span>
    <Chip pequeno tono={salud.tono} texto={salud.texto} />
  </div>
  <p class="linea num">
    {#if salud.ultima}<span use:tip={fechaLarga(salud.ultima)}>Última copia <strong>{relativo(salud.ultima, ahora)}</strong></span>{:else}Sin copias todavía{/if}
    {#if salud.equipos}<span class="faint">{" · "}{salud.alDia} de {salud.equipos} al día</span>{/if}
    {#if salud.protegido}<span class="faint">{" · "}{bytes(salud.protegido)}</span>{/if}
  </p>
  {#if enMarcha.length}
    <span class="badge badge-sm tone-info vivo">
      <LoaderCircle size={12} class="spin" aria-hidden="true" />{enMarcha.length === 1 ? "1 tarea en marcha" : `${enMarcha.length} tareas en marcha`}{pct != null ? ` · ${pct} %` : ""}
    </span>
  {/if}
  {#if cliente.avisos_abiertos}<span class="faint avisos">{plural(cliente.avisos_abiertos, "aviso sin revisar", "avisos sin revisar")}</span>{/if}
  <div class="dias">
    {#if conDatos}
      <DiasCuadros dias={salud.dias} tamano="mini" etiqueta="Copias de {cliente.nombre} en los últimos 14 días" />
    {:else}
      <span class="faint">Sin copias en 14 días</span>
    {/if}
    <span class="faint">14 días</span>
  </div>
</a>

<style>
  .tile {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
    padding: var(--sp-4);
    overflow: hidden;
    color: inherit;
    text-decoration: none;
  }
  .franja {
    position: absolute;
    inset: 0 0 auto;
    height: 3px;
    background: var(--marca);
    opacity: 0.85;
  }
  .cab {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
    min-width: 0;
  }
  .nombre {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  .nombre strong {
    font-weight: 600;
    overflow-wrap: break-word;
  }
  .sub {
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    color: var(--text-3);
  }
  .linea {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
    color: var(--text-2);
  }
  .linea strong {
    font-weight: 500;
    color: var(--text-1);
  }
  .vivo {
    align-self: flex-start;
  }
  .avisos {
    font-size: var(--fs-xs);
  }
  .dias {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
    margin-top: auto;
    padding-top: var(--sp-3);
    font-size: var(--fs-xs);
    border-top: 1px solid var(--border);
  }
</style>
