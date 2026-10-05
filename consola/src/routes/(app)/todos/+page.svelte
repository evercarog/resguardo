<script lang="ts">
  // «Todos los clientes» (v1.3x): el Estado de todos los clientes de la
  // cuenta a la vez, como el de cada uno. Arriba, lo que necesita atención en
  // cualquiera (lo más grave primero, con su cliente y su acción); las
  // cifras de todos; lo que está en marcha con sus ondas; la salud de cada
  // cliente; el mapa de la protección con los clientes delante; y
  // «¿Cuándo se llena?» de todos los almacenes y destinos.
  //
  // Solo los clientes de los que la cuenta es miembro (GET /api/panel): el
  // propietario del servidor no ve aquí los ajenos (para eso, las cifras de
  // «Clientes del servidor»).
  import { onMount } from "svelte";
  import { Archive, Building2, CalendarClock, CircleAlert, CircleCheck, Database, Info, Layers, LockKeyhole, ShieldCheck, TriangleAlert } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { app, cargarClientes, reloj } from "$lib/estado.svelte";
  import { bytes, cuandoFrase, numero, plural, relativo } from "$lib/formato";
  import { tip } from "$lib/tooltip";
  import { atencion as atencionDe, llenadoGlobal, saludCliente, totales as totalesDe, type Atencion } from "$lib/global";
  import { cargarTodos, todos, vigilarTodos } from "$lib/todos.svelte";
  import { PESO, type Tono } from "$lib/salud";
  import type { Cliente } from "$lib/tipos";
  import Anuncio from "$lib/componentes/Anuncio.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import MarcaCliente from "$lib/componentes/MarcaCliente.svelte";
  import OrdenDialog from "$lib/componentes/OrdenDialog.svelte";
  import PantallaError from "$lib/componentes/PantallaError.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import TarjetaCliente from "$lib/componentes/todos/TarjetaCliente.svelte";
  import EnMarchaTodos from "$lib/componentes/todos/EnMarchaTodos.svelte";
  import MapaTodos from "$lib/componentes/todos/MapaTodos.svelte";
  import LlenadoTodos from "$lib/componentes/todos/LlenadoTodos.svelte";

  onMount(() => {
    void cargarClientes();
    return vigilarTodos();
  });

  const clientes = $derived(todos.clientes);
  const saludes = $derived(new Map(clientes.map((c) => [c.id, saludCliente(c, reloj.ahora)])));
  /** Las tarjetas de los clientes: lo urgente arriba y por nombre. */
  const ordenados = $derived([...clientes].sort((a, b) => PESO[saludes.get(a.id)!.tono] - PESO[saludes.get(b.id)!.tono] || a.nombre.localeCompare(b.nombre)));
  const lista = $derived(atencionDe(clientes, reloj.ahora));
  const urgentes = $derived(lista.filter((x) => x.tono === "bad" || x.tono === "warn"));
  const t = $derived(totalesDe(clientes, reloj.ahora));
  const llenado = $derived(llenadoGlobal(clientes, reloj.ahora));
  const ultima = $derived(
    [...saludes.values()]
      .map((s) => s.ultima)
      .filter((x): x is string => !!x)
      .sort()
      .at(-1),
  );

  const conProblemas = $derived(ordenados.filter((c) => ["bad", "warn"].includes(saludes.get(c.id)!.tono)).length);
  const titular = $derived(
    !clientes.length
      ? "Todavía no hay clientes"
      : urgentes.length
        ? urgentes.length === 1
          ? "1 cosa necesita atención"
          : `${urgentes.length} cosas necesitan atención`
        : t.equipos && t.alDia === 0
          ? "Sin copias todavía"
          : "Todo protegido",
  );
  const tonoTitular = $derived<Tono>(!clientes.length ? "neutral" : urgentes.length ? (urgentes.some((u) => u.tono === "bad") ? "bad" : "warn") : t.equipos && t.alDia === 0 ? "neutral" : "ok");

  // Lo que necesita atención: los 8 primeros y «Ver todo».
  const PRIMEROS = 8;
  let verTodo = $state(false);
  const vistos = $derived(verTodo ? lista : lista.slice(0, PRIMEROS));

  // «Copiar ahora» desde aquí: el mismo diálogo de siempre, con el cliente completo (su sal).
  let copiar = $state<{ cliente: Cliente; x: NonNullable<Atencion["accion"]["orden"]> } | null>(null);
  let abriendo = $state("");
  async function abrirCopia(a: Atencion) {
    if (!a.accion.orden) return;
    abriendo = a.titulo;
    try {
      copiar = { cliente: await api.cliente(a.cliente.id), x: a.accion.orden };
    } finally {
      abriendo = "";
    }
  }
  const ICONO_TONO = { bad: CircleAlert, warn: TriangleAlert, info: Info, ok: CircleCheck, paused: Info, neutral: Info };
</script>

<svelte:head><title>Todos los clientes · Resguardo Server</title></svelte:head>

