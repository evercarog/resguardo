<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Un equipo en el panel de Estado: su salud de un vistazo. Nombre y estado
  // (icono y texto), la última copia y la próxima, los cuadros de 14 días de
  // todas sus copias y si está conectado; si hay algo en marcha, su chip
  // («Copiando… 42 %», v1.25). Pulsar lleva a su ficha.
  import { HardDrive, Laptop, Monitor, Server } from "@lucide/svelte";
  import { cuandoFrase, relativo } from "$lib/formato";
  import { diasEquipo } from "$lib/panel";
  import { proximaCopia, saludEquipo } from "$lib/salud";
  import type { Equipo, Informe } from "$lib/tipos";
  import Chip from "./Chip.svelte";
  import DiasCuadros from "./repo/DiasCuadros.svelte";
  import EnMarcha from "./EnMarcha.svelte";

  let { equipo, informe, cliente, ahora }: { equipo: Equipo; informe: Informe | null | undefined; cliente: string; ahora: number } = $props();

  const salud = $derived(saludEquipo(equipo, ahora));
  const copias = $derived(equipo.resumen?.copias ?? []);
  const ultima = $derived(
    [...copias.map((c) => c.ultima?.cuando), ...(informe?.datos.copias ?? []).map((c) => c.cuando)]
      .filter((x): x is string => !!x)
      .sort()
      .at(-1),
  );
  const proxima = $derived(proximaCopia(copias, ahora));
  const cuadros = $derived(informe ? diasEquipo(informe, 14, ahora) : []);
  const Icono = $derived(equipo.rol === "almacenamiento" ? Server : /portatil|laptop/i.test(equipo.nombre) ? Laptop : /linux|debian|ubuntu/i.test(equipo.so) ? HardDrive : Monitor);
</script>

<a class="card tile tone-{salud.tono}" href="/c/{cliente}/equipos/{equipo.id}" class:apagado={equipo.modo === "trasladado"}>
  <div class="cab">
    <span class="ic"><Icono size={16} /></span>
    <span class="nombre">
      <strong>{equipo.nombre}</strong>
      <span class="conn">
        {#if equipo.conectado}<span class="dot" style="--tone: var(--ok)" aria-hidden="true"></span>Conectado{:else}Visto {relativo(equipo.ultimo_contacto, ahora)}{/if}
      </span>
    </span>
    <Chip pequeno tono={salud.tono} texto={salud.texto} />
  </div>
  <p class="linea num">
    {#if ultima}Última copia <strong>{relativo(ultima, ahora)}</strong>{:else if equipo.rol === "almacenamiento" && !copias.length}Guarda copias de otros equipos{:else}Sin copias todavía{/if}
    {#if proxima}<span class="faint" use:tip={`Próxima copia ${cuandoFrase(proxima, ahora)}`}>{" · "}próxima {relativo(proxima, ahora)}</span>{/if}
  </p>
  <EnMarcha equipo={equipo.id} compacto />
  {#if cuadros.length}
    <div class="dias">
      <DiasCuadros dias={cuadros} tamano="mini" etiqueta="Copias de {equipo.nombre} en los últimos 14 días" />
      <span class="faint">14 días</span>
    </div>
  {/if}
</a>

<style>
  .tile {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
    padding: var(--sp-4);
  }
  .apagado {
    opacity: 0.6;
  }
  .cab {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
    min-width: 0;
  }
  .ic {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    color: var(--text-2);
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .nombre {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  .nombre strong {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .conn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
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
  .dias {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
    margin-top: auto;
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
    font-size: var(--fs-xs);
  }
</style>
