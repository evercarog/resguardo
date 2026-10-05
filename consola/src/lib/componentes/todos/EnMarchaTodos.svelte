<script lang="ts">
  // Lo que está en marcha ahora en todos los clientes: una fila por tarea
  // (cliente, equipo, qué hace, la barra con su porcentaje y sus cifras) y,
  // si el equipo manda sus ritmos (v1.36), las ondas en vivo de lectura y
  // subida. Con «reducir movimiento», las ondas no fluyen (GraficaOndas).
  import { Activity, LoaderCircle } from "@lucide/svelte";
  import GraficaOndas from "$ui/componentes/GraficaOndas.svelte";
  import { seriesBytes } from "$ui/ritmos";
  import type { PanelCliente } from "$lib/global";
  import { ritmoTodos, todos } from "$lib/todos.svelte";
  import { cifrasTarea, porcentaje, textoFase } from "$lib/textoProgreso";
  import MarcaCliente from "../MarcaCliente.svelte";

  let { clientes }: { clientes: PanelCliente[] } = $props();
  const porId = $derived(new Map(clientes.map((c) => [c.id, c])));
  const filas = $derived(
    todos.progreso.flatMap((x) => {
      const c = porId.get(x.cliente);
      const e = c?.equipos.find((y) => y.id === x.equipo);
      return c && e ? x.tareas.map((t) => ({ c, e, t, clave: `${x.equipo}|${t.tipo}|${t.repo}|${t.copia ?? ""}` })) : [];
    }),
  );
</script>

{#if filas.length}
  <section aria-labelledby="t-en-marcha">
    <div class="section-head">
      <h2 id="t-en-marcha"><Activity size={16} />En marcha <span class="count">· {filas.length}</span></h2>
    </div>
    <div class="card p-0 lista">
      {#each filas as f (f.clave)}
        {@const p = porcentaje(f.t)}
        {@const ondas = seriesBytes(f.t.tipo, ritmoTodos(f.e.id, f.t)).filter((s) => s.puntos.length >= 2)}
        {@const nombre = f.t.nombre ?? f.e.resumen?.repositorios?.find((r) => r.id === f.t.repo)?.nombre ?? ""}
        <div class="tarea">
          <div class="cab">
            <MarcaCliente nombre={f.c.nombre} marca={f.c.marca} tam={28} />
            <span class="quien">
              <a href="/c/{f.c.id}/equipos/{f.e.id}"><strong>{f.e.nombre}</strong></a>
              <span class="faint">{f.c.nombre}{nombre ? ` · «${nombre}»` : ""}</span>
            </span>
            <span class="fase"><LoaderCircle size={13} class="spin" aria-hidden="true" />{textoFase(f.t)}</span>
            {#if p != null}<strong class="pct num" aria-hidden="true">{p} %</strong>{/if}
          </div>
          <div
            class="barra"
            class:indeterminada={p == null}
            role="progressbar"
            aria-label="Progreso de {f.e.nombre}{nombre ? ` («${nombre}»)` : ''}, {f.c.nombre}"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={p ?? undefined}
            aria-valuetext={[textoFase(f.t), p != null ? `${p} %` : null, ...cifrasTarea(f.t)].filter(Boolean).join(", ")}
          >
            <span style:width={p == null ? undefined : `${p}%`}></span>
          </div>
          {#if cifrasTarea(f.t).length}<p class="meta faint num">{cifrasTarea(f.t).join(" · ")}</p>{/if}
          {#if ondas.length}
            <GraficaOndas series={ondas} titulo={`Ritmo de ${f.e.nombre} (${f.c.nombre})`} alto={56} retraso={6000} />
          {/if}
        </div>
      {/each}
    </div>
  </section>
{/if}

<style>
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  h2 :global(svg) {
    color: var(--text-3);
  }
  .lista > .tarea + .tarea {
    border-top: 1px solid var(--border);
  }
  .tarea {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: var(--sp-4);
  }
  .cab {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-width: 0;
  }
  .quien {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
  }
  .quien a {
    color: var(--text-1);
  }
  .quien .faint {
    overflow: hidden;
    font-size: var(--fs-xs);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .fase {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: var(--fs-sm);
    color: var(--info);
    white-space: nowrap;
  }
  .pct {
    min-width: 44px;
    text-align: right;
  }
  .barra {
    height: 6px;
    overflow: hidden;
    background: var(--surface-2);
    border-radius: 999px;
  }
  .barra > span {
    display: block;
    height: 100%;
    background: var(--info);
    border-radius: 999px;
    transition: width 0.6s var(--ease);
  }
  .barra.indeterminada > span {
    width: 30%;
    animation: ir 1.4s ease-in-out infinite;
  }
  @keyframes ir {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .barra > span {
      transition: none;
    }
    .barra.indeterminada > span {
      width: 100%;
      opacity: 0.4;
      animation: none;
    }
  }
  .meta {
    margin: 0;
    font-size: var(--fs-xs);
  }
  @media (max-width: 640px) {
    .cab {
      flex-wrap: wrap;
    }
    .quien {
      flex-basis: calc(100% - 44px);
    }
    .fase {
      margin-left: 40px;
    }
  }
</style>
