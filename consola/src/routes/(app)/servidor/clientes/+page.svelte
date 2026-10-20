<script lang="ts">
  // «Clientes del servidor» (v1.34): lo que ve quien administra un servidor
  // compartido, sin entrar en los clientes. Solo cifras (personas, equipos,
  // última actividad, uso de las cuotas); ni nombres de equipos, ni correos,
  // ni avisos. Desde aquí se da de alta un cliente para otra persona (con su
  // invitación de propietario) y se ponen las cuotas. 15nn: y se archiva, se
  // recupera o se borra un cliente (solo sin equipos, escribiendo su nombre).
  import Chip from "$lib/componentes/Chip.svelte";
  import { onMount } from "svelte";
  import { Archive, ArchiveRestore, Building2, Gauge, Link2, Plus, RefreshCw, Trash2, UserPlus } from "@lucide/svelte";
  import BotonCargando from "$lib/componentes/BotonCargando.svelte";
  import BotonFalta from "$lib/componentes/BotonFalta.svelte";
  import AvisoNombreRepetido from "$lib/componentes/AvisoNombreRepetido.svelte";
  import { tip } from "$lib/tooltip";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { app, cargarClientes } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { bytes, fechaLarga, numero, plural } from "$lib/formato";
  import type { ClienteDelServidor, ClientesDelServidor, Cuotas, Invitacion } from "$lib/tipos";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import BloqueCopiable from "$lib/componentes/BloqueCopiable.svelte";

  let datos = $state<ClientesDelServidor | null>(null);
  let error = $state("");

  async function cargar() {
    error = "";
    try {
      datos = await api.clientesDelServidor();
    } catch (e) {
      error = (e as Error).message;
    }
  }
  onMount(cargar);

  // --- Cuotas (de un cliente o las predeterminadas) -------------------------
  const CAMPOS: { k: keyof Cuotas; nombre: string; unidad: string; ayuda: string }[] = [
    { k: "equipos", nombre: "Equipos", unidad: "equipos", ayuda: "Equipos dados de alta en el cliente." },
    { k: "historial", nombre: "Historial por equipo", unidad: "entradas", ayuda: "Lo que se guarda del historial de cada equipo; lo más antiguo se borra." },
    { k: "relevo_mb_mes", nombre: "Relé de descargas", unidad: "MB al mes", ayuda: "Lo que los equipos pueden subir para restaurar en el navegador, por mes. Las copias nunca pasan por aquí." },
    { k: "ordenes_min", nombre: "Órdenes", unidad: "por minuto", ayuda: "Órdenes de todas las personas del cliente en un minuto." },
  ];
  type Texto = Record<keyof Cuotas, string>;
  let editando = $state<{ cliente: ClienteDelServidor | null; valores: Texto } | null>(null);
  let guardando = $state(false);
  let errorCuotas = $state("");

  const aTexto = (c: Cuotas): Texto => ({
    equipos: c.equipos?.toString() ?? "",
    historial: c.historial?.toString() ?? "",
    relevo_mb_mes: c.relevo_mb_mes?.toString() ?? "",
    ordenes_min: c.ordenes_min?.toString() ?? "",
  });
  function deTexto(t: Texto): Cuotas | string {
    const out: Cuotas = { equipos: null, historial: null, relevo_mb_mes: null, ordenes_min: null };
    for (const { k, nombre } of CAMPOS) {
      const v = t[k].trim();
      if (!v) continue;
      if (!/^\d+$/.test(v)) return `«${nombre}»: un número entero (0 = sin límite) o vacío.`;
      out[k] = Number(v);
    }
    return out;
  }
  const limiteEnFrase = (v: number | null, unidad: string) => (v === null ? "—" : v === 0 ? "sin límite" : `${numero(v)} ${unidad}`.trim());

  async function guardarCuotas(e: SubmitEvent) {
    e.preventDefault();
    if (!editando) return;
    const c = deTexto(editando.valores);
    if (typeof c === "string") {
      errorCuotas = c;
      return;
    }
    guardando = true;
    errorCuotas = "";
    try {
      if (editando.cliente) {
        await api.ponerCuotas(editando.cliente.id, c);
        avisar(`Cuotas de «${editando.cliente.nombre}» guardadas.`);
      } else {
        await api.ponerCuotasPredeterminadas(c);
        avisar("Cuotas predeterminadas guardadas.");
      }
      editando = null;
      await cargar();
    } catch (e) {
      errorCuotas = (e as Error).message;
    } finally {
      guardando = false;
    }
  }

  // --- Cliente para otra persona ---------------------------------------------
  let nuevo = $state(false);
  let nombre = $state("");
  let espera = $state(24);
  let creando = $state(false);
  let errorNuevo = $state("");
  let invitacion = $state<{ nombre: string; inv: Invitacion } | null>(null);

  async function crear(e: SubmitEvent) {
    e.preventDefault();
    if (creando) return;
    creando = true;
    errorNuevo = "";
    try {
      const r = await api.crearClienteParaOtro({ nombre: nombre.trim(), espera_min_horas: espera });
      nuevo = false;
      nombre = "";
      invitacion = { nombre: r.nombre, inv: r.invitacion };
      await Promise.all([cargar(), cargarClientes()]);
    } catch (e) {
      errorNuevo = (e as Error).message;
    } finally {
      creando = false;
    }
  }

  /** El cliente al que se está pidiendo una invitación (un doble clic daría dos enlaces válidos). */
  let invitandoA = $state<string | null>(null);
  async function otraInvitacion(c: ClienteDelServidor) {
    if (invitandoA) return;
    invitandoA = c.id;
    try {
      const r = await api.invitarPropietario(c.id);
      invitacion = { nombre: c.nombre, inv: r.invitacion };
    } catch (e) {
      avisar((e as Error).message, "bad");
    } finally {
      invitandoA = null;
    }
  }

  const enlace = $derived(invitacion ? `${location.origin}${invitacion.inv.enlace}` : "");

  // --- 15nn: archivar, recuperar y borrar ------------------------------------
  const activos = $derived(datos?.clientes.filter((c) => !c.archivado) ?? []);
  const archivados = $derived(datos?.clientes.filter((c) => c.archivado) ?? []);

  let archivando = $state<ClienteDelServidor | null>(null);
  let errorArchivar = $state("");
  /** El cliente que se está archivando o recuperando ahora. */
  let cambiando = $state<string | null>(null);

  async function archivar() {
    if (!archivando || cambiando) return;
    const c = archivando;
    cambiando = c.id;
    errorArchivar = "";
    try {
      await api.archivarCliente(c.id);
      archivando = null;
      avisar(`«${c.nombre}» archivado. Lo tienes en «Archivados», al final de esta página.`);
      await Promise.all([cargar(), cargarClientes()]);
    } catch (e) {
      errorArchivar = (e as Error).message;
    } finally {
      cambiando = null;
    }
  }

  async function recuperar(c: ClienteDelServidor) {
    if (cambiando) return;
    cambiando = c.id;
    try {
      await api.recuperarCliente(c.id);
      avisar(`«${c.nombre}» recuperado: vuelve a estar como antes.`);
      await Promise.all([cargar(), cargarClientes()]);
    } catch (e) {
      avisar((e as Error).message, "bad");
    } finally {
      cambiando = null;
    }
  }

  let borrando = $state<{ c: ClienteDelServidor; escrito: string } | null>(null);
  let borrandoAhora = $state(false);
  let errorBorrar = $state("");
  const nombreCoincide = $derived(!!borrando && borrando.escrito.trim() === borrando.c.nombre.trim());

  async function borrar(e: SubmitEvent) {
    e.preventDefault();
    if (!borrando || !nombreCoincide || borrandoAhora) return;
    const c = borrando.c;
    borrandoAhora = true;
    errorBorrar = "";
    try {
      await api.borrarCliente(c.id, borrando.escrito);
      borrando = null;
      avisar(`«${c.nombre}» borrado.`);
      await Promise.all([cargar(), cargarClientes()]);
    } catch (e) {
      errorBorrar = (e as Error).message;
    } finally {
      borrandoAhora = false;
    }
  }
  const abrirBorrar = (c: ClienteDelServidor) => ((errorBorrar = ""), (borrando = { c, escrito: "" }));
  const abrirArchivar = (c: ClienteDelServidor) => ((errorArchivar = ""), (archivando = c));
  const conEquipos = (c: ClienteDelServidor) =>
    avisar(`«${c.nombre}» tiene ${plural(c.equipos, "equipo", "equipos")}: para borrarlo, quítalos antes desde el cliente. Si solo quieres que no salga, archívalo.`, "warn");
  /** El uso frente a su límite, para una barrita (null: sin límite). */
  function proporcion(uso: number, limite: number | null): number | null {
    if (!limite) return null;
    return Math.min(1, uso / limite);
  }
