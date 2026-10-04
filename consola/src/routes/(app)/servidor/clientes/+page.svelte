<script lang="ts">
  // «Clientes del servidor» (v1.34): lo que ve quien administra un servidor
  // compartido, sin entrar en los clientes. Solo cifras (personas, equipos,
  // última actividad, uso de las cuotas); ni nombres de equipos, ni correos,
  // ni avisos. Desde aquí se da de alta un cliente para otra persona (con su
  // invitación de propietario) y se ponen las cuotas.
  import { onMount } from "svelte";
  import { Building2, Gauge, Link2, Plus, RefreshCw, UserPlus } from "@lucide/svelte";
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

  async function otraInvitacion(c: ClienteDelServidor) {
    try {
      const r = await api.invitarPropietario(c.id);
      invitacion = { nombre: c.nombre, inv: r.invitacion };
    } catch (e) {
      avisar((e as Error).message, "bad");
    }
  }

  const enlace = $derived(invitacion ? `${location.origin}${invitacion.inv.enlace}` : "");
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
    {#if !datos.clientes.length}
      <div class="card">
        <Vacio icono={Building2} titulo="Todavía no hay clientes" texto="Crea uno para otra persona: le llega una invitación de propietario y tú no entras en él.">
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
            {#each datos.clientes as c (c.id)}
              {@const relLimite = c.efectivas.relevo_mb_mes ? c.efectivas.relevo_mb_mes * 1024 * 1024 : null}
              {@const eqP = proporcion(c.equipos, c.efectivas.equipos)}
              <tr>
                <td>
                  {#if c.soy_miembro}<a href="/c/{c.id}"><strong>{c.nombre}</strong></a>{:else}<strong>{c.nombre}</strong>{/if}
                  <span class="faint pequeno">desde {fechaLarga(c.creado)}{c.soy_miembro ? " · eres miembro" : ""}</span>
                </td>
                <td class="num">
                  {numero(c.personas)}
                  {#if !c.propietarios}<span class="badge badge-sm tone-warn">sin propietario</span>{/if}
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
                  {#if !c.propietarios}<button class="btn btn-sm" onclick={() => otraInvitacion(c)}><UserPlus size={14} />Invitación</button>{/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/if}
</div>

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
        <input id="nombre-ajeno" class="input" bind:value={nombre} placeholder="Por ejemplo: Ferretería Altamar" aria-required="true" />
      </div>
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
  <Modal labelledby="t-invitacion" onclose={() => (invitacion = null)} width={560}>
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
        <button class="btn btn-primary" onclick={() => (invitacion = null)}>Hecho</button>
      </footer>
    </div>
  </Modal>
{/if}

<style>
  .resumen {
    margin: 0 0 var(--sp-3);
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
</style>
