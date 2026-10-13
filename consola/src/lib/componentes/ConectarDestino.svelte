<script lang="ts">
  // El formulario de un destino de nube o de red (plan 0.8.0, B2; plan-mejoras 14b):
  // el mismo en «Nuevo destino» (Repositorios y destinos), en «Conectar otro destino»
  // de un almacén y en el paso «Adónde» del espejo. Todas las nubes al mismo nivel.
  //
  // - **Bucket y carpeta base en campos separados** («mi-bucket» y «Resguardo»): el
  //   destino los recuerda (catálogo del cliente, en claro y sin secretos), así que el
  //   espejo y los repositorios nuevos no los vuelven a pedir. Con ellos el equipo
  //   prueba que entra al conectar (no hay otra «carpeta para probar»).
  // - **Dónde**: B2, S3 o un servidor de copias se pueden dejar solo en la lista (las
  //   credenciales se piden al crear un repositorio y van selladas para ese equipo), o
  //   conectar ya en un almacén para sus espejos. Dropbox, SFTP, un NAS (SMB) y WebDAV
  //   se conectan en un equipo.
  // - Al conectar, los datos viajan SELLADOS para el equipo en `conectar_nube` (clave de
  //   administración): el servidor no los ve. El equipo prueba que entra antes de guardarlos.
  // - Aviso si ya hay un destino con ese bucket o ese servidor («Ya tienes este bucket como …»).
  import { onDestroy } from "svelte";
  import { Check, Cloud, LoaderCircle, Server, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { avisar } from "$lib/avisos.svelte";
  import { actual, cargarCliente } from "$lib/estado.svelte";
  import { nombreNubeValido } from "$lib/nubes";
  import { admiteMasDestinos, CAMPOS_DESTINO, errorCampoDestino, TIPOS_NUBE } from "$lib/espejo";
  import { ErrorLlavesCambiadas, mandarOrden } from "$lib/ordenar";
  import { catalogoDe, cargarCatalogo, guardarEnCatalogo } from "$lib/catalogoDestinos.svelte";
  import {
    almacenesDe,
    avisoRepetido,
    claveNube,
    destinosDelCliente,
    errorBucket,
    errorDondeCatalogo,
    errorNombreDestino,
    errorSubcarpeta,
    idDestinoNuevo,
    unirCarpeta,
  } from "$lib/destinos";
  import type { Cliente, Equipo, Orden } from "$lib/tipos";
  import AlertaLlaves from "./AlertaLlaves.svelte";
  import Ayuda from "./Ayuda.svelte";
  import CampoClave from "./CampoClave.svelte";
  import ConectarNube from "./ConectarNube.svelte";

  type Tipo = "b2" | "s3" | "rest" | "dropbox" | "sftp" | "smb" | "webdav";
  interface Props {
    cliente: Cliente;
    /** Dónde se conecta (un almacén, o el equipo de un espejo propio). Sin él, se elige (o solo la lista). */
    equipo?: Equipo | null;
    /** Lo ya sabido: p. ej. un destino del catálogo que aún no está conectado en quien hace el espejo. */
    inicial?: { tipo?: string; nombre?: string; bucket?: string | null; base?: string | null; servidor?: string | null } | null;
    onclose: () => void;
    /** Cuando el equipo ya la tiene conectada (para elegirla ya en el espejo). */
    alConectar?: (nombre: string) => void;
  }
  let { cliente, equipo = null, inicial = null, onclose, alConectar }: Props = $props();

  const TIPOS: { tipo: Tipo; texto: string; detalle: string }[] = [
    { tipo: "b2", texto: "Backblaze B2", detalle: "Bucket; inmutable con bloqueo de objetos" },
    { tipo: "s3", texto: "S3 compatible", detalle: "Wasabi, R2, MinIO, AWS…" },
    { tipo: "dropbox", texto: "Dropbox", detalle: "Solo su carpeta de aplicación" },
    { tipo: "smb", texto: "NAS (carpeta de red)", detalle: "Con su propio usuario" },
    { tipo: "sftp", texto: "SFTP", detalle: "Un servidor con SSH" },
    { tipo: "webdav", texto: "WebDAV", detalle: "Nextcloud, ownCloud…" },
    { tipo: "rest", texto: "Servidor de copias de fuera", detalle: "Otro servidor de copias (https)" },
  ];
  /** Los que se pueden dejar solo en la lista (sin conectarlos aún en ningún equipo). */
  const SOLO_LISTA: Tipo[] = ["b2", "s3", "rest"];
  /** Los que llevan bucket (o recurso) delante de la carpeta base. */
  const CON_BUCKET: Tipo[] = ["b2", "s3", "smb"];

  const equipos = $derived(actual.equipos);
  const almacenes = $derived(almacenesDe(equipos));
  /** Dónde se puede conectar: el equipo dado o los almacenes del cliente con un agente que lo admite. */
  const conectables = $derived(equipo ? [equipo] : almacenes.filter((a) => tipo === "dropbox" || admiteMasDestinos(a)));
  const tiposPosibles = $derived(TIPOS.filter((t) => (equipo ? t.tipo !== "rest" && (t.tipo === "dropbox" || admiteMasDestinos(equipo)) : true)));

  const tipoInicial = (): Tipo => {
    const t = inicial?.tipo as Tipo | undefined;
    if (t && TIPOS.some((x) => x.tipo === t)) return t;
    // Un equipo con un agente que solo conecta Dropbox.
    return equipo && !admiteMasDestinos(equipo) ? "dropbox" : "b2";
  };
  let tipo = $state<Tipo>(tipoInicial());
  // svelte-ignore state_referenced_locally
  let nombre = $state(inicial?.nombre ?? "");
  // svelte-ignore state_referenced_locally
  let bucket = $state(inicial?.bucket ?? "");
  // svelte-ignore state_referenced_locally
  let base = $state(inicial?.base ?? "Resguardo");
  /** S3: el servidor (endpoint); un servidor de copias: su dirección. */
  // svelte-ignore state_referenced_locally
  let servidor = $state(inicial?.servidor ?? "");
  /** «lista» (solo el catálogo) o el id del equipo donde se conecta. */
  const modoInicial = () => (equipo ? equipo.id : "lista");
  let modo = $state(modoInicial());
  const conectarEn = $derived(modo === "lista" ? null : (conectables.find((e) => e.id === modo) ?? null));
  /** Todos los campos del tipo, vacíos (CampoClave no admite `bind:value` a algo sin definir). */
  const camposDe = (t: Tipo) => (t === "b2" || t === "s3" || t === "sftp" || t === "smb" || t === "webdav" ? CAMPOS_DESTINO[t] : []);
  const vacios = (t: Tipo) => Object.fromEntries(camposDe(t).map((c) => [c.clave, ""])) as Record<string, string>;
  let valores = $state<Record<string, string>>(vacios(tipoInicial()));
  let claveAdmin = $state("");
  let ocupado = $state(false);
  let pasoTxt = $state("");
  let error = $state("");
  let cambiadas = $state(false);
  let hecho = $state<string | null>(null);
  /** Dropbox: el flujo de su permiso (en el equipo elegido). */
  let dropboxEn = $state<Equipo | null>(null);

  // svelte-ignore state_referenced_locally
  void cargarCatalogo(cliente.id);
  const vistas = $derived(destinosDelCliente(equipos, catalogoDe(cliente.id)));

  onDestroy(() => {
    claveAdmin = "";
    valores = {};
  });

  function cambiarTipo(t: Tipo) {
    tipo = t;
    // Los datos de un tipo no pasan a otro (sobre todo las contraseñas).
    valores = vacios(t);
    error = "";
    if (!SOLO_LISTA.includes(t) && modo === "lista") modo = conectables[0]?.id ?? "lista";
    if (t === "rest") modo = "lista";
  }

  const textoTipo = $derived(TIPOS.find((t) => t.tipo === tipo)?.texto ?? tipo);
  const nombreFinal = $derived(nombre.trim() || `${textoTipo} ${cliente.nombre}`.slice(0, 40));
  /** El host de S3 (sin https://): lo que va delante del bucket en la dirección del catálogo. */
  const hostS3 = $derived(servidor.trim().replace(/^https?:\/\//i, "").replace(/\/+$/, "") || "s3.amazonaws.com");
  /** La dirección que recuerda el catálogo: el bucket (B2), «servidor/bucket» (S3), el recurso (SMB) o la del servidor de copias. */
  const donde = $derived(tipo === "b2" ? bucket.trim() : tipo === "s3" ? `${hostS3}/${bucket.trim()}` : tipo === "smb" ? bucket.trim() : tipo === "rest" ? servidor.trim() : "");
  /** La carpeta donde quedará todo (y donde el equipo prueba que entra). */
  const carpetaFinal = $derived(CON_BUCKET.includes(tipo) ? unirCarpeta(bucket, base) : unirCarpeta(base));

  const existentes = $derived(conectarEn ? [...(conectarEn.resumen?.guarda_copias?.nubes ?? []), ...(conectarEn.resumen?.nubes ?? [])].map((n) => n.nombre.toLowerCase()) : []);
  const errorNombre = $derived(
    modo === "lista"
      ? errorNombreDestino(nombreFinal)
      : !nombreNubeValido(nombreFinal)
        ? "Usa letras, números, espacios, guiones o puntos (hasta 40)."
        : existentes.includes(nombreFinal.toLowerCase())
          ? `Ya hay un destino con ese nombre en ${conectarEn?.nombre}.`
          : null,
  );
  const errorDeBucket = $derived(CON_BUCKET.includes(tipo) ? errorBucket(tipo, bucket) : null);
  const errorDeBase = $derived(errorSubcarpeta(base));
  const errorServidor = $derived(tipo === "rest" ? errorDondeCatalogo("rest", servidor) || (/^https:\/\//i.test(servidor.trim()) ? null : "Tiene que empezar por https://.") : tipo === "s3" && servidor.trim() ? errorCampoDestino("s3", "endpoint", servidor) : null);
  /** Los campos de las credenciales (solo al conectar): en S3 el servidor ya está arriba. */
  const campos = $derived(conectarEn ? camposDe(tipo).filter((c) => !(tipo === "s3" && c.clave === "endpoint")) : []);
  const errores = $derived(Object.fromEntries(campos.map((c) => [c.clave, errorCampoDestino(tipo as keyof typeof CAMPOS_DESTINO, c.clave, valores[c.clave] ?? "")])));
  const repetido = $derived(donde ? avisoRepetido(vistas, tipo === "smb" ? "" : tipo, donde) : null);
  const valido = $derived(
    !errorNombre &&
      !errorDeBucket &&
      !errorDeBase &&
      !errorServidor &&
      (modo === "lista" ? SOLO_LISTA.includes(tipo) : !!conectarEn && (tipo === "dropbox" || (Object.values(errores).every((e) => !e) && !!claveAdmin))),
  );
  /** Lo que falta para seguir (el botón apagado lo dice). */
  const falta = $derived(
    errorNombre
      ? "el nombre"
      : errorDeBucket
        ? tipo === "smb"
          ? "el recurso compartido"
          : "el bucket"
        : errorDeBase
          ? "la carpeta base"
          : errorServidor
            ? "la dirección"
            : modo !== "lista" && !conectarEn
              ? "dónde conectarlo"
              : modo !== "lista" && tipo !== "dropbox" && Object.values(errores).some((e) => !!e)
                ? "las credenciales"
                : modo !== "lista" && tipo !== "dropbox" && !claveAdmin
                  ? "la clave de administración"
                  : null,
  );

  async function respuesta(eq: Equipo, o: Orden): Promise<Orden> {
    for (let i = 0; i < 80; i++) {
      const x = (await api.ordenesEquipo(cliente.id, eq.id, 10)).find((y) => y.id === o.id);
      if (x && ["hecha", "fallida", "rechazada", "cancelada", "caducada"].includes(x.estado)) return x;
      await new Promise((r) => setTimeout(r, 1500));
    }
    throw new Error(`${eq.nombre} no ha respondido todavía. Mira sus órdenes en un momento.`);
  }

  /** Lo que recuerda el catálogo de una nube conectada (bucket y carpeta base). Un servidor anterior no lo guarda: no pasa nada. */
  async function recordar(eq: Equipo, n: string) {
    try {
      await guardarEnCatalogo(cliente.id, claveNube(eq.id, n), { nombre: n, tipo: "nube", donde: donde || null, carpeta: unirCarpeta(base) || null });
    } catch {
      /* el espejo pedirá el bucket la primera vez */
    }
  }

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    if (!valido) return;
    error = "";
    if (modo === "lista") {
      ocupado = true;
      try {
        await guardarEnCatalogo(cliente.id, idDestinoNuevo(), { nombre: nombreFinal, tipo: tipo as "b2" | "s3" | "rest", donde, carpeta: tipo === "rest" ? null : unirCarpeta(base) || null });
        hecho = `«${nombreFinal}» ya está en la lista de destinos. Elígelo al crear un repositorio; las credenciales se piden entonces, solo para ese equipo.`;
        avisar(`«${nombreFinal}» ya está en la lista de destinos.`);
      } catch (err) {
        error = (err as Error).message;
      } finally {
        ocupado = false;
      }
      return;
    }
    const eq = conectarEn!;
    if (tipo === "dropbox") {
      dropboxEn = eq;
      return;
    }
    ocupado = true;
    try {
      const parametros: Record<string, string> = {};
      for (const c of campos) {
        const v = c.secreto ? (valores[c.clave] ?? "") : (valores[c.clave] ?? "").trim();
        if (v) parametros[c.clave] = v;
      }
      if (tipo === "s3" && servidor.trim()) parametros.endpoint = servidor.trim();
      const o = await mandarOrden({
        cliente,
        equipo: eq,
        tipo: "conectar_nube",
        cuerpo: { tipo, nombre: nombreFinal, parametros, ...(carpetaFinal ? { carpeta_prueba: carpetaFinal } : {}) },
        secretos: { claveAdmin },
        alPaso: (t) => (pasoTxt = t),
      });
      pasoTxt = `${eq.nombre} está probando que entra…`;
      const r = await respuesta(eq, o);
      if (r.estado !== "hecha") throw new Error(r.mensaje ?? `${eq.nombre} no pudo conectar ese destino.`);
      claveAdmin = "";
      valores = vacios(tipo);
      await recordar(eq, nombreFinal);
      hecho = `«${nombreFinal}» está conectado en ${eq.nombre} y el equipo ha podido entrar${carpetaFinal ? ` en ${carpetaFinal}` : ""}. Ya puedes usarlo en un espejo.`;
      avisar(`«${nombreFinal}» conectado en ${eq.nombre}.`);
      void cargarCliente(cliente.id, { silencioso: true });
      alConectar?.(nombreFinal);
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) cambiadas = true;
      else error = (err as Error).message;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }
</script>

{#if dropboxEn}
  <ConectarNube
    {cliente}
    equipo={dropboxEn}
    nombreInicial={nombreFinal}
    onclose={() => ((dropboxEn = null), onclose())}
    alConectar={(n) => {
      void recordar(dropboxEn!, n);
      alConectar?.(n);
    }}
  />
{:else}
  <Modal labelledby="t-destino" {onclose} width={600} dismissible={false}>
    <div class="dlg-title">
      <span class="ticon"><Server size={18} /></span>
      <div>
        <h2 id="t-destino">{equipo ? `Conectar un destino en ${equipo.nombre}` : "Nube o servidor"}</h2>
        <p>
          {equipo
            ? "Para sus espejos. Los datos viajan cifrados solo para el equipo y se guardan protegidos allí: el servidor no los ve."
            : "El destino recuerda su bucket y su carpeta: el espejo y los repositorios nuevos no te los vuelven a pedir."}
          <Ayuda id="catalogo-destinos" />
        </p>
      </div>
    </div>

    {#if hecho}
      <div class="form">
        <div class="notice notice-success" role="status"><Check size={16} /><p>{hecho}</p></div>
        <footer><button class="btn btn-primary" onclick={onclose}>Hecho</button></footer>
      </div>
    {:else}
      <form class="form" onsubmit={guardar}>
        <div class="field">
          <span class="field-label" id="cd-tipo-l">Qué es</span>
          <div class="tipos" role="radiogroup" aria-labelledby="cd-tipo-l">
            {#each tiposPosibles as t (t.tipo)}
              <label class="tipo" class:on={tipo === t.tipo}>
                <input type="radio" name="cd-tipo" checked={tipo === t.tipo} onchange={() => cambiarTipo(t.tipo)} />
                <Cloud size={14} /><span><strong>{t.texto}</strong><span class="faint">{t.detalle}</span></span>
              </label>
            {/each}
          </div>
          <span class="field-hint">
            {#if TIPOS_NUBE[tipo]?.inmutable}Con bloqueo de objetos (Object Lock) en el bucket es inmutable: márcalo después en «Tipo y marcas».{:else if tipo === "rest"}Un servidor de copias de fuera, en solo añadir si lo tiene así.{:else}No es inmutable: quien tenga sus credenciales puede borrar lo copiado. Conviene que otro destino sí lo sea.{/if}
          </span>
        </div>

        <div class="field">
          <label class="field-label" for="cd-nombre">Nombre</label>
          <input id="cd-nombre" class="input" bind:value={nombre} maxlength="80" placeholder={nombreFinal} />
          {#if errorNombre && nombre}<p class="error-campo">{errorNombre}</p>{:else}<span class="field-hint">Así lo verás en la lista de destinos y en el espejo.</span>{/if}
        </div>

        {#if tipo === "s3" || tipo === "rest"}
          <div class="field">
            <label class="field-label" for="cd-servidor">{tipo === "rest" ? "Dirección" : "Servidor (endpoint)"}{#if tipo === "s3"}<span class="faint"> (vacío en AWS)</span>{/if}</label>
            <input id="cd-servidor" class="input mono" bind:value={servidor} spellcheck="false" placeholder={tipo === "rest" ? "https://copias.ejemplo.com:8000" : "https://s3.eu-central-1.ejemplo.com"} />
            {#if errorServidor && servidor}<p class="error-campo">{errorServidor}</p>{/if}
          </div>
        {/if}
        {#if CON_BUCKET.includes(tipo) || tipo !== "rest"}
          <div class="fila-campos">
            {#if CON_BUCKET.includes(tipo)}
              <div class="field">
                <label class="field-label" for="cd-bucket">{tipo === "smb" ? "Recurso compartido" : "Bucket"}</label>
                <input id="cd-bucket" class="input mono" bind:value={bucket} spellcheck="false" placeholder={tipo === "smb" ? "copias" : "mi-bucket"} />
                {#if errorDeBucket && bucket}<p class="error-campo">{errorDeBucket}</p>{/if}
              </div>
            {/if}
            <div class="field">
              <label class="field-label" for="cd-base">Carpeta base <span class="faint">(opcional)</span></label>
              <input id="cd-base" class="input mono" bind:value={base} spellcheck="false" placeholder="Resguardo" />
              {#if errorDeBase}<p class="error-campo">{errorDeBase}</p>{/if}
            </div>
          </div>
          <p class="ruta faint">Todo irá en <code>{carpetaFinal || "(la raíz)"}/…</code>{#if tipo === "dropbox"}, dentro de Aplicaciones/Resguardo{/if}.</p>
        {/if}
        {#if repetido}<div class="notice notice-warn" role="status"><TriangleAlert size={16} /><p>{repetido}</p></div>{/if}

        {#if !equipo}
          <div class="field">
            <span class="field-label" id="cd-modo-l">Dónde</span>
            <div class="opc" role="radiogroup" aria-labelledby="cd-modo-l">
              {#if SOLO_LISTA.includes(tipo)}
                <label class="radio"><input type="radio" name="cd-modo" value="lista" bind:group={modo} /><span>Solo en la lista<span class="faint">Las credenciales se piden al crear un repositorio en un equipo y van selladas solo para él.</span></span></label>
              {/if}
              {#each conectables as a (a.id)}
                {#if tipo !== "rest"}
                  <label class="radio"><input type="radio" name="cd-modo" value={a.id} bind:group={modo} /><span>Conectarlo ya en {a.nombre}<span class="faint">Para los espejos de ese almacén. {tipo === "dropbox" ? "Se abre Dropbox para dar el permiso." : "Pide sus credenciales y la clave de administración."}</span></span></label>
                {/if}
              {/each}
              {#if !SOLO_LISTA.includes(tipo) && !conectables.length}
                <p class="faint">{almacenes.length ? "Los almacenes de este cliente aún no pueden conectar otros destinos: actualiza su agente." : "Este cliente no tiene ningún almacén donde conectarlo todavía."}</p>
              {/if}
            </div>
          </div>
        {/if}

        {#if conectarEn && tipo !== "dropbox"}
          {#each campos as c (tipo + c.clave)}
            {#if c.secreto}
              <CampoClave requerido id="cd-{c.clave}" etiqueta={c.etiqueta} bind:value={valores[c.clave]} ayuda={c.ayuda} />
            {:else}
              <div class="field">
                <label class="field-label" for="cd-{c.clave}">{c.etiqueta}{#if c.opcional}<span class="faint"> (opcional)</span>{/if}</label>
                {#if c.opciones}
                  <select id="cd-{c.clave}" class="input" bind:value={valores[c.clave]}>
                    <option value="">—</option>
                    {#each c.opciones as op (op)}<option value={op}>{op}</option>{/each}
                  </select>
                {:else if c.largo}
                  <textarea id="cd-{c.clave}" class="input mono" rows="3" bind:value={valores[c.clave]} placeholder={c.ejemplo} spellcheck="false"></textarea>
                {:else}
                  <input id="cd-{c.clave}" class="input" class:mono={c.clave !== "usuario"} bind:value={valores[c.clave]} placeholder={c.ejemplo} autocomplete="off" spellcheck="false" />
                {/if}
                {#if valores[c.clave] && errores[c.clave]}<p class="error-campo">{errores[c.clave]}</p>{:else if c.ayuda}<span class="field-hint">{c.ayuda}</span>{/if}
              </div>
            {/if}
          {/each}
          <CampoClave requerido id="cd-clave" etiqueta="Clave de administración" bind:value={claveAdmin}>
            {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
          </CampoClave>
        {/if}
        {#if cambiadas && conectarEn}<AlertaLlaves equipo={conectarEn} cliente={cliente.id} />{/if}
        {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
        <footer>
          {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{:else if falta}<span class="falta faint">Falta {falta}.</span>{/if}
          <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
          <button class="btn btn-primary" disabled={ocupado || !valido}>{modo === "lista" ? "Crear el destino" : tipo === "dropbox" ? "Seguir en Dropbox" : "Conectar y probar"}</button>
        </footer>
      </form>
    {/if}
  </Modal>
{/if}

<style>
  .mono {
    font-family: var(--mono, ui-monospace, monospace);
  }
  textarea.input {
    resize: vertical;
    min-height: 4.5rem;
  }
  .tipos {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 170px), 1fr));
    gap: var(--sp-2);
  }
  .tipo {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    font-size: var(--fs-sm);
  }
  .tipo input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .tipo:focus-within {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .tipo.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .tipo :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--text-3);
  }
  .tipo > span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .tipo .faint {
    font-size: var(--fs-xs);
  }
  .fila-campos {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 200px), 1fr));
    gap: var(--sp-3);
  }
  .ruta {
    margin: calc(-1 * var(--sp-2)) 0 0;
    font-size: var(--fs-sm);
    overflow-wrap: anywhere;
  }
  .opc {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .radio {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .radio > span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .espera,
  .falta {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
</style>
