<script lang="ts">
  // «Todos los clientes» → Avisos (plan 0.8.0, D4, 14xx): los avisos sin revisar de
  // todos los clientes de la cuenta (GET /api/panel/avisos), lo urgente primero, con
  // su cliente y su equipo. «Visto» como en cada cliente (técnicos o más); «Ver» lleva
  // a los avisos de su cliente, donde está su acción directa.
  import { onMount } from "svelte";
  import { BellOff, BellRing, Check, CircleAlert, Info, TriangleAlert } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { puede } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { plural } from "$lib/formato";
  import { sinTildes } from "$lib/global";
  import { PESO, type Tono } from "$lib/salud";
  import { avisosDeTodos } from "$lib/todosSecciones.svelte";
  import { avisosTodos, type AvisoTodos } from "$lib/todosSecciones";
  import { todos } from "$lib/todos.svelte";
  import { seguirTodos } from "$lib/todosVivo";
  import type { Aviso, TipoAviso } from "$lib/tipos";
  import type { PanelDe } from "$lib/global";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import ErrorCarga from "$lib/componentes/ErrorCarga.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import MarcaCliente from "$lib/componentes/MarcaCliente.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import FiltroCliente from "$lib/componentes/todos/FiltroCliente.svelte";

  let datos = $state<PanelDe<Aviso[]> | null>(null);
  let error = $state("");
  async function cargar() {
    try {
      datos = await avisosDeTodos();
      error = "";
    } catch (e) {
      error = (e as Error).message;
    }
  }
  onMount(() => seguirTodos(cargar));

  // Lo mismo que en los avisos de cada cliente, en corto: qué es y lo grave que es.
  const INFO: Partial<Record<TipoAviso, [string, Tono]>> = {
    intentos_fallidos: ["Intentos fallidos", "bad"],
    bloqueo: ["Equipo bloqueado", "bad"],
    orden_destructiva: ["Orden con espera", "warn"],
    equipo_sin_contacto: ["Sin contacto", "bad"],
    copia_fallida: ["Copia fallida", "bad"],
    copia_atrasada: ["Copia atrasada", "warn"],
    servicio_detenido: ["Servicio detenido", "bad"],
    cambio_inusual: ["Cambio inusual", "warn"],
    verificacion_fallida: ["Verificación fallida", "bad"],
    externa_fallida: ["Copia derivada fallida", "warn"],
    prueba_fallida: ["Prueba de restauración fallida", "warn"],
    espejo_fallido: ["Espejo fallido", "warn"],
    retencion_fallida: ["Retención del almacén fallida", "warn"],
    cadena_parada: ["Cadena parada", "warn"],
    orden_no_aplicada: ["Orden sin aplicar", "warn"],
    orden_aprobada_en_equipo: ["Orden aprobada en el equipo", "warn"],
    orden_en_espera: ["Orden en espera desde otra consola", "warn"],
    auditoria_rehecha: ["Actividad rehecha", "bad"],
    actualizacion_fallida: ["Actualización fallida", "warn"],
    nube_sin_anular: ["Permiso de nube sin anular", "warn"],
    disco_lleno: ["Disco casi lleno", "warn"],
    sin_soporte_pronto: ["Sistema sin soporte pronto", "neutral"],
    sin_soporte: ["Sistema sin soporte", "warn"],
    kit_por_comprobar: ["Kit por comprobar", "neutral"],
    cambio_clave: ["Clave de administración cambiada", "bad"],
  };
  const info = (a: Aviso): [string, Tono] => INFO[a.tipo] ?? [a.tipo, "neutral"];
  const ICONO = { bad: CircleAlert, warn: TriangleAlert, info: Info, ok: Check, paused: Info, neutral: Info };

  let cliente = $state("");
  let buscar = $state("");
  const filas = $derived(avisosTodos(datos, todos.clientes));
  const vistas = $derived(
    filas
      .filter((a) => {
        if (cliente && a.cliente.id !== cliente) return false;
        const p = sinTildes(buscar).split(/\s+/).filter(Boolean);
        const t = sinTildes([a.mensaje, a.cliente.nombre, a.equipoNombre ?? "", info(a)[0]].join(" "));
        return p.every((w) => t.includes(w));
      })
      .sort((a, b) => PESO[info(a)[1]] - PESO[info(b)[1]] || b.creado.localeCompare(a.creado)),
  );
  const urgentes = $derived(filas.filter((a) => info(a)[1] === "bad").length);

  let marcando = $state<Set<string>>(new Set());
  async function visto(a: AvisoTodos) {
    marcando = new Set([...marcando, a.clave]);
    try {
      await api.marcarVisto(a.cliente.id, a.id);
      avisar(`Visto: «${info(a)[0]}» de ${a.cliente.nombre}.`);
      await cargar();
    } catch (e) {
      fallo(e);
    } finally {
      marcando = new Set([...marcando].filter((x) => x !== a.clave));
    }
  }
