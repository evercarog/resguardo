<script lang="ts">
  // «Instalar muchos equipos» (bloque 7 de la 0.7.26): un código para varios equipos (lo genera
  // este navegador; el servidor solo guarda su hash), la línea de PowerShell y la de Linux para
  // pegar en cada equipo, y «Equipos esperando confirmación». Nada se da de alta solo: cada equipo
  // enseña su número de comprobación y aquí se marca, después de compararlo, uno a uno o varios.
  import Migas from "$lib/componentes/Migas.svelte";
  import { onDestroy, untrack } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { Ban, Check, Copy, KeyRound, Layers, LoaderCircle, Monitor, Plus, Shuffle, Terminal, TriangleAlert, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { enFondo } from "$lib/actividad.svelte";
  import { actual, app, cargarCliente, reloj } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { argon2Navegador } from "$lib/cripto/argon2";
  import { aleatorio, borrar } from "$lib/cripto/bytes";
  import { etiquetaEquipo, etiquetaValida, hashCodigo, kCfg, materialCliente, normalizarCodigo } from "$lib/cripto/claves";
  import { Codigos, codigoDeHash, generarCodigo, LARGO_PREPARADO } from "$lib/codigo";
  import { mandarOrden } from "$lib/ordenar";
  import { cuentaAtras } from "$lib/formato";
  import {
    aConfirmar,
    comandoCodificado,
    DIAS_POR_DEFECTO,
    errorNombreLote,
    errorUsosDias,
    esperando,
    lineaLinuxVarios,
    lineaPowerShell,
    MAX_DIAS,
    MAX_USOS,
    resumenLote,
    revisar,
    USOS_POR_DEFECTO,
    type Lote,
    type LoteDetalle,
    type Revision,
  } from "$lib/despliegue";
  import { esperaDe, mensajeAlPedir } from "$lib/emparejar";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import CampoClave from "$lib/componentes/CampoClave.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import BotonCargando from "$lib/componentes/BotonCargando.svelte";

  const c = $derived(actual.id);
  const soportado = $derived(app.servidor?.codigo_varios === true);
  const codigos = new Codigos();
  const loteId = $derived(page.url.searchParams.get("l"));

  // --- Lista y creación ---------------------------------------------------------
  let lotes = $state<Lote[]>([]);
  let usos = $state(USOS_POR_DEFECTO);
  let dias = $state(DIAS_POR_DEFECTO);
  let nombre = $state("");
  let creando = $state(false);
  let error = $state("");
  const errorForm = $derived(errorUsosDias(Number(usos), Number(dias)) ?? errorNombreLote(nombre));

  const cargarLista = (cc: string) =>
    api.codigosVarios(cc).then(
      (x) => cc === c && (lotes = x),
      () => {},
    );
  $effect(() => {
    const cc = c;
    if (!cc || !soportado) return;
    untrack(() => void cargarLista(cc));
  });

  async function crear(e: SubmitEvent) {
    e.preventDefault();
    if (errorForm) return;
    error = "";
    creando = true;
    try {
      for (let intento = 0; ; intento++) {
        const codigo = generarCodigo(LARGO_PREPARADO);
        try {
          const l = await api.crearCodigoVarios(c, { codigo_hash: hashCodigo(codigo), usos: Number(usos), dias: Number(dias), nombre: nombre.trim() || null });
          // Hasta que caduca y 2 días más (para confirmar a los últimos que se unan).
          codigos.guardar({ id: l.id, cliente: c, codigo, lote: true, hasta: Date.parse(l.caduca) + 2 * 86_400_000 });
          nombre = "";
          await goto(`/c/${c}/emparejar/varios?l=${l.id}`);
          return;
        } catch (err) {
          // Un hash repetido (rarísimo): otro código, una vez.
          if ((err as { estado?: number }).estado !== 409 || (err as { codigo?: string }).codigo !== "codigo_repetido" || intento > 0) throw err;
        }
      }
    } catch (err) {
      error = esperaDe(err) ? mensajeAlPedir(err) : (err as Error).message;
    } finally {
      creando = false;
    }
  }

  // --- Un código: detalle, líneas y los que esperan --------------------------------
  let detalle = $state<LoteDetalle | null>(null);
  let noExiste = $state(false);
  let sondeo: ReturnType<typeof setInterval> | null = null;
  const cargarDetalle = async (cc: string, l: string) => {
    try {
      const d = await api.codigoVarios(cc, l);
      if (cc !== c || l !== loteId) return;
      detalle = d;
      noExiste = false;
      // Ya no sirve y nadie espera: el código no hace falta en este navegador.
      if (d.estado !== "activo" && !esperando(d.equipos).length) codigos.olvidar(l);
    } catch (e) {
      if ((e as { estado?: number }).estado === 404) noExiste = true;
    }
  };
  $effect(() => {
    const cc = c;
    const l = loteId;
    if (!cc || !l || !soportado) return;
    untrack(() => {
      detalle = null;
      marcados = [];
      void cargarDetalle(cc, l);
    });
    sondeo = setInterval(() => document.visibilityState === "visible" && void enFondo(() => cargarDetalle(cc, l)), 3000);
    return () => {
      if (sondeo) clearInterval(sondeo);
      sondeo = null;
    };
  });

  /** El código, si lo tiene este navegador (comprobado con su hash). */
  let escrito = $state("");
  let codigoVersion = $state(0);
  const codigo = $derived.by(() => {
    void codigoVersion;
    return detalle ? codigos.de(detalle.id, detalle.codigo_hash) : null;
  });
  function guardarEscrito() {
    if (!detalle) return;
    if (!codigoDeHash(escrito, detalle.codigo_hash)) {
      error = "Ese no es el código. Escríbelo tal cual (da igual mayúsculas, espacios y guiones).";
      return;
    }
    error = "";
    codigos.guardar({ id: detalle.id, cliente: c, codigo: normalizarCodigo(escrito).replace(/(.{4})(?=.)/g, "$1-"), lote: true, hasta: Date.parse(detalle.caduca) + 2 * 86_400_000 });
    escrito = "";
    codigoVersion++;
  }

  // La dirección con la que los equipos llegan a este servidor (como en «Añadir equipo»).
  const origenHttps = typeof location !== "undefined" && location.protocol === "https:";
  let servidorUrl = $state(app.servidor?.url_agentes || (origenHttps ? location.origin : ""));
  let editarServidor = $state(false);
  const servidorLimpio = $derived(servidorUrl.trim().replace(/\/+$/, ""));
  const servidorOk = $derived(/^https:\/\/[A-Za-z0-9.\-:[\]]+$/.test(servidorLimpio));

  // El SHA-256 del instalador que sirve este servidor (va fijado en la línea de PowerShell).
  let huella = $state<{ sha256: string; bytes: number } | null>(null);
  let sinInstalador = $state(false);
  $effect(() => {
    const cc = c;
    if (!cc || !soportado || !loteId) return;
    if (app.servidor?.instalador_agente !== true) {
      sinInstalador = true;
      return;
    }
    untrack(() =>
      api.huellaInstalador(cc).then(
        (h) => cc === c && ((huella = h), (sinInstalador = false)),
        () => (sinInstalador = true),
      ),
    );
  });

  const activo = $derived(detalle?.estado === "activo");
  const linea = $derived(detalle && codigo && huella && servidorOk ? lineaPowerShell({ servidor: servidorLimpio, lote: detalle.id, sha256: huella.sha256, codigo }) : "");
  const lineaDespliegue = $derived(comandoCodificado(linea));
  const lineaLinux = $derived(codigo && servidorOk && app.servidor ? lineaLinuxVarios(codigo, servidorLimpio, app.servidor.huella_ca) : "");

  async function copiar(t: string, que: string) {
    await navigator.clipboard.writeText(t);
    avisar(`${que} copiada.`);
  }

  // «Anular el código»
  let anulando = $state(false);
  async function anular() {
    if (!detalle) return;
    try {
      await api.anularCodigoVarios(c, detalle.id);
      avisar("Anulado: ningún equipo más podrá unirse con ese código.");
      await cargarDetalle(c, detalle.id);
      void cargarLista(c);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      anulando = false;
    }
  }

  // --- Equipos esperando confirmación -------------------------------------------
  const revisiones = $derived<Revision[]>(detalle && app.servidor ? esperando(detalle.equipos).map((e) => revisar(e, app.servidor!.identidad, app.servidor!.huella_ca)) : []);
  const hechos = $derived(detalle ? detalle.equipos.filter((e) => !esperando([e]).length) : []);
  /** Los que la persona ha marcado después de comparar su número (empieza sin ninguno). */
  let marcados = $state<string[]>([]);
  const marcadosSet = $derived(new Set(marcados));
  const seleccion = $derived(aConfirmar(revisiones, marcadosSet));
  function marcar(id: string, si: boolean) {
    marcados = si ? [...marcados.filter((x) => x !== id), id] : marcados.filter((x) => x !== id);
  }

  // Confirmar (uno o los seleccionados): pide la clave una vez y da de alta uno tras otro.
  let paraConfirmar = $state<Revision[] | null>(null);
  let clave = $state("");
  let repetir = $state("");
  let guardada = $state(false);
  let ocupado = $state(false);
  let paso2 = $state("");
  /** Cómo va cada uno: «Enviando el alta…», «Alta enviada» o el error. */
  let progreso = $state<Record<string, { texto: string; mal?: boolean }>>({});
  const primero = $derived(!actual.equipos.some((e) => e.confirmado && e.etiqueta));
  const claveValida = $derived(primero ? clave.length >= 16 && clave === repetir && guardada : clave.length > 0);

  function pedirClave(rs: Revision[]) {
    if (!rs.length) return;
    error = "";
    paraConfirmar = rs;
    window.scrollTo({ top: 0, behavior: "smooth" });
  }
  function cancelarClave() {
    paraConfirmar = null;
    clave = repetir = "";
    guardada = false;
  }
  function generar() {
    const letras = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    const b = aleatorio(25);
    clave = repetir = Array.from(b, (x) => letras[x % 32]).join("").match(/.{5}/g)!.join("-");
    b.fill(0);
  }

  async function confirmar(e: SubmitEvent) {
    e.preventDefault();
    if (!paraConfirmar || !actual.cliente || !detalle) return;
    const cod = codigo;
    if (!cod) {
      error = "Este navegador no tiene el código: escríbelo arriba para poder dar de alta los equipos.";
      return;
    }
    error = "";
    ocupado = true;
    let kcfg: Uint8Array | null = null;
    const lista = paraConfirmar;
    try {
      paso2 = "Comprobando la clave de administración…";
      const material = await materialCliente(argon2Navegador, clave, actual.cliente.sal_cliente);
      kcfg = kCfg(material);
      borrar(material);
      const otros = actual.equipos.filter((x) => x.confirmado && x.etiqueta);
      if (otros.length && !otros.some((x) => etiquetaValida(kcfg!, x)))
        throw new Error("Esa no es la clave de administración de este cliente (no coincide con la de sus otros equipos).");
      let n = 0;
      for (const r of lista) {
        const eq = r.e.equipo;
        if (!eq || !r.coincide) continue;
        n++;
        paso2 = `Dando de alta ${eq.nombre} (${n} de ${lista.length})…`;
        progreso[r.e.id] = { texto: "Dando de alta…" };
        try {
          if (r.e.estado !== "confirmado") await api.confirmarEmparejamiento(c, r.e.id, etiquetaEquipo(kcfg, eq.id, eq.box_pub, eq.sign_pub));
          const equipo = await api.equipo(c, eq.id);
          await mandarOrden({ cliente: actual.cliente, equipo, tipo: "alta", alta: { codigo: cod }, secretos: { claveAdmin: clave } });
          progreso[r.e.id] = { texto: "Alta enviada" };
          marcar(r.e.id, false);
        } catch (err) {
          progreso[r.e.id] = { texto: (err as Error).message, mal: true };
        }
      }
      const malos = lista.filter((r) => progreso[r.e.id]?.mal).length;
      avisar(malos ? `${lista.length - malos} dados de alta; ${malos} con error (míralo en la lista).` : lista.length === 1 ? "Alta enviada." : `${lista.length} altas enviadas.`, malos ? "warn" : "ok");
      cancelarClave();
      void cargarCliente(c, { silencioso: true });
    } catch (err) {
      error = (err as Error).message;
    } finally {
      borrar(kcfg);
      ocupado = false;
      paso2 = "";
      void cargarDetalle(c, detalle.id);
    }
  }

  // «Rechazar»: el equipo se quita (el uso del código no se devuelve).
  let rechazando = $state<string | null>(null);
  async function rechazar(id: string) {
    try {
      await api.cancelarEmparejamiento(c, id);
      avisar("Rechazado: el equipo se ha quitado.");
      marcar(id, false);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      rechazando = null;
      if (detalle) void cargarDetalle(c, detalle.id);
    }
  }

  onDestroy(() => {
    clave = repetir = "";
  });

  const ESTADO_HECHO: Record<string, string> = { dado_de_alta: "Dado de alta", cancelado: "Rechazado", caducado: "Caducó sin confirmar", confirmado: "Confirmado" };
</script>

<svelte:head><title>Instalar muchos equipos · Resguardo Server</title></svelte:head>

<div class="page estrecha">
  <Migas items={[{ texto: "Equipos", href: `/c/${c}/equipos` }, { texto: "Añadir equipo", href: `/c/${c}/emparejar` }, { texto: "Muchos equipos" }]} />
  <div class="page-top">
    <div>
      <h1>Instalar muchos equipos</h1>
      <p>Un código para varios equipos y una línea para pegar en cada uno. Nada entra sin que compares su número de comprobación.</p>
    </div>
  </div>

  {#if !soportado}
    <div class="notice notice-info"><p>Este servidor aún no admite códigos para varios equipos. Actualiza Resguardo Server o añade los equipos uno a uno desde «Añadir equipo».</p></div>
  {:else if paraConfirmar}
    <!-- La clave, una vez para todos los marcados. -->
    <section class="card p">
      <form class="form" onsubmit={confirmar}>
        <div class="dlg-title">
          <span class="ticon"><KeyRound size={18} /></span>
          <div>
            <h2>{primero ? "Elige la clave de administración" : "Confirma con la clave de administración"}</h2>
            <p>
              {paraConfirmar.length === 1 ? `Para dar de alta ${paraConfirmar[0].e.equipo?.nombre}.` : `Para dar de alta estos ${paraConfirmar.length} equipos.`}
              {primero ? ` Será la clave de ${actual.cliente?.nombre}: el servidor nunca la ve.` : ` La misma que usan los demás equipos de ${actual.cliente?.nombre}.`}
            </p>
          </div>
        </div>
        <ul class="a-confirmar">
          {#each paraConfirmar as r (r.e.id)}
            <li><strong>{r.e.equipo?.nombre}</strong> <span class="sas-chico mono">{r.sas}</span>{#if progreso[r.e.id]}<span class:error-campo={progreso[r.e.id].mal} class="faint"> · {progreso[r.e.id].texto}</span>{/if}</li>
          {/each}
        </ul>
        <CampoClave requerido id="clave-admin" etiqueta="Clave de administración" bind:value={clave} autofocus ayuda={primero ? "Al menos 16 caracteres. Mejor una generada." : undefined}>
          {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
        </CampoClave>
        {#if primero}
          <CampoClave requerido id="clave-repetir" etiqueta="Repite la clave" bind:value={repetir} error={repetir && repetir !== clave ? "No coincide." : ""} />
          <button type="button" class="btn btn-sm generar" onclick={generar}><Shuffle size={14} />Generar una clave segura</button>
          <div class="notice notice-warn">
            <TriangleAlert size={16} />
            <p>Guárdala en un gestor de contraseñas o imprímela. <strong>Si la pierdes</strong>, los equipos siguen copiando, pero para cambiarlos habrá que restablecerla en cada uno.</p>
          </div>
          <label class="switch-row"><input type="checkbox" bind:checked={guardada} /><span>La he guardado en un sitio seguro</span></label>
        {/if}
        <p class="faint pequeno-izq">Tarda unos segundos por equipo (la clave se comprueba en este navegador).</p>
        {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
        <footer class="pie">
          {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{paso2}</span>{/if}
          <button type="button" class="btn btn-ghost" onclick={cancelarClave} disabled={ocupado}>Cancelar</button>
          <button class="btn btn-primary" disabled={!claveValida || ocupado}>{paraConfirmar.length === 1 ? "Dar de alta" : `Dar de alta los ${paraConfirmar.length}`}</button>
        </footer>
      </form>
    </section>
  {:else if loteId}
    {#if noExiste}
      <div class="notice notice-warn"><p>Ese código no existe en este cliente.</p></div>
      <a class="btn" href="/c/{c}/emparejar/varios">Volver</a>
    {:else if !detalle}
      <p class="espera" role="status"><LoaderCircle size={15} class="spin" />Cargando…</p>
    {:else}
      <!-- El código -->
      <section class="card p codigo-card">
        <div class="cabeza">
          <div>
            <h2 class="section-title">{detalle.nombre ?? "Código para varios equipos"}</h2>
            <p class="faint">
              {resumenLote(detalle)}{#if activo} · caduca en {cuentaAtras(detalle.caduca, reloj.ahora)}{/if}
              {#if detalle.rechazos}<span> · {detalle.rechazos} {detalle.rechazos === 1 ? "intento rechazado" : "intentos rechazados"} después</span>{/if}
            </p>
          </div>
          <Chip tono={activo ? "ok" : "neutral"} texto={activo ? "Activo" : detalle.estado === "anulado" ? "Anulado" : detalle.estado === "agotado" ? "Agotado" : "Caducado"} />
        </div>
        {#if codigo}
          <p class="codigo-grande selectable"><span aria-hidden="true">{codigo}</span><span class="sr-only">Código: {codigo.split("").join(" ")}</span></p>
        {:else}
          <div class="field">
            <label class="field-label" for="codigo-escrito">El código</label>
            <div class="fila-campo">
              <input id="codigo-escrito" class="input mono" bind:value={escrito} autocomplete="off" spellcheck="false" maxlength="40" placeholder="ABCD-EFGH-JKMN-PQRS" />
              <button class="btn" onclick={guardarEscrito} disabled={!escrito.trim()}>Usar</button>
            </div>
            <span class="field-hint">El código se creó en otro navegador y solo lo tiene ese (el servidor no lo conoce). Escríbelo para ver las líneas y dar de alta los equipos aquí, o hazlo desde aquel navegador.</span>
          </div>
        {/if}
        {#if activo}
          <div class="acciones-izq">
            {#if anulando}
              <span class="confirmar-anular">¿Anular el código? Ningún equipo más podrá unirse con él; los que esperan siguen abajo. <button class="btn btn-sm btn-danger" onclick={anular}>Sí, anular</button><button class="btn btn-sm btn-ghost" onclick={() => (anulando = false)}>No</button></span>
            {:else}
              <button class="btn btn-sm btn-ghost" onclick={() => (anulando = true)}><Ban size={14} />Anular el código</button>
            {/if}
          </div>
        {/if}
      </section>

      {#if error && !paraConfirmar}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}

      <!-- Las líneas -->
      {#if activo && codigo}
        <section class="card p form">
          <div class="field">
            <span class="field-label">Los equipos llegarán a este servidor en</span>
            {#if editarServidor || !servidorOk}
              <input class="input mono" bind:value={servidorUrl} spellcheck="false" aria-label="Dirección del servidor para los equipos" placeholder="https://192.168.1.20:8443" />
              <span class="field-hint">Con https:// y el puerto, sin ruta: la que usan los equipos de la red (por ejemplo, la IP del servidor).</span>
            {:else}
              <span class="dir-servidor"><code>{servidorUrl}</code><button type="button" class="link" onclick={() => (editarServidor = true)}>Cambiar</button></span>
            {/if}
          </div>

          <h2 class="section-title"><Monitor size={16} /> Windows</h2>
          {#if sinInstalador}
            <div class="notice notice-info"><p>Este servidor no tiene el instalador del agente: viene con Resguardo Server para Windows y, en un servidor Linux, se pone con <code>sudo resguardo-server poner-instalador-agente Resguardo-Agente-setup.exe</code>.</p></div>
          {:else if !huella}
            <p class="espera" role="status"><LoaderCircle size={15} class="spin" />Calculando la huella del instalador…</p>
          {:else if linea}
            <p>Abre <strong>PowerShell como administrador</strong> en cada equipo y pega esta línea. Baja el instalador de este servidor, comprueba su huella SHA-256, lo instala sin preguntar y enseña el número de comprobación.</p>
            <div class="linea">
              <code class="selectable">{linea}</code>
              <button class="icon-btn" aria-label="Copiar la línea de PowerShell" onclick={() => copiar(linea, "Línea de PowerShell")}><Copy size={14} /></button>
            </div>
            <details class="mas">
              <summary>Para una herramienta de despliegue</summary>
              <p class="faint">La misma línea, codificada para no tener que escapar comillas (se ejecuta como administrador o como SYSTEM):</p>
              <div class="linea">
                <code class="selectable">{lineaDespliegue}</code>
                <button class="icon-btn" aria-label="Copiar la orden codificada" onclick={() => copiar(lineaDespliegue, "Orden")}><Copy size={14} /></button>
              </div>
            </details>
          {:else}
            <div class="notice notice-warn"><p>La dirección, el código o la huella no tienen la forma esperada: no se enseña la línea para no pegar algo distinto de lo que parece. Revisa la dirección.</p></div>
          {/if}

          <h2 class="section-title"><Terminal size={16} /> Linux</h2>
          {#if lineaLinux}
            <p>Con el agente ya instalado (<a class="link" href="https://github.com/evercarog/resguardo/blob/main/docs/agente-linux.md" target="_blank" rel="noopener noreferrer">cómo</a>), como root. Comprueba la huella de este servidor antes de enviar nada y enseña el número de comprobación.</p>
            <div class="linea">
              <code class="selectable">{lineaLinux}</code>
              <button class="icon-btn" aria-label="Copiar la línea de Linux" onclick={() => copiar(lineaLinux, "Línea de Linux")}><Copy size={14} /></button>
            </div>
          {:else}
            <div class="notice notice-warn"><p>La dirección, el código o la huella no tienen la forma esperada: no se enseña la línea.</p></div>
          {/if}

          <div class="notice notice-warn" role="note">
            <TriangleAlert size={16} />
            <p>Las líneas llevan el código: queda en el historial de PowerShell o de la terminal y en los registros de la herramienta de despliegue. Con él, un equipo solo puede <strong>pedir</strong> entrar (lo confirmas tú aquí). Anúlalo cuando termines.</p>
          </div>
        </section>
      {/if}

      <!-- Equipos esperando confirmación -->
      <section>
        <div class="section-head">
          <h2>Equipos esperando confirmación <span class="count">· {revisiones.length}</span></h2>
          {#if revisiones.length}
            <button class="btn btn-sm btn-primary" disabled={!seleccion.length || !codigo} onclick={() => pedirClave(seleccion)}><Check size={14} />Confirmar los seleccionados{seleccion.length ? ` (${seleccion.length})` : ""}</button>
          {/if}
        </div>
        {#if !revisiones.length}
          <div class="card p vacio">
            <p class="faint">{activo ? "Cuando un equipo se una con el código, aparecerá aquí con su número de comprobación." : "Ningún equipo espera confirmación."}</p>
          </div>
        {:else}
          <p class="faint pequeno-izq">Compara el número de cada equipo con el que enseña en su pantalla (la línea lo escribe al terminar). Marca solo los que coinciden. <Ayuda id="sas" /></p>
          <div class="card p-0 lista">
            {#each revisiones as r (r.e.id)}
              <div class="fila" class:mal={!r.coincide}>
                <label class="marca" title={r.coincide ? "Marcar: su número coincide" : "No se puede confirmar"}>
                  <input type="checkbox" disabled={!r.coincide || ocupado} checked={marcadosSet.has(r.e.id)} onchange={(ev) => marcar(r.e.id, (ev.currentTarget as HTMLInputElement).checked)} aria-label="Coincide el número de {r.e.equipo?.nombre}" />
                </label>
                <span class="fila-texto">
                  <span class="fila-titulo">{r.e.equipo?.nombre}</span>
                  <span class="fila-sub">{r.e.equipo?.so}{#if r.e.ip} · IP {r.e.ip}{/if} · se unió <Tiempo iso={r.e.unido} />{#if r.e.estado === "confirmado"} · falta el alta{/if}</span>
                  {#if progreso[r.e.id]}<span class="fila-sub" class:error-campo={progreso[r.e.id].mal}>{progreso[r.e.id].texto}</span>{/if}
                </span>
                {#if r.coincide}
                  <span class="sas selectable"><span aria-hidden="true">{r.sas}</span><span class="sr-only">Número de comprobación: {r.sas?.split("").join(" ")}</span></span>
                {:else}
                  <span class="notice notice-danger aviso-sas" role="alert"><TriangleAlert size={14} />El número del servidor no coincide con el de este navegador: no lo confirmes.</span>
                {/if}
                {#if r.antiguo && r.coincide}
                  <span class="faint pequeno-izq ancho">Agente antiguo: comprueba también la huella de la autoridad TLS (<code class="selectable">{app.servidor?.huella_ca}</code>).</span>
                {/if}
                <span class="botones">
                  {#if r.coincide}
                    <button class="btn btn-sm" disabled={ocupado || !codigo} onclick={() => pedirClave([r])}><Check size={14} />Confirmar</button>
                  {/if}
                  {#if rechazando === r.e.id}
                    <span class="confirmar-anular">¿Rechazar? Se quita el equipo. <button class="btn btn-sm btn-danger" onclick={() => rechazar(r.e.id)}>Sí</button><button class="btn btn-sm btn-ghost" onclick={() => (rechazando = null)}>No</button></span>
                  {:else}
                    <button class="btn btn-sm btn-ghost" disabled={ocupado} onclick={() => (rechazando = r.e.id)}><X size={14} />Rechazar</button>
                  {/if}
                </span>
              </div>
            {/each}
          </div>
        {/if}
      </section>

      {#if hechos.length}
        <details class="mas">
          <summary>Ya revisados · {hechos.length}</summary>
          <div class="card p-0 lista">
            {#each hechos as e (e.id)}
              <div class="fila">
                <span class="fila-texto">
                  <span class="fila-titulo">{e.equipo?.nombre ?? "Equipo quitado"}</span>
                  <span class="fila-sub">{ESTADO_HECHO[e.estado] ?? e.estado}{#if e.ip} · IP {e.ip}{/if} · se unió <Tiempo iso={e.unido} /></span>
                </span>
                {#if e.estado === "dado_de_alta" && e.equipo}<a class="btn btn-sm btn-ghost" href="/c/{c}/equipos/{e.equipo.id}">Ver</a>{/if}
              </div>
            {/each}
          </div>
        </details>
      {/if}
    {/if}
  {:else}
    <!-- Crear un código y la lista de los que hay -->
    <section class="card p">
      <form class="form" onsubmit={crear}>
        <h2 class="section-title"><Layers size={16} /> Código para varios equipos</h2>
        <div class="dos">
          <div class="field">
            <label class="field-label" for="v-usos">Equipos</label>
            <input id="v-usos" class="input" type="number" min="1" max={MAX_USOS} step="1" bind:value={usos} />
            <span class="field-hint">Cuántos pueden unirse con él (hasta {MAX_USOS}).</span>
          </div>
          <div class="field">
            <label class="field-label" for="v-dias">Días</label>
            <input id="v-dias" class="input" type="number" min="1" max={MAX_DIAS} step="1" bind:value={dias} />
            <span class="field-hint">Cuánto sirve (hasta {MAX_DIAS}).</span>
          </div>
        </div>
        <div class="field">
          <label class="field-label" for="v-nombre">Nombre <span class="faint">(opcional)</span></label>
          <input id="v-nombre" class="input" bind:value={nombre} maxlength="60" placeholder="Por ejemplo: Oficina de la planta 2" autocomplete="off" />
        </div>
        {#if errorForm}<p class="error-campo">{errorForm}</p>{/if}
        {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
        <p class="faint pequeno-izq">El código lo crea este navegador y aquí se queda: el servidor solo guarda su huella. Cada equipo que se una tendrá que confirmarse aquí.</p>
        <div class="fin">
          <BotonCargando class="btn btn-primary" disabled={!!errorForm} cargando={creando} textoCargando="Creando…" type="submit"><Plus size={16} />Crear el código</BotonCargando>
        </div>
      </form>
    </section>
    {#if lotes.length}
      <section>
        <div class="section-head"><h2>Códigos <span class="count">· {lotes.length}</span></h2></div>
        <div class="card p-0 lista">
          {#each lotes as l (l.id)}
            <a class="fila enlace" href="/c/{c}/emparejar/varios?l={l.id}">
              <span class="fila-texto">
                <span class="fila-titulo">{l.nombre ?? "Código para varios equipos"}</span>
                <span class="fila-sub">{resumenLote(l)}{#if l.estado === "activo"} · caduca en {cuentaAtras(l.caduca, reloj.ahora)}{/if}</span>
              </span>
              {#if l.pendientes}<Chip tono="info" texto={`${l.pendientes} esperando`} />{/if}
              <Chip tono={l.estado === "activo" ? "ok" : "neutral"} texto={l.estado === "activo" ? "Activo" : l.estado === "anulado" ? "Anulado" : l.estado === "agotado" ? "Agotado" : "Caducado"} />
            </a>
          {/each}
        </div>
      </section>
    {/if}
  {/if}
</div>

<style>
  .estrecha {
    max-width: 820px;
  }
  .codigo-card {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .cabeza {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: flex-start;
    gap: var(--sp-2);
  }
  .cabeza p {
    margin: 4px 0 0;
  }
  .codigo-card .codigo-grande {
    margin: 0;
    text-align: center;
    overflow-wrap: anywhere;
  }
  .fila-campo {
    display: flex;
    gap: 8px;
  }
  .fila-campo .input {
    flex: 1;
    min-width: 0;
  }
  .acciones-izq {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .linea {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    max-width: 100%;
    padding: 8px 8px 8px 12px;
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .linea code {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    word-break: break-all;
    white-space: pre-wrap;
  }
  .section-title :global(svg) {
    vertical-align: -2px;
  }
  .dir-servidor {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 12px;
  }
  .dir-servidor code {
    overflow-wrap: anywhere;
  }
  .mas summary {
    cursor: pointer;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .mas[open] summary {
    margin-bottom: var(--sp-2);
  }
  .lista .fila {
    flex-wrap: wrap;
    align-items: center;
  }
  .lista .fila-texto {
    flex: 1 1 calc(100% - 48px);
  }
  @media (min-width: 641px) {
    .lista .fila-texto {
      flex: 1 1 200px;
    }
  }
  .fila.mal {
    background: color-mix(in srgb, var(--bad) 6%, transparent);
  }
  .fila.enlace {
    color: inherit;
    text-decoration: none;
  }
  .fila.enlace:hover .fila-titulo {
    text-decoration: underline;
  }
  .marca {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    flex: none;
  }
  .marca input {
    width: 18px;
    height: 18px;
  }
  /* El número, grande: es lo que se compara con la pantalla del equipo. */
  .sas {
    font-family: var(--mono);
    font-size: 30px;
    line-height: 38px;
    font-weight: 650;
    letter-spacing: 0.08em;
    font-variant-numeric: tabular-nums;
    padding: 2px var(--sp-3);
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .sas-chico {
    font-variant-numeric: tabular-nums;
    margin-left: 6px;
  }
  .aviso-sas {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
  }
  .ancho {
    flex-basis: 100%;
  }
  .botones {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .confirmar-anular {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .a-confirmar {
    margin: 0;
    padding-left: 20px;
    display: grid;
    gap: 4px;
  }
  .dos {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--sp-3);
  }
  .fin,
  .pie {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    align-items: center;
    gap: 8px;
  }
  .pie .espera {
    margin-right: auto;
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .generar {
    align-self: flex-start;
  }
  .pequeno-izq {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .vacio p {
    margin: 0;
  }
  @media (max-width: 640px) {
    .dos {
      grid-template-columns: 1fr;
    }
    .sas {
      font-size: 26px;
      line-height: 34px;
    }
  }
</style>