</script>

<svelte:head><title>Clientes del servidor · Resguardo Server</title></svelte:head>

<div class="page">
  <div class="page-top">
    <div>
      <h1>Clientes del servidor</h1>
      <p>Cuánto usa cada cliente, sin entrar en él: aquí no se ven sus equipos, sus personas ni sus avisos. Para eso hay que ser miembro del cliente.</p>
    </div>
    {#if app.cuenta?.superusuario}
      <div class="page-actions">
        <button class="btn" onclick={cargar}><RefreshCw size={16} />Actualizar</button>
        <button class="btn" disabled={!datos} onclick={() => datos && (editando = { cliente: null, valores: aTexto(datos.predeterminadas) })}><Gauge size={16} />Cuotas predeterminadas</button>
        <button class="btn btn-primary" onclick={() => (nuevo = true)}><Plus size={16} />Cliente para otra persona</button>
      </div>
    {/if}
  </div>

  {#if !app.cuenta?.superusuario}
    <div class="card"><Vacio icono={Building2} titulo="Solo para quien administra el servidor" texto="Esta página es del propietario del servidor." /></div>
  {:else if error}
    <div class="card"><p class="error-campo" role="alert">{error}</p></div>
  {:else if !datos}
    <Cargando />
  {:else}
    <p class="faint resumen">
      {datos.publico ? "Consola en internet" : "Consola en la red local"} · cuotas predeterminadas:
      {#each CAMPOS as campo, i (campo.k)}{i ? " · " : ""}{campo.nombre.toLowerCase()}: {limiteEnFrase(datos.predeterminadas[campo.k], campo.k === "equipos" ? "" : campo.unidad)}{/each}
    </p>
    {#if !activos.length}
      <div class="card">
        <Vacio icono={Building2} titulo={archivados.length ? "No hay clientes activos" : "Todavía no hay clientes"} texto="Crea uno para otra persona: le llega una invitación de propietario y tú no entras en él.">
          <button class="btn btn-primary" onclick={() => (nuevo = true)}><Plus size={16} />Cliente para otra persona</button>
        </Vacio>
      </div>
    {:else}
      <div class="card tabla-envoltura">
        <table class="tabla">
          <thead>
            <tr>
              <th scope="col">Cliente</th>
              <th scope="col">Personas</th>
              <th scope="col">Equipos</th>
              <th scope="col">Última actividad</th>
              <th scope="col">Relé este mes</th>
              <th scope="col">Historial</th>
              <th scope="col">Disco</th>
              <th scope="col"><span class="sr-only">Acciones</span></th>
            </tr>
          </thead>
          <tbody>
            {#each activos as c (c.id)}
              {@const relLimite = c.efectivas.relevo_mb_mes ? c.efectivas.relevo_mb_mes * 1024 * 1024 : null}
              {@const eqP = proporcion(c.equipos, c.efectivas.equipos)}
              <tr>
                <td>
                  {#if c.soy_miembro}<a href="/c/{c.id}"><strong>{c.nombre}</strong></a>{:else}<strong>{c.nombre}</strong>{/if}
                  <span class="faint pequeno">desde {fechaLarga(c.creado)}{c.soy_miembro ? " · eres miembro" : ""}</span>
                </td>
                <td class="num">
                  {numero(c.personas)}
                  {#if !c.propietarios}<Chip pequeno tono="warn" texto="Sin propietario" />{/if}
                </td>
                <td class="num">
                  {numero(c.equipos)}{#if c.efectivas.equipos}{" / "}{numero(c.efectivas.equipos)}{/if}
                  <span class="faint pequeno">{plural(c.equipos_conectados, "conectado", "conectados")}</span>
                  {#if eqP !== null}<span class="barra" class:llena={eqP >= 1} style="--p: {eqP}"></span>{/if}
                </td>
                <td><Tiempo iso={c.ultima_actividad} /></td>
                <td class="num">
                  {bytes(c.relevo_mes_bytes)}{#if relLimite}{" / "}{bytes(relLimite)}{/if}
                  {#if relLimite}<span class="barra" class:llena={c.relevo_mes_bytes >= relLimite} style="--p: {Math.min(1, c.relevo_mes_bytes / relLimite)}"></span>{/if}
                </td>
                <td class="num">{plural(c.historial, "entrada", "entradas")}</td>
                <td class="num">{bytes(c.bytes)}</td>
                <td class="acciones">
                  <button class="btn btn-sm" onclick={() => (editando = { cliente: c, valores: aTexto(c.cuotas) })}><Gauge size={14} />Cuotas</button>
                  {#if !c.propietarios}<BotonCargando class="btn btn-sm" cargando={invitandoA === c.id} disabled={!!invitandoA} onclick={() => otraInvitacion(c)}><UserPlus size={14} />Invitación</BotonCargando>{/if}
                  <button class="btn btn-sm" onclick={() => abrirArchivar(c)} use:tip={"Sale de las listas y no recibe nada. Se puede recuperar."}><Archive size={14} />Archivar</button>
                  {#if c.equipos}
                    <button class="btn btn-sm btn-ghost apagado" aria-disabled="true" onclick={() => conEquipos(c)} use:tip={`Tiene ${plural(c.equipos, "equipo", "equipos")}: archívalo, o quítalos antes`}><Trash2 size={14} />Borrar</button>
                  {:else}
                    <button class="btn btn-sm btn-ghost peligro" onclick={() => abrirBorrar(c)}><Trash2 size={14} />Borrar</button>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}

    {#if archivados.length}
      <section class="card archivados" aria-labelledby="t-archivados">
        <h2 id="t-archivados"><Archive size={16} />Archivados</h2>
        <p class="faint">No salen en ninguna lista, nadie los abre y no reciben nada: sus equipos no pueden conectarse a esta consola y no se mandan sus avisos. Nada se ha borrado: al recuperarlos vuelven como estaban.</p>
        <ul>
          {#each archivados as c (c.id)}
            <li>
              <span class="texto">
                <strong>{c.nombre}</strong>
                <span class="faint pequeno">archivado {c.archivado ? fechaLarga(c.archivado) : ""} · {plural(c.equipos, "equipo", "equipos")} · {plural(c.personas, "persona", "personas")} · {bytes(c.bytes)}</span>
              </span>
              <span class="acciones">
                <BotonCargando class="btn btn-sm" cargando={cambiando === c.id} disabled={!!cambiando} onclick={() => recuperar(c)}><ArchiveRestore size={14} />Recuperar</BotonCargando>
                {#if !c.equipos}<button class="btn btn-sm btn-ghost peligro" onclick={() => abrirBorrar(c)}><Trash2 size={14} />Borrar</button>{/if}
              </span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  {/if}
</div>

{#if archivando}
  <Modal labelledby="t-archivar" onclose={() => (archivando = null)} width={500} ocupado={cambiando === archivando.id}>
    <div class="form">
      <div class="dlg-title">
        <span class="ticon"><Archive size={18} /></span>
        <div>
          <h2 id="t-archivar">¿Archivar «{archivando.nombre}»?</h2>
          <p>Para un cliente que ya no se lleva desde aquí, pero cuyo historial quieres guardar. Nada se borra.</p>
        </div>
      </div>
      <ul class="efectos">
        <li>Sale de todas las listas (también de «Todos los clientes» y del buscador) y nadie lo abre, ni sus personas.</li>
        <li>
          No recibe nada:
          {archivando.equipos
            ? archivando.equipos === 1
              ? "su equipo no puede conectarse a esta consola (sigue copiando igual y, si está también en otra consola, allí sigue como siempre)"
              : `sus ${archivando.equipos} equipos no pueden conectarse a esta consola (siguen copiando igual y, si están también en otra consola, allí siguen como siempre)`
            : "sus invitaciones y códigos dejan de valer"},
          y no se mandan sus avisos ni sus informes.
        </li>
        <li>Lo recuperas cuando quieras en «Archivados», en esta misma página.</li>
      </ul>
      {#if errorArchivar}<p class="error-campo" role="alert">{errorArchivar}</p>{/if}
      <footer>
        <button type="button" class="btn btn-ghost" onclick={() => (archivando = null)}>Cancelar</button>
        <BotonCargando class="btn btn-primary" cargando={cambiando === archivando.id} textoCargando="Archivando…" onclick={archivar}><Archive size={16} />Archivar</BotonCargando>
      </footer>
    </div>
  </Modal>
{/if}

{#if borrando}
  <Modal labelledby="t-borrar" onclose={() => (borrando = null)} width={500} dismissible={false} ocupado={borrandoAhora}>
    <form class="form" onsubmit={borrar}>
      <div class="dlg-title">
        <span class="ticon danger"><Trash2 size={18} /></span>
        <div>
          <h2 id="t-borrar">¿Borrar «{borrando.c.nombre}»?</h2>
          <p>No tiene equipos. Se borra para siempre, con sus personas, invitaciones, ajustes y su registro de actividad. No se puede deshacer: si dudas, archívalo.</p>
        </div>
      </div>
      <div class="field">
        <label class="field-label" for="borrar-nombre">Escribe su nombre para confirmar: <strong>{borrando.c.nombre}</strong></label>
        <input id="borrar-nombre" class="input" bind:value={borrando.escrito} autocomplete="off" spellcheck="false" aria-required="true" />
      </div>
      {#if errorBorrar}<p class="error-campo" role="alert">{errorBorrar}</p>{/if}
      <footer>
        <button type="button" class="btn btn-ghost" onclick={() => (borrando = null)}>Cancelar</button>
        <BotonFalta
          class="btn btn-danger"
          cargando={borrandoAhora}
          textoCargando="Borrando…"
          falta={nombreCoincide ? null : { texto: borrando.escrito.trim() ? "El nombre no coincide" : "Escribe su nombre", campo: "borrar-nombre" }}><Trash2 size={16} />Borrar para siempre</BotonFalta>
      </footer>
    </form>
  </Modal>
{/if}

{#if editando}
  <Modal labelledby="t-cuotas" onclose={() => (editando = null)} width={520} dismissible={false}>
    <form class="form" onsubmit={guardarCuotas}>
      <div class="dlg-title">
        <span class="ticon"><Gauge size={18} /></span>
        <div>
          <h2 id="t-cuotas">{editando.cliente ? `Cuotas de «${editando.cliente.nombre}»` : "Cuotas predeterminadas"}</h2>
          <p>
            {editando.cliente
              ? "Vacío: la predeterminada del servidor. 0: sin límite."
              : "Las de todos los clientes que no tengan otras. Vacío: la de fábrica. 0: sin límite."}
          </p>
        </div>
      </div>
      {#each CAMPOS as campo (campo.k)}
        {@const base = editando.cliente ? datos?.predeterminadas[campo.k] : datos?.de_fabrica[campo.k]}
        <div class="field">
          <label class="field-label" for="cuota-{campo.k}">{campo.nombre} <span class="faint">({campo.unidad})</span></label>
          <input id="cuota-{campo.k}" class="input num" inputmode="numeric" bind:value={editando.valores[campo.k]} placeholder={base === undefined ? "" : base === null || base === 0 ? "sin límite" : String(base)} />
          <span class="field-hint">{campo.ayuda}</span>
        </div>
      {/each}
      {#if errorCuotas}<p class="error-campo" role="alert">{errorCuotas}</p>{/if}
      <footer>
        <button type="button" class="btn btn-ghost" onclick={() => (editando = null)}>Cancelar</button>
        <button class="btn btn-primary" disabled={guardando}>{guardando ? "Guardando…" : "Guardar"}</button>
      </footer>
    </form>
  </Modal>
{/if}

{#if nuevo}
  <Modal labelledby="t-para-otro" onclose={() => (nuevo = false)} width={480} dismissible={false}>
    <form class="form" onsubmit={crear}>
      <div class="dlg-title">
        <span class="ticon"><Building2 size={18} /></span>
        <div>
          <h2 id="t-para-otro">Cliente para otra persona</h2>
          <p>Se crea vacío y sin ti dentro. Le das un enlace de invitación de propietario: desde ahí lo lleva esa persona.</p>
        </div>
      </div>
      <div class="field">
        <label class="field-label" for="nombre-ajeno">Nombre</label>
        <input id="nombre-ajeno" class="input" bind:value={nombre} placeholder="Por ejemplo: Oficina Norte" aria-required="true" />
      </div>
      <AvisoNombreRepetido {nombre} clientes={datos?.clientes ?? []} />
      <div class="field">
        <label class="field-label" for="espera-ajeno">Espera antes de borrar</label>
        <select id="espera-ajeno" class="input" bind:value={espera}>
          <option value={1}>1 hora (mínimo)</option>
          <option value={12}>12 horas</option>
          <option value={24}>24 horas (recomendado)</option>
          <option value={48}>48 horas</option>
          <option value={72}>3 días</option>
          <option value={168}>7 días</option>
        </select>
      </div>
      {#if errorNuevo}<p class="error-campo" role="alert">{errorNuevo}</p>{/if}
      <footer>
        <button type="button" class="btn btn-ghost" onclick={() => (nuevo = false)}>Cancelar</button>
        <button class="btn btn-primary" disabled={creando || !nombre.trim()}>{creando ? "Creando…" : "Crear y dar la invitación"}</button>
      </footer>
    </form>
  </Modal>
{/if}

{#if invitacion}
  <Modal labelledby="t-invitacion" onclose={() => (invitacion = null)} width={560} dismissible={false} sePierde="El enlace no se vuelve a enseñar: cópialo antes de cerrar.">
    <div class="form">
      <div class="dlg-title">
        <span class="ticon"><Link2 size={18} /></span>
        <div>
          <h2 id="t-invitacion">Invitación de propietario de «{invitacion.nombre}»</h2>
          <p>Sirve una vez, hasta el {fechaLarga(invitacion.inv.caduca)}. Mándasela por un canal de confianza: quien la abra primero se queda con el cliente.</p>
        </div>
      </div>
      <BloqueCopiable texto={enlace} etiqueta="Copiar el enlace" alto={3} />
      <footer>
        <button class="btn btn-primary" onclick={() => (invitacion = null)}>Listo</button>
      </footer>
    </div>
  </Modal>
{/if}

<style>
  .resumen {
    margin: 0 0 var(--sp-3);
  }
  /* A 375 px las acciones no caben en una fila (flex: none): que se repartan en varias. */
  @media (max-width: 640px) {
    .page-actions {
      flex: 1 1 100%;
      min-width: 0;
    }
  }
  .tabla-envoltura {
    overflow-x: auto;
    padding: 0;
  }
  .tabla td {
    vertical-align: top;
  }
  .tabla td.num {
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .pequeno {
    display: block;
    font-size: 12px;
  }
  .acciones {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
    flex-wrap: wrap;
  }
  .barra {
    display: block;
    height: 4px;
    margin-top: 4px;
    border-radius: 2px;
    background: linear-gradient(to right, var(--accent) calc(var(--p) * 100%), var(--surface-2) 0);
  }
  .barra.llena {
    background: var(--bad);
  }
  .peligro {
    color: var(--bad);
  }
  .apagado {
    opacity: 0.55;
  }
  .archivados {
    margin-top: var(--sp-4);
    padding: var(--sp-4) var(--sp-5);
  }
  .archivados h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 4px;
    font-size: var(--fs-body);
  }
  .archivados > p {
    margin: 0 0 var(--sp-2);
    font-size: var(--fs-sm);
  }
  .archivados ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .archivados li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px var(--sp-3);
    padding: 10px 0;
    border-top: 1px solid var(--border);
  }
  .archivados .texto {
    flex: 1;
    min-width: 12em;
  }
  .efectos {
    display: grid;
    gap: 6px;
    margin: 0;
    padding-left: 1.2em;
    font-size: var(--fs-sm);
  }
</style>
