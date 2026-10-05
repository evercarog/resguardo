<script lang="ts">
  // Avisos del cliente: qué pasó, en qué equipo y qué hacer. Agrupados por
  // gravedad (lo urgente arriba) o por equipo; cada uno con su acción directa
  // y «Marcar como visto» (queda anotado quién lo vio).
  import { slide } from "svelte/transition";
  import { BellOff, BellRing, Check, CheckCheck, CircleAlert, Clock, KeyRound, Monitor, Power, ShieldAlert, TriangleAlert, WifiOff } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { dur } from "$ui/movimiento";
  import { actual, cargarCliente, puede } from "$lib/estado.svelte";
  import { onMount, untrack } from "svelte";
  import { enFondo } from "$lib/actividad.svelte";
  import { seguirCambios } from "$lib/vivo.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { plural } from "$lib/formato";
  import type { Aviso, TipoAviso } from "$lib/tipos";
  import type { Tono } from "$lib/salud";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import FiltroEtiquetas from "$lib/componentes/FiltroEtiquetas.svelte";
  import { filtroEtiqueta, pasaFiltro } from "$lib/etiquetas.svelte";
  // «Pulsar para ver más»: cada aviso lleva a la vuelta o al repositorio que lo explica.
  import { cargarInformes, ultimos } from "$lib/informes.svelte";
  import { destinoAviso } from "$lib/detalle";
  import { tip } from "$lib/tooltip";
  import "$lib/componentes/detalle/pulsable.css";

  let solo = $state(true);
  let agrupar = $state<"gravedad" | "equipo">("gravedad");
  let avisos = $state<Aviso[] | null>(null);
  let marcando = $state<Set<string>>(new Set());
  let todos = $state(false);

  async function cargar() {
    if (!actual.id) return;
    try {
      avisos = await api.avisos(actual.id, solo);
    } catch (e) {
      fallo(e);
    }
  }
  $effect(() => {
    void solo;
    void actual.id;
    untrack(() => void cargar());
  });
  // Los avisos nuevos (o vistos desde otra consola) aparecen solos.
  onMount(() => seguirCambios(() => enFondo(cargar), { ms: 0, toca: (x) => x.t === "avisos" }));
  $effect(() => {
    // Las cargas, sin seguir lo que leen (si no, cada respuesta podría volver a lanzar el efecto).
    const [cc, ids] = [actual.id, actual.equipos.map((e) => e.id)];
    untrack(() => void cargarInformes(cc, ids));
  });
  /** La vuelta (o el repositorio) que explica un aviso, en el último informe de su equipo. */
  function relacionado(a: Aviso): { href: string; texto: string } | null {
    if (!a.equipo) return null;
    const d = destinoAviso(a, ultimos.porEquipo[a.equipo]?.datos?.repos ?? []);
    if (!d) return null;
    const base = `/c/${actual.id}/equipos/${a.equipo}/repositorios/${encodeURIComponent(d.repo)}`;
    return d.vuelta ? { href: `${base}?${new URLSearchParams({ vuelta: d.vuelta })}`, texto: "Ver esa copia" } : { href: `${base}?vista=estado`, texto: "Ver el repositorio" };
  }

  const c = $derived(actual.id);
  const INFO: Record<TipoAviso, { texto: string; tono: Tono; icono: typeof CircleAlert; que: string; accion: (equipo: string | null) => { texto: string; href: string } | null }> = {
    intentos_fallidos: { texto: "Intentos fallidos", tono: "bad", icono: KeyRound, que: "Si no fuiste tú, cambia la clave de administración.", accion: () => ({ texto: "Ver la actividad", href: `/c/${c}/auditoria` }) },
    bloqueo: { texto: "Equipo bloqueado", tono: "bad", icono: ShieldAlert, que: "Demasiados intentos: el equipo no acepta órdenes protegidas durante un tiempo.", accion: (e) => (e ? { texto: "Ver el equipo", href: `/c/${c}/equipos/${e}` } : null) },
    orden_destructiva: { texto: "Orden con espera", tono: "warn", icono: Clock, que: "Si no la esperabas, cancélala en Órdenes.", accion: () => ({ texto: "Ver las órdenes", href: `/c/${c}/ordenes` }) },
    equipo_sin_contacto: { texto: "Sin contacto", tono: "bad", icono: WifiOff, que: "Comprueba que el equipo está encendido y con red.", accion: (e) => (e ? { texto: "Ver el equipo", href: `/c/${c}/equipos/${e}` } : null) },
    copia_fallida: { texto: "Copia fallida", tono: "bad", icono: CircleAlert, que: "Abre el equipo, mira qué dijo y prueba «Copiar ahora».", accion: (e) => (e ? { texto: "Ver sus copias", href: `/c/${c}/equipos/${e}` } : null) },
    copia_atrasada: { texto: "Copia atrasada", tono: "warn", icono: TriangleAlert, que: "Puede que el equipo estuviera apagado a su hora. Si sigue, prueba «Copiar ahora».", accion: (e) => (e ? { texto: "Ver sus copias", href: `/c/${c}/equipos/${e}` } : null) },
    servicio_detenido: { texto: "Servicio detenido", tono: "bad", icono: Power, que: "Alguien paró Resguardo en el equipo. Lo ya copiado sigue a salvo.", accion: (e) => (e ? { texto: "Ver el equipo", href: `/c/${c}/equipos/${e}` } : null) },
    cambio_inusual: { texto: "Cambio inusual", tono: "warn", icono: ShieldAlert, que: "Revisa la actividad del cliente.", accion: () => ({ texto: "Ver la actividad", href: `/c/${c}/auditoria` }) },
    // v1.29: los crea el servidor a partir de lo que cuentan los equipos.
    verificacion_fallida: { texto: "Verificación fallida", tono: "bad", icono: ShieldAlert, que: "Puede haber datos dañados en el repositorio: ábrelo, mira qué dijo y verifícalo otra vez.", accion: (e) => (e ? { texto: "Ver el equipo", href: `/c/${c}/equipos/${e}` } : null) },
    externa_fallida: { texto: "Copia externa fallida", tono: "warn", icono: CircleAlert, que: "Revisa el destino de la copia externa (red, credenciales o espacio).", accion: (e) => (e ? { texto: "Ver el equipo", href: `/c/${c}/equipos/${e}` } : null) },
    prueba_fallida: { texto: "Prueba de restauración fallida", tono: "warn", icono: TriangleAlert, que: "La copia no se pudo restaurar en la prueba: mira qué dijo y pruébala otra vez.", accion: (e) => (e ? { texto: "Ver el equipo", href: `/c/${c}/equipos/${e}` } : null) },
    espejo_fallido: { texto: "Espejo fallido", tono: "warn", icono: TriangleAlert, que: "El espejo del almacén no terminó: revisa su disco o su nube.", accion: (e) => (e ? { texto: "Ver el almacén", href: `/c/${c}/equipos/${e}` } : null) },
    cambio_clave: { texto: "Clave de administración cambiada", tono: "bad", icono: KeyRound, que: "Si no fuiste tú ni alguien de confianza, revisa la actividad del cliente.", accion: () => ({ texto: "Ver la actividad", href: `/c/${c}/auditoria` }) },
  };
  const info = (a: Aviso) => INFO[a.tipo] ?? { texto: a.tipo, tono: "neutral" as Tono, icono: CircleAlert, que: "", accion: () => null };
  const equipo = (id: string | null) => actual.equipos.find((e) => e.id === id);
  // Con una etiqueta elegida, solo los avisos de sus equipos (los del cliente en general, no).
  const visibles = $derived((avisos ?? []).filter((a) => !filtroEtiqueta.valor || pasaFiltro(equipo(a.equipo), filtroEtiqueta.valor)));
  const sinVer = $derived(visibles.filter((a) => !a.visto_por));
  const urgentes = $derived(sinVer.filter((a) => info(a).tono === "bad").length);

  /** Los grupos: por gravedad (urgente, atención, ya vistos) o por equipo (y los del cliente). */
  const grupos = $derived.by(() => {
    type Grupo = { id: string; titulo: string; tono: Tono; avisos: Aviso[]; href?: string };
    const orden = (xs: Aviso[]) => [...xs].sort((a, b) => Date.parse(b.creado) - Date.parse(a.creado));
    if (agrupar === "gravedad") {
      return [
        { id: "bad", titulo: "Urgente", tono: "bad" as Tono, avisos: orden(visibles.filter((a) => !a.visto_por && info(a).tono === "bad")) },
        { id: "warn", titulo: "Para revisar", tono: "warn" as Tono, avisos: orden(visibles.filter((a) => !a.visto_por && info(a).tono !== "bad")) },
        { id: "vistos", titulo: "Ya vistos", tono: "neutral" as Tono, avisos: orden(visibles.filter((a) => !!a.visto_por)) },
      ].filter((g) => g.avisos.length) as Grupo[];
    }
    const m = new Map<string, Aviso[]>();
    for (const a of visibles) m.set(a.equipo ?? "", [...(m.get(a.equipo ?? "") ?? []), a]);
    return [...m.entries()]
      .map(([id, xs]): Grupo => ({ id: id || "cliente", titulo: id ? (equipo(id)?.nombre ?? "Equipo") : `Todo ${actual.cliente?.nombre ?? "el cliente"}`, tono: (xs.some((a) => !a.visto_por && info(a).tono === "bad") ? "bad" : xs.some((a) => !a.visto_por) ? "warn" : "neutral") as Tono, avisos: orden(xs), href: id ? `/c/${c}/equipos/${id}` : undefined }))
      .sort((a, b) => ["bad", "warn", "neutral"].indexOf(a.tono) - ["bad", "warn", "neutral"].indexOf(b.tono) || a.titulo.localeCompare(b.titulo));
  });

  async function visto(a: Aviso) {
    marcando = new Set([...marcando, a.id]);
    try {
      await api.marcarVisto(actual.id, a.id);
      avisar(`Marcado como visto: «${info(a).texto}»${equipo(a.equipo) ? ` en ${equipo(a.equipo)!.nombre}` : ""}.`);
      await Promise.all([cargar(), cargarCliente(actual.id, { silencioso: true })]);
    } catch (e) {
      fallo(e);
    } finally {
      marcando = new Set([...marcando].filter((x) => x !== a.id));
    }
  }
  /** Todos los que se ven, uno a uno (la misma llamada que el botón de cada uno). */
  async function vistosTodos() {
    const xs = sinVer;
    if (!xs.length) return;
    todos = true;
    let hechos = 0;
    try {
      for (const a of xs) {
        await api.marcarVisto(actual.id, a.id);
        hechos++;
      }
      avisar(`${plural(hechos, "aviso marcado", "avisos marcados")} como visto${hechos === 1 ? "" : "s"}.`);
    } catch (e) {
      fallo(e);
    } finally {
      todos = false;
      await Promise.all([cargar(), cargarCliente(actual.id, { silencioso: true })]);
    }
  }
