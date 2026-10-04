<script lang="ts">
  // El estado de un equipo («Copia fallida», «Atrasado»…) que se pulsa para
  // ver por qué: la explicación y los últimos problemas de sus copias, cada
  // uno con enlace a esa vuelta en la página de su repositorio.
  import { ArrowRight } from "@lucide/svelte";
  import type { EquipoDetalle } from "$lib/tipos";
  import { saludEquipo } from "$lib/salud";
  import { problemasRecientes } from "$lib/detalle";
  import { explicarError } from "$lib/copia";
  import { TEXTO_RESULTADO, TONO_RESULTADO } from "$lib/repo";
  import { fechaCorta, relativo } from "$lib/formato";
  import { tip } from "$lib/tooltip";
  import Chip from "../Chip.svelte";
  import "./pulsable.css";

  let { cliente, equipo, ahora }: { cliente: string; equipo: EquipoDetalle; ahora: number } = $props();

  let abierto = $state(false);
  let caja = $state<HTMLElement>();
  const salud = $derived(saludEquipo(equipo, ahora));
  const problemas = $derived(
    (equipo.ultimo_informe?.datos.repos ?? [])
      .flatMap((r) => problemasRecientes(r.ejecuciones ?? [], 5).map((e) => ({ e, repo: r.id, nombre: r.nombre })))
      .sort((a, b) => Date.parse(b.e.hora) - Date.parse(a.e.hora))
      .slice(0, 6),
  );
  const nombreCopia = (id: string | null) => equipo.resumen?.copias?.find((k) => k.id === id)?.nombre ?? null;
  const enlace = (repo: string, hora: string) => `/c/${cliente}/equipos/${equipo.id}/repositorios/${encodeURIComponent(repo)}?${new URLSearchParams({ vuelta: hora })}`;

  function fuera(e: MouseEvent) {
    if (abierto && caja && !caja.contains(e.target as Node)) abierto = false;
  }
  function tecla(e: KeyboardEvent) {
    if (abierto && e.key === "Escape") {
      abierto = false;
      caja?.querySelector<HTMLElement>("button")?.focus();
    }
  }
</script>

<svelte:window onclick={fuera} onkeydown={tecla} />

<span class="estado-equipo" bind:this={caja}>
  <button class="pulsable-bloque boton" aria-expanded={abierto} aria-controls="por-que-{equipo.id}" use:tip={"¿Por qué? Ver detalle"} onclick={() => (abierto = !abierto)}>
    <Chip tono={salud.tono} texto={salud.texto} />
  </button>
  {#if abierto}
    <div class="globo card" id="por-que-{equipo.id}" role="region" aria-label="Por qué «{salud.texto}»">
      <p class="detalle">{salud.detalle}</p>
      {#if problemas.length}
        <h3>Últimos problemas</h3>
        <ul>
          {#each problemas as p (p.repo + p.e.hora)}
            <li>
              <a class="fila" href={enlace(p.repo, p.e.hora)} onclick={() => (abierto = false)}>
                <Chip pequeno tono={TONO_RESULTADO[p.e.resultado]} texto={TEXTO_RESULTADO[p.e.resultado]} />
                <span class="que">
                  <strong>{nombreCopia(p.e.copia) ?? p.nombre}</strong>
                  <span class="faint">{explicarError(p.e.mensaje_corto).titulo} · {fechaCorta(p.e.hora)}</span>
                </span>
                <ArrowRight size={14} />
              </a>
            </li>
          {/each}
        </ul>
      {:else if equipo.ultimo_informe}
        <p class="faint">Sin fallos ni avisos en las copias de los últimos 60 días.</p>
      {/if}
      {#if equipo.ultimo_contacto}<p class="faint pie">Último contacto {relativo(equipo.ultimo_contacto, ahora)}.</p>{/if}
    </div>
  {/if}
</span>

<style>
  .estado-equipo {
    position: relative;
    display: inline-flex;
  }
  .boton {
    width: auto;
    border-radius: 999px;
  }
  .globo {
    position: absolute;
    top: calc(100% + 8px);
    left: 0;
    z-index: 7;
    width: min(420px, calc(100vw - 32px));
    padding: var(--sp-4);
    box-shadow: var(--shadow-lg);
  }
  .detalle {
    margin: 0;
    font-size: var(--fs-sm);
  }
  h3 {
    margin: var(--sp-3) 0 6px;
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-3);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .fila {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 4px;
    color: inherit;
    text-decoration: none;
    border-top: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-size: var(--fs-sm);
  }
  .fila:hover {
    background: var(--bg-subtle);
  }
  .que {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  .que .faint {
    font-size: var(--fs-xs);
  }
  .pie {
    margin: var(--sp-3) 0 0;
    font-size: var(--fs-xs);
  }
  /* En el móvil, a lo ancho de la pantalla (sin salirse por la derecha). */
  @media (max-width: 640px) {
    .globo {
      position: fixed;
      top: 112px;
      left: 16px;
      right: 16px;
      width: auto;
      max-height: calc(100dvh - 140px);
      overflow: auto;
    }
  }
</style>
