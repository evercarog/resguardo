<script lang="ts">
  // Servidor del cliente (api-servidor.md §11): moverlo a otro servidor (y
  // seguir cada equipo), servidores de respaldo, exportar o importar el
  // historial y dar fichas de este servidor. Al propietario del servidor,
  // también la copia de la consola (v1.23) y las notificaciones (v1.29).
  import { onDestroy, onMount, untrack } from "svelte";
  import { Archive, ArrowRightLeft, BellRing, Download, FileUp, LifeBuoy, Monitor, Ticket, Trash2 } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { enFondo } from "$lib/actividad.svelte";
  import { actual, app, cargarCliente, puede } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { guardar } from "$lib/descarga";
  import { bloqueServidor, huellaCorta } from "$lib/servidores";
  import { fechaLarga, plural } from "$lib/formato";
  import { nombreArchivo } from "$lib/exportar";
  import type { Ficha, Orden } from "$lib/tipos";
  import type { Tono } from "$lib/salud";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import BloqueCopiable from "$lib/componentes/BloqueCopiable.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import HistorialPaquete from "$lib/componentes/HistorialPaquete.svelte";
  import MoverCliente from "$lib/componentes/MoverCliente.svelte";
  import ConectarConsola from "$lib/componentes/ConectarConsola.svelte";
  import DarCodigoConexion from "$lib/componentes/DarCodigoConexion.svelte";
  import MoverAConsola from "$lib/componentes/MoverAConsola.svelte";
  import ServidoresRespaldo from "$lib/componentes/ServidoresRespaldo.svelte";
  import CopiaConsola from "$lib/componentes/CopiaConsola.svelte";
  import PublicacionesServidor from "$lib/componentes/PublicacionesServidor.svelte";
  import NotificacionesServidor from "$lib/componentes/NotificacionesServidor.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import IndicePagina from "$lib/componentes/IndicePagina.svelte";

  let ordenes = $state<Orden[] | null>(null);
  let mover = $state(false);
  let conectar = $state(false);
  let darCodigo = $state(false);
  let moverConsola = $state(false);
  let respaldo = $state(false);
  let historial = $state<"exportar" | "importar" | null>(null);
  let fichaDlg = $state(false);
  let usos = $state(10);
  let dias = $state(365);
  let ficha = $state<Ficha | null>(null);
  let pidiendo = $state(false);
  let importadas = $state<number | null>(null);

  const rol = $derived(actual.cliente?.rol);
  const nombreEquipo = (id?: string) => actual.equipos.find((e) => e.id === id)?.nombre ?? "Un equipo";

  async function cargar() {
    try {
      const r = await api.ordenesCliente(actual.id, { limite: 200 });
      ordenes = r.ordenes.filter((o) => ["cambiar_servidor", "servidores_respaldo", "anadir_consola", "quitar_consola"].includes(o.tipo));
    } catch {
      ordenes = [];
    }
  }
  /** La última de cada tipo por equipo. */
  const ultima = (tipo: string) => {
    const m = new Map<string, Orden>();
    for (const o of ordenes ?? []) if (o.tipo === tipo && o.equipo && !m.has(o.equipo)) m.set(o.equipo, o);
    return [...m.values()];
  };
  const traslados = $derived(ultima("cambiar_servidor"));
  /** Lo que dice cada equipo en su resumen (v1.6): manda sobre las órdenes. */
  const activos = $derived(actual.equipos.filter((e) => e.confirmado));
  const enTraslado = $derived(activos.filter((e) => e.resumen?.traslado));
  const conRespaldo = $derived(activos.filter((e) => e.modo !== "trasladado" && e.resumen?.servidores_respaldo !== undefined));
  const hostDe = (url: string) => {
    try {
      return new URL(url).host;
    } catch {
      return url;
    }
  };
  const respaldos = $derived(ultima("servidores_respaldo"));
  /** v1.36: las otras consolas de cada equipo (lo dice su resumen) y las conexiones aún en curso. */
  const conOtras = $derived(activos.filter((e) => (e.resumen?.consolas ?? []).some((x) => !x.esta)));
  const conexiones = $derived(ultima("anadir_consola").filter((o) => o.estado !== "hecha" && o.estado !== "cancelada"));
  const enCurso = $derived(
    traslados.some((o) => ["pendiente", "entregada", "en_marcha"].includes(o.estado)) ||
      conexiones.some((o) => ["pendiente", "entregada", "en_marcha"].includes(o.estado)) ||
      actual.equipos.some((e) => e.resumen?.traslado),
  );

  const ESTADO_TRASLADO: Record<string, { texto: string; tono: Tono }> = {
    pendiente: { texto: "Esperando al equipo", tono: "neutral" },
    entregada: { texto: "Recibida", tono: "info" },
    en_marcha: { texto: "En marcha", tono: "info" },
    hecha: { texto: "Trasladado", tono: "ok" },
    fallida: { texto: "Falló", tono: "bad" },
    rechazada: { texto: "Rechazada", tono: "bad" },
    cancelada: { texto: "Cancelada", tono: "neutral" },
    caducada: { texto: "Caducada", tono: "warn" },
  };

  let t: ReturnType<typeof setInterval> | undefined;
  // Al entrar (y al cambiar de cliente): sus traslados y si ya importó un historial.
  const clienteListo = $derived(actual.cliente ? actual.id : "");
  $effect(() => {
    if (!clienteListo) return;
    untrack(() => void cargar());
    void api
      .auditoriaImportada(actual.id, 0, 1)
      .then((x) => (importadas = x.length))
      .catch(() => (importadas = 0));
  });
  onMount(() => {
    t = setInterval(() => {
      if (enCurso) {
        void enFondo(cargar);
        void cargarCliente(actual.id, { silencioso: true });
      }
    }, 4000);
  });
  onDestroy(() => clearInterval(t));

  async function darFicha(e: SubmitEvent) {
    e.preventDefault();
    pidiendo = true;
    try {
      ficha = await api.nuevaFicha(actual.id, { usos, dias });
    } catch (err) {
      fallo(err);
    } finally {
      pidiendo = false;
    }
  }

  async function bajarGuardada() {
    try {
      const p = await api.bajarPaquete(actual.id);
      if (!p) return avisar("No hay ninguna copia guardada en este servidor.");
      await guardar(new Blob([new Uint8Array(p)]), nombreArchivo(actual.cliente!));
    } catch (err) {
      fallo(err);
    }
  }
  async function borrarGuardada() {
    try {
      await api.borrarPaquete(actual.id);
      avisar("Copia guardada borrada de este servidor.");
    } catch (err) {
      fallo(err);
    }
  }
