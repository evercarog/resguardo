<script lang="ts">
  // «¿Por qué este estado?»: la última vuelta y lo que dijo, los problemas
  // recientes (cada uno abre su vuelta) y lo que falta en la protección.
  import { CircleHelp } from "@lucide/svelte";
  import type { RepoInforme } from "$lib/tipos";
  import { problemasRecientes } from "$lib/detalle";
  import { proteccion, TEXTO_TAREA, textoResultado, TONO_COMPROBACION, TONO_TAREA, tonoResultado, ultimaEjecucion } from "$lib/repo";
  import { explicarError } from "$lib/copia";
  import { fraseError } from "$lib/errorCopia";
  import ErrorCopia from "../ErrorCopia.svelte";
  import { fechaCorta, relativo } from "$lib/formato";
  import { tip } from "$lib/tooltip";
  import Chip from "../Chip.svelte";
  import { abrirVuelta } from "./navegar";
  import "./pulsable.css";

  // B4 (0.8.2): `conExterna`, si el repositorio aún tiene su copia derivada «de cada día» (sin
  // ella, la última vuelta que queda en el informe es de una copia ya quitada).
  let { inf, ahora, conExterna = true }: { inf: RepoInforme | null; ahora: number; conExterna?: boolean } = $props();

  const ultima = $derived(ultimaEjecucion(inf));
  const problemas = $derived(problemasRecientes(inf?.ejecuciones ?? []));
  const prot = $derived(proteccion(inf));
  const pendientes = $derived(prot?.items.filter((i) => i.estado !== "ok") ?? []);
  const tareas = $derived(
    (
      [
        ["Verificación", inf?.verificacion],
        ["Prueba de restauración", inf?.prueba_restauracion],
        ["Copia derivada", conExterna ? inf?.externa : null],
      ] as const
    ).filter(([, t]) => t && t.resultado !== "ok"),
  );
</script>

<div class="estado">
  <section>
    <h3>La última copia</h3>
    {#if ultima}
      <button class="pulsable-bloque fila" use:tip={"Ver el detalle"} onclick={() => abrirVuelta(ultima!.hora)}>
        <Chip tono={tonoResultado(ultima)} texto={textoResultado(ultima)} />
        <span>{relativo(ultima.hora, ahora)}</span>
        {#if ultima.mensaje_corto && ultima.resultado !== "ok"}<span class="msg">{fraseError(ultima.mensaje_corto)}</span>{/if}
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
            <button class="pulsable-bloque fila" use:tip={"Ver el detalle"} onclick={() => abrirVuelta(e.hora)}>
              <span class="num cuando">{fechaCorta(e.hora)}</span>
              <Chip pequeno tono={tonoResultado(e)} texto={textoResultado(e)} />
              <span class="msg">{e.mensaje_corto ? fraseError(e.mensaje_corto) : explicarError(e.mensaje_corto).titulo}</span>
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
          <li class="fila"><Chip pequeno tono={TONO_TAREA[t!.resultado]} texto={`${nombre}: ${TEXTO_TAREA[t!.resultado].toLowerCase()}`} />{#if t!.ultima}<span class="faint">{relativo(t!.ultima, ahora)}</span>{/if}{#if t!.mensaje_corto}<span class="msg"><ErrorCopia mensaje={t!.mensaje_corto} detalle={t!.detalle} /></span>{/if}</li>
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
