<script lang="ts">
  // «Todos los clientes» → Restauraciones (plan 0.8.0, D4, 14xx; 13d): el registro de
  // restauraciones de todos los clientes de la cuenta (GET /api/panel/restauraciones),
  // la más reciente primero, con sus cifras (cuántas, éxito, tiempo medio). Cada fila
  // lleva a las restauraciones de su cliente, donde está su ficha.
  // 0.8.1 (E3, 15aa): también las pruebas, con el filtro «Reales / Pruebas automáticas /
  // Pruebas a mano» y sus cifras aparte.
  import { onMount } from "svelte";
  import { ArchiveRestore, CircleAlert, CircleCheck, Clock, ListChecks } from "@lucide/svelte";
  import { reloj } from "$lib/estado.svelte";
  import { bytes, plural, relativo } from "$lib/formato";
  import { sinTildes } from "$lib/global";
  import { claseRestauracion, estadisticas, NOMBRES_CLASE, textoDesde, textoDestino, textoMotivo, textoMuestra, textoQuien, textoTardo, type ClaseRestauracion } from "$lib/restauraciones";
  import { restauracionesDeTodos } from "$lib/todosSecciones.svelte";
  import { restauracionesTodos } from "$lib/todosSecciones";
  import { todos } from "$lib/todos.svelte";
  import { seguirTodos } from "$lib/todosVivo";
  import type { Restauraciones } from "$lib/tipos";
  import type { PanelDe } from "$lib/global";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Cifra from "$lib/componentes/Cifra.svelte";
  import ErrorCarga from "$lib/componentes/ErrorCarga.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import MarcaCliente from "$lib/componentes/MarcaCliente.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import FiltroCliente from "$lib/componentes/todos/FiltroCliente.svelte";

  let datos = $state<PanelDe<Restauraciones> | null>(null);
  let error = $state("");
  async function cargar() {
    try {
      datos = await restauracionesDeTodos();
      error = "";
    } catch (e) {
      error = (e as Error).message;
    }
  }
  onMount(() => seguirTodos(cargar, 60_000));

  let cliente = $state("");
  let buscar = $state("");
  let clase = $state<ClaseRestauracion>("real");
  const filas = $derived(restauracionesTodos(datos, todos.clientes));
  const porClase = $derived({
    real: filas.filter((r) => claseRestauracion(r) === "real").length,
    prueba_auto: filas.filter((r) => claseRestauracion(r) === "prueba_auto").length,
    prueba_mano: filas.filter((r) => claseRestauracion(r) === "prueba_mano").length,
  });
  const vistas = $derived(
    filas.filter((r) => {
      if (claseRestauracion(r) !== clase) return false;
      if (cliente && r.cliente.id !== cliente) return false;
      const p = sinTildes(buscar).split(/\s+/).filter(Boolean);
      const t = sinTildes([r.cliente.nombre, r.equipoNombre ?? "", r.repo, r.por ?? "", textoMotivo(r.motivoFinal?.tipo) ?? ""].join(" "));
      return p.every((w) => t.includes(w));
    }),
  );
  const cifras = $derived(estadisticas(vistas));
</script>

<svelte:head><title>Restauraciones · Todos los clientes · Resguardo Server</title></svelte:head>

