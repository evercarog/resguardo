<script lang="ts">
  // «Conectar también a otra consola…» (docs/consolas-multiples.md): los
  // equipos de este cliente pasan a estar gestionados TAMBIÉN desde otra
  // consola (p. ej. la local y la en línea a la vez), sin dejar esta.
  // 1) Se pega el código de conexión que da la otra consola y se comprueban
  //    de palabra sus huellas (identidad y autoridad TLS).
  // 2) Con la clave de administración (una vez) se calcula la K_cfg de la otra
  //    consola (con SU sal) y se manda `anadir_consola`, sellada, a cada equipo:
  //    la ficha nunca pasa en claro por este servidor.
  // 3) Una lista enseña cómo va cada equipo hasta que todos terminan.
  // Desde el aviso «Equipos que no están en todas las consolas»
  // (AvisoConsolas.svelte) llega con la consola esperada (la que ya tienen los
  // demás equipos) y los equipos que le faltan ya elegidos.
  import { onDestroy } from "svelte";
  import { CircleCheck, CircleX, KeyRound, LoaderCircle, Monitor, ShieldCheck, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { urlAgentes } from "$lib/estado.svelte";
  import { ErrorLlavesCambiadas, mandarOrden } from "$lib/ordenar";
  import { argon2Navegador } from "$lib/cripto/argon2";
  import { aB64, borrar } from "$lib/cripto/bytes";
  import { kCfg, materialCliente } from "$lib/cripto/claves";
  import { cuerpoAnadir, hostDe, huellaConPuntos, leerCodigo, type CodigoConexion } from "$lib/conexion";
  import { huellaCorta } from "$lib/servidores";
  import { fechaLarga, plural } from "$lib/formato";
  import type { Cliente, Equipo, Orden } from "$lib/tipos";
  import Ayuda from "./Ayuda.svelte";
  import AlertaLlaves from "./AlertaLlaves.svelte";
  import CampoClave from "./CampoClave.svelte";

  let {
    cliente,
    equipos,
    onclose,
    alTerminar,
    esperada,
    soloEquipos,
  }: {
    cliente: Cliente;
    equipos: Equipo[];
    onclose: () => void;
    alTerminar?: () => void;
    /** La consola a la que se quiere conectar (la que ya tienen otros equipos del cliente). */
    esperada?: { identidad: string; nombre: string; url: string };
    /** Los equipos que se eligen de entrada (si no, todos los que se puede). */
    soloEquipos?: string[];
  } = $props();

  let paso = $state<"codigo" | "equipos" | "progreso">("codigo");
  let pegado = $state("");
  let comprobado = $state(false);
  let elegidos = $state<Record<string, boolean>>({});
  let claveAdmin = $state("");
  let ocupado = $state(false);
  let pasoTxt = $state("");
  let error = $state("");
  let cambiadas = $state<Equipo | null>(null);
  /** Lo mandado: equipo → orden (o el error al mandarla). */
  let enviadas = $state<{ equipo: Equipo; orden: Orden | null; error: string | null }[]>([]);
  let t: ReturnType<typeof setInterval> | undefined;

  const codigo = $derived(pegado.trim() ? leerCodigo(pegado, new Date(), [location.origin, urlAgentes()]) : null);
  const datos = $derived(codigo && typeof codigo !== "string" ? (codigo as CodigoConexion) : null);
  const activos = $derived(equipos.filter((e) => e.confirmado && e.modo === "gestionado"));
  /** ¿El código es de la consola esperada? (la identidad es lo que fijan los equipos) */
  const coincide = $derived(!!(esperada && datos && datos.identidad === esperada.identidad));
  /** Por qué un equipo no puede (o no hace falta): su agente es anterior, o ya está conectado. */
  const motivo = (e: Equipo) => {
    if (datos && e.resumen?.consolas?.some((x) => x.identidad === datos.identidad)) return "ya la tiene";
    if (!e.resumen) return "aún no ha informado: espera a que se conecte";
    if (!e.resumen.admite?.includes("consolas_multiples")) return "actualiza el agente";
    return null;
  };
  $effect(() => {
    for (const e of activos) if (!(e.id in elegidos)) elegidos[e.id] = !motivo(e) && (!soloEquipos || soloEquipos.includes(e.id));
  });
  const aConectar = $derived(activos.filter((e) => elegidos[e.id] && !motivo(e)));
  const terminadas = $derived(enviadas.filter((x) => x.error || (x.orden && ["hecha", "fallida", "rechazada", "cancelada", "caducada"].includes(x.orden.estado))).length);
  /** Repetirla en un equipo que ya la tiene es inofensivo: el equipo contesta «ya gestiona este equipo» y no cambia nada. */
  const yaEstaba = (o: Orden | null) => !!o && o.estado === "fallida" && /ya gestiona este equipo/i.test(o.mensaje ?? "");

  onDestroy(() => {
    claveAdmin = pegado = "";
    clearInterval(t);
  });

  async function seguir() {
    const ids = enviadas.filter((x) => x.orden).map((x) => x.orden!.id);
    if (!ids.length) return;
    try {
      const r = await api.ordenesCliente(cliente.id, { limite: 200 });
      enviadas = enviadas.map((x) => ({ ...x, orden: r.ordenes.find((o) => o.id === x.orden?.id) ?? x.orden }));
      if (terminadas === enviadas.length) {
        clearInterval(t);
        alTerminar?.();
      }
    } catch {
      /* se reintenta en la siguiente vuelta */
    }
  }

  async function conectar(e: SubmitEvent) {
    e.preventDefault();
    if (!datos) return;
    error = "";
    cambiadas = null;
    ocupado = true;
    let kcfg: Uint8Array | null = null;
    try {
      pasoTxt = "Preparando la clave de la otra consola…";
      // La K_cfg de la otra consola: misma clave, SU sal (el cliente allí puede tener otra).
      const material = await materialCliente(argon2Navegador, claveAdmin, datos.sal_cliente);
      kcfg = kCfg(material);
      borrar(material);
      const cuerpo = cuerpoAnadir(datos, aB64(kcfg), cliente.sal_cliente);
      const lista = aConectar.slice();
      enviadas = lista.map((equipo) => ({ equipo, orden: null, error: null }));
      paso = "progreso";
      for (const [i, eq] of lista.entries()) {
        pasoTxt = `${eq.nombre} (${i + 1} de ${lista.length})…`;
        try {
          const o = await mandarOrden({ cliente, equipo: eq, tipo: "anadir_consola", cuerpo, secretos: { claveAdmin }, alPaso: (x) => (pasoTxt = `${eq.nombre}: ${x.toLowerCase()}`) });
          enviadas[i] = { equipo: eq, orden: o, error: null };
        } catch (err) {
          if (err instanceof ErrorLlavesCambiadas) cambiadas = eq;
          enviadas[i] = { equipo: eq, orden: null, error: (err as Error).message };
          // Una clave equivocada falla igual en todos: se para aquí.
          if (i === 0) throw err;
        }
      }
      cuerpo.k_cfg = "";
      claveAdmin = pegado = "";
      t = setInterval(seguir, 3000);
      void seguir();
    } catch (err) {
      if (!(err instanceof ErrorLlavesCambiadas)) error = (err as Error).message;
      if (!enviadas.some((x) => x.orden)) paso = "equipos";
    } finally {
      if (kcfg) borrar(kcfg);
      ocupado = false;
      pasoTxt = "";
    }
  }

  const ESTADO: Record<string, string> = {
    pendiente: "Esperando al equipo (la recibe al conectarse)",
    entregada: "Recibida",
    en_marcha: "Conectando…",
    hecha: "Hecho",
    fallida: "Falló",
    rechazada: "Rechazada",
    cancelada: "Cancelada",
    caducada: "Caducó",
  };
</script>

<Modal labelledby="t-conectar" {onclose} width={620} dismissible={paso !== "equipos" || !ocupado}>
  <div class="dlg-title">
    <span class="ticon"><Monitor size={18} /></span>
    <div>
      <h2 id="t-conectar">{esperada ? `Conectar también a «${esperada.nombre}»` : "Conectar también a otra consola"}</h2>
      <p>Los equipos de «{cliente.nombre}» seguirán aquí y, además, se podrán gestionar desde la otra consola. Cada una funciona sola: si quitas una, la otra sigue igual.</p>
    </div>
  </div>

  {#if paso === "codigo"}
    <div class="form">
      <ol class="guia">
        {#if esperada}<li>Los demás equipos ya están en <strong>{esperada.nombre}</strong> ({hostDe(esperada.url)}). Pide allí un código de conexión para este cliente.</li>{/if}
        <li>En la <strong>otra consola</strong>: <strong>Clientes → Recibir un cliente → «Gestionarlo también desde aquí»</strong> (o, si el cliente ya existe allí, en su página <strong>Servidor → «Dar un código de conexión»</strong>).</li>
        <li>Te dará un <strong>código de conexión</strong> (empieza por «RGC1.»). Pégalo aquí.</li>
      </ol>
      <div class="field">
        <label class="field-label" for="codigo-conexion">Código de conexión</label>
        <textarea id="codigo-conexion" class="input mono" rows="3" bind:value={pegado} placeholder="RGC1.…" spellcheck="false" autocomplete="off"></textarea>
        {#if typeof codigo === "string"}<p class="error-campo">{codigo}</p>{/if}
      </div>
      {#if datos}
        <div class="ficha-consola">
          <div><span class="faint">Consola</span><strong>{datos.nombre}</strong></div>
          <div><span class="faint">Dirección</span><code>{datos.url}</code></div>
          <div><span class="faint">Identidad</span><code class="huella">{huellaCorta(datos.identidad)}</code></div>
          <div><span class="faint">Autoridad TLS</span><code class="huella">{huellaConPuntos(datos.huella_ca)}</code></div>
          {#if datos.cliente}<div><span class="faint">Cliente allí</span><span>{datos.cliente}</span></div>{/if}
          <div><span class="faint">Vale hasta</span><span>{fechaLarga(datos.caduca)}</span></div>
        </div>
        {#if esperada && coincide}
          <div class="notice notice-success">
            <CircleCheck size={16} />
            <p>Es <strong>{esperada.nombre}</strong>: la misma identidad que ya tienen fijada los demás equipos de este cliente.</p>
          </div>
        {:else if esperada}
          <div class="notice notice-warn" role="alert">
            <TriangleAlert size={16} />
            <p>Este código <strong>no es de {esperada.nombre}</strong>: es de otra consola (otra identidad). Si no esperabas conectar los equipos a otra, no sigas.</p>
          </div>
        {/if}
        <div class="notice notice-warn">
          <ShieldCheck size={16} />
          <p>Compara de palabra (por teléfono, en persona) la <strong>identidad</strong> y la <strong>autoridad TLS</strong> con quien administra la otra consola. Esa consola verá los nombres y el estado de los equipos, como esta; nunca las contraseñas ni los archivos.</p>
        </div>
        <label class="check"><input type="checkbox" bind:checked={comprobado} />He comprobado las huellas: es la consola que esperaba.</label>
      {/if}
      <footer>
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
        <button type="button" class="btn btn-primary" disabled={!datos || !comprobado} onclick={() => (paso = "equipos")}>Seguir</button>
      </footer>
    </div>
  {:else if paso === "equipos" && datos}
    <form class="form" onsubmit={conectar}>
      <p class="resumen-dest">Se conectarán a <strong>{datos.nombre}</strong> ({hostDe(datos.url)}), identidad <span class="pastilla mono">{huellaCorta(datos.identidad)}</span>.</p>
      <fieldset class="field equipos">
        <legend class="field-label">Equipos</legend>
        {#each activos as e (e.id)}
          {@const m = motivo(e)}
          <label class="check" class:apagado={!!m}>
            <input type="checkbox" bind:checked={elegidos[e.id]} disabled={!!m} />{e.nombre}
            {#if m}<span class="faint">{" · "}{m}</span>{:else if !e.conectado}<span class="faint">{" · "}desconectado: la recibirá al volver (7 días)</span>{/if}
          </label>
        {:else}
          <p class="faint">No hay equipos gestionados desde aquí.</p>
        {/each}
      </fieldset>
      <CampoClave requerido id="clave-admin-conectar" etiqueta="Clave de administración" bind:value={claveAdmin}>
        {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
      </CampoClave>
      <p class="faint pequeno">Se escribe una sola vez: sirve para todos los equipos y para preparar la clave de la otra consola. No se guarda ni sale de este navegador.</p>
      {#if cambiadas}<AlertaLlaves equipo={cambiadas} cliente={cliente.id} />{/if}
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={() => (paso = "codigo")}>Atrás</button>
        <button class="btn btn-primary" disabled={ocupado || !aConectar.length || !claveAdmin}>
          <KeyRound size={15} />Conectar {plural(aConectar.length, "equipo", "equipos")}
        </button>
      </footer>
    </form>
  {:else}
    <div class="form">
      <ul class="progreso" aria-live="polite">
        {#each enviadas as x (x.equipo.id)}
          {@const est = x.error ? "fallida" : yaEstaba(x.orden) ? "hecha" : (x.orden?.estado ?? "enviando")}
          <li>
            <span class="icono">
              {#if est === "hecha"}<CircleCheck size={16} class="ok" />
              {:else if ["fallida", "rechazada", "cancelada", "caducada"].includes(est)}<CircleX size={16} class="mal" />
              {:else}<LoaderCircle size={16} class="spin" />{/if}
            </span>
            <span class="nombre">{x.equipo.nombre}</span>
            <span class="faint">{x.error ?? (yaEstaba(x.orden) ? "Ya estaba conectado: no hacía falta nada" : x.orden ? (ESTADO[x.orden.estado] ?? x.orden.estado) + (x.orden.mensaje && x.orden.estado !== "hecha" ? ` · ${x.orden.mensaje}` : "") : "Enviando…")}</span>
          </li>
        {/each}
      </ul>
      {#if ocupado}<p class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</p>{/if}
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      {#if !ocupado && terminadas < enviadas.length}
        <p class="faint pequeno">Puedes cerrar: cada equipo se conecta en cuanto recibe la orden (dura 7 días). Lo verás en su página («También lo gestiona») y en «Servidor».</p>
      {/if}
      <footer>
        <button class="btn btn-primary" disabled={ocupado} onclick={onclose}>{terminadas === enviadas.length ? "Hecho" : "Cerrar"}</button>
      </footer>
    </div>
  {/if}
</Modal>

<style>
  .guia {
    margin: 0;
    padding-left: 1.3em;
    color: var(--text-2);
    font-size: var(--fs-sm);
    line-height: 1.6;
  }
  .mono {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 12px;
    word-break: break-all;
  }
  .ficha-consola {
    display: grid;
    gap: 6px;
    padding: 10px 12px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius, 8px);
    font-size: var(--fs-sm);
  }
  .ficha-consola > div {
    display: grid;
    grid-template-columns: 8.5em 1fr;
    gap: 8px;
    align-items: baseline;
  }
  .ficha-consola code {
    word-break: break-all;
  }
  .huella {
    font-size: 12px;
    letter-spacing: 0.02em;
  }
  .resumen-dest {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .equipos {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    border: none;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .apagado {
    color: var(--text-3, var(--text-2));
  }
  .pequeno {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .progreso {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
    border-top: 1px solid var(--border);
  }
  .progreso li {
    display: grid;
    grid-template-columns: 20px minmax(0, 10em) 1fr;
    gap: 8px;
    align-items: center;
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
    font-size: var(--fs-sm);
  }
  .progreso .nombre {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .icono {
    display: inline-flex;
  }
  .icono :global(.ok) {
    color: var(--ok, #16a34a);
  }
  .icono :global(.mal) {
    color: var(--bad, #dc2626);
  }
  @media (max-width: 520px) {
    .ficha-consola > div {
      grid-template-columns: 1fr;
      gap: 0;
    }
    .progreso li {
      grid-template-columns: 20px 1fr;
    }
    .progreso li > .faint {
      grid-column: 2;
    }
  }
</style>
