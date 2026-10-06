<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Un equipo en una lista: icono, nombre, sistema y conexión, etiquetas,
  // última copia y próxima, los cuadros de 14 días (si llega su informe), el
  // estado (icono y texto) y, con `acciones`, un menú rápido: copiar ahora
  // cualquiera de sus copias, restaurar o ver sus órdenes.
  import { goto } from "$app/navigation";
  import { ClipboardList, HardDrive, History, Laptop, Monitor, Play, Server } from "@lucide/svelte";
  import { actual, puede, reloj } from "$lib/estado.svelte";
  import { duplicadoDe, proximaCopia, saludEquipo } from "$lib/salud";
  import { cuandoFrase, relativo } from "$lib/formato";
  import { diasEquipo } from "$lib/panel";
  import type { Equipo, Informe } from "$lib/tipos";
  import Chip from "./Chip.svelte";
  import Tiempo from "./Tiempo.svelte";
  import EtiquetaChip from "./EtiquetaChip.svelte";
  import DiasCuadros from "./repo/DiasCuadros.svelte";
  import MenuAcciones, { type AccionMenu } from "./MenuAcciones.svelte";
  import OrdenDialog from "./OrdenDialog.svelte";
  import EnMarcha from "./EnMarcha.svelte";
  import ContadorNotas from "./notas/ContadorNotas.svelte";
  import SinConfirmar from "./SinConfirmar.svelte";

  let { equipo, cliente, informe, acciones = false }: { equipo: Equipo; cliente: string; informe?: Informe | null; acciones?: boolean } = $props();
  /** La misma máquina ya dada de alta con otra entrada (un intento anterior sin confirmar). */
  const duplicado = $derived(duplicadoDe(equipo, actual.equipos));
  const salud = $derived(duplicado ? { tono: "neutral" as const, texto: "Duplicado", detalle: "" } : saludEquipo(equipo, reloj.ahora));
  const sinConfirmar = $derived(!equipo.confirmado && equipo.modo !== "trasladado");
  const copias = $derived(equipo.resumen?.copias ?? []);
  const ultima = $derived(
    copias
      .map((c) => c.ultima?.cuando)
      .filter((x): x is string => !!x)
      .sort()
      .at(-1),
  );
  const proxima = $derived(proximaCopia(copias, reloj.ahora));
  const cuadros = $derived(informe ? diasEquipo(informe, 14, reloj.ahora) : []);
  const Icono = $derived(equipo.rol === "almacenamiento" ? Server : /portatil|laptop/i.test(equipo.nombre) ? Laptop : /linux|debian|ubuntu/i.test(equipo.so) ? HardDrive : Monitor);

  // Menú rápido (solo quien puede mandar órdenes, y a equipos confirmados que siguen aquí).
  let copiar = $state<{ repo: string; copia: string; nombre: string } | null>(null);
  const activo = $derived(equipo.confirmado && equipo.modo !== "trasladado");
  const menu = $derived.by((): AccionMenu[][] => {
    const base = `/c/${cliente}/equipos/${equipo.id}`;
    const deCopias = puede.ordenar(actual.cliente?.rol)
      ? copias
          .filter((k) => k.activa !== false)
          .map((k) => ({ texto: copias.length > 1 ? `Copiar ahora «${k.nombre}»` : "Copiar ahora", icono: Play, onclick: () => (copiar = { repo: k.repo, copia: k.id, nombre: k.nombre }) }))
      : [];
    return [
      deCopias,
      [
        ...(equipo.resumen?.repositorios?.length ? [{ texto: "Restaurar archivos…", icono: History, onclick: () => goto(`/c/${cliente}/restaurar?equipo=${equipo.id}`) }] : []),
        { texto: "Ver sus órdenes", icono: ClipboardList, onclick: () => goto(`${base}?tab=ordenes`) },
      ],
    ];
  });
</script>