<div class="page ancha">
  {#if todos.error && !todos.cargado}
    {#if todos.errorCodigo === "red"}
      <PantallaError ilustracion="sin-conexion" titulo="Sin conexión con el servidor" texto={todos.error}><button class="btn btn-primary" onclick={() => cargarTodos()}>Reintentar</button></PantallaError>
    {:else}
      <div class="notice notice-danger"><p>{todos.error}</p></div>
    {/if}
  {:else if !todos.cargado}
    <Esqueleto forma="cifras" n={4} etiqueta="Cargando todos los clientes…" />
    <Esqueleto forma="tarjetas" n={3} />
  {:else if !clientes.length}
    <div class="card">
      <Vacio icono={Building2} ilustracion="bienvenida" titulo="Todavía no hay clientes" texto={app.cuenta?.superusuario ? "Crea el primero desde «Clientes»." : "Pide a quien administra el servidor que te invite a un cliente."}>
        <a class="btn btn-primary" href="/clientes">Ir a Clientes</a>
      </Vacio>
    </div>
  {:else}
    <section class="card resumen tone-{tonoTitular}" aria-labelledby="titular">
      <Anuncio texto={titular} />
      <div class="cab">
        <span class="icono">
          {#if tonoTitular === "ok"}<CircleCheck size={28} />{:else if tonoTitular === "bad"}<CircleAlert size={28} />{:else if tonoTitular === "warn"}<TriangleAlert size={28} />{:else}<Layers size={28} />{/if}
        </span>
        <div>
          <p class="antetitulo"><Layers size={13} />Todos los clientes</p>
          <h1 id="titular">{titular}</h1>
          <p>
            {plural(clientes.length, "cliente", "clientes")}{#if conProblemas}{" "}({conProblemas} con problemas){/if} · {plural(t.equipos, "equipo", "equipos")} · {#if ultima}última copia <Tiempo iso={ultima} />{:else}todavía sin copias{/if}
          </p>
        </div>
      </div>
      {#if lista.length}
        <ul class="urgentes" aria-label="Lo que necesita atención">
          {#each vistos as u, i (i)}
            {@const Ic = ICONO_TONO[u.tono]}
            <li>
              <span class="u-ic tone-{u.tono}" aria-hidden="true"><Ic size={16} /></span>
              <span class="texto">
                <span class="quien"><MarcaCliente nombre={u.cliente.nombre} marca={u.cliente.marca} tam={16} /><a href="/c/{u.cliente.id}">{u.cliente.nombre}</a></span>
                <strong>{u.titulo}</strong>
                <span class="det">{u.detalle}</span>
              </span>
              {#if u.accion.href}
                <a class="btn btn-sm" href={u.accion.href}>{u.accion.texto}</a>
              {:else if u.accion.orden}
                <button class="btn btn-sm" disabled={!!abriendo} onclick={() => abrirCopia(u)}>{u.accion.texto}</button>
              {/if}
            </li>
          {/each}
        </ul>
        {#if lista.length > PRIMEROS}
          <button type="button" class="btn btn-sm btn-ghost mas" aria-expanded={verTodo} onclick={() => (verTodo = !verTodo)}>{verTodo ? "Ver menos" : `Ver todo (${lista.length})`}</button>
        {/if}
      {/if}
    </section>

    {#if todos.omitidos}
      <div class="notice"><p>Se ven {clientes.length} clientes; {plural(todos.omitidos, "cliente más no cabe", "clientes más no caben")} aquí. Ábrelos desde <a href="/clientes">Clientes</a>.</p></div>
    {/if}

    <div class="cifras" role="list" aria-label="Cifras de todos los clientes">
      <div class="cifra" role="listitem">
        <span class="c-et"><span class="c-ic" aria-hidden="true"><ShieldCheck size={14} /></span>Equipos al día</span>
        <span class="c-val num">{t.alDia}<small>{` de ${t.equipos}`}</small></span>
        <span class="barra" aria-hidden="true">
          {#each ["ok", "warn", "bad", "paused", "neutral"] as const as k (k)}
            {#if t.porTono[k]}<span style:flex={t.porTono[k]} style="--tone: var(--{k})"></span>{/if}
          {/each}
        </span>
        <span class="c-sub">{t.porTono.bad ? plural(t.porTono.bad, "con un problema", "con problemas") : t.porTono.warn ? plural(t.porTono.warn, "con avisos", "con avisos") : "ninguno con problemas"}</span>
      </div>
      <div class="cifra" role="listitem">
        <span class="c-et"><span class="c-ic" aria-hidden="true"><Database size={14} /></span>Protegido</span>
        <span class="c-val num">{t.protegido ? bytes(t.protegido) : "—"}</span>
        <span class="c-sub">en {plural(t.repos, "repositorio", "repositorios")}</span>
      </div>
      <div class="cifra" role="listitem">
        <span class="c-et"><span class="c-ic" aria-hidden="true"><Archive size={14} /></span>Copias en 24 h</span>
        <span class="c-val num">{numero(t.versiones24)}</span>
        <span class="c-sub">{#if t.fallos24}<span class="mal">{plural(t.fallos24, "copia fallida", "copias fallidas")}</span>{:else}ninguna fallida{/if}</span>
      </div>
      <div class="cifra" role="listitem">
        <span class="c-et"><span class="c-ic" aria-hidden="true"><CalendarClock size={14} /></span>Próxima copia</span>
        <span class="c-val" use:tip={t.proxima ? cuandoFrase(t.proxima.cuando, reloj.ahora) : undefined}>{t.proxima ? relativo(t.proxima.cuando, reloj.ahora) : "—"}</span>
        <span class="c-sub">{t.proxima ? `${t.proxima.copia} · ${t.proxima.equipo.nombre} · ${t.proxima.cliente.nombre}` : "nada programado"}</span>
      </div>
    </div>

    <EnMarchaTodos {clientes} />

    <section aria-labelledby="t-clientes">
      <div class="section-head">
        <h2 id="t-clientes">Clientes <span class="count">· {clientes.length}</span></h2>
        <a class="btn btn-sm btn-ghost" href="/clientes">Gestionar clientes</a>
      </div>
      <div class="rejilla clientes">
        {#each ordenados as c (c.id)}<TarjetaCliente cliente={c} salud={saludes.get(c.id)!} ahora={reloj.ahora} />{/each}
      </div>
    </section>

    <MapaTodos {clientes} ahora={reloj.ahora} />

    <LlenadoTodos lista={llenado} ahora={reloj.ahora} />

    {#if clientes.some((c) => !c.informes_completos)}
      <p class="faint nota">Algunos equipos no traen aquí su informe (son muchos): sus cuadros y lo que ocupan se ven completos al abrir su cliente.</p>
    {/if}
    <p class="privacidad"><LockKeyhole size={12} />Solo ves los clientes de los que formas parte. El servidor no puede leer tus archivos ni tus contraseñas.</p>
  {/if}
</div>

{#if copiar}
  {#key copiar}
    <OrdenDialog
      cliente={copiar.cliente}
      equipo={copiar.x.equipo}
      tipo="copiar_ahora"
      cuerpo={{ repo: copiar.x.repo, copia: copiar.x.copia }}
      descripcion="Se hará ahora la copia «{copiar.x.nombre}», sin esperar a su hora. No borra nada."
      accion="Copiar ahora"
      alEnviar={() => setTimeout(() => void cargarTodos(), 2_000)}
      alTerminar={() => void cargarTodos()}
      onclose={() => (copiar = null)}
    />
  {/key}
{/if}

<style>
  /* Un panel de todo: algo más ancho que las demás vistas (el mapa tiene una columna más). */
  .ancha {
    max-width: 1400px;
  }
  .resumen {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    padding: var(--sp-6);
    border-radius: var(--radius-xl);
  }
  .cab {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-4);
  }
  .icono {
    display: grid;
    flex: none;
    place-items: center;
    width: 52px;
    height: 52px;
    color: var(--tone);
    background: color-mix(in srgb, var(--tone) var(--soft), transparent);
    border-radius: var(--radius-lg);
  }
  .antetitulo {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    margin: 0 0 2px !important;
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    color: var(--text-3) !important;
  }
  h1 {
    font-size: var(--fs-display);
    line-height: var(--lh-display);
    font-weight: 650;
    letter-spacing: -0.022em;
  }
  .cab p {
    margin: 6px 0 0;
    color: var(--text-2);
  }
  .urgentes {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .urgentes li {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
    padding: 10px 0;
    border-top: 1px solid var(--border);
  }
  .urgentes .texto {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .quien {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    color: var(--text-3);
  }
  .quien a {
    color: var(--text-2);
    font-weight: 500;
  }
  .urgentes strong {
    font-weight: 500;
  }
  .urgentes .det {
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
    color: var(--text-2);
  }
  .u-ic {
    display: grid;
    flex: none;
    margin-top: 2px;
    color: var(--tone);
  }
  .mas {
    align-self: flex-start;
    margin-top: calc(-1 * var(--sp-3));
  }
  .rejilla.clientes {
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 290px), 1fr));
  }
  .barra {
    display: flex;
    gap: 2px;
    height: 4px;
    margin: 6px 0 4px;
    overflow: hidden;
    border-radius: 999px;
  }
  .barra > span {
    background: var(--tone);
  }
  .nota {
    margin: 0;
    font-size: var(--fs-xs);
  }
  @media (max-width: 640px) {
    .resumen {
      padding: var(--sp-5);
    }
    .icono {
      width: 40px;
      height: 40px;
    }
    .icono :global(svg) {
      width: 22px;
      height: 22px;
    }
    .urgentes li {
      flex-wrap: wrap;
    }
    .urgentes .texto {
      flex-basis: calc(100% - 28px);
    }
    .urgentes .btn {
      margin-left: 28px;
    }
  }
</style>
