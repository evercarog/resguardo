<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Añadir un equipo: código de un solo uso → el equipo se une → número de
  // comprobación (SAS) a ojo en los dos lados → clave de administración →
  // confirmar (etiqueta) → orden «alta». La consola calcula el SAS por su
  // cuenta con la identidad del servidor (y, en v3, la huella de su autoridad
  // TLS); si no coincide con el que da el servidor, no se sigue.
  import Migas from "$lib/componentes/Migas.svelte";
  import { onDestroy } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { Apple, Ban, Check, CircleCheck, Copy, Download, KeyRound, LoaderCircle, Monitor, RefreshCw, Server, Shuffle, Terminal, TriangleAlert, X } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { enFondo } from "$lib/actividad.svelte";
  import { actual, app, cargarCliente, reloj, urlAgentes } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { argon2Navegador } from "$lib/cripto/argon2";
  import { aleatorio, borrar } from "$lib/cripto/bytes";
  import { etiquetaEquipo, etiquetaValida, kCfg, materialCliente, sasV2, sasV3 } from "$lib/cripto/claves";
  import { mandarOrden } from "$lib/ordenar";
  import { cuentaAtras } from "$lib/formato";
  import type { Emparejamiento, Equipo, EstadoDeEmparejamiento, Orden, Preparado, PreparadoLinux } from "$lib/tipos";
  import { guardar } from "$lib/descarga";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import CampoClave from "$lib/componentes/CampoClave.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import BotonCargando from "$lib/componentes/BotonCargando.svelte";
  import Pasos from "$lib/componentes/Pasos.svelte";

  type Paso = "sistema" | "codigo" | "sas" | "clave" | "listo";
  let paso = $state<Paso>("sistema");
  let so = $state<"windows" | "linux" | "mac">("windows");
  let emp = $state<Emparejamiento | null>(null);
  let estadoEmp = $state<EstadoDeEmparejamiento | null>(null);
  let sondeo: ReturnType<typeof setInterval> | null = null;
  let ocupado = $state(false);
  let error = $state("");
  let paso2 = $state("");
  let clave = $state("");
  let repetir = $state("");
  let guardada = $state(false);
  let alta = $state<Orden | null>(null);
  let equipoNuevo = $state<Equipo | null>(null);

  const c = $derived(actual.id);
  /** ¿Es el primer equipo confirmado? Entonces la clave de administración se elige ahora. */
  const primero = $derived(!actual.equipos.some((e) => e.confirmado && e.etiqueta));
  // v1.26: v3 (con la huella de la autoridad TLS) si el equipo lo anunció; si no, v2 y se avisa.
  const sasAntiguo = $derived((estadoEmp?.sas_version ?? 2) < 3);
  const sasLocal = $derived(
    estadoEmp?.equipo && app.servidor
      ? sasAntiguo
        ? sasV2(app.servidor.identidad, estadoEmp.equipo.box_pub, estadoEmp.equipo.sign_pub)
        : sasV3(app.servidor.identidad, estadoEmp.equipo.box_pub, estadoEmp.equipo.sign_pub, app.servidor.huella_ca)
      : null,
  );
  const sasCoincide = $derived(!!sasLocal && sasLocal === estadoEmp?.sas);
  const claveValida = $derived(primero ? clave.length >= 16 && clave === repetir && guardada : clave.length > 0);

  async function abrir() {
    error = "";
    ocupado = true;
    try {
      emp = await api.abrirEmparejamiento(c);
      paso = "codigo";
      sondeo = setInterval(() => enFondo(consultar), 2000);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      ocupado = false;
    }
  }

  async function consultar() {
    if (!emp) return;
    try {
      estadoEmp = await api.emparejamiento(c, emp.id);
      if (estadoEmp.estado === "unido" && paso === "codigo") paso = "sas";
      if (estadoEmp.estado === "caducado" || estadoEmp.estado === "cancelado") parar();
    } catch {
      /* se reintenta */
    }
  }
  function parar() {
    if (sondeo) clearInterval(sondeo);
    sondeo = null;
  }
  onDestroy(() => {
    parar();
    clave = repetir = "";
  });

  async function cancelar() {
    parar();
    if (emp) {
      try {
        await api.cancelarEmparejamiento(c, emp.id);
      } catch {
        /* ya no existía */
      }
    }
    emp = null;
    estadoEmp = null;
    clave = repetir = "";
    paso = "sistema";
  }

  /** Clave fuerte para el primer equipo: 5 grupos de 5 (≈ 125 bits), sin letras que se confundan. */
  function generar() {
    const letras = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    const b = aleatorio(25);
    const t = Array.from(b, (x) => letras[x % 32]).join("");
    clave = repetir = t.match(/.{5}/g)!.join("-");
    b.fill(0);
  }

  async function confirmar(e: SubmitEvent) {
    e.preventDefault();
    if (!emp || !estadoEmp?.equipo || !actual.cliente) return;
    error = "";
    ocupado = true;
    const eq = estadoEmp.equipo;
    let kcfg: Uint8Array | null = null;
    try {
      paso2 = "Comprobando la clave de administración…";
      const material = await materialCliente(argon2Navegador, clave, actual.cliente.sal_cliente);
      kcfg = kCfg(material);
      borrar(material);
      // Si el cliente ya tiene equipos, la clave tiene que ser la suya: se comprueba con ellos. Con un
      // cambio de clave a medias (equipos sin conectar aún con la anterior), vale la de cualquiera.
      const otros = actual.equipos.filter((x) => x.confirmado && x.etiqueta);
      if (otros.length && !otros.some((x) => etiquetaValida(kcfg!, x)))
        throw new Error("Esa no es la clave de administración de este cliente (no coincide con la de sus otros equipos).");
      paso2 = "Confirmando el equipo…";
      await api.confirmarEmparejamiento(c, emp.id, etiquetaEquipo(kcfg, eq.id, eq.box_pub, eq.sign_pub));
      parar();
      const equipo = await api.equipo(c, eq.id);
      equipoNuevo = equipo;
      alta = await mandarOrden({ cliente: actual.cliente, equipo, tipo: "alta", alta: { codigo: emp.codigo }, secretos: { claveAdmin: clave }, alPaso: (t) => (paso2 = t) });
      clave = repetir = "";
      paso = "listo";
      void cargarCliente(c, { silencioso: true });
      seguirAlta(equipo.id, alta.id);
    } catch (err) {
      error = (err as Error).message;
    } finally {
      borrar(kcfg);
      ocupado = false;
      paso2 = "";
    }
  }

  function seguirAlta(equipoId: string, ordenId: string) {
    const t = setInterval(async () => {
      try {
        const o = (await enFondo(() => api.ordenesEquipo(c, equipoId, 5))).find((x) => x.id === ordenId);
        if (o) alta = o;
        if (o && !["pendiente", "entregada", "en_marcha"].includes(o.estado)) clearInterval(t);
      } catch {
        /* se reintenta */
      }
    }, 1500);
    setTimeout(() => clearInterval(t), 120_000);
  }

  // --- Equipos preparados (v1.17) ------------------------------------------
  // Windows: el instalador del agente «listo», con los datos y un código de un
  // solo uso (24 h) al final. Linux: la línea con el código y la huella de la
  // autoridad TLS. En los dos, el número de comprobación se sigue mirando aquí.
  let modoWin = $state<"listo" | "codigo">("listo");
  let nombreEq = $state("");
  // Si esta consola se abrió sin https (p. ej. en local), no vale como dirección para los equipos: se pide.
  const origenHttps = typeof location !== "undefined" && location.protocol === "https:";
  // v1.34: en una consola en internet, la de los agentes (agentes.<dominio>), no la del navegador.
  let servidorUrl = $state(app.servidor?.url_agentes || (origenHttps ? location.origin : ""));
  let editarServidor = $state(false);
  let preparando = $state(false);
  let descargado = $state<{ archivo: string; caduca: string; nombre: string } | null>(null);
  let linux = $state<PreparadoLinux | null>(null);
  let lista = $state<Preparado[] | null>(null);
  let anular = $state<string | null>(null);
  const puedeListo = $derived(app.servidor?.instalador_agente === true);
  const errorNombreEq = $derived.by(() => {
    const t = nombreEq.trim();
    if (!t) return null;
    if (t.length > 80) return "Hasta 80 caracteres.";
    if (/["\u0000-\u001f]/.test(t)) return "Sin comillas.";
    return null;
  });
  const servidorValido = $derived(/^https:\/\/[A-Za-z0-9.\-:[\]]+$/.test(servidorUrl.trim().replace(/\/+$/, "")));
  /** El error solo cuando ya se ha escrito algo (vacío, se explica sin rojo). */
  const errorServidor = $derived(servidorValido || !servidorUrl.trim() ? null : "Escribe la dirección con https:// y el puerto, sin ruta (p. ej. https://192.168.1.20:8443).");
  const listoParaPreparar = $derived(!!nombreEq.trim() && !errorNombreEq && servidorValido);

  async function cargarLista() {
    try {
      const antes = new Map((lista ?? []).map((p) => [p.id, p.estado]));
      const nueva = await api.preparados(c);
      // Aviso en cuanto un preparado se une: es lo que se está esperando.
      for (const p of nueva) if (p.estado === "unido" && antes.get(p.id) && antes.get(p.id) !== "unido") avisar(`«${p.nombre}» se ha unido. Compruébalo y dale de alta.`, "info");
      lista = nueva;
    } catch {
      lista ??= [];
    }
  }
  $effect(() => {
    if (!c) return;
    void cargarLista();
    const t = setInterval(() => document.visibilityState === "visible" && enFondo(cargarLista), 5000);
    return () => clearInterval(t);
  });

  async function descargarListo() {
    error = "";
    preparando = true;
    try {
      const r = await api.prepararInstalador(c, nombreEq.trim(), servidorUrl.trim().replace(/\/+$/, ""));
      await guardar(r.datos, r.archivo);
      descargado = { archivo: r.archivo, caduca: r.caduca, nombre: nombreEq.trim() };
      nombreEq = "";
      void cargarLista();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      preparando = false;
    }
  }
  async function prepararLinea() {
    error = "";
    preparando = true;
    try {
      linux = await api.prepararLinux(c, nombreEq.trim(), servidorUrl.trim().replace(/\/+$/, ""));
      nombreEq = "";
      void cargarLista();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      preparando = false;
    }
  }
  async function anularPreparado(id: string) {
    try {
      await api.cancelarEmparejamiento(c, id);
      avisar("Anulado: ese código ya no sirve.");
      if (linux?.id === id) linux = null;
    } catch (e) {
      error = (e as Error).message;
    } finally {
      anular = null;
      void cargarLista();
    }
  }
  /** Un preparado que ya se unió: se sigue el mismo camino (número de comprobación, clave, alta). */
  async function seguirPreparado(id: string) {
    error = "";
    try {
      const est = await api.emparejamiento(c, id);
      if (!est.codigo) throw new Error("Ese emparejamiento ya no se puede confirmar desde aquí. Prepara otro.");
      emp = { id, codigo: est.codigo, caduca: est.caduca };
      estadoEmp = est;
      paso = est.estado === "unido" ? "sas" : "codigo";
      parar();
      sondeo = setInterval(() => enFondo(consultar), 2000);
      window.scrollTo({ top: 0 });
    } catch (e) {
      error = (e as Error).message;
    }
  }
  // --- «Vincular este servidor» (v1.19) ------------------------------------
  // El agente instalado en la máquina del servidor («Este equipo también guarda
  // copias» al instalarlo) se une solo; aquí se compara el número y se da de
  // alta con la clave, como cualquier otro. Después, a «Este equipo guarda copias».
  let local = $state(false);
  let vinculandoLocal = $state(false);
  const pideLocal = $derived(page.url.searchParams.get("local") === "1");
  async function vincularEsteServidor() {
    error = "";
    vinculandoLocal = true;
    try {
      const p = await api.vincularLocal(c);
      local = true;
      await seguirPreparado(p.id);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      vinculandoLocal = false;
    }
  }

  const lineaPreparada = $derived(linux ? `sudo resguardo-agente vincular ${linux.codigo} --servidor ${linux.servidor} --huella-ca ${linux.huella_ca}` : "");

  const lineaLinux = $derived(emp ? `sudo resguardo-agente vincular ${emp.codigo} --servidor ${urlAgentes()}` : "");
  async function copiar(t: string, que: string) {
    await navigator.clipboard.writeText(t);
    avisar(`${que} copiado.`);
  }
</script>

<svelte:head><title>Añadir equipo · Resguardo Server</title></svelte:head>

<div class="page estrecha">
  <Migas items={[{ texto: "Equipos", href: `/c/${c}/equipos` }, { texto: "Añadir equipo" }]} />
  <div class="page-top">
    <div>
      <h1>Añadir un equipo a {actual.cliente?.nombre ?? ""}</h1>
      <p>Instala el agente en el equipo, escribe el código y compara el número de comprobación.</p>
    </div>
  </div>

  <Pasos
    pasos={[
      { id: "sistema", texto: "Instalar" },
      { id: "codigo", texto: "Código" },
      { id: "sas", texto: "Comprobar" },
      { id: "clave", texto: "Clave" },
      { id: "listo", texto: "Listo" },
    ]}
    actual={paso}
    completo={paso === "listo" && alta?.estado === "hecha"}
  />

  {#if paso === "sistema"}
    {#if app.servidor?.agente_local}
      <section class="card p local" class:destacada={pideLocal}>
        <span class="ic-local"><Server size={18} /></span>
        <div>
          <h2 class="section-title">Este servidor también puede guardar copias</h2>
          <p class="faint">Resguardo Agente está instalado en la máquina del servidor. Vincúlalo aquí (se une solo), compara su número, dalo de alta con la clave de administración y elige la carpeta donde guardará las copias de los demás.</p>
          {#if error && pideLocal}<p class="error-campo" role="alert">{error}</p>{/if}
          <BotonCargando class="btn {pideLocal ? 'btn-primary' : ''}" cargando={vinculandoLocal} textoCargando="Preparando…" onclick={vincularEsteServidor}><Server size={15} />Vincular este servidor</BotonCargando>
        </div>
      </section>
    {/if}
    <section class="card p form">
      <h2 class="section-title">¿Qué sistema tiene el equipo?</h2>
      <div class="segmented" role="group" aria-label="Sistema">
        <button class:on={so === "windows"} aria-pressed={so === "windows"} onclick={() => (so = "windows")}><Monitor size={14} /> Windows</button>
        <button class:on={so === "linux"} aria-pressed={so === "linux"} onclick={() => (so = "linux")}><Terminal size={14} /> Linux</button>
        <button class:on={so === "mac"} aria-pressed={so === "mac"} onclick={() => (so = "mac")}><Apple size={14} /> Mac</button>
      </div>
      {#if so === "windows"}
        <div class="opciones" role="radiogroup" aria-label="Cómo añadirlo">
          <label class="opcion" class:on={modoWin === "listo"}>
            <input type="radio" bind:group={modoWin} value="listo" />
            <span><strong>Descargar instalador listo</strong> <span class="badge badge-sm tone-ok">Recomendado</span><span class="faint">Se vincula solo al instalarlo: nadie tiene que escribir códigos ni direcciones.</span></span>
          </label>
          <label class="opcion" class:on={modoWin === "codigo"}>
            <input type="radio" bind:group={modoWin} value="codigo" />
            <span><strong>Instalador normal y un código</strong><span class="faint">El instalador pide el código (sirve 15 min) y la dirección de este servidor.</span></span>
          </label>
        </div>
      {:else if so === "linux"}
        <p>Para Debian, Ubuntu o un CT de Proxmox: escribe su nombre y te damos la línea para vincularlo, con un código que sirve 24 h.</p>
      {:else}
        <p>Instala el paquete del agente (<code>.pkg</code>) y, en Terminal, ejecuta <code>sudo resguardo-agente vincular CÓDIGO --servidor {urlAgentes()}</code>.</p>
      {/if}

      <p class="faint pequeno-izq">Si el equipo viene de otro servidor, usa la misma clave de administración del cliente: conserva toda su configuración.</p>

      {#if (so === "windows" && modoWin === "listo") || so === "linux"}
        {#if so === "windows" && !puedeListo}
          <div class="notice notice-info"><p>Este servidor no tiene el instalador del agente: viene con Resguardo Server para Windows y, en un servidor Linux, se pone con <code>sudo resguardo-server poner-instalador-agente Resguardo-Agente-setup.exe</code>. Mientras tanto, usa «Instalador normal y un código».</p></div>
        {:else}
          <div class="field">
            <label class="field-label" for="p-nombre">Nombre del equipo</label>
            <input id="p-nombre" class="input" bind:value={nombreEq} maxlength="80" placeholder={so === "linux" ? "Por ejemplo: srv-datos" : "Por ejemplo: SERVIDOR-01 o Recepción"} autocomplete="off" />
            {#if errorNombreEq}<p class="error-campo">{errorNombreEq}</p>{:else}<span class="field-hint">Así aparecerá en la consola. Se puede cambiar después.</span>{/if}
          </div>
          <div class="field">
            <span class="field-label">Los equipos llegarán a este servidor en</span>
            {#if editarServidor || !servidorValido}
              <input class="input mono" bind:value={servidorUrl} spellcheck="false" aria-label="Dirección del servidor para los equipos" placeholder="https://192.168.1.20:8443" aria-required="true" aria-invalid={!!errorServidor} aria-describedby="servidor-url-ayuda" />
              {#if errorServidor}<p class="error-campo" id="servidor-url-ayuda">{errorServidor}</p>{:else if !servidorUrl.trim()}<span class="field-hint" id="servidor-url-ayuda">La dirección con la que los equipos de la red llegan a este servidor, con https:// y el puerto (por ejemplo, https://192.168.1.20:8443).</span>{:else}<span class="field-hint" id="servidor-url-ayuda">La que usan los equipos de la red (por ejemplo, la IP del servidor), no la de este navegador si es otra.</span>{/if}
            {:else}
              <span class="dir-servidor"><code>{servidorUrl}</code><button type="button" class="link" onclick={() => (editarServidor = true)}>Cambiar</button></span>
            {/if}
          </div>
          {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
          <div class="fin">
            {#if so === "windows"}
              <BotonCargando class="btn btn-primary" disabled={!listoParaPreparar} cargando={preparando} textoCargando="Preparando el instalador…" onclick={descargarListo}><Download size={16} />Descargar instalador listo</BotonCargando>
            {:else}
              <BotonCargando class="btn btn-primary" disabled={!listoParaPreparar} cargando={preparando} textoCargando="Preparando…" onclick={prepararLinea}><Terminal size={16} />Preparar la línea</BotonCargando>
            {/if}
          </div>
          <p class="faint pequeno-izq">El código sirve una sola vez y caduca en 24 h. Viaja dentro del {so === "windows" ? "instalador" : "comando"}: trátalo como una llave temporal (y anúlalo abajo si no lo vas a usar).</p>
        {/if}
      {:else}
        {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
        <div class="fin"><BotonCargando class="btn btn-primary" cargando={ocupado} textoCargando="Generando…" onclick={abrir}>Generar el código</BotonCargando></div>
      {/if}
    </section>

    {#if descargado && so === "windows"}
      <section class="card p listo-descarga" role="status">
        <span class="ok-icono"><CircleCheck size={22} /></span>
        <div>
          <h2 class="section-title">Instalador de «{descargado.nombre}» descargado</h2>
          <p>Llévalo al equipo y ábrelo como administrador (<code>{descargado.archivo}</code>). Se vinculará solo; después, aquí abajo, comprueba su número y dale de alta. Caduca <Tiempo iso={descargado.caduca} />.</p>
        </div>
      </section>
    {/if}
    {#if linux && so === "linux"}
      <section class="card p form">
        <h2 class="section-title">Vincular «{linux.nombre}»</h2>
        <ol class="pasos-linux">
          <li>Instala el agente (paquete <code>.deb</code> o <code>.tar.gz</code>): <a class="link" href="https://github.com/evercarog/resguardo/blob/main/docs/agente-linux.md" target="_blank" rel="noopener noreferrer">cómo</a>.</li>
          <li>
            Como root, vincúlalo (comprueba la huella de este servidor antes de enviar nada):
            <div class="linea">
              <code class="selectable">{lineaPreparada}</code>
              <button class="icon-btn" aria-label="Copiar la línea" onclick={() => copiar(lineaPreparada, "Línea")}><Copy size={14} /></button>
            </div>
          </li>
          <li>Vuelve aquí: cuando se una, compara su número de comprobación y dale de alta.</li>
        </ol>
        <p class="faint pequeno-izq">Un agente anterior a 0.7.7 no conoce <code>--huella-ca</code>: lo ignora y se vincula igual (el número de comprobación sigue protegiendo). El código caduca <Tiempo iso={linux.caduca} />.</p>
      </section>
    {/if}

    {#if lista?.length}
      <section>
        <div class="section-head"><h2>Preparados <span class="count">· {lista.length}</span></h2></div>
        <div class="card p-0 lista">
          {#each lista as p (p.id)}
            <div class="fila">
              <span class="ic-so">{#if p.so === "linux"}<Terminal size={16} />{:else}<Monitor size={16} />{/if}</span>
              <span class="fila-texto">
                <span class="fila-titulo">{p.nombre}</span>
                <span class="fila-sub">{p.so === "linux" ? "Línea de Linux" : "Instalador listo"} · caduca en {cuentaAtras(p.caduca, reloj.ahora)}</span>
              </span>
              {#if p.estado === "unido"}
                <Chip tono="info" texto="Se ha unido" />
                <button class="btn btn-sm btn-primary" onclick={() => seguirPreparado(p.id)}><Check size={14} />Comprobar y dar de alta</button>
              {:else}
                <Chip tono="neutral" texto="Esperando al equipo" girando />
              {/if}
              {#if anular === p.id}
                <span class="confirmar-anular">¿Anular? <button class="btn btn-sm btn-danger" onclick={() => anularPreparado(p.id)}>Sí, anular</button><button class="btn btn-sm btn-ghost" onclick={() => (anular = null)}>No</button></span>
              {:else}
                <button class="btn btn-sm btn-ghost" onclick={() => (anular = p.id)} use:tip={"El código deja de servir (y, si ya se unió, el equipo se quita)"}><Ban size={14} />Anular</button>
              {/if}
            </div>
          {/each}
        </div>
      </section>
    {/if}
  {:else if paso === "codigo" && emp}
    <section class="card p centro">
      {#if local}
        <p class="faint">El agente de este servidor se vincula solo, en menos de un minuto. No hay que escribir nada.</p>
      {:else}
        <p class="faint">Escribe este código en el equipo</p>
        <p class="codigo-grande selectable"><span aria-hidden="true">{emp.codigo}</span><span class="sr-only">Código: {emp.codigo.split('').join(' ')}</span></p>
        <div class="acciones">
          <button class="btn btn-sm" onclick={() => copiar(emp!.codigo, "Código")}><Copy size={14} />Copiar</button>
        </div>
      {/if}
      {#if so === "linux" && !local}
        <div class="linea">
          <code class="selectable">{lineaLinux}</code>
          <button class="icon-btn" aria-label="Copiar la línea" onclick={() => copiar(lineaLinux, "Línea")}><Copy size={14} /></button>
        </div>
      {/if}
      {#if estadoEmp?.estado === "caducado" || estadoEmp?.estado === "cancelado"}
        <div class="notice notice-warn"><p>El código caducó. Genera otro.</p></div>
        <button class="btn" onclick={() => ((paso = "sistema"), (emp = null), (estadoEmp = null))}><RefreshCw size={14} />Empezar de nuevo</button>
      {:else}
        <p class="espera" role="status"><LoaderCircle size={15} class="spin" />Esperando al equipo… caduca en {cuentaAtras(emp.caduca, reloj.ahora)}</p>
        <button class="btn btn-ghost btn-sm" onclick={cancelar}>Cancelar</button>
      {/if}
    </section>
  {:else if paso === "sas" && estadoEmp?.equipo}
    <section class="card p centro">
      <p class="faint">Se ha unido <strong>{estadoEmp.equipo.nombre}</strong> ({estadoEmp.equipo.so})</p>
      {#if !sasCoincide}
        <div class="notice notice-danger" role="alert">
          <TriangleAlert size={16} />
          <p>El número que da el servidor no coincide con el que calcula este navegador. No sigas: puede haber alguien en medio. Cancela y revisa el servidor.</p>
        </div>
        <button class="btn" onclick={cancelar}><X size={14} />Cancelar</button>
      {:else}
        <p>¿El equipo muestra este número de comprobación? <Ayuda id="sas" /></p>
        <p class="sas selectable"><span aria-hidden="true">{sasLocal}</span><span class="sr-only">Número de comprobación: {sasLocal?.split('').join(' ')}</span></p>
        {#if sasAntiguo}
          <div class="notice notice-warn" role="note">
            <TriangleAlert size={16} />
            <p>
              Este agente es antiguo: comprueba también la huella del certificado. La que fijó el equipo la dice <code>resguardo-agente vincular</code> al terminar y tiene que ser
              <code class="selectable">{app.servidor?.huella_ca || "(el servidor no tiene autoridad TLS)"}</code>. Si no coincide, cancela: puede haber alguien en medio. Mejor aún: actualiza el agente.
            </p>
          </div>
        {/if}
        {#if local}
          <p class="faint pequeno">En la máquina del servidor, como administrador, <code>resguardo-agente registro 5</code> muestra el número con el que se vinculó. Compáralo cifra a cifra.</p>
        {:else}
          <p class="faint pequeno">Compáralo cifra a cifra en la pantalla del equipo o en su instalador. Si no coincide, alguien podría estar haciéndose pasar por él.</p>
        {/if}
        <div class="acciones">
          <button class="btn" onclick={cancelar}><X size={14} />No coincide</button>
          <button class="btn btn-primary" onclick={() => (paso = "clave")}><Check size={14} />Coincide</button>
        </div>
      {/if}
    </section>
  {:else if paso === "clave" && estadoEmp?.equipo}
    <section class="card p">
      <form class="form" onsubmit={confirmar}>
        <div class="dlg-title">
          <span class="ticon"><KeyRound size={18} /></span>
          <div>
            <h2>{primero ? "Elige la clave de administración" : "Confirma con la clave de administración"}</h2>
            <p>{primero ? `Será la clave de ${actual.cliente?.nombre} para cambiar copias, dar de alta equipos y pausar. El servidor nunca la ve.` : `La misma que usan los demás equipos de ${actual.cliente?.nombre}.`}</p>
            <p class="faint pequeno-izq">Si el equipo viene de otro servidor, usa la misma clave de administración del cliente: conserva toda su configuración.</p>
          </div>
        </div>
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
        {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
        <footer class="pie">
          {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{paso2}</span>{/if}
          <button type="button" class="btn btn-ghost" onclick={cancelar} disabled={ocupado}>Cancelar</button>
          <button class="btn btn-primary" disabled={!claveValida || ocupado}>Dar de alta {estadoEmp.equipo.nombre}</button>
        </footer>
      </form>
    </section>
  {:else if paso === "listo" && equipoNuevo}
    <section class="card p centro">
      <span class="ok-icono"><CircleCheck size={32} /></span>
      <h2 class="section-title">{equipoNuevo.nombre} ya es de {actual.cliente?.nombre}</h2>
      {#if alta}
        <Chip tono={alta.estado === "hecha" ? "ok" : alta.estado === "rechazada" || alta.estado === "fallida" ? "bad" : "info"} texto={alta.estado === "hecha" ? "Alta hecha" : alta.estado === "rechazada" || alta.estado === "fallida" ? "El equipo rechazó el alta" : "Dando de alta…"} girando={["pendiente", "entregada", "en_marcha"].includes(alta.estado)} />
        {#if alta.mensaje}<p class="faint">{alta.mensaje}</p>{/if}
      {/if}
      <div class="acciones">
        <button
          class="btn btn-ghost"
          onclick={() => {
            paso = "sistema";
            emp = null;
            estadoEmp = null;
            alta = null;
            equipoNuevo = null;
            local = false;
            window.scrollTo({ top: 0 });
          }}>Añadir otro equipo</button
        >
        <a class="btn" href="/c/{c}/equipos/{equipoNuevo.id}">Ver el equipo</a>
        {#if local}
          <button class="btn btn-primary" onclick={() => goto(`/c/${c}/equipos/${equipoNuevo!.id}?guardar=1`)}><Server size={15} />Usarlo como almacén</button>
        {:else}
          <button class="btn btn-primary" onclick={() => goto(`/c/${c}/equipos/${equipoNuevo!.id}/copias`)}>Crear su primera copia</button>
        {/if}
      </div>
    </section>
  {/if}
</div>


<style>
  .estrecha {
    max-width: 720px;
  }
  .centro {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-3);
    text-align: center;
  }
  .centro p {
    margin: 0;
  }
  .sas {
    font-family: var(--mono);
    font-size: 48px;
    line-height: 56px;
    font-weight: 650;
    letter-spacing: 0.08em;
    font-variant-numeric: tabular-nums;
    padding: var(--sp-3) var(--sp-6);
    background: var(--surface-2);
    border-radius: var(--radius-lg);
  }
  .pequeno {
    max-width: 420px;
    font-size: var(--fs-sm);
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 8px;
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .linea {
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: 100%;
    padding: 8px 8px 8px 12px;
    background: var(--surface-2);
    border-radius: var(--radius);
    text-align: left;
  }
  .linea code {
    font-size: 12px;
    word-break: break-all;
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
  .generar {
    align-self: flex-start;
  }
  .local {
    display: flex;
    gap: var(--sp-3);
    align-items: flex-start;
  }
  .local > div {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-2);
  }
  .local p {
    margin: 0;
  }
  .local.destacada {
    border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
    box-shadow: var(--focus);
  }
  .ic-local {
    display: grid;
    place-items: center;
    flex: none;
    width: 36px;
    height: 36px;
    color: var(--accent-text);
    background: var(--accent-soft);
    border-radius: var(--radius);
  }
  .opciones {
    display: grid;
    gap: var(--sp-2);
  }
  .opcion {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: var(--sp-3) var(--sp-4);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
  }
  .opcion.on {
    border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
    background: var(--accent-soft);
  }
  .opcion input {
    margin-top: 3px;
  }
  .opcion > span {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px 8px;
  }
  .opcion .faint {
    flex-basis: 100%;
    font-size: var(--fs-sm);
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
  .pequeno-izq {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .listo-descarga {
    display: flex;
    gap: var(--sp-3);
    align-items: flex-start;
  }
  .listo-descarga p {
    margin: 4px 0 0;
  }
  .listo-descarga code {
    overflow-wrap: anywhere;
  }
  .pasos-linux {
    display: grid;
    gap: var(--sp-3);
    margin: 0;
    padding-left: 20px;
  }
  .ic-so {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    color: var(--text-2);
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .lista .fila {
    flex-wrap: wrap;
  }
  /* En móvil, el nombre ocupa su línea y el estado y los botones van debajo. */
  .lista .fila-texto {
    flex: 1 1 calc(100% - 48px);
  }
  @media (min-width: 641px) {
    .lista .fila-texto {
      flex: 1 1 180px;
    }
  }
  .confirmar-anular {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .ok-icono {
    color: var(--ok);
  }
  .segmented :global(svg) {
    vertical-align: -2px;
  }
  @media (max-width: 640px) {
    .sas {
      font-size: 38px;
      line-height: 46px;
    }
  }
</style>
