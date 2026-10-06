<script lang="ts">
  // «Conectar Dropbox» en el equipo que guarda copias, sin CLI:
  //   1. nombre de la nube y qué podrá hacer (solo Aplicaciones/Resguardo);
  //   2. se abre Dropbox (OAuth con PKCE); la persona da permiso y copia el código;
  //   3. este navegador cambia el código por el token y lo manda SELLADO al
  //      equipo en `conectar_nube` (clave de administración). El servidor no lo ve.
  // El token solo vive en variables de esta pantalla y se borra al cerrarla.
  import { onDestroy } from "svelte";
  import { Check, Cloud, ExternalLink, KeyRound, LoaderCircle, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { avisar } from "$lib/avisos.svelte";
  import { cargarCliente } from "$lib/estado.svelte";
  import { appKeyConfigurada, cambiarCodigo, DROPBOX_APP_KEY_POR_DEFECTO, nombreNubeValido, pkce, urlAutorizar, type TokenDropbox } from "$lib/nubes";
  import { ErrorLlavesCambiadas, mandarOrden } from "$lib/ordenar";
  import type { Cliente, Equipo, Orden } from "$lib/tipos";
  import AlertaLlaves from "./AlertaLlaves.svelte";
  import Ayuda from "./Ayuda.svelte";
  import CampoClave from "./CampoClave.svelte";

  let {
    cliente,
    equipo,
    onclose,
    nombreInicial,
    alConectar,
  }: {
    cliente: Cliente;
    equipo: Equipo;
    onclose: () => void;
    /** El nombre que ya tiene en otro equipo («Dropbox Oficina»). */
    nombreInicial?: string;
    /** Cuando el equipo la ha guardado (para elegirla ya, p. ej. en «Nuevo repositorio»). */
    alConectar?: (nombre: string) => void;
  } = $props();

  /** En un almacén, para su espejo; en otro equipo (4a), para sus copias derivadas. */
  const enAlmacen = $derived(!!equipo.resumen?.guarda_copias?.activo);
  const existentes = $derived([...(equipo.resumen?.guarda_copias?.nubes ?? []), ...(equipo.resumen?.nubes ?? [])].map((n) => n.nombre.toLowerCase()));
  let paso = $state<1 | 2 | 3>(1);
  const propuesto = () => `Dropbox ${cliente.nombre}`;
  // svelte-ignore state_referenced_locally
  let nombre = $state(nombreInicial ?? propuesto());
  let codigo = $state("");
  let claveAdmin = $state("");
  let ocupado = $state(false);
  let pasoTxt = $state("");
  let error = $state("");
  let cambiadas = $state(false);
  let hecho = $state(false);
  let verifier = "";
  let token: TokenDropbox | null = null;

  // App key: la que diga el servidor o la de la consola (marcador hasta registrar la app).
  let appKey = $state(DROPBOX_APP_KEY_POR_DEFECTO);
  const simulador = import.meta.env.MODE === "mock";
  api
    .servidor()
    // La del servidor manda si la da (aunque sea vacía: entonces, sin configurar).
    .then((s) => typeof s.dropbox_app_key === "string" && (appKey = s.dropbox_app_key.trim()))
    .catch(() => {});
  const listo = $derived(appKeyConfigurada(appKey) || simulador);

  const nombreError = $derived(
    !nombreNubeValido(nombre) ? "Usa letras, números, espacios, guiones o puntos (hasta 40)." : existentes.includes(nombre.trim().toLowerCase()) ? "Ya hay una nube con ese nombre en este equipo." : "",
  );

  onDestroy(() => {
    claveAdmin = codigo = verifier = "";
    token = null;
  });

  function abrirDropbox() {
    const p = pkce();
    verifier = p.verifier;
    window.open(urlAutorizar(appKey, p.challenge), "_blank", "noopener,noreferrer,width=720,height=760");
    paso = 2;
  }

  async function respuesta(o: Orden): Promise<Orden> {
    for (let i = 0; i < 80; i++) {
      const x = (await api.ordenesEquipo(cliente.id, equipo.id, 10)).find((y) => y.id === o.id);
      if (x && ["hecha", "fallida", "rechazada", "cancelada", "caducada"].includes(x.estado)) return x;
      await new Promise((r) => setTimeout(r, 1500));
    }
    throw new Error(`${equipo.nombre} no ha respondido todavía. Mira sus órdenes en un momento.`);
  }

  async function conectar(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    ocupado = true;
    try {
      pasoTxt = "Pidiendo el permiso a Dropbox…";
      // En el simulador no se habla con Dropbox: se inventa un token de prueba.
      token = simulador ? { refresh_token: `simulado-${crypto.randomUUID()}`, access_token: "simulado", expira: new Date(Date.now() + 4 * 3600_000).toISOString() } : await cambiarCodigo(appKey, codigo, verifier);
      codigo = verifier = "";
      paso = 3;
      const o = await mandarOrden({
        cliente,
        equipo,
        tipo: "conectar_nube",
        cuerpo: { tipo: "dropbox", nombre: nombre.trim(), ...token, app_key: appKey },
        secretos: { claveAdmin },
        alPaso: (t) => (pasoTxt = t),
      });
      token = null;
      pasoTxt = `Esperando a ${equipo.nombre}…`;
      const r = await respuesta(o);
      if (r.estado !== "hecha") throw new Error(r.mensaje ?? `${equipo.nombre} no pudo guardar la conexión.`);
      claveAdmin = "";
      hecho = true;
      avisar(`«${nombre.trim()}» conectada en ${equipo.nombre}.`);
      void cargarCliente(cliente.id, { silencioso: true });
      alConectar?.(nombre.trim());
    } catch (err) {
      token = null;
      if (err instanceof ErrorLlavesCambiadas) cambiadas = true;
      else error = (err as Error).message;
      if (paso === 3) paso = 2;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }

  const PASOS = ["Nombre", "Permiso en Dropbox", "Guardar en el equipo"];
</script>

<Modal labelledby="t-nube" {onclose} width={540} dismissible={false}>
  <div class="dlg-title">
    <span class="ticon"><Cloud size={18} /></span>
    <div>
      <h2 id="t-nube">Conectar Dropbox en {equipo.nombre}</h2>
      <p>{enAlmacen ? "Para el espejo de lo que guarda este equipo (y sus propias copias)." : `Para las copias de ${equipo.nombre} a esta nube.`} El permiso se guarda protegido solo en el equipo: ni el servidor ni esta consola lo conservan.</p>
    </div>
  </div>

  {#if hecho}
    <div class="form">
      <div class="notice notice-success" role="status"><Check size={16} /><p>«{nombre.trim()}» está conectada. {alConectar ? "Ya puedes elegirla como destino." : enAlmacen ? "Ya puedes añadirla como destino del espejo." : "Ya puedes elegirla como destino de un repositorio o al añadir un paso."}</p></div>
      <footer><button class="btn btn-primary" onclick={onclose}>Hecho</button></footer>
    </div>
  {:else}
    <ol class="progreso" aria-label="Pasos">
      {#each PASOS as t, i (t)}
        <li class:on={paso === i + 1} class:hecho={paso > i + 1}><span class="n">{#if paso > i + 1}<Check size={12} />{:else}{i + 1}{/if}</span>{t}</li>
      {/each}
    </ol>

    {#if !listo}
      <div class="notice notice-warn"><TriangleAlert size={16} /><p>La app «Resguardo» de Dropbox aún no está configurada en este servidor. Avisa a quien lo administra.</p></div>
      <footer><button class="btn btn-ghost" onclick={onclose}>Cerrar</button></footer>
    {:else if paso === 1}
      <div class="form">
        <div class="field">
          <label class="field-label" for="n-nombre">Nombre de la nube</label>
          <input id="n-nombre" class="input" bind:value={nombre} />
          {#if nombreError}<p class="error-campo">{nombreError}</p>{:else}<span class="field-hint">Así la verás al elegir los destinos{enAlmacen ? " del espejo" : ""}.</span>{/if}
        </div>
        <div class="notice notice-info">
          <p>
            Resguardo solo podrá leer y escribir en <strong>Aplicaciones/Resguardo</strong> de tu Dropbox: no ve ni toca nada más. Lo que sube va cifrado.
            Dropbox no es inmutable: quien tenga la cuenta puede borrar lo subido. <Ayuda id="espejo" />
          </p>
        </div>
        <footer>
          <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
          <button type="button" class="btn btn-primary" disabled={!!nombreError} onclick={abrirDropbox}><ExternalLink size={15} />Abrir Dropbox</button>
        </footer>
      </div>
    {:else}
      <form class="form" onsubmit={conectar}>
        <ol class="guia">
          <li>En la ventana de Dropbox, entra con la cuenta de la oficina y pulsa «Permitir».</li>
          <li>Dropbox te enseña un código: cópialo y pégalo aquí.</li>
        </ol>
        <div class="field">
          <label class="field-label" for="n-codigo">Código de Dropbox</label>
          <input id="n-codigo" class="input mono" bind:value={codigo} autocomplete="off" spellcheck="false" placeholder={simulador ? "En el simulador vale cualquiera" : ""} />
          <button type="button" class="link otra" onclick={abrirDropbox}>Volver a abrir Dropbox</button>
        </div>
        <CampoClave requerido id="n-clave" etiqueta="Clave de administración" bind:value={claveAdmin}>
          {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
        </CampoClave>
        {#if cambiadas}<AlertaLlaves {equipo} cliente={cliente.id} />{/if}
        {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
        <footer>
          {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
          <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
          <button class="btn btn-primary" disabled={ocupado || !codigo.trim() || !claveAdmin}><KeyRound size={15} />Conectar</button>
        </footer>
      </form>
    {/if}
  {/if}
</Modal>

<style>
  .progreso {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    margin: var(--sp-3) 0 var(--sp-4);
    padding: 0;
    list-style: none;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .progreso li {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .progreso .n {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    font-size: var(--fs-xs);
    font-weight: 600;
    background: var(--surface-3);
    border-radius: 999px;
  }
  .progreso .on {
    color: var(--text-1);
    font-weight: 500;
  }
  .progreso .on .n {
    color: var(--accent-contrast);
    background: var(--accent);
  }
  .progreso .hecho .n {
    color: var(--accent-contrast);
    background: var(--ok);
  }
  .guia {
    margin: 0;
    padding-left: 1.3em;
    font-size: var(--fs-sm);
    line-height: 1.6;
    color: var(--text-2);
  }
  .mono {
    font-family: var(--mono, ui-monospace, monospace);
  }
  .otra {
    align-self: flex-start;
    margin-top: 4px;
    font-size: var(--fs-sm);
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
</style>
