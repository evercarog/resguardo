<script lang="ts">
  // «Copiar a "‹equipo que guarda copias›"»: con la clave de administración,
  // 1) se pide al equipo de almacenamiento que dé acceso al equipo cliente
  //    (guarda_copias { anadir }), y su respuesta llega sellada solo para
  //    este navegador (responder_a): usuario, contraseña, dirección, CA y huella;
  // 2) se genera aquí la contraseña del repositorio y se muestra el kit;
  // 3) se crea el repositorio en el equipo cliente (crear_repositorio) con ese
  //    destino. Nada de esto lo ve el servidor.
  // Con `origen` (v1.14, «Para traer el historial de otro repositorio»), nace
  // con los parámetros de troceado de ese repositorio (`parametros_de`).
  import { onDestroy } from "svelte";
  import { Check, KeyRound, LoaderCircle, Printer, Server, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { aB64, aleatorio, borrar, deB64, deUtf8 } from "$lib/cripto/bytes";
  import { abrir, parEfimero } from "$lib/cripto/sobre";
  import { ErrorLlavesCambiadas, mandarOrden } from "$lib/ordenar";
  import { fechaLarga } from "$lib/formato";
  import { avisar } from "$lib/avisos.svelte";
  import { cargarCliente } from "$lib/estado.svelte";
  import type { Cliente, Equipo, Orden } from "$lib/tipos";
  import { TEXTO_TROCEADO, origenCuerpo, recordarOrigen, type RepoExistente } from "$lib/adoptar";
  import { TEXTO_ALMACEN_PROPIO } from "$lib/retencion";
  import Ayuda from "./Ayuda.svelte";
  import CampoClave from "./CampoClave.svelte";
  import AlertaLlaves from "./AlertaLlaves.svelte";
  // Tarea 7b: en otra zona del almacén (otro disco, con su puerto).
  import { errorRespuestaZona, idDestinoZona, PRINCIPAL, type ZonaVista } from "$lib/destinos";

  interface Acceso {
    usuario: string;
    contrasena: string;
    destino: { tipo: "rest"; donde: string; usuario: string; secreto: string; ca_pem: string };
    huella_tls: string;
    /** Tarea 7b: la zona del acceso (si se pidió una). */
    zona?: string | null;
  }

  let {
    cliente,
    equipo,
    almacen,
    nombreInicial,
    origen,
    zona,
    alCreado,
    onclose,
    parametrosDe,
  }: {
    cliente: Cliente;
    equipo: Equipo;
    almacen: Equipo;
    /** El nombre ya escrito (desde «Nuevo repositorio»). */
    nombreInicial?: string;
    /** El repositorio del que se traerá el historial (desde «Nuevo repositorio»). */
    origen?: RepoExistente;
    /** Tarea 7b: una zona del almacén que no es la principal (sin ella, la principal). */
    zona?: ZonaVista;
    /** Desde el editor de copias: el repositorio ya creado (para elegirlo en la copia). */
    alCreado?: (r: { id: string; nombre: string; destino: string }) => void;
    onclose: () => void;
    /** Plan 0.7.26: nace con el troceado de otro repositorio de este equipo (`{ repo }`), para traer después sus versiones. */
    parametrosDe?: { repo: string };
  } = $props();

  let paso = $state<"clave" | "kit" | "listo">("clave");
  // La zona de cuando se abrió: después el equipo ya copia en ella y quien
  // abrió el diálogo deja de ofrecerla (y dejaría de pasarla).
  const zonaAlAbrir = () => (zona && !zona.principal ? zona : undefined);
  const enZona = $state.raw(zonaAlAbrir());
  /** «ALMACEN-01» o, en otra zona, «ALMACEN-01 · Disco E». */
  const nombreAlmacen = $derived(enZona ? `${almacen.nombre} · ${enZona.nombre ?? enZona.id}` : almacen.nombre);
  /** v1.28: el equipo copia en su propio almacén. */
  const propio = $derived(equipo.id === almacen.id);
  let claveAdmin = $state("");
  // Nombre propuesto (al abrir; después manda lo que se escriba).
  const propuesto = () => nombreInicial || (equipo.id === almacen.id ? "Consola de Resguardo" : `Copias en ${almacen.nombre}`);
  let nombre = $state(propuesto());
  let ocupado = $state(false);
  let pasoTxt = $state("");
  let error = $state("");
  let cambiadas = $state<Equipo | null>(null);
  let acceso: Acceso | null = null;
  let contrasena = $state("");
  let impreso = $state(false);
  let huella = $state("");
  let donde = $state("");
  /** Id del repositorio (va en el kit: hace falta para restaurar sin el equipo). */
  let repoId = $state("");

  onDestroy(() => {
    claveAdmin = contrasena = "";
    if (origen) origen.contrasena = origen.secreto = "";
    acceso = null;
  });

  /** Espera la respuesta firmada de una orden (como mucho 2 min). */
  async function respuesta(eq: Equipo, o: Orden): Promise<Orden> {
    for (let i = 0; i < 80; i++) {
      const x = (await api.ordenesEquipo(cliente.id, eq.id, 10)).find((y) => y.id === o.id);
      if (x && ["hecha", "fallida", "rechazada", "cancelada", "caducada"].includes(x.estado)) return x;
      await new Promise((r) => setTimeout(r, 1500));
    }
    throw new Error(`${eq.nombre} no ha respondido todavía. Mira sus órdenes en un momento.`);
  }

  async function pedirAcceso(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    ocupado = true;
    const eph = parEfimero();
    try {
      const o = await mandarOrden({
        cliente,
        equipo: almacen,
        tipo: "guarda_copias",
        // v1.28: su propio almacén, por localhost (no depende de su IP ni del cortafuegos).
        cuerpo: { anadir: equipo.id, ...(propio ? { local: true } : {}), ...(enZona ? { zona: enZona.id } : {}) },
        secretos: { claveAdmin },
        responderA: aB64(eph.publica),
        alPaso: (t) => (pasoTxt = t),
      });
      pasoTxt = `Esperando a ${almacen.nombre}…`;
      const r = await respuesta(almacen, o);
      if (r.estado !== "hecha" || !r.detalle) throw new Error(r.mensaje ?? `${almacen.nombre} no pudo dar acceso.`);
      const sellado = (JSON.parse(r.detalle) as { sellado?: string }).sellado;
      if (!sellado) throw new Error("La respuesta no trae el acceso sellado.");
      acceso = JSON.parse(deUtf8(abrir(eph.secreta, deB64(sellado)))) as Acceso;
      // Un agente anterior ignoraría `zona` y daría un usuario de la principal: entonces no se sigue.
      const malZona = enZona ? errorRespuestaZona(acceso, enZona) : null;
      if (malZona) {
        acceso = null;
        throw new Error(malZona);
      }
      huella = acceso.huella_tls;
      donde = acceso.destino.donde;
      const b = aleatorio(32);
      contrasena = btoa(String.fromCharCode(...b)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
      b.fill(0);
      repoId = `${almacen.nombre.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 40) || "almacen"}-${crypto.randomUUID().slice(0, 4)}`;
      paso = "kit";
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) cambiadas = almacen;
      else error = (err as Error).message;
    } finally {
      borrar(eph.secreta);
      ocupado = false;
      pasoTxt = "";
    }
  }

  async function crear() {
    if (!acceso) return;
    error = "";
    ocupado = true;
    try {
      const id = repoId;
      const o = await mandarOrden({
        cliente,
        equipo,
        tipo: "crear_repositorio",
        cuerpo: {
          id,
          nombre: nombre.trim(),
          contrasena,
          // v1.30: `equipo_almacen`, para que el equipo diga en su resumen de qué almacén es el destino
          // (un agente anterior lo ignora y la consola lo reconoce por el id o el nombre, como antes).
          destino: {
            id: idDestinoZona(almacen.id, enZona?.id ?? PRINCIPAL),
            nombre: nombreAlmacen,
            tipo: "rest",
            donde: acceso.destino.donde,
            usuario: acceso.destino.usuario,
            secreto: acceso.destino.secreto,
            ca_pem: acceso.destino.ca_pem,
            equipo_almacen: almacen.id,
          },
          ...(origen ? { parametros_de: origenCuerpo(origen) } : parametrosDe ? { parametros_de: parametrosDe } : {}),
        },
        secretos: { claveAdmin },
        alPaso: (t) => (pasoTxt = t),
      });
      pasoTxt = `Esperando a ${equipo.nombre}…`;
      const r = await respuesta(equipo, o);
      if (r.estado !== "hecha") throw new Error(r.mensaje ?? "El equipo no pudo crear el repositorio.");
      if (origen) {
        recordarOrigen(cliente.id, equipo.id, id, origen);
        origen.contrasena = origen.secreto = "";
      }
      claveAdmin = contrasena = "";
      acceso = null;
      paso = "listo";
      alCreado?.({ id, nombre: nombre.trim(), destino: idDestinoZona(almacen.id, enZona?.id ?? PRINCIPAL) });
      avisar(`${equipo.nombre} ya copia en ${nombreAlmacen}.`);
      void cargarCliente(cliente.id, { silencioso: true });
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) cambiadas = equipo;
      else error = (err as Error).message;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }
</script>

<Modal labelledby="t-almacen" {onclose} width={560} dismissible={false}>
  <div class="dlg-title">
    <span class="ticon"><Server size={18} /></span>
    <div>
      <h2 id="t-almacen">{propio ? `Copiar en este mismo almacén${enZona ? ` · ${enZona.nombre ?? ""}` : ""}` : `Copiar en ${nombreAlmacen}`}</h2>
      {#if propio}
        <p>{TEXTO_ALMACEN_PROPIO} <Ayuda id="guarda-copias" /></p>
      {:else}
        <p>{equipo.nombre} guardará sus copias en {nombreAlmacen}, con su propio usuario y sin poder borrar lo ya copiado. <Ayuda id="guarda-copias" /></p>
      {/if}
      {#if origen}<p class="faint">Para traer el historial de <code>{origen.direccion}</code>. {TEXTO_TROCEADO}</p>{/if}
    </div>
  </div>

  {#if cambiadas}
    <AlertaLlaves equipo={cambiadas} cliente={cliente.id} />
    <footer><button class="btn btn-primary" onclick={onclose}>Entendido</button></footer>
  {:else if paso === "clave"}
    <form class="form" onsubmit={pedirAcceso}>
      <div class="field">
        <label class="field-label" for="a-nombre">Nombre del repositorio</label>
        <input id="a-nombre" class="input" bind:value={nombre} />
      </div>
      <CampoClave requerido id="a-clave" etiqueta="Clave de administración de {cliente.nombre}" bind:value={claveAdmin} autofocus ayuda="Se usa para las dos órdenes (dar acceso y crear el repositorio). El servidor nunca la ve.">
        {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
      </CampoClave>
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
        <button class="btn btn-primary" disabled={ocupado || !claveAdmin || !nombre.trim()}><KeyRound size={15} />Confirmar con la clave de administración</button>
      </footer>
    </form>
  {:else if paso === "kit"}
    <div class="form">
      <p>Este es el <strong>kit de recuperación</strong> del repositorio. Imprímelo o guárdalo en PDF antes de seguir.</p>
      <article class="kit" id="kit-imprimible">
        <h3>Kit de recuperación · Resguardo</h3>
        <dl>
          <dt>Cliente</dt><dd>{cliente.nombre}</dd>
          <dt>Equipo</dt><dd>{equipo.nombre}</dd>
          <dt>Repositorio</dt><dd>{nombre} · <span class="pastilla mono selectable">{repoId}</span></dd>
          <dt>Destino</dt><dd>{nombreAlmacen} · <span class="pastilla mono ajusta">{donde}</span></dd>
          <dt>Huella TLS</dt><dd><code>{huella}</code></dd>
          <dt>Contraseña</dt><dd><code class="selectable pw">{contrasena}</code></dd>
          <dt>Creado</dt><dd>{fechaLarga(new Date().toISOString())}</dd>
        </dl>
      </article>
      <div><button type="button" class="btn" onclick={() => window.print()}><Printer size={15} />Imprimir o guardar en PDF</button></div>
      <label class="switch-row"><input type="checkbox" bind:checked={impreso} /><span>He guardado el kit en un sitio seguro</span></label>
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
        <button type="button" class="btn btn-primary" disabled={!impreso || ocupado} onclick={crear}><Check size={15} />Crear el repositorio en {equipo.nombre}</button>
      </footer>
    </div>
  {:else}
    {#if origen}
      <p>Listo: {equipo.nombre} ya tiene el repositorio «{nombre}» en {nombreAlmacen}, con la misma forma de trocear que <code>{origen.direccion}</code>. Ahora, en su página, «Traer historial» (ya tiene puesto ese origen: solo falta su contraseña).</p>
    {:else}
      <p>Listo: {equipo.nombre} ya tiene el repositorio «{nombre}» en {nombreAlmacen}. Ahora elige qué copiar.</p>
    {/if}
    <footer>
      <button class="btn btn-ghost" onclick={onclose}>Cerrar</button>
      {#if origen}
        <a class="btn btn-primary" href="/c/{cliente.id}/equipos/{equipo.id}/repositorios/{encodeURIComponent(repoId)}?traer=1" onclick={onclose}>Traer el historial</a>
      {:else}
        <a class="btn btn-primary" href="/c/{cliente.id}/equipos/{equipo.id}/copias">Elegir qué copiar</a>
      {/if}
    </footer>
  {/if}
</Modal>

<style>
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .kit {
    padding: var(--sp-5);
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
  }
  .kit h3 {
    margin-bottom: var(--sp-3);
    font-size: var(--fs-h2);
  }
  .kit dl {
    display: grid;
    grid-template-columns: 110px minmax(0, 1fr);
    gap: 6px 12px;
    margin: 0;
    font-size: var(--fs-sm);
  }
  .kit dt {
    color: var(--text-3);
  }
  .kit dd {
    margin: 0;
    word-break: break-all;
  }
  .pw {
    font-size: 14px;
  }
  @media print {
    :global(body *) {
      visibility: hidden;
    }
    :global(#kit-imprimible),
    :global(#kit-imprimible *) {
      visibility: visible;
    }
    :global(#kit-imprimible) {
      position: fixed;
      inset: 0 auto auto 0;
      width: 100%;
      border: none;
      color: #000;
    }
  }
</style>