</script>

<svelte:head><title>Avisos · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <CabeceraPagina
    titulo="Avisos"
    icono={BellRing}
    migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${c}` }, { texto: "Avisos" }]}
    resumen={avisos === null ? "Lo que necesita que alguien lo mire." : sinVer.length ? `${plural(sinVer.length, "aviso sin revisar", "avisos sin revisar")}${urgentes ? ` · ${plural(urgentes, "urgente", "urgentes")}` : ""}. Al marcarlo como visto, queda anotado quién lo vio.` : "Nada sin revisar. Al marcar un aviso como visto, queda anotado quién lo vio."}
  >
    {#snippet acciones()}
      {#if sinVer.length > 1 && puede.ordenar(actual.cliente?.rol)}
        <button class="btn" disabled={todos} onclick={vistosTodos}><CheckCheck size={16} />{todos ? "Marcando…" : "Marcar todos como vistos"}</button>
      {/if}
    {/snippet}
  </CabeceraPagina>

  <div class="barra-filtros">
    <div class="segmented inline" role="group" aria-label="Qué avisos">
      <button class:on={solo} aria-pressed={solo} onclick={() => (solo = true)}>Sin revisar</button>
      <button class:on={!solo} aria-pressed={!solo} onclick={() => (solo = false)}>Todos</button>
    </div>
    <div class="segmented inline" role="group" aria-label="Agrupar">
      <button class:on={agrupar === "gravedad"} aria-pressed={agrupar === "gravedad"} onclick={() => (agrupar = "gravedad")}>Por gravedad</button>
      <button class:on={agrupar === "equipo"} aria-pressed={agrupar === "equipo"} onclick={() => (agrupar = "equipo")}>Por equipo</button>
    </div>
  </div>

  <FiltroEtiquetas />

  {#if avisos === null}
    <Cargando />
  {:else if !visibles.length}
    <div class="card">
      <Vacio icono={BellOff} ilustracion="todo-en-orden" titulo={solo ? "Todo tranquilo" : "Sin avisos"} texto={solo ? "No hay nada sin revisar. Si algo falla, lo verás aquí, en el Estado y con un punto en la pestaña del navegador." : "Este cliente aún no ha tenido avisos."}>
        {#if solo}<button class="btn btn-sm" onclick={() => (solo = false)}>Ver los ya vistos</button>{/if}
      </Vacio>
    </div>
  {:else}
    {#each grupos as g (g.id)}
      <section aria-labelledby="g-{g.id}">
        <div class="section-head">
          <h2 id="g-{g.id}">
            <span class="dot" style="--tone: var(--{g.tono})" aria-hidden="true"></span>
            {#if g.href}<a class="g-enlace" href={g.href}>{g.titulo}</a>{:else}{g.titulo}{/if}
            <span class="count">· {g.avisos.length}</span>
          </h2>
        </div>
        <div class="card p-0 lista">
          {#each g.avisos as a (a.id)}
            {@const i = info(a)}
            {@const eq = equipo(a.equipo)}
            {@const acc = i.accion(a.equipo)}
            {@const rel = relacionado(a)}
            <div class="fila aviso" class:visto={!!a.visto_por} transition:slide={{ duration: dur(180) }}>
              <span class="icono tone-{a.visto_por ? 'neutral' : i.tono}"><i.icono size={16} /></span>
              <span class="fila-texto">
                <span class="fila-titulo">{i.texto}{#if eq && agrupar === "gravedad"}<a class="eq" href="/c/{c}/equipos/{eq.id}"><Monitor size={12} />{eq.nombre}</a>{/if}</span>
                {#if rel}<a class="mensaje pulsable" href={rel.href} use:tip={"Ver detalle"}>{a.mensaje}</a>{:else}<span class="mensaje">{a.mensaje}</span>{/if}
                {#if i.que && !a.visto_por}<span class="fila-sub">{i.que}</span>{/if}
                <span class="fila-sub cuando"><Tiempo iso={a.creado} />{#if a.visto_por}<span class="visto-por"><Check size={12} />Visto por {a.visto_por}</span>{/if}</span>
              </span>
              <span class="acciones">
                {#if rel}<a class="btn btn-sm btn-ghost" href={rel.href}>{rel.texto}</a>{/if}
                {#if acc && !a.visto_por}<a class="btn btn-sm" href={acc.href}>{acc.texto}</a>{/if}
                {#if !a.visto_por && puede.ordenar(actual.cliente?.rol)}
                  <button class="btn btn-sm btn-ghost" disabled={marcando.has(a.id) || todos} onclick={() => visto(a)}><Check size={14} />{marcando.has(a.id) ? "Marcando…" : "Visto"}</button>
                {/if}
              </span>
            </div>
          {/each}
        </div>
      </section>
    {/each}
  {/if}
</div>

<style>
  .barra-filtros {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: calc(-1 * var(--sp-2));
  }
  .section-head h2 {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .g-enlace {
    color: inherit;
    text-decoration: none;
  }
  .g-enlace:hover {
    text-decoration: underline;
  }
  .aviso {
    align-items: flex-start;
    flex-wrap: wrap;
    padding-top: 12px;
    padding-bottom: 12px;
  }
  .aviso.visto {
    opacity: 0.75;
  }
  .icono {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    color: var(--tone);
    background: color-mix(in srgb, var(--tone) var(--soft), transparent);
    border-radius: var(--radius);
  }
  .fila-titulo {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
  }
  .eq {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 0 8px;
    height: 20px;
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--text-2);
    text-decoration: none;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
  }
  .eq:hover {
    color: var(--text-1);
    border-color: var(--border-strong);
  }
  .mensaje {
    font-size: var(--fs-sm);
    color: var(--text-1);
    white-space: normal;
  }
  .fila-sub {
    white-space: normal;
  }
  .cuando {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    font-size: var(--fs-xs);
  }
  .visto-por {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .aviso .fila-texto {
    flex: 1 1 260px;
    min-width: 0;
  }
  .aviso .fila-titulo {
    white-space: normal;
    overflow-wrap: anywhere;
  }
  .acciones {
    display: flex;
    flex: none;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  @media (max-width: 640px) {
    /* En móvil: icono y texto arriba; las acciones debajo, alineadas con el texto. */
    .aviso .fila-texto {
      flex-basis: calc(100% - 48px);
    }
    .acciones {
      margin-left: 44px;
    }
  }
</style>
