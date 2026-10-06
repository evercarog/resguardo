<script lang="ts">
  import { untrack } from "svelte";
  // Personas y ajustes del cliente (solo propietarios): nombre, papeles,
  // invitaciones por enlace, la espera de lo destructivo y (v1.29) qué avisos
  // recibe cada persona y los canales de notificación propios del cliente.
  import { BellRing, Check, Clock, Copy, KeyRound, Link2, LoaderCircle, Pencil, ShieldOff, Trash2, TriangleAlert, UserPlus, X } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { actual, app, cargarClientes, cargarCliente, NOMBRE_ROL, puede } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { fechaLarga, plural } from "$lib/formato";
  import { mandarOrden } from "$lib/ordenar";
  import type { CodigoRestablecimiento, Invitacion, Miembro, PersonaNotif, PrefEtiqueta, Rol, Severidad } from "$lib/tipos";
  import { etiquetasDe } from "$lib/etiquetasGrupos";
  import AvisosPorEtiqueta from "$lib/componentes/AvisosPorEtiqueta.svelte";
  import { SEVERIDAD, SEVERIDADES, textoSeveridades } from "$lib/notificaciones";
  import NotificacionesCliente from "$lib/componentes/NotificacionesCliente.svelte";
  import BotonCargando from "$lib/componentes/BotonCargando.svelte";
  import { ApiError } from "$lib/api";
  import CampoCodigo from "$lib/componentes/CampoCodigo.svelte";
  import MenuAcciones, { type AccionMenu } from "$lib/componentes/MenuAcciones.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import CampoClave from "$lib/componentes/CampoClave.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import { Palette, Users } from "@lucide/svelte";
  import MarcaCliente from "$lib/componentes/MarcaCliente.svelte";
  import EditorMarca from "$lib/componentes/EditorMarca.svelte";
  import CambiarClaveAdmin from "$lib/componentes/CambiarClaveAdmin.svelte";
  import { equiposDelCambio, pendientesDeCambio } from "$lib/cambioClave";

  let miembros = $state<Miembro[] | null>(null);
  let rolInvitacion = $state<Rol>("tecnico");
  let invitacion = $state<Invitacion | null>(null);
  let editarMarca = $state(false);
  let renombrar = $state(false);
  let nombre = $state("");
  let quitar = $state<Miembro | null>(null);
  // v1.27: restablecer la verificación en dos pasos de alguien que perdió el móvil.
  let restablecer = $state<Miembro | null>(null);
  let codigoYo = $state("");
  let errorRest = $state("");
  let ocupadoRest = $state(false);
  let codigoRest = $state<CodigoRestablecimiento | null>(null);
  let espera = $state(false);
  let horas = $state(24);
  let claveAdmin = $state("");
  let progreso = $state("");
  let error = $state("");

  // ---- La clave de administración: cambiarla y si hay un cambio a medias ----
  let cambiarClave = $state(false);
  let pendientesClave = $state<{ equipo: string; caduca: string }[]>([]);
  async function cargarCambioClave() {
    try {
      // Las últimas órdenes del cliente (el cambio caduca a los 7 días): la última de cada equipo.
      const ordenes = [];
      let antes: string | null = null;
      for (let i = 0; i < 3; i++) {
        const p = await api.ordenesCliente(actual.id, { limite: 200, antes });
        ordenes.push(...p.ordenes);
        if (!p.siguiente) break;
        antes = p.siguiente;
      }
      pendientesClave = pendientesDeCambio(ordenes);
    } catch {
      pendientesClave = [];
    }
  }
  const nombreEquipo = (id: string) => actual.equipos.find((e) => e.id === id)?.nombre ?? "un equipo";

  async function cargar() {
    void cargarCambioClave();
    try {
      miembros = await api.miembros(actual.id);
    } catch (e) {
      fallo(e);
      miembros = [];
    }
    // v1.29: qué recibe cada persona (null con un servidor anterior: no se enseña).
    notif = await api.personasNotif(actual.id).catch(() => null);
  }

  // ---- Qué avisos recibe cada persona (por correo) ----
  let notif = $state<PersonaNotif[] | null>(null);
  const notifDe = (cuenta: string) => notif?.find((p) => p.cuenta === cuenta);
  let editarAvisos = $state<PersonaNotif | null>(null);
  let inmediatos = $state<Severidad[]>([]);
  let conResumen = $state(false);
  /** v1.52: lo que recibe de los equipos con ciertas etiquetas. */
  let avisosEtiqueta = $state<PrefEtiqueta[]>([]);
  const etiquetasCliente = $derived([...new Set([...etiquetasDe(actual.equipos).map((t) => t.nombre), ...avisosEtiqueta.map((x) => x.etiqueta)])]);
  let guardandoAvisos = $state(false);
  function abrirAvisos(p: PersonaNotif) {
    editarAvisos = p;
    inmediatos = [...p.preferencias.inmediatos];
    conResumen = p.preferencias.resumen;
    avisosEtiqueta = [...(p.preferencias.etiquetas ?? [])];
  }
  async function guardarAvisos(e: SubmitEvent) {
    e.preventDefault();
    if (!editarAvisos) return;
    guardandoAvisos = true;
    try {
      const pr = await api.ponerPrefsNotif(actual.id, editarAvisos.cuenta, { inmediatos, resumen: conResumen, etiquetas: avisosEtiqueta });
      notif = (notif ?? []).map((p) => (p.cuenta === editarAvisos!.cuenta ? { ...p, preferencias: pr } : p));
      avisar(`Avisos de ${editarAvisos.nombre} guardados.`);
      editarAvisos = null;
    } catch (err) {
      fallo(err);
    } finally {
      guardandoAvisos = false;
    }
  }
  // Al entrar directamente, el cliente se carga después: se pide cuando ya se sabe cuál es.
  $effect(() => {
    if (actual.cliente?.id && puede.propietario(actual.cliente.rol)) untrack(() => void cargar());
  });

  const ROLES: Rol[] = ["propietario", "administrador", "tecnico", "lectura"];
  const DESC: Record<Rol, string> = {
    propietario: "Todo, también las personas y los ajustes.",
    administrador: "Todo menos personas y ajustes.",
    tecnico: "Ver y mandar órdenes, salvo dar de baja, desvincular, cambiar de servidor o la clave.",
    lectura: "Solo ver.",
  };

  async function cambiarRol(m: Miembro, rol: Rol) {
    try {
      await api.cambiarRol(actual.id, m.cuenta, rol);
      avisar(`${m.nombre} ahora es ${NOMBRE_ROL[rol].toLowerCase()}.`);
      await cargar();
    } catch (e) {
      fallo(e);
      await cargar();
    }
  }

  async function confirmarQuitar() {
    if (!quitar) return;
    try {
      await api.quitarMiembro(actual.id, quitar.cuenta);
      avisar(`${quitar.nombre} ya no tiene acceso a ${actual.cliente?.nombre}.`);
      quitar = null;
      await cargar();
    } catch (e) {
      fallo(e);
    }
  }

  /** El propietario del servidor, con cualquiera; el del cliente, con quien no es propietario (el servidor lo comprueba de verdad). */
  const puedeRestablecer = (m: Miembro) => m.cuenta !== app.cuenta?.id && (app.cuenta?.superusuario || m.rol !== "propietario");

  function menuDe(m: Miembro): AccionMenu[][] {
    return [
      puedeRestablecer(m) ? [{ texto: "Restablecer verificación en dos pasos", icono: ShieldOff, onclick: () => abrirRestablecer(m) }] : [],
      [{ texto: "Quitar del cliente", icono: Trash2, peligro: true, onclick: () => (quitar = m) }],
    ];
  }

  function abrirRestablecer(m: Miembro) {
    restablecer = m;
    codigoYo = "";
    errorRest = "";
    codigoRest = null;
  }

  async function confirmarRestablecer(codigo = codigoYo.replace(/\D/g, "")) {
    if (!restablecer || ocupadoRest || codigo.length !== 6) return;
    errorRest = "";
    ocupadoRest = true;
    try {
      codigoRest = await api.restablecerTotp(actual.id, restablecer.cuenta, codigo);
      avisar(`Verificación en dos pasos de ${restablecer.nombre} restablecida.`);
    } catch (e) {
      errorRest = e instanceof ApiError && e.estado === 404 ? "Esa persona ya no está en el cliente (o el servidor es anterior y no lo admite: actualízalo)." : (e as Error).message;
      codigoYo = "";
    } finally {
      ocupadoRest = false;
    }
  }
  const mensajeRest = $derived(
    codigoRest && restablecer ? `Entra en ${location.origin}/entrar con tu correo (${restablecer.correo}) y tu contraseña, y escribe este código de un solo uso: ${codigoRest.codigo}. Caduca el ${fechaLarga(codigoRest.caduca)}.` : "",
  );

  async function invitar() {
    try {
      invitacion = await api.invitar(actual.id, rolInvitacion);
    } catch (e) {
      fallo(e);
    }
  }
  const enlaceCompleto = $derived(invitacion ? `${location.origin}${invitacion.enlace}` : "");

  async function guardarNombre(e: SubmitEvent) {
    e.preventDefault();
    try {
      await api.renombrarCliente(actual.id, nombre.trim());
      renombrar = false;
      avisar("Nombre cambiado.");
      await Promise.all([cargarCliente(actual.id), cargarClientes()]);
    } catch (e) {
      fallo(e);
    }
  }

  /** La espera la guarda cada equipo: se manda «cambiar_espera» a todos, con la clave de administración. */
  async function cambiarEspera(e: SubmitEvent) {
    e.preventDefault();
    if (!actual.cliente) return;
    error = "";
    const equipos = actual.equipos.filter((x) => x.confirmado && x.modo !== "trasladado");
    let hechos = 0;
    try {
      for (const eq of equipos) {
        progreso = `Enviando a ${eq.nombre} (${hechos + 1} de ${equipos.length})…`;
        await mandarOrden({ cliente: actual.cliente, equipo: eq, tipo: "cambiar_espera", cuerpo: { horas }, secretos: { claveAdmin } });
        hechos++;
      }
      claveAdmin = "";
      espera = false;
      avisar(`Pedido a ${plural(hechos, "equipo", "equipos")}: la espera pasa a ${horas} h cuando lo apliquen.`);
    } catch (err) {
      error = `${(err as Error).message}${hechos ? ` (ya se envió a ${plural(hechos, "equipo", "equipos")})` : ""}`;
    } finally {
      progreso = "";
    }
  }