</script>

<svelte:head><title>Avisos · Todos los clientes · Resguardo Server</title></svelte:head>

<div class="page ancha">
  <CabeceraPagina titulo="Avisos" icono={BellRing} migas={[{ texto: "Todos los clientes", href: "/todos" }, { texto: "Avisos" }]} resumen={datos ? `${plural(filas.length, "aviso sin revisar", "avisos sin revisar")}${urgentes ? ` · ${plural(urgentes, "urgente", "urgentes")}` : ""}` : undefined} />

  {#if error && !datos}
    <ErrorCarga que="los avisos" {error} reintentar={cargar} />
  {:else if !datos}
    <Esqueleto forma="filas" n={5} etiqueta="Cargando los avisos de todos los clientes…" />
  {:else if !filas.length}
    <div class="card"><Vacio icono={BellOff} ilustracion="todo-en-orden" titulo="Nada que revisar" texto="Ningún cliente tiene avisos sin revisar." /></div>
  {:else}
    <FiltroCliente seccion="avisos" bind:cliente bind:buscar placeholder="Buscar en los avisos" />
    <ul class="card p-0 lista">
      {#each vistas as a (a.clave)}
        {@const [titulo, tono] = info(a)}
        {@const Ic = ICONO[tono]}
        <li>
          <span class="ic tone-{tono}" aria-hidden="true"><Ic size={16} /></span>
          <span class="txt">
            <span class="quien"><MarcaCliente nombre={a.cliente.nombre} marca={a.cliente.marca} tam={16} /><a class="link-suave" href="/c/{a.cliente.id}">{a.cliente.nombre}</a>{#if a.equipoNombre}{" · "}<a class="link-suave" href="/c/{a.cliente.id}/equipos/{a.equipo}">{a.equipoNombre}</a>{/if}</span>
            <strong>{titulo}<span class="sr-only"> ({tono === "bad" ? "urgente" : tono === "warn" ? "para revisar" : "informativo"})</span></strong>
            <span class="msg">{a.mensaje}</span>
            <span class="faint"><Tiempo iso={a.creado} /></span>
          </span>
          <span class="acc">
            <a class="btn btn-sm" href="/c/{a.cliente.id}/avisos">Ver</a>
            {#if puede.ordenar(a.cliente.rol)}<button class="btn btn-sm btn-ghost" disabled={marcando.has(a.clave)} onclick={() => visto(a)}><Check size={14} />Visto</button>{/if}
          </span>
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
    color: var(--tone);
  }
  .tone-bad {
    --tone: var(--bad);
  }
  .tone-warn {
    --tone: var(--warn);
  }
  .tone-neutral,
  .tone-info,
  .tone-ok,
  .tone-paused {
    --tone: var(--text-3);
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
    align-items: center;
    gap: 6px;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .msg {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .faint {
    font-size: var(--fs-xs);
  }
  .acc {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
</style>