</script>

<svelte:head><title>Servidor · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <CabeceraPagina
    titulo="Servidor"
    icono={ArrowRightLeft}
    migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${actual.id}` }, { texto: "Servidor" }]}
    resumen="Dónde viven los equipos de {actual.cliente?.nombre ?? 'este cliente'}: moverlos a otro servidor, tener servidores de respaldo y llevarse el historial."
  />
  {#if actual.cargado && actual.cliente && puede.administrar(rol)}
    <IndicePagina
      items={[
        { id: "sec-consolas", texto: "Otras consolas" },
        { id: "sec-mover", texto: "Mover a otro servidor" },
        { id: "sec-respaldo", texto: "Servidores de respaldo" },
        { id: "sec-historial", texto: "Historial" },
        ...(puede.propietario(rol) ? [{ id: "sec-ficha", texto: "Ficha" }] : []),
        ...(app.cuenta?.superusuario ? [{ id: "notificaciones", texto: "Notificaciones" }] : puede.propietario(rol) ? [{ id: "sec-notif", texto: "Notificaciones" }] : []),
        ...(app.cuenta?.superusuario ? [{ id: "copia-consola", texto: "Copia de la consola" }] : []),
        ...(app.cuenta?.superusuario ? [{ id: "publicaciones", texto: "Actualizaciones de los agentes" }] : []),
      ]}
    />
  {/if}

  {#if !actual.cargado || !actual.cliente}
    <Cargando />
  {:else if !puede.administrar(rol)}
    <div class="notice notice-info"><p>Solo las personas administradoras o propietarias pueden ver y cambiar esto.</p></div>
  {:else}
    <section class="card p" id="sec-consolas">
      <div class="cab">
        <span class="card-icon"><Monitor size={18} /></span>
        <div class="cab-texto">
          <h2>Gestionarlo también desde otra consola</h2>
          <p class="faint">Los equipos siguen aquí y, además, se gestionan desde otra consola: cualquier Resguardo Server que alcancen (la de otra oficina por VPN, una en línea, otra de la misma red). Cada una funciona sola: si quitas una, la otra sigue igual. Para irse del todo a otra, «Mover a otra consola…».</p>
        </div>
        <div class="botones">
          <button class="btn btn-primary" onclick={() => (conectar = true)} disabled={!actual.equipos.some((e) => e.confirmado && e.modo === "gestionado")}>Conectar también a otra consola…</button>
          <button class="btn btn-sm btn-ghost" onclick={() => (moverConsola = true)} disabled={!actual.equipos.some((e) => e.confirmado && e.modo === "gestionado")}>Mover a otra consola…</button>
          {#if puede.propietario(rol)}<button class="btn btn-sm btn-ghost" onclick={() => (darCodigo = true)}>Dar un código de conexión…</button>{/if}
        </div>
      </div>
      {#if conOtras.length || conexiones.length}
        <div class="lista-equipos" aria-live="polite">
          {#each conOtras as e (e.id)}
            {@const otras = (e.resumen?.consolas ?? []).filter((x) => !x.esta)}
            <div class="fila-eq">
              <a class="nombre" href="/c/{actual.id}/equipos/{e.id}">{e.nombre}</a>
              <span class="faint pequeno">también: {otras.map((x) => `${x.nombre}${x.nombre === hostDe(x.url) ? "" : ` (${hostDe(x.url)})`}`).join(", ")}</span>
              <Chip pequeno tono="ok" texto={plural(otras.length + 1, "consola", "consolas")} />
            </div>
          {/each}
          {#each conexiones as o (o.id)}
            {@const est = ESTADO_TRASLADO[o.estado]}
            <div class="fila-eq">
              <span class="nombre">{nombreEquipo(o.equipo)}</span>
              <span class="faint pequeno">conectar a otra consola · pedido <Tiempo iso={o.emitida} />{o.mensaje ? ` · ${o.mensaje}` : ""}</span>
              <Chip pequeno tono={est.tono} texto={o.estado === "en_marcha" ? "Conectando" : est.texto} />
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <section class="card p" id="sec-mover">
      <div class="cab">
        <span class="card-icon"><ArrowRightLeft size={18} /></span>
        <div class="cab-texto">
          <h2>Mover este cliente a otro servidor</h2>
          <p class="faint">Los equipos se van solos al nuevo con sus llaves, sus copias y su configuración. La clave de administración sigue siendo la misma.</p>
        </div>
        <button class="btn btn-primary" onclick={() => (mover = true)} disabled={!actual.equipos.some((e) => e.confirmado && e.modo !== "trasladado")}>Mover…</button>
      </div>
      {#if enTraslado.length}
        <div class="lista-equipos" aria-live="polite">
          {#each enTraslado as e (e.id)}
            {@const tr = e.resumen!.traslado!}
            <div class="fila-eq">
              <span class="nombre">{e.nombre}</span>
              <span class="faint pequeno">hacia {hostDe(tr.hacia)} · lo intenta hasta {fechaLarga(tr.hasta)}</span>
              <Chip pequeno tono="info" texto="En marcha" />
            </div>
          {/each}
        </div>
      {/if}
      {#if traslados.filter((o) => !enTraslado.some((e) => e.id === o.equipo)).length}
        <div class="lista-equipos" aria-live="polite">
          {#each traslados.filter((o) => !enTraslado.some((e) => e.id === o.equipo)) as o (o.id)}
            {@const est = ESTADO_TRASLADO[o.estado]}
            <div class="fila-eq">
              <span class="nombre">{nombreEquipo(o.equipo)}</span>
              <span class="faint pequeno">pedido <Tiempo iso={o.emitida} />{o.mensaje && o.estado !== "hecha" ? ` · ${o.mensaje}` : ""}</span>
              <Chip pequeno tono={est.tono} texto={est.texto} />
            </div>
          {/each}
        </div>
        {#if enCurso}<p class="faint pequeno">Cada equipo lo vuelve a intentar cada poco durante 24 h. Puedes cerrar esta página: el estado se queda aquí y en «Órdenes».</p>{/if}
      {/if}
    </section>

    <section class="card p" id="sec-respaldo">
      <div class="cab">
        <span class="card-icon"><LifeBuoy size={18} /></span>
        <div class="cab-texto">
          <h2>Servidores de respaldo <Ayuda id="respaldo" /></h2>
          <p class="faint">Hasta tres servidores a los que los equipos se van solos si este deja de responder unos días.</p>
        </div>
        <button class="btn" onclick={() => (respaldo = true)} disabled={!actual.equipos.some((e) => e.confirmado && e.modo !== "trasladado")}>Configurar…</button>
      </div>
      {#if conRespaldo.length}
        <div class="lista-equipos">
          {#each conRespaldo as e (e.id)}
            {@const lista = e.resumen?.servidores_respaldo ?? []}
            <div class="fila-eq">
              <span class="nombre">{e.nombre}</span>
              <span class="faint pequeno">
                {#if lista.length}
                  {lista.map((x) => `${hostDe(x.url)} (${x.identidad_corta})`).join(", ")} · si este no responde en {plural(e.resumen?.respaldo_dias ?? 3, "día", "días")}
                {:else}Sin servidores de respaldo{/if}
              </span>
              <Chip pequeno tono={lista.length ? "ok" : "neutral"} texto={lista.length ? plural(lista.length, "respaldo", "respaldos") : "Ninguno"} />
            </div>
          {/each}
        </div>
      {:else if respaldos.length}
        <div class="lista-equipos">
          {#each respaldos as o (o.id)}
            <div class="fila-eq">
              <span class="nombre">{nombreEquipo(o.equipo)}</span>
              <span class="faint pequeno">última lista mandada <Tiempo iso={o.emitida} /></span>
              <Chip pequeno tono={o.estado === "hecha" ? "ok" : o.estado === "fallida" || o.estado === "rechazada" ? "bad" : "neutral"} texto={o.estado === "hecha" ? "Aplicada" : ESTADO_TRASLADO[o.estado].texto} />
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <section class="card p" id="sec-historial">
      <div class="cab">
        <span class="card-icon"><Archive size={18} /></span>
        <div class="cab-texto">
          <h2>Historial <Ayuda id="paquete" /></h2>
          <p class="faint">Para llevarte la actividad, los informes y los avisos al servidor nuevo. Se cifra y se descifra en el navegador.</p>
        </div>
      </div>
      <div class="acciones">
        <button class="btn btn-sm" onclick={() => (historial = "exportar")}><Download size={14} />Exportar el historial</button>
        {#if puede.propietario(rol)}
          <button class="btn btn-sm" onclick={() => (historial = "importar")} disabled={!!importadas}><FileUp size={14} />Importar el de otro servidor</button>
        {/if}
        <button class="btn btn-sm btn-ghost" onclick={bajarGuardada}>Descargar la copia guardada</button>
        <button class="btn btn-sm btn-ghost" onclick={borrarGuardada}><Trash2 size={14} />Borrar la copia guardada</button>
      </div>
      {#if importadas}<p class="faint pequeno">Este cliente ya tiene el historial de su servidor anterior: está en <a href="/c/{actual.id}/auditoria?importada=1">Actividad</a>.</p>{/if}
    </section>

    {#if puede.propietario(rol)}
      <section class="card p" id="sec-ficha">
        <div class="cab">
          <span class="card-icon"><Ticket size={18} /></span>
          <div class="cab-texto">
            <h2>Dar una ficha de este servidor <Ayuda id="ficha" /></h2>
            <p class="faint">Para que lleguen más equipos de este cliente desde otro servidor, o para usar este como respaldo del otro (hasta 365 días).</p>
          </div>
          <button class="btn" onclick={() => ((ficha = null), (fichaDlg = true))}>Dar una ficha…</button>
        </div>
      </section>
    {/if}
  {/if}
  {#if app.cuenta?.superusuario}
    <NotificacionesServidor />
  {:else if actual.cliente && puede.propietario(rol)}
    <section class="card p" id="sec-notif">
      <div class="cab">
        <span class="card-icon"><BellRing size={18} /></span>
        <div class="cab-texto">
          <h2>Notificaciones <Ayuda id="notificaciones" /></h2>
          <p class="faint">El correo y los canales del servidor los pone su propietario. Los de {actual.cliente.nombre}, y qué recibe cada persona, en «Personas y ajustes».</p>
        </div>
        <a class="btn" href="/c/{actual.id}/ajustes#sec-notificaciones">Abrir</a>
      </div>
    </section>
  {/if}
  {#if app.cuenta?.superusuario}<CopiaConsola />{/if}
  {#if app.cuenta?.superusuario}<PublicacionesServidor />{/if}
</div>

{#if mover && actual.cliente}
  <MoverCliente
    cliente={actual.cliente}
    equipos={actual.equipos}
    onclose={() => (mover = false)}
    alEnviar={(n) => {
      avisar(`Orden mandada a ${plural(n, "equipo", "equipos")}. Aquí verás cómo va cada uno.`);
      void cargar();
    }}
  />
{/if}
{#if conectar && actual.cliente}
  <ConectarConsola
    cliente={actual.cliente}
    equipos={actual.equipos}
    onclose={() => {
      conectar = false;
      void cargar();
      void cargarCliente(actual.id, { silencioso: true });
    }}
    alTerminar={() => void cargarCliente(actual.id, { silencioso: true })}
  />
{/if}
{#if moverConsola && actual.cliente}
  <MoverAConsola
    cliente={actual.cliente}
    equipos={actual.equipos}
    onclose={() => {
      moverConsola = false;
      void cargar();
      void cargarCliente(actual.id, { silencioso: true });
    }}
    alTerminar={() => void cargarCliente(actual.id, { silencioso: true })}
  />
{/if}
{#if darCodigo && actual.cliente}
  <DarCodigoConexion cliente={actual.cliente} onclose={() => (darCodigo = false)} />
{/if}
{#if respaldo && actual.cliente}
  <ServidoresRespaldo
    cliente={actual.cliente}
    equipos={actual.equipos}
    onclose={() => (respaldo = false)}
    alEnviar={(txt) => {
      avisar(txt);
      void cargar();
    }}
  />
{/if}
{#if historial && actual.cliente}
  <HistorialPaquete
    cliente={actual.cliente}
    modo={historial}
    onclose={() => (historial = null)}
    alTerminar={(txt) => {
      avisar(txt);
      if (historial === "importar") importadas = 1;
    }}
  />
{/if}
{#if fichaDlg}
  <Modal labelledby="t-ficha" onclose={() => ((fichaDlg = false), (ficha = null))} width={540} dismissible={!ficha}>
    {#if !ficha}
      <form class="form" onsubmit={darFicha}>
        <div class="dlg-title">
          <span class="ticon"><Ticket size={18} /></span>
          <div>
            <h2 id="t-ficha">Dar una ficha de este servidor</h2>
            <p>Con ella, los equipos de {actual.cliente?.nombre} pueden darse de alta aquí desde otro servidor.</p>
          </div>
        </div>
        <div class="dos">
          <div class="field">
            <label class="field-label" for="usos-f">Equipos</label>
            <input id="usos-f" class="input" type="number" min="1" max="1000" bind:value={usos} />
          </div>
          <div class="field">
            <label class="field-label" for="dias-f">Vale durante</label>
            <select id="dias-f" class="input" bind:value={dias}>
              {#each [7, 30, 90, 180, 365] as d (d)}<option value={d}>{d} días{d === 365 ? " (para respaldo)" : ""}</option>{/each}
            </select>
          </div>
        </div>
        <footer>
          <button type="button" class="btn btn-ghost" onclick={() => (fichaDlg = false)}>Cancelar</button>
          <button class="btn btn-primary" disabled={pidiendo}>{pidiendo ? "Pidiendo…" : "Dar la ficha"}</button>
        </footer>
      </form>
    {:else}
      <div class="form">
        <div class="dlg-title">
          <span class="ticon"><Ticket size={18} /></span>
          <div>
            <h2 id="t-ficha">Copia este bloque</h2>
            <p>Pégalo en el otro servidor («Mover este cliente» o «Servidores de respaldo»). Solo se muestra ahora.</p>
          </div>
        </div>
        <BloqueCopiable texto={bloqueServidor(ficha)} etiqueta="Copiar el bloque" />
        <p class="faint pequeno">Identidad de este servidor: <code>{huellaCorta(ficha.servidor.identidad)}</code> · vale para {plural(ficha.usos, "equipo", "equipos")} hasta el {fechaLarga(ficha.caduca)}.</p>
        <footer><button class="btn btn-primary" onclick={() => ((fichaDlg = false), (ficha = null))}>Hecho</button></footer>
      </div>
    {/if}
  </Modal>
{/if}

<style>
  .cab {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
  }
  .cab-texto {
    flex: 1;
    min-width: 0;
  }
  .cab h2 {
    display: flex;
    align-items: center;
    margin: 0;
    font-size: var(--fs-md, 15px);
  }
  .cab p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .botones {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
  }
  .lista-equipos {
    display: flex;
    flex-direction: column;
    margin-top: var(--sp-4);
    border-top: 1px solid var(--border);
  }
  .fila-eq {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
  }
  .fila-eq .nombre {
    font-weight: 600;
  }
  .fila-eq .faint {
    flex: 1;
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: var(--sp-4);
  }
  .pequeno {
    margin: var(--sp-3) 0 0;
    font-size: var(--fs-xs);
  }
  .fila-eq .pequeno {
    margin: 0;
  }
  .dos {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--sp-3);
  }
  @media (max-width: 640px) {
    .cab {
      flex-wrap: wrap;
    }
    .cab-texto {
      flex-basis: calc(100% - 52px);
    }
    .cab > :global(.btn) {
      margin-left: 52px;
    }
    .fila-eq {
      flex-wrap: wrap;
    }
  }
</style>