<div class="page ancha">
  <CabeceraPagina titulo="Restauraciones" icono={ArchiveRestore} migas={[{ texto: "Todos los clientes", href: "/todos" }, { texto: "Restauraciones" }]} resumen={datos ? plural(filas.length, "restauración", "restauraciones") : undefined} />

  {#if error && !datos}
    <ErrorCarga que="las restauraciones" {error} reintentar={cargar} />
  {:else if !datos}
    <Esqueleto forma="filas" n={4} etiqueta="Cargando las restauraciones…" />
  {:else if !filas.length}
    <div class="card"><Vacio icono={ArchiveRestore} ilustracion="restaurado" titulo="Todavía no hay restauraciones" texto="Cada restauración de cualquier cliente queda aquí: quién, qué, cuánto y cuánto tardó." /></div>
  {:else}
    <div class="segmented inline" role="group" aria-label="Qué restauraciones">
      {#each ["real", "prueba_auto", "prueba_mano"] as const as k (k)}
        <button class:on={clase === k} aria-pressed={clase === k} onclick={() => (clase = k)}>{NOMBRES_CLASE[k]}<span class="num cuenta">{porClase[k]}</span></button>
      {/each}
    </div>
    <div class="cifras">
      <Cifra icono={ListChecks} etiqueta={clase === "real" ? "Restauraciones" : "Pruebas"} valor={String(cifras.total)} sub={plural(cifras.fallidas, "fallida", "fallidas")} />
      <Cifra icono={CircleCheck} etiqueta="Correctas" valor={cifras.total ? `${Math.round((cifras.correctas / cifras.total) * 100)} %` : "—"} sub={plural(cifras.correctas, "correcta", "correctas")} />
      <Cifra icono={Clock} etiqueta={clase === "real" ? "Tiempo medio (RTO real)" : "Tiempo medio de una prueba"} valor={textoTardo(cifras.medio_s)} sub={cifras.maximo_s != null ? `la más larga, ${textoTardo(cifras.maximo_s)}` : "sin correctas"} />
    </div>
    <FiltroCliente seccion="restauraciones" bind:cliente bind:buscar placeholder="Buscar cliente, equipo, repositorio o quién" />
    <ul class="card p-0 lista">
      {#each vistas as r (r.clave)}
        <li>
          <span class="ic" class:mal={r.resultado !== "ok"} aria-hidden="true">{#if r.resultado !== "ok"}<CircleAlert size={16} />{:else}<CircleCheck size={16} />{/if}</span>
          <span class="txt">
            <span class="quien"><MarcaCliente nombre={r.cliente.nombre} marca={r.cliente.marca} tam={16} /><a class="link-suave" href="/c/{r.cliente.id}/recuperar/restauraciones?clase={clase}">{r.cliente.nombre}</a>{#if r.equipoNombre}{" · "}<a class="link-suave" href="/c/{r.cliente.id}/equipos/{r.equipo}">{r.equipoNombre}</a>{/if}</span>
            <strong>{r.resultado === "ok" ? (r.tipo === "prueba_restauracion" ? "Prueba correcta" : "Restaurado") : "Falló"} · {r.repo}<span class="sr-only">{r.resultado === "ok" ? " (correcta)" : " (fallida)"}</span></strong>
            {#if r.tipo === "prueba_restauracion"}
              <span class="det">{textoMuestra(r)}{r.bytes ? ` · ${bytes(r.bytes)}` : ""} · {textoDesde(r)} · {textoQuien(r)}</span>
            {:else}
              <span class="det">{plural(r.elementos ?? 0, "elemento", "elementos")}{r.bytes ? ` · ${bytes(r.bytes)}` : ""} · {textoDestino(r.destino)} · {textoQuien(r)}{#if r.motivoFinal}{" · "}{textoMotivo(r.motivoFinal.tipo)}{/if}</span>
            {/if}
            {#if r.resultado !== "ok" && r.mensaje}<span class="det mal">{r.mensaje}</span>{/if}
          </span>
          <span class="cuando num">{relativo(r.hora, reloj.ahora)}<span class="faint">tardó {textoTardo(r.duracion_s)}</span></span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .cifras {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: var(--sp-4);
  }
  .lista {
    list-style: none;
    margin: 0;
  }
  li {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    padding: 12px var(--sp-4);
    border-top: 1px solid var(--border);
  }
  li:first-child {
    border-top: none;
  }
  .ic {
    display: inline-flex;
    margin-top: 2px;
    color: var(--ok);
  }
  .ic.mal,
  .mal {
    color: var(--bad);
  }
  .txt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .quien {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .det {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .cuenta {
    margin-left: 6px;
    color: var(--text-3);
    font-weight: 400;
  }
  .cuando {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    font-size: var(--fs-sm);
  }
</style>
