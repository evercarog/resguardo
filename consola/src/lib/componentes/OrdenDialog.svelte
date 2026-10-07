<script lang="ts">
  // Confirmar y mandar una orden a un equipo.
  //
  // Según el tipo pide la contraseña del repositorio («Confirmar con la
  // contraseña del repositorio») o la clave de administración («…con la clave
  // de administración»). Las destructivas muestran cuándo se aplicarán y que
  // se pueden cancelar. Después enseña la respuesta del equipo, firmada.
  // Los secretos solo viven en este diálogo y se borran al cerrarlo.
  import { onDestroy, onMount, type Snippet } from "svelte";
  import { Clock, KeyRound, LoaderCircle, LockKeyhole, Send, ShieldCheck, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { enFondo } from "$lib/actividad.svelte";
  import { ErrorEtiqueta, ErrorFaltaAdmin, ErrorLlavesCambiadas, mandarOrden, necesitaAdmin, necesitaRepo } from "$lib/ordenar";
  import { comprobarLlaves } from "$lib/fijadas";
  import { esperarRespuesta } from "$lib/adoptar";
  import AlertaLlaves from "./AlertaLlaves.svelte";
  import { esDestructiva } from "$lib/cripto/ordenes";
  import { resultadoFirmado } from "$lib/cripto/claves";
  import { bytes, cuentaAtras, fechaLarga } from "$lib/formato";
  import { infCopia, ultimaVuelta, type Vuelta } from "$lib/copia";
  import { anadidoDe, duracion, informeDe, TEXTO_RESULTADO, TONO_RESULTADO, versionDeVuelta } from "$lib/repo";
  import { ESTADO_ORDEN, nombreOrden } from "$lib/salud";
  import { mensajeOrden } from "$lib/textosEquipo";
  import { cargarCliente } from "$lib/estado.svelte";
  import { seguirEnFondo, vigilarCopiaEnFondo } from "$lib/seguimiento.svelte";
  import { progresoPronto } from "$lib/progreso.svelte";
  import type { Cliente, Equipo, Orden } from "$lib/tipos";
  import CampoClave from "./CampoClave.svelte";
  import BotonCargando from "./BotonCargando.svelte";
  import Chip from "./Chip.svelte";
  import EnMarcha from "./EnMarcha.svelte";
  import Ayuda from "./Ayuda.svelte";

  interface Props {
    cliente: Cliente;
    equipo: Equipo;
    tipo: string;
    cuerpo?: Record<string, unknown>;
    /** Título del diálogo (por defecto, el nombre de la orden). */
    titulo?: string;
    /** Qué va a pasar, en una frase. */
    descripcion: string;
    /** Repositorio cuya contraseña hace falta (si el tipo la pide). */
    repo?: { id: string; nombre: string };
    /** Texto del botón principal si no pide secretos. */
    accion?: string;
    /** Campos propios de la orden (cambian `cuerpo`, que es reactivo). */
    campos?: Snippet;
    /** false: algún campo propio aún no es válido. */
    valido?: boolean;
    onclose: () => void;
    alEnviar?: (o: Orden) => void;
    /** Cuando el equipo responde (hecha, fallida…): para refrescar lo que cambió (p. ej. el espejo tras quitar una nube). */
    alTerminar?: (o: Orden) => void;
    /**
     * «Probar» (opcional): manda la misma orden con estos campos de más (p. ej.
     * `{ solo_probar: true }`), espera la respuesta del equipo y la enseña aquí,
     * sin cerrar el diálogo ni olvidar las claves (luego se confirma como siempre).
     */
    probar?: { cuerpo: Record<string, unknown>; texto?: string } | null;
  }
  let { cliente, equipo, tipo, cuerpo = {}, titulo, descripcion, repo, accion, campos, valido = true, onclose, alEnviar, alTerminar, probar = null }: Props = $props();

  let claveAdmin = $state("");
  let contrasenaRepo = $state("");
  let fase = $state<"pedir" | "enviando" | "esperando" | "fin">("pedir");
  let paso = $state("");
  let error = $state("");
  let errorAdmin = $state("");
  let orden = $state<Orden | null>(null);
  let sondeo: ReturnType<typeof setInterval> | null = null;

  /** Orden con la contraseña del repositorio a un equipo cuyas llaves aún no están fijadas en este navegador. */
  let fijarPrimero = $state(false);
  let llavesCambiadas = $state(false);
  onMount(async () => {
    if (!necesitaRepo(tipo) && !necesitaAdmin(tipo)) return;
    const estado = await comprobarLlaves(cliente.id, equipo);
    // Si las llaves cambiaron, ni se piden secretos: alerta directamente.
    if (estado === "cambiada") llavesCambiadas = true;
    else if (necesitaRepo(tipo) && !necesitaAdmin(tipo)) fijarPrimero = estado === "sin_fijar";
  });
  const pideAdmin = $derived(necesitaAdmin(tipo) || fijarPrimero);
  const pideRepo = $derived(necesitaRepo(tipo));
  const espera = $derived(equipo.espera_min_horas ?? cliente.espera_min_horas);
  const destructiva = $derived(
    esDestructiva(tipo, cuerpo, espera, {
      espejo: equipo.resumen?.guarda_copias?.espejo ?? null,
      espejoEquipo: equipo.resumen?.espejo_equipo ?? null,
      copiasActivas: (equipo.resumen?.copias ?? []).filter((k) => k.activa !== false).length,
      derivadas: (equipo.resumen?.repositorios ?? []).flatMap((r) => (r.derivadas ?? []).map((d) => ({ repo: r.id, id: d.id, destino_id: d.destino_id }))),
    }),
  );
  const cuando = $derived(new Date(Date.now() + espera * 3600_000));
  const listo = $derived(valido && (!pideAdmin || claveAdmin.length > 0) && (!pideRepo || contrasenaRepo.length > 0));
  const firmada = $derived(orden ? resultadoFirmado(equipo.sign_pub, orden) : false);
  const textoBoton = $derived(
    pideAdmin && pideRepo
      ? "Confirmar con las dos claves"
      : pideAdmin
        ? "Confirmar con la clave de administración"
        : pideRepo
          ? "Confirmar con la contraseña del repositorio"
          : (accion ?? nombreOrden(tipo)),
  );

  function olvidar() {
    claveAdmin = "";
    contrasenaRepo = "";
  }

  // Los campos vacíos (p. ej. «repo» = todos) no se mandan.
  const cuerpoLimpio = (extra: Record<string, unknown> = {}) =>
    Object.fromEntries(Object.entries({ ...($state.snapshot(cuerpo) as Record<string, unknown>), ...extra }).filter(([, v]) => v !== ""));
  const secretos = () => ({ claveAdmin: pideAdmin ? claveAdmin : undefined, repo: pideRepo && repo ? { repo: repo.id, contrasena: contrasenaRepo } : undefined });

  /** Lo que respondió el equipo a «Probar» (la misma orden, sin guardar nada). */
  let prueba = $state<{ ok: boolean; mensaje: string } | null>(null);
  let probando = $state(false);
  async function probarAhora() {
    if (!probar) return;
    error = errorAdmin = "";
    prueba = null;
    probando = true;
    try {
      const o = await mandarOrden({ cliente, equipo, tipo, cuerpo: cuerpoLimpio(probar.cuerpo), secretos: secretos(), alPaso: (t) => (paso = t) });
      paso = `Esperando a ${equipo.nombre}…`;
      const r = await esperarRespuesta(cliente.id, equipo, o, 240);
      prueba = { ok: r.estado === "hecha", mensaje: r.mensaje ?? (r.estado === "hecha" ? "Todo en orden." : "No se pudo comprobar.") };
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) {
        olvidar();
        llavesCambiadas = true;
      } else if (err instanceof ErrorFaltaAdmin) {
        fijarPrimero = true;
        errorAdmin = err.message;
      } else if (err instanceof ErrorEtiqueta) errorAdmin = err.message;
      else error = (err as Error).message;
    } finally {
      probando = false;
      paso = "";
    }
  }

  async function enviar(e: SubmitEvent) {
    e.preventDefault();
    error = errorAdmin = "";
    fase = "enviando";
    try {
      const o = await mandarOrden({
        cliente,
        equipo,
        tipo,
        cuerpo: cuerpoLimpio(),
        secretos: secretos(),
        alPaso: (t) => (paso = t),
      });
      olvidar();
      orden = o;
      alEnviar?.(o);
      void cargarCliente(cliente.id, { silencioso: true });
      // Lo que pone algo en marcha: el progreso se pregunta deprisa un rato.
      if (["copiar_ahora", "verificar_ahora", "subir_ahora", "probar_restauracion"].includes(tipo)) progresoPronto();
      if (o.not_before) {
        fase = "fin";
        return;
      }
      fase = "esperando";
      seguir(o.id);
    } catch (err) {
      fase = "pedir";
      if (err instanceof ErrorLlavesCambiadas) {
        olvidar();
        llavesCambiadas = true;
      } else if (err instanceof ErrorFaltaAdmin) {
        fijarPrimero = true;
        errorAdmin = err.message;
      } else if (err instanceof ErrorEtiqueta) errorAdmin = err.message;
      else error = (err as Error).message;
    }
  }

  /** Sigue la orden hasta que el equipo responda (como mucho 2 min aquí; luego sigue en «Órdenes»). */
  function seguir(id: string) {
    const inicio = Date.now();
    sondeo = setInterval(async () => {
      try {
        const lista = await enFondo(() => api.ordenesEquipo(cliente.id, equipo.id, 15));
        const o = lista.find((x) => x.id === id);
        if (o) orden = o;
        if (o && ["hecha", "fallida", "rechazada", "cancelada", "caducada"].includes(o.estado)) {
          parar();
          if (o.estado === "hecha" && o.tipo === "copiar_ahora") vigilarCopia(o);
          alTerminar?.(o);
          void cargarCliente(cliente.id, { silencioso: true });
        }
        else if (o && o.estado === "en_marcha" && ["abrir_sesion", "explorar", "elegir_carpetas"].includes(o.tipo)) parar();
      } catch {
        /* se reintenta */
      }
      if (Date.now() - inicio > 120_000) parar();
    }, 1200);
  }
  // «Copiar ahora»: la orden solo la pide; aquí se espera a que el equipo
  // informe de esa vuelta (agentes ≥ 0.7.4 lo hacen al terminar; los
  // anteriores, en su informe de cada 5 min).
  let copia = $state<{ estado: "en_marcha" | "fin" | "sin_noticias"; vuelta?: Vuelta; meta?: string } | null>(null);
  let vigilancia: ReturnType<typeof setInterval> | null = null;
  function vigilarCopia(o: Orden) {
    const id = String(cuerpo.copia ?? "");
    const k = equipo.resumen?.copias?.find((x) => x.id === id);
    if (!k) return;
    const desde = Date.parse(o.emitida);
    copia = { estado: "en_marcha" };
    const inicio = Date.now();
    vigilancia = setInterval(async () => {
      try {
        const e = await enFondo(() => api.equipo(cliente.id, equipo.id));
        // Con el resumen de ahora (su `ultima`), no el de cuando se abrió el diálogo.
        const v = ultimaVuelta(e.resumen?.copias?.find((x) => x.id === id) ?? k, e.ultimo_informe);
        if (v && Date.parse(v.cuando) > desde) {
          const inf = infCopia(informeDe(e.ultimo_informe, k.repo), k.id);
          const ej = inf?.ejecuciones.find((x) => x.hora === v.cuando);
          const ver = ej && inf ? versionDeVuelta(inf.versiones, ej) : null;
          const dur = ej?.duracion_s ?? ver?.duracion_s;
          const anadido = ej?.anadido ?? (ver ? anadidoDe(ver) : null);
          const meta = [dur != null ? duracion(dur) : null, anadido && v.resultado !== "sin_cambios" ? `+${bytes(anadido)}` : null].filter(Boolean).join(" · ");
          copia = { estado: "fin", vuelta: v, meta };
          if (vigilancia) clearInterval(vigilancia);
          vigilancia = null;
          void cargarCliente(cliente.id, { silencioso: true });
        }
      } catch {
        /* se reintenta */
      }
      if (vigilancia && Date.now() - inicio > 6 * 60_000) {
        clearInterval(vigilancia);
        vigilancia = null;
        copia = { estado: "sin_noticias" };
      }
    }, 4000);
  }

  function parar() {
    if (sondeo) clearInterval(sondeo);
    sondeo = null;
    fase = "fin";
  }
  onDestroy(() => {
    if (sondeo) clearInterval(sondeo);
    if (vigilancia) clearInterval(vigilancia);
    olvidar();
  });

  function cerrar() {
    olvidar();
    // Si aún no ha terminado, se sigue en segundo plano y se avisa al acabar.
    if (orden && fase === "esperando") seguirEnFondo(cliente.id, equipo, orden);
    if (orden && copia?.estado === "en_marcha") {
      const k = equipo.resumen?.copias?.find((x) => x.id === String(cuerpo.copia ?? ""));
      if (k) vigilarCopiaEnFondo(cliente.id, equipo, k, Date.parse(orden.emitida));
    }
    onclose();
  }