</script>

<svelte:head><title>Personas y ajustes · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <CabeceraPagina titulo="Personas y ajustes" icono={Users} migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${actual.id}` }, { texto: "Personas y ajustes" }]}>
    {#snippet detalle()}Quién entra en {actual.cliente?.nombre ?? "este cliente"} y con qué papel, y cómo se llama. <Ayuda id="rol" />{/snippet}
    {#snippet acciones()}
      {#if actual.cliente && puede.propietario(actual.cliente.rol)}
        <a class="btn btn-primary" href="#sec-invitar"><UserPlus size={16} />Invitar a alguien</a>
      {/if}
    {/snippet}
  </CabeceraPagina>

  {#if !actual.cargado || !actual.cliente}
    <Cargando />
  {:else if !puede.propietario(actual.cliente.rol)}
    <div class="notice notice-info"><p>Solo las personas propietarias pueden ver y cambiar esto.</p></div>
  {:else}
    <section class="card p">
      <div class="fila-ajuste">
        <span>
          <strong>Nombre del cliente</strong>
          {#if renombrar}
            <form class="renombrar" onsubmit={guardarNombre}>
              <!-- svelte-ignore a11y_autofocus -->
              <input class="input" bind:value={nombre} aria-label="Nombre del cliente" autofocus />
              <button class="icon-btn" aria-label="Guardar"><Check size={16} /></button>
              <button type="button" class="icon-btn" aria-label="Cancelar" onclick={() => (renombrar = false)}><X size={16} /></button>
            </form>
          {:else}
            <span class="faint">{actual.cliente.nombre}</span>
          {/if}
        </span>
        {#if !renombrar}<button class="btn btn-sm" onclick={() => ((nombre = actual.cliente!.nombre), (renombrar = true))}><Pencil size={14} />Cambiar</button>{/if}
      </div>
      <div class="fila-ajuste">
        <span>
          <strong>Espera antes de borrar <Ayuda id="espera" /></strong>
          <span class="faint">{actual.cliente.espera_min_horas} h. La aplica cada equipo; cambiarla pide la clave de administración.</span>
        </span>
        <button class="btn btn-sm" onclick={() => ((horas = actual.cliente!.espera_min_horas), (espera = true))} disabled={!actual.equipos.length}><Clock size={14} />Cambiar</button>
      </div>
      <div class="fila-ajuste">
        <span class="con-marca">
          <MarcaCliente nombre={actual.cliente.nombre} marca={actual.cliente.marca} tam={36} />
          <span>
            <strong>Marca</strong>
            <span class="faint">{actual.cliente.marca?.logo ? "Con su logo" : "Sin logo"}{actual.cliente.marca?.acento ? " y su color" : ""}. Sale en el selector de clientes, en su cabecera y en sus informes.</span>
          </span>
        </span>
        <button class="btn btn-sm" onclick={() => (editarMarca = true)}><Palette size={14} />Cambiar</button>
      </div>
      <div class="fila-ajuste">
        <span>
          <strong>Clave de administración <Ayuda id="clave-admin" /></strong>
          {#if pendientesClave.length}
            <span class="faint aviso-clave" role="status">
              <TriangleAlert size={13} aria-hidden="true" />Cambio a medias: {pendientesClave.map((p) => nombreEquipo(p.equipo)).join(", ")}
              {pendientesClave.length === 1 ? "aún no ha aplicado" : "aún no han aplicado"} la clave nueva (en {pendientesClave.length === 1 ? "él" : "ellos"} sigue valiendo la anterior hasta que se conecten; la orden caduca el {fechaLarga(pendientesClave.map((p) => p.caduca).sort()[0])}).
            </span>
          {:else}
            <span class="faint">La guarda cada equipo; el servidor nunca la ve. Cambiarla pide la actual.</span>
          {/if}
        </span>
        <button class="btn btn-sm" onclick={() => (cambiarClave = true)} disabled={!equiposDelCambio(actual.equipos).length}><KeyRound size={14} />Cambiar</button>
      </div>
    </section>

    <section>
      <div class="section-head"><h2>Personas <span class="count">· {miembros?.length ?? ""}</span></h2></div>
      {#if !miembros}
        <Cargando />
      {:else}
        <div class="card p-0 lista">
          {#each miembros as m (m.cuenta)}
            <div class="fila">
              <span class="ini" aria-hidden="true">{m.nombre.slice(0, 1).toUpperCase()}</span>
              <span class="fila-texto">
                <span class="fila-titulo">{m.nombre}{#if m.cuenta === app.cuenta?.id} <Chip pequeno tono="neutral" texto="Tú" />{/if}</span>
                <span class="fila-sub">{m.correo}</span>
              </span>
              {#if notifDe(m.cuenta)}
                {@const pn = notifDe(m.cuenta)!}
                <button class="btn btn-sm btn-ghost avisos" onclick={() => abrirAvisos(pn)} aria-label="Qué avisos recibe {m.nombre}">
                  <BellRing size={14} />{textoSeveridades(pn.preferencias.inmediatos)}{pn.preferencias.resumen ? " · resumen" : ""}{pn.preferencias.etiquetas?.length ? ` · ${pn.preferencias.etiquetas.length} por etiqueta` : ""}
                </button>
              {/if}
              <select class="input rol" value={m.rol} aria-label="Papel de {m.nombre}" onchange={(e) => cambiarRol(m, e.currentTarget.value as Rol)} disabled={m.cuenta === app.cuenta?.id}>
                {#each ROLES as r (r)}<option value={r}>{NOMBRE_ROL[r]}</option>{/each}
              </select>
              {#if m.cuenta !== app.cuenta?.id}
                <MenuAcciones etiqueta="Acciones para {m.nombre}" texto="" grupos={menuDe(m)} />
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <NotificacionesCliente cliente={actual.id} />

    <section class="card p" id="sec-invitar">
      <h2 class="section-title"><UserPlus size={16} /> Invitar a alguien</h2>
      <p class="faint">Se crea un enlace de un solo uso que caduca en 7 días. Envíalo por un canal de confianza.</p>
      <div class="invitar">
        <select class="input" bind:value={rolInvitacion} aria-label="Papel de la persona invitada">
          {#each ROLES as r (r)}<option value={r}>{NOMBRE_ROL[r]}</option>{/each}
        </select>
        <button class="btn btn-primary" onclick={invitar}><Link2 size={15} />Crear enlace</button>
      </div>
      <p class="faint pequeno">{DESC[rolInvitacion]}</p>
      {#if invitacion}
        <div class="enlace">
          <code class="selectable">{enlaceCompleto}</code>
          <button
            class="btn btn-sm"
            onclick={async () => {
              await navigator.clipboard.writeText(enlaceCompleto);
              avisar("Enlace copiado.");
            }}><Copy size={14} />Copiar</button
          >
        </div>
        <p class="faint pequeno">Caduca el {fechaLarga(invitacion.caduca)}.</p>
      {/if}
    </section>
  {/if}
</div>

{#if cambiarClave && actual.cliente}
  <CambiarClaveAdmin
    cliente={actual.cliente}
    equipos={actual.equipos}
    onclose={() => {
      cambiarClave = false;
      void cargarCambioClave();
    }}
  />
{/if}

{#if editarMarca && actual.cliente}
  <EditorMarca cliente={actual.id} nombre={actual.cliente.nombre} marca={actual.cliente.marca} onclose={() => (editarMarca = false)} />
{/if}

{#if editarAvisos}
  <Modal labelledby="t-avisos" onclose={() => (editarAvisos = null)} width={500}>
    <form class="form" onsubmit={guardarAvisos}>
      <div class="dlg-title">
        <span class="ticon"><BellRing size={18} /></span>
        <div>
          <h2 id="t-avisos">Avisos de {editarAvisos.nombre}</h2>
          <p>Lo que le llega por correo ({editarAvisos.correo}) de {actual.cliente?.nombre}. Sus horas de silencio y si quiere los resúmenes los elige en su cuenta.</p>
        </div>
      </div>
      <fieldset class="grupo-avisos">
        <legend>Al momento</legend>
        {#each SEVERIDADES as sev (sev)}
          <label class="check-avisos">
            <input type="checkbox" checked={inmediatos.includes(sev)} onchange={(e) => (inmediatos = e.currentTarget.checked ? [...inmediatos, sev] : inmediatos.filter((x) => x !== sev))} />
            <span><strong>{SEVERIDAD[sev].texto}</strong> <span class="faint">{SEVERIDAD[sev].que}</span></span>
          </label>
        {/each}
      </fieldset>
      <AvisosPorEtiqueta id="avisos-persona-et" etiquetas={etiquetasCliente} bind:valor={avisosEtiqueta} />
      <label class="switch-row">
        <input class="switch" type="checkbox" bind:checked={conResumen} />
        <span><strong>En sus resúmenes</strong><span class="faint">El «Resumen semanal de copias» (y el diario, si lo quiere) incluye este cliente.</span></span>
      </label>
      {#if !editarAvisos.preferencias.propias}<p class="faint pequeno">Ahora tiene las de su papel ({NOMBRE_ROL[editarAvisos.rol].toLowerCase()}).</p>{/if}
      <footer>
        <button type="button" class="btn btn-ghost" onclick={() => (editarAvisos = null)}>Cancelar</button>
        <BotonCargando class="btn btn-primary" type="submit" cargando={guardandoAvisos}>Guardar</BotonCargando>
      </footer>
    </form>
  </Modal>
{/if}

{#if quitar}
  <Modal labelledby="t-quitar" onclose={() => (quitar = null)} width={440}>
    <div class="dlg-title">
      <span class="ticon danger"><Trash2 size={18} /></span>
      <div>
        <h2 id="t-quitar">¿Quitar a {quitar.nombre}?</h2>
        <p>Dejará de ver {actual.cliente?.nombre} y de poder mandar órdenes. Su cuenta sigue existiendo.</p>
      </div>
    </div>
    <footer>
      <button class="btn btn-ghost" onclick={() => (quitar = null)}>Cancelar</button>
      <button class="btn btn-danger" onclick={confirmarQuitar}>Quitar</button>
    </footer>
  </Modal>
{/if}

{#if restablecer}
  <Modal labelledby="t-restablecer" onclose={() => (restablecer = null)} width={500} dismissible={!codigoRest}>
    {#if !codigoRest}
      <form
        class="form"
        onsubmit={(e) => {
          e.preventDefault();
          void confirmarRestablecer();
        }}
      >
        <div class="dlg-title">
          <span class="ticon warn"><ShieldOff size={18} /></span>
          <div>
            <h2 id="t-restablecer">¿Restablecer la verificación en dos pasos de {restablecer.nombre}?</h2>
            <p>Para quien perdió el móvil y también sus códigos de recuperación.</p>
          </div>
        </div>
        <div class="aviso-rest">
          <TriangleAlert size={16} aria-hidden="true" />
          <ul>
            <li>Se cierran todas sus sesiones y dejan de valer su aplicación de verificación y sus códigos de recuperación.</li>
            <li>Te daremos un <strong>código de un solo uso</strong> (24 horas). Sin él no podrá volver a entrar: al poner su contraseña, se lo pedirá para vincular el móvil nuevo.</li>
            <li>Hazlo solo si te lo pidió <strong>esa persona</strong> y lo has comprobado (por teléfono o en persona): quien tenga el código y su contraseña entra como ella.</li>
          </ul>
        </div>
        <p class="faint pequeno">Para confirmar que eres tú, escribe un código de <strong>tu</strong> aplicación de verificación.</p>
        <CampoCodigo bind:value={codigoYo} error={errorRest} autofocus alCompletar={(c) => void confirmarRestablecer(c)} />
        <footer>
          <button type="button" class="btn btn-ghost" onclick={() => (restablecer = null)}>Cancelar</button>
          <button class="btn btn-danger" disabled={ocupadoRest || codigoYo.replace(/\D/g, "").length !== 6}>{ocupadoRest ? "Restableciendo…" : "Restablecer"}</button>
        </footer>
      </form>
    {:else}
      <div class="dlg-title">
        <span class="ticon"><KeyRound size={18} /></span>
        <div>
          <h2 id="t-restablecer">Código para {restablecer.nombre}</h2>
          <p>Pásaselo por un canal de confianza (en persona o por teléfono). Solo se enseña ahora.</p>
        </div>
      </div>
      <p class="codigo-rest selectable">{codigoRest.codigo}</p>
      <p class="faint pequeno">Sirve una vez y caduca el {fechaLarga(codigoRest.caduca)}. Al entrar con su correo y su contraseña, la consola se lo pedirá y vinculará su móvil nuevo.</p>
      <footer>
        <button
          class="btn"
          onclick={async () => {
            await navigator.clipboard.writeText(mensajeRest);
            avisar("Instrucciones y código copiados.");
          }}><Copy size={14} />Copiar con las instrucciones</button
        >
        <button class="btn btn-primary" onclick={() => (restablecer = null)}>Hecho</button>
      </footer>
    {/if}
  </Modal>
{/if}

{#if espera && actual.cliente}
  <Modal labelledby="t-espera" onclose={() => (espera = false)} width={480} dismissible={false}>
    <form class="form" onsubmit={cambiarEspera}>
      <div class="dlg-title">
        <span class="ticon"><Clock size={18} /></span>
        <div>
          <h2 id="t-espera">Espera antes de borrar</h2>
          <p>Se manda a {plural(actual.equipos.filter((x) => x.confirmado && x.modo !== "trasladado").length, "equipo", "equipos")}: cada uno la guarda y la aplica.</p>
        </div>
      </div>
      <div class="field">
        <label class="field-label" for="horas">Esperar</label>
        <select id="horas" class="input" bind:value={horas}>
          {#each [1, 6, 12, 24, 48, 72, 168] as h (h)}<option value={h}>{h === 1 ? "1 hora (mínimo)" : h < 48 ? `${h} horas` : `${h / 24} días`}</option>{/each}
        </select>
      </div>
      {#if horas < actual.cliente.espera_min_horas}
        <div class="notice notice-warn"><TriangleAlert size={16} /><p>Acortarla da menos tiempo para cancelar algo que no esperabas.</p></div>
      {/if}
      <CampoClave requerido id="clave-admin" etiqueta="Clave de administración" bind:value={claveAdmin}>
        {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
      </CampoClave>
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if progreso}<span class="espera-txt" role="status"><LoaderCircle size={15} class="spin" />{progreso}</span>{/if}
        <button type="button" class="btn btn-ghost" disabled={!!progreso} onclick={() => (espera = false)}>Cancelar</button>
        <button class="btn btn-primary" disabled={!claveAdmin || !!progreso}><KeyRound size={15} />Confirmar con la clave de administración</button>
      </footer>
    </form>
  </Modal>
{/if}

<style>
  .fila-ajuste > .con-marca {
    flex-direction: row;
    align-items: center;
    gap: var(--sp-3);
  }
  .con-marca > span {
    display: flex;
    flex-direction: column;
  }
  /* El menú de cada persona se sale de la tarjeta: sin recortarlo. */
  .card.lista {
    overflow: visible;
  }
  .aviso-rest {
    display: flex;
    gap: var(--sp-2);
    align-items: flex-start;
    padding: var(--sp-3);
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
    background: var(--warn-soft);
    border-radius: var(--radius);
  }
  .aviso-rest :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--warn);
  }
  .aviso-rest ul {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    padding-left: 18px;
  }
  .codigo-rest {
    margin: var(--sp-4) 0 var(--sp-2);
    padding: var(--sp-3);
    font-family: var(--mono);
    font-size: 1.35rem;
    letter-spacing: 0.06em;
    text-align: center;
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .fila-ajuste {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-4);
    padding: var(--sp-3) 0;
    border-top: 1px solid var(--border);
  }
  .fila-ajuste:first-child {
    padding-top: 0;
    border-top: none;
  }
  .fila-ajuste:last-child {
    padding-bottom: 0;
  }
  .fila-ajuste > span {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .fila-ajuste strong {
    display: inline-flex;
    align-items: center;
    font-weight: 600;
  }
  .fila-ajuste .faint {
    font-size: var(--fs-sm);
  }
  .aviso-clave {
    display: inline;
    color: var(--warn);
  }
  .aviso-clave :global(svg) {
    margin-right: 4px;
    vertical-align: -2px;
  }
  .renombrar {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .ini {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    font-weight: 600;
    color: var(--text-2);
    background: var(--surface-3);
    border-radius: 999px;
  }
  .rol {
    width: 160px;
  }
  .avisos {
    flex: none;
    max-width: 220px;
    overflow: hidden;
    font-weight: 500;
    color: var(--text-2);
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .grupo-avisos {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: var(--sp-3) var(--sp-4);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .grupo-avisos legend {
    padding: 0 4px;
    font-size: var(--fs-sm);
    font-weight: 600;
  }
  .check-avisos {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  .check-avisos input {
    margin-top: 3px;
  }
  .check-avisos .faint {
    font-size: var(--fs-xs);
  }
  .section-title {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .invitar {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: var(--sp-3);
  }
  .invitar .input {
    width: 200px;
  }
  .pequeno {
    margin: 6px 0 0;
    font-size: var(--fs-xs);
  }
  .card > p.faint {
    margin: 4px 0 0;
    font-size: var(--fs-sm);
  }
  .enlace {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: var(--sp-3);
    padding: 8px 8px 8px 12px;
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .enlace code {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    word-break: break-all;
  }
  .espera-txt {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  @media (max-width: 640px) {
    .rol {
      width: 130px;
    }
    .fila {
      flex-wrap: wrap;
    }
  }
</style>
