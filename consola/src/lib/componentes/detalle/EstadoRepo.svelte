<script lang="ts">
  // «¿Por qué este estado?»: la última vuelta y lo que dijo, los problemas
  // recientes (cada uno abre su vuelta) y lo que falta en la protección.
  import { CircleHelp } from "@lucide/svelte";
  import type { RepoInforme } from "$lib/tipos";
  import { problemasRecientes } from "$lib/detalle";
  import { proteccion, TEXTO_RESULTADO, TONO_COMPROBACION, TONO_RESULTADO, TEXTO_TAREA, TONO_TAREA, ultimaEjecucion } from "$lib/repo";
  import { explicarError } from "$lib/copia";
  import { fechaCorta, relativo } from "$lib/formato";
  import { tip } from "$lib/tooltip";
  import Chip from "../Chip.svelte";
  import { abrirVuelta } from "./navegar";
  import "./pulsable.css";

  let { inf, ahora }: { inf: RepoInforme | null; ahora: number } = $props();

  const ultima = $derived(ultimaEjecucion(inf));
  const problemas = $derived(problemasRecientes(inf?.ejecuciones ?? []));
  const prot = $derived(proteccion(inf));
  const pendientes = $derived(prot?.items.filter((i) => i.estado !== "ok") ?? []);
  const tareas = $derived(
    (
      [
        ["Verificación", inf?.verificacion],
        ["Prueba de restauración", inf?.prueba_restauracion],
        ["Copia externa", inf?.externa],
      ] as const
    ).filter(([, t]) => t && t.resultado !== "ok"),
  );
</script>

<div class="estado">
  <section>
    <h3>La última copia</h3>
    {#if ultima}
      <button class="pulsable-bloque fila" use:tip={"Ver detalle"} onclick={() => abrirVuelta(ultima!.hora)}>
        <Chip tono={TONO_RESULTADO[ultima.resultado]} texto={TEXTO_RESULTADO[ultima.resultado]} />
        <span>{relativo(ultima.hora, ahora)}</span>
        {#if ultima.mensaje_corto && ultima.resultado !== "ok"}<span class="msg">{explicarError(ultima.mensaje_corto).titulo}: «{ultima.mensaje_corto}»</span>{/if}
      </button>
    {:else}
      <p class="faint">Todavía no hay copias en el informe.</p>
    {/if}
  </section>

  <section>
    <h3>Problemas recientes <span class="faint">· 60 días</span></h3>
    {#if !problemas.length}
      <p class="faint">Ninguno: todas las copias fueron bien.</p>
    {:else}
      <ul>
        {#each problemas as e (e.hora)}
          <li>
            <button class="pulsable-bloque fila" use:tip={"Ver detalle"} onclick={() => abrirVuelta(e.hora)}>
              <span class="num cuando">{fechaCorta(e.hora)}</span>
              <Chip pequeno tono={TONO_RESULTADO[e.resultado]} texto={TEXTO_RESULTADO[e.resultado]} />
              <span class="msg">{explicarError(e.mensaje_corto).titulo}{#if e.mensaje_corto}: «{e.mensaje_corto}»{/if}</span>
            </button>
          </li>
        {/each}
      </ul>
      <a class="btn btn-sm btn-ghost" href="/ayuda#si-copia-falla"><CircleHelp size={14} />¿Qué hago si una copia falla?</a>
    {/if}
  </section>

  {#if tareas.length}
    <section>
      <h3>Comprobaciones</h3>
      <ul>
        {#each tareas as [nombre, t] (nombre)}
          <li class="fila"><Chip pequeno tono={TONO_TAREA[t!.resultado]} texto={`${nombre}: ${TEXTO_TAREA[t!.resultado].toLowerCase()}`} />{#if t!.ultima}<span class="faint">{relativo(t!.ultima, ahora)}</span>{/if}{#if t!.mensaje_corto}<span class="msg">{t!.mensaje_corto}</span>{/if}</li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if pendientes.length}
    <section>
      <h3>Lo que falta en la protección</h3>
      <ul>
        {#each pendientes as i (i.id)}
          <li class="fila"><Chip pequeno tono={TONO_COMPROBACION[i.estado]} texto={i.etiqueta} /><span class="msg">{i.detalle}</span></li>
        {/each}
      </ul>
    </section>
  {/if}
</div>

<style>
  .estado {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
  }
  h3 {
    margin: 0 0 6px;
    font-size: var(--fs-sm);
    font-weight: 600;
  }
  h3 .faint {
    font-weight: 400;
  }
  ul {
    margin: 0 0 6px;
    padding: 0;
    list-style: none;
  }
  .fila {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
    padding: 8px 4px;
    border-top: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-size: var(--fs-sm);
  }
  button.fila:hover {
    background: var(--bg-subtle);
  }
  .msg {
    flex-basis: 100%;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
</style>