</script>

<Modal labelledby="t-orden" onclose={cerrar} width={500} dismissible={fase !== "enviando"}>
  <form class="form" onsubmit={enviar}>
    <div class="dlg-title">
      <span class="ticon" class:danger={destructiva}>
        {#if pideAdmin}<KeyRound size={18} />{:else if pideRepo}<LockKeyhole size={18} />{:else}<Send size={18} />{/if}
      </span>
      <div>
        <h2 id="t-orden">{titulo ?? nombreOrden(tipo)}</h2>
        <p><strong>{equipo.nombre}</strong> · {cliente.nombre}</p>
      </div>
    </div>

    {#if llavesCambiadas}
      <AlertaLlaves {equipo} cliente={cliente.id} />
      <footer><button type="button" class="btn btn-primary" onclick={cerrar}>Entendido</button></footer>
    {:else if fase === "pedir" || fase === "enviando"}
      <p class="desc">{descripcion}</p>
      {@render campos?.()}

      {#if destructiva}
        <div class="notice notice-warn">
          <Clock size={16} />
          <p>
            Esta orden reduce la protección o borra copias, así que <strong>espera {espera} h</strong> antes de aplicarse (hacia el {fechaLarga(cuando.toISOString())}). Hasta entonces, cualquiera del cliente puede cancelarla.
            <Ayuda id="espera" />
          </p>
        </div>
      {/if}

      {#if pideRepo}
        <CampoClave requerido id="clave-repo" etiqueta={repo ? `Contraseña del repositorio «${repo.nombre}»` : "Contraseña del repositorio"} bind:value={contrasenaRepo} autofocus ayuda="Está en el kit de recuperación. Va cifrada solo para el equipo, que la comprueba.">
          {#snippet extra()}<Ayuda id="contrasena-repo" />{/snippet}
        </CampoClave>
      {/if}
      {#if pideAdmin}
        {#if fijarPrimero && !necesitaAdmin(tipo)}
          <p class="faint nota-fijar">Es la primera vez que este navegador manda una contraseña a {equipo.nombre}: con la clave de administración se comprueba que sus llaves son las auténticas y se recuerdan para las próximas veces.</p>
        {/if}
        <CampoClave requerido id="clave-admin" etiqueta="Clave de administración de {cliente.nombre}" bind:value={claveAdmin} autofocus={!pideRepo} error={errorAdmin} ayuda="El servidor nunca la ve: se comprueba en el equipo.">
          {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
        </CampoClave>
      {/if}

      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      {#if prueba}
        <div class="notice {prueba.ok ? 'notice-success' : 'notice-danger'}" role="status">
          {#if prueba.ok}<ShieldCheck size={16} />{:else}<TriangleAlert size={16} />{/if}
          <p><strong>{prueba.ok ? "Prueba superada." : "La prueba falló."}</strong> {prueba.mensaje}</p>
        </div>
      {/if}

      <footer>
        {#if fase === "enviando" || probando}
          <span class="paso" role="status"><LoaderCircle size={15} class="spin" />{paso}</span>
        {/if}
        <button type="button" class="btn btn-ghost" onclick={cerrar} disabled={fase === "enviando" || probando}>Cancelar</button>
        {#if probar}
          <BotonCargando type="button" class="btn" disabled={!listo || fase === "enviando"} cargando={probando} textoCargando="Probando…" onclick={probarAhora}>{probar.texto ?? "Probar"}</BotonCargando>
        {/if}
        <BotonCargando class="btn {destructiva ? 'btn-danger' : 'btn-primary'}" disabled={!listo || probando} cargando={fase === "enviando"} textoCargando="Enviando…">{textoBoton}</BotonCargando>
      </footer>
    {:else if orden}
      <div class="resultado">
        {#if orden.not_before && orden.estado === "pendiente"}
          <Chip tono="warn" texto="Esperando su turno" />
          <p>Se aplicará dentro de <strong>{cuentaAtras(orden.not_before)}</strong>, salvo que alguien la cancele desde <a href="/c/{cliente.id}/ordenes">Órdenes</a>. Todo el cliente ha recibido un aviso.</p>
        {:else}
          <Chip tono={ESTADO_ORDEN[orden.estado].tono} texto={ESTADO_ORDEN[orden.estado].texto} girando={fase === "esperando"} />
          <p>
            {#if fase === "esperando"}
              {orden.estado === "pendiente" ? (equipo.conectado ? "Enviada. Esperando a que el equipo la recoja…" : "Enviada. El equipo la recogerá en cuanto se conecte.") : "El equipo está en ello…"}
              <span class="faint">Puedes cerrar: te avisaremos al terminar.</span>
            {:else}
              {orden.mensaje ? mensajeOrden(orden.tipo, orden.mensaje) : "Sin mensaje del equipo."}
            {/if}
          </p>
          {#if orden.firma_agente}
            <p class="firma" class:ok={firmada}>
              <ShieldCheck size={14} />{firmada ? "Respuesta firmada por el equipo" : "La firma de la respuesta no es válida: no te fíes de este resultado"}<Ayuda id="firma-equipo" />
            </p>
          {/if}
        {/if}
        {#if copia}
          <div class="vuelta" role="status" aria-live="polite">
            {#if copia.estado === "en_marcha"}
              <Chip tono="info" texto="Copiando" girando />
              <p>Esperando a que {equipo.nombre} diga cómo terminó. Puedes cerrar esta ventana: la copia sigue en el equipo y te avisaremos al terminar.</p>
            {:else if copia.estado === "fin" && copia.vuelta}
              <Chip tono={TONO_RESULTADO[copia.vuelta.resultado]} texto={copia.vuelta.resultado === "ok" ? "Copia terminada" : TEXTO_RESULTADO[copia.vuelta.resultado]} />
              <p>
                {copia.vuelta.resultado === "sin_cambios" ? "No había nada nuevo: no hizo falta guardar otra versión." : copia.vuelta.resultado === "fallo" ? (copia.vuelta.mensaje ?? "La copia no terminó bien.") : copia.vuelta.resultado === "aviso" ? (copia.vuelta.mensaje ?? "Terminó con avisos.") : "Guardada una versión nueva."}
                {#if copia.meta}<span class="faint">{" "}({copia.meta})</span>{/if}
              </p>
            {:else}
              <p class="faint">El equipo aún no ha informado de esta copia. La verás en la página de la copia en unos minutos.</p>
            {/if}
            <a class="link" href="/c/{cliente.id}/equipos/{equipo.id}/copias/{encodeURIComponent(String(cuerpo.copia ?? ''))}" onclick={cerrar}>Ver la copia</a>
          </div>
          <!-- Fuera de la región «status»: las cifras cambian cada pocos segundos y no se anuncian. -->
          {#if copia.estado === "en_marcha"}<EnMarcha equipo={equipo.id} copia={String(cuerpo.copia ?? "")} />{/if}
        {/if}
        <p class="faint seq">Orden n.º {orden.seq} <Ayuda id="seq" /></p>
      </div>
      <footer>
        <button type="button" class="btn btn-primary" onclick={cerrar}>Cerrar</button>
      </footer>
    {/if}
  </form>
</Modal>

<style>
  .nota-fijar {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .desc {
    margin: 0;
    color: var(--text-1);
  }
  .paso {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .resultado {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-3);
  }
  .resultado p {
    margin: 0;
  }
  .firma {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
    color: var(--bad);
  }
  .firma.ok {
    color: var(--ok);
  }
  .vuelta {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-2);
    width: 100%;
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .vuelta a {
    font-size: var(--fs-sm);
    font-weight: 500;
  }
  .seq {
    display: inline-flex;
    align-items: center;
    font-size: var(--fs-xs);
  }
</style>