<div class="fila-eq" class:trasladado={equipo.modo === "trasladado"}>
  <a class="fila" href="/c/{cliente}/equipos/{equipo.id}">
    <span class="icono"><Icono size={16} /></span>
    <span class="fila-texto">
      <span class="fila-titulo">{equipo.nombre} <ContadorNotas tipo="equipo" objeto={equipo.id} /></span>
      <span class="solo-movil est-movil"><EnMarcha equipo={equipo.id} compacto /><Chip pequeno tono={salud.tono} texto={salud.texto} /></span>
      {#if sinConfirmar}<span class="fila-sub">{#if duplicado}Duplicado: este equipo ya está dado de alta como «{duplicado.nombre}»{:else}Falta confirmar el número de comprobación{/if}</span>{/if}
      <span class="fila-sub">
        {#if equipo.modo === "trasladado"}Trasladado a otro servidor{" · "}{/if}{equipo.so}{equipo.rol === "almacenamiento" ? " · Guarda copias" : ""}{" · "}
        {#if equipo.conectado}<span class="conn"><span class="dot" style="--tone: var(--ok)" aria-hidden="true"></span>Conectado</span>{:else}visto <Tiempo iso={equipo.ultimo_contacto} />{/if}
      </span>
      <!-- En móvil, la última copia va aquí (la columna de la derecha no cabe). -->
      <span class="fila-sub solo-movil">{#if sinConfirmar}{:else if ultima}Última copia {relativo(ultima, reloj.ahora)}{:else if proxima}Primera copia {cuandoFrase(proxima, reloj.ahora)}{:else}Sin copias todavía{/if}</span>
      {#if equipo.etiquetas?.length}<span class="etiq">{#each equipo.etiquetas as t (t)}<EtiquetaChip nombre={t} />{/each}</span>{/if}
    </span>
    {#if cuadros.length}
      <span class="dias solo-ancho"><DiasCuadros dias={cuadros} tamano="mini" etiqueta="Copias de {equipo.nombre} en los últimos 14 días" /></span>
    {/if}
    <span class="fila-meta cuando solo-ancho num">
      {#if sinConfirmar}{:else if ultima}<span>Última <Tiempo iso={ultima} /></span>{:else}<span>Sin copias todavía</span>{/if}
      {#if proxima}<span class="faint" use:tip={`Próxima copia ${cuandoFrase(proxima, reloj.ahora)}`}>Próxima {relativo(proxima, reloj.ahora)}</span>{/if}
    </span>
    <span class="estado solo-ancho"><EnMarcha equipo={equipo.id} compacto /><Chip tono={salud.tono} texto={salud.texto} /></span>
  </a>
  {#if acciones && activo}
    <span class="acc"><MenuAcciones etiqueta="Acciones rápidas de {equipo.nombre}" texto="" grupos={menu} /></span>
  {:else if acciones && sinConfirmar}
    <span class="acc acc-confirmar"><SinConfirmar {cliente} {equipo} compacto /></span>
  {/if}
</div>

{#if copiar && actual.cliente}
  <OrdenDialog
    cliente={actual.cliente}
    {equipo}
    tipo="copiar_ahora"
    cuerpo={{ repo: copiar.repo, copia: copiar.copia }}
    descripcion="Se hará ahora la copia «{copiar.nombre}», sin esperar a su hora. No borra nada."
    accion="Copiar ahora"
    onclose={() => (copiar = null)}
  />
{/if}

<style>
  .fila-eq {
    position: relative;
    display: flex;
    align-items: center;
    border-top: 1px solid var(--border);
  }
  .fila-eq:first-child {
    border-top: none;
    border-radius: calc(var(--radius-lg) - 1px) calc(var(--radius-lg) - 1px) 0 0;
  }
  .fila-eq:last-child {
    border-radius: 0 0 calc(var(--radius-lg) - 1px) calc(var(--radius-lg) - 1px);
  }
  .fila-eq:only-child {
    border-radius: calc(var(--radius-lg) - 1px);
  }
  /* El fondo al pasar el ratón es de toda la fila (también bajo el menú rápido). */
  .fila-eq {
    transition: background var(--dur-fast) var(--ease);
  }
  .fila-eq:hover {
    background: var(--surface-2);
  }
  .fila-eq > .fila {
    flex: 1;
    min-width: 0;
    border-top: none;
    background: none;
  }
  .acc {
    flex: none;
    padding-right: var(--sp-2);
  }
  .trasladado {
    opacity: 0.6;
  }
  .etiq {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 4px;
  }
  .icono {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    color: var(--text-2);
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .conn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .dias {
    flex: none;
  }
  .cuando {
    display: flex;
    flex-direction: column;
    min-width: 128px;
    line-height: var(--lh-sm);
  }
  .solo-movil {
    display: none;
  }
  /* El estado en una columna de ancho fijo: así los cuadros y las fechas quedan alineados entre filas. */
  /* Con algo en marcha, su chip encima del estado (en la misma columna). */
  .estado {
    display: flex;
    flex: none;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
    min-width: 118px;
  }
  @media (max-width: 900px) {
    .dias {
      display: none;
    }
  }
  @media (max-width: 640px) {
    .solo-ancho {
      display: none;
    }
    .solo-movil {
      display: block;
    }
    .fila-eq > .fila {
      align-items: flex-start;
    }
    .est-movil {
      display: flex;
      flex-wrap: wrap;
      gap: 4px;
      margin: 2px 0;
    }
    .acc {
      align-self: flex-start;
      padding-top: 10px;
    }
  }
</style>
