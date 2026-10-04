<script lang="ts">
  // Canales de notificación de un ámbito (el servidor o un cliente): correo,
  // webhook, ntfy y Telegram (api-servidor.md §13). Los secretos nunca vuelven
  // del servidor: aquí solo se sabe si están puestos («Configurado»), y para
  // cambiar a dónde va un canal o un secreto hace falta un código de la
  // aplicación de autenticación (el servidor lo comprueba).
  import { BellRing, CircleAlert, Mail, MoonStar, Pencil, Plus, Power, Send, Trash2, Webhook } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { ApiError, type AmbitoNotif } from "$lib/api";
  import { app } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { cambioSensible, reglasPorDefecto, SECRETOS, SEVERIDAD, SEVERIDADES, textoSeveridades, TIPO_CANAL } from "$lib/notificaciones";
  import type { CambioCanal, CanalNotif, ConfigCanal, ReglasCanal, Severidad, TipoCanal } from "$lib/tipos";
  import BotonCargando from "$lib/componentes/BotonCargando.svelte";
  import CampoClave from "$lib/componentes/CampoClave.svelte";
  import CampoCodigo from "$lib/componentes/CampoCodigo.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import MenuAcciones, { type AccionMenu } from "$lib/componentes/MenuAcciones.svelte";

  let {
    ambito,
    canales = $bindable(),
    /** Qué decir si no hay ninguno. */
    vacio = "Aún no hay canales.",
    alCambiar,
  }: { ambito: AmbitoNotif; canales: CanalNotif[]; vacio?: string; alCambiar?: () => void } = $props();

  const delServidor = $derived(!("cliente" in ambito));
  const ICONO = { correo: Mail, webhook: Webhook, ntfy: BellRing, telegram: Send };
  const hayCorreo = $derived(canales.some((c) => c.tipo === "correo"));
  const NUEVO: Record<TipoCanal, string> = { correo: "Añadir el correo", webhook: "Añadir un webhook", ntfy: "Añadir ntfy", telegram: "Añadir Telegram" };
  const SILENCIO = { desde: "22:00", hasta: "07:00", salvo_criticos: true };

  function destino(c: CanalNotif): string {
    switch (c.tipo) {
      case "correo":
        return c.config.host ? `${c.config.host}:${c.config.puerto ?? ""} · desde ${c.config.remitente ?? ""}` : "Sin servidor de correo";
      case "telegram":
        return c.config.chat_id ? `Chat ${c.config.chat_id}` : "Sin chat";
      default:
        return c.config.servidor ?? "Sin dirección";
    }
  }
  function que(c: CanalNotif): string {
    if (c.tipo === "correo") return "A cada persona, según sus preferencias";
    const partes = [textoSeveridades(c.reglas.severidades)];
    if (delServidor) partes.push(c.reglas.clientes ? `${c.reglas.clientes.length} ${c.reglas.clientes.length === 1 ? "cliente" : "clientes"}` : "todos los clientes");
    const res = [c.reglas.resumen_diario && "diario", c.reglas.resumen_semanal && "semanal"].filter(Boolean);
    if (res.length) partes.push(`resumen ${res.join(" y ")}`);
    return partes.join(" · ");
  }

  // ---- Probar ----
  let probando = $state<string | null>(null);
  async function probar(c: CanalNotif) {
    probando = c.id;
    try {
      const r = await api.probarCanal(ambito, c.id);
      avisar(r.ok ? (c.tipo === "correo" ? `Prueba enviada a ${app.cuenta?.correo ?? "tu correo"}. Mira si ha llegado.` : "Prueba enviada. Mira si ha llegado.") : `No se pudo enviar: ${r.mensaje}`, r.ok ? "ok" : "bad");
    } catch (e) {
      fallo(e);
    } finally {
      probando = null;
    }
  }

  async function encender(c: CanalNotif) {
    try {
      const n = await api.cambiarCanal(ambito, c.id, { activo: !c.activo });
      canales = canales.map((x) => (x.id === n.id ? n : x));
      alCambiar?.();
      avisar(n.activo ? `«${n.nombre}» encendido.` : `«${n.nombre}» apagado: no manda nada hasta que lo enciendas.`);
    } catch (e) {
      fallo(e);
    }
  }

  let quitar = $state<CanalNotif | null>(null);
  async function confirmarQuitar() {
    if (!quitar) return;
    try {
      await api.borrarCanal(ambito, quitar.id);
      canales = canales.filter((x) => x.id !== quitar!.id);
      alCambiar?.();
      avisar(`«${quitar.nombre}» quitado.`);
      quitar = null;
    } catch (e) {
      fallo(e);
    }
  }

  const menuDe = (c: CanalNotif): AccionMenu[][] => [
    [
      { texto: "Cambiar…", icono: Pencil, onclick: () => abrir(c) },
      { texto: c.activo ? "Apagar" : "Encender", icono: Power, onclick: () => void encender(c) },
    ],
    [{ texto: "Quitar", icono: Trash2, peligro: true, onclick: () => (quitar = c) }],
  ];

  // ---- Crear o cambiar ----
  let dlg = $state(false);
  let previo = $state<CanalNotif | null>(null);
  let tipo = $state<TipoCanal>("correo");
  let nombre = $state("");
  let config = $state<ConfigCanal>({});
  let secretos = $state<Record<string, string>>({});
  let quitarSecreto = $state<Record<string, boolean>>({});
  let reglas = $state<ReglasCanal>(reglasPorDefecto());
  let conSilencio = $state(false);
  let soloAlgunos = $state(false);
  let codigo = $state("");
  let pedirCodigo = $state(false);
  let error = $state("");
  let errorCodigo = $state("");
  let guardando = $state(false);

  function abrir(c: CanalNotif | null, t: TipoCanal = "correo") {
    previo = c;
    tipo = c?.tipo ?? t;
    nombre = c?.nombre ?? "";
    config = c ? { ...c.config } : t === "correo" ? { seguridad: "starttls", puerto: 587 } : {};
    // Todos los campos con valor (los enlazados con `bind:` no pueden empezar sin él).
    secretos = Object.fromEntries(SECRETOS[tipo].map((x) => [x.campo, ""]));
    quitarSecreto = Object.fromEntries(SECRETOS[tipo].map((x) => [x.campo, false]));
    reglas = c ? structuredClone($state.snapshot(c.reglas)) : reglasPorDefecto();
    conSilencio = !!reglas.silencio;
    soloAlgunos = !!reglas.clientes;
    codigo = "";
    pedirCodigo = false;
    error = "";
    errorCodigo = "";
    dlg = true;
  }

  /** Lo que se manda: solo lo que cambia (los secretos vacíos se quedan como estaban). */
  const cambio = $derived.by((): CambioCanal => {
    const s: Record<string, string> = {};
    for (const [k, v] of Object.entries(secretos)) if (v.trim()) s[k] = v.trim();
    for (const [k, q] of Object.entries(quitarSecreto)) if (q) s[k] = "";
    const r: ReglasCanal = { ...reglas, silencio: conSilencio ? (reglas.silencio ?? { ...SILENCIO }) : null, clientes: delServidor && soloAlgunos ? (reglas.clientes ?? []) : null };
    const c: CambioCanal = { nombre: nombre.trim() || undefined, reglas: tipo === "correo" ? undefined : r };
    if (!previo) c.tipo = tipo;
    if (tipo === "correo" || tipo === "telegram") c.config = $state.snapshot(config) as ConfigCanal;
    if (Object.keys(s).length) c.secretos = s;
    return c;
  });
  const sensible = $derived(cambioSensible(previo, cambio));
  const faltan = $derived.by(() => {
    const puesto = (campo: string) => !!secretos[campo]?.trim() || (!!previo?.secretos[campo] && !quitarSecreto[campo]);
    const f: string[] = [];
    if (tipo === "correo" && (!config.host?.trim() || !config.remitente?.trim())) f.push("el servidor y el remitente");
    if (tipo === "telegram" && !config.chat_id?.trim()) f.push("el chat");
    for (const s of SECRETOS[tipo]) if (s.requerido && !puesto(s.campo)) f.push(s.etiqueta.toLowerCase());
    return f;
  });

  function marcar(sev: Severidad, si: boolean) {
    reglas.severidades = si ? [...new Set([...reglas.severidades, sev])] : reglas.severidades.filter((x) => x !== sev);
  }
  function marcarCliente(id: string, si: boolean) {
    const l = reglas.clientes ?? [];
    reglas.clientes = si ? [...new Set([...l, id])] : l.filter((x) => x !== id);
  }

  async function guardar(ev: SubmitEvent) {
    ev.preventDefault();
    if (faltan.length || guardando) return;
    const cifras = codigo.replace(/\D/g, "");
    if ((sensible || pedirCodigo) && cifras.length !== 6) {
      pedirCodigo = true;
      errorCodigo = "Escribe el código de 6 cifras de tu aplicación.";
      return;
    }
    guardando = true;
    error = "";
    errorCodigo = "";
    try {
      const b = { ...cambio, ...(sensible || pedirCodigo ? { codigo: cifras } : {}) };
      const n = previo ? await api.cambiarCanal(ambito, previo.id, b) : await api.crearCanal(ambito, b);
      canales = previo ? canales.map((x) => (x.id === n.id ? n : x)) : [...canales, n];
      alCambiar?.();
      secretos = {};
      dlg = false;
      avisar(previo ? `«${n.nombre}» guardado.` : `«${n.nombre}» añadido. Pulsa «Enviar prueba» para comprobarlo.`);
    } catch (e) {
      if (e instanceof ApiError && e.codigo === "codigo") {
        pedirCodigo = true;
        errorCodigo = e.message;
        codigo = "";
      } else {
        error = (e as Error).message;
      }
    } finally {
      guardando = false;
    }
  }
  function cerrar() {
    secretos = {};
    dlg = false;
  }
</script>

<div class="canales">
  {#if canales.length}
    <div class="card p-0 lista">
      {#each canales as c (c.id)}
        {@const Icono = ICONO[c.tipo]}
        <div class="fila">
          <span class="ico" class:apagado={!c.activo} aria-hidden="true"><Icono size={16} /></span>
          <span class="fila-texto">
            <span class="fila-titulo">
              {c.nombre}
              <span class="tipo">{TIPO_CANAL[c.tipo].texto}</span>
              {#if !c.activo}<Chip pequeno tono="neutral" texto="Apagado" />{:else if !c.completo}<Chip pequeno tono="warn" texto="Faltan datos" />{/if}
              {#if c.reglas.silencio && c.tipo !== "correo"}<span class="silencio" title="Horas de silencio"><MoonStar size={12} />{c.reglas.silencio.desde}–{c.reglas.silencio.hasta}</span>{/if}
            </span>
            <span class="fila-sub">{destino(c)} · {que(c)}</span>
          </span>
          <BotonCargando class="btn btn-sm" cargando={probando === c.id} textoCargando="Enviando…" disabled={!c.activo || !c.completo} onclick={() => probar(c)}>
            <Send size={14} />Enviar prueba
          </BotonCargando>
          <MenuAcciones etiqueta="Acciones para {c.nombre}" texto="" grupos={menuDe(c)} />
        </div>
      {/each}
    </div>
  {:else}
    <p class="faint vacio">{vacio}</p>
  {/if}
  <div class="anadir">
    {#each ["correo", "webhook", "ntfy", "telegram"] as t (t)}
      {@const Icono = ICONO[t as TipoCanal]}
      {#if t !== "correo" || !hayCorreo}
        <button class="btn btn-sm" onclick={() => abrir(null, t as TipoCanal)}><Plus size={14} /><Icono size={14} />{TIPO_CANAL[t as TipoCanal].texto}</button>
      {/if}
    {/each}
  </div>
</div>

{#if dlg}
  {@const Icono = ICONO[tipo]}
  <Modal labelledby="t-canal" onclose={cerrar} width={600} dismissible={false}>
    <form class="form" onsubmit={guardar} autocomplete="off">
      <div class="dlg-title">
        <span class="ticon"><Icono size={18} /></span>
        <div>
          <h2 id="t-canal">{previo ? `Cambiar «${previo.nombre}»` : NUEVO[tipo]}</h2>
          <p>{TIPO_CANAL[tipo].que}</p>
        </div>
      </div>

      <div class="field">
        <label class="field-label" for="cn-nombre">Nombre</label>
        <input id="cn-nombre" class="input" bind:value={nombre} maxlength="80" placeholder={TIPO_CANAL[tipo].texto} />
      </div>

      {#if tipo === "correo"}
        <div class="dos">
          <div class="field">
            <label class="field-label" for="cn-host">Servidor de correo (SMTP)</label>
            <input id="cn-host" class="input mono" bind:value={config.host} placeholder="smtp.empresa.com" spellcheck="false" required />
          </div>
          <div class="field">
            <label class="field-label" for="cn-seg">Seguridad</label>
            <select
              id="cn-seg"
              class="input"
              bind:value={config.seguridad}
              onchange={() => (config.puerto = config.seguridad === "tls" ? 465 : config.seguridad === "starttls" ? 587 : (config.puerto ?? 25))}
            >
              <option value="starttls">STARTTLS (puerto 587)</option>
              <option value="tls">TLS (puerto 465)</option>
              <option value="ninguna">Sin cifrar (solo en este equipo)</option>
            </select>
          </div>
        </div>
        <div class="dos">
          <div class="field">
            <label class="field-label" for="cn-puerto">Puerto</label>
            <input id="cn-puerto" class="input" type="number" min="1" max="65535" bind:value={config.puerto} />
          </div>
          <div class="field">
            <label class="field-label" for="cn-usuario">Usuario</label>
            <input id="cn-usuario" class="input" bind:value={config.usuario} placeholder="avisos@empresa.com" autocomplete="off" spellcheck="false" />
          </div>
        </div>
        <div class="field">
          <label class="field-label" for="cn-remitente">Remitente</label>
          <input id="cn-remitente" class="input" bind:value={config.remitente} placeholder="Resguardo <copias@empresa.com>" spellcheck="false" required />
          <span class="field-hint">Como aparecerá en el «De:». Tiene que poder enviar con ese usuario.</span>
        </div>
      {:else if tipo === "telegram"}
        <div class="field">
          <label class="field-label" for="cn-chat">Chat</label>
          <input id="cn-chat" class="input mono" bind:value={config.chat_id} placeholder="-1001234567890 o @mi_canal" spellcheck="false" required />
          <span class="field-hint">Añade el bot al grupo y pon aquí el número del chat (empieza por -100 en los grupos) o el @ del canal.</span>
        </div>
      {/if}

      {#each SECRETOS[tipo] as s (s.campo)}
        {@const puesto = !!previo?.secretos[s.campo]}
        <CampoClave
          id="cn-{s.campo}"
          etiqueta={s.etiqueta}
          bind:value={secretos[s.campo]}
          autocomplete="new-password"
          requerido={s.requerido && !puesto}
          ayuda={puesto ? `Configurado. Déjalo vacío para no cambiarlo. ${s.ayuda}` : s.ayuda}
        />
        {#if puesto && !s.requerido}
          <label class="switch-row quitar"><input type="checkbox" bind:checked={quitarSecreto[s.campo]} /><span>Quitar {s.etiqueta.toLowerCase()}</span></label>
        {/if}
      {/each}

      {#if tipo !== "correo"}
        <fieldset class="grupo">
          <legend>Qué manda al momento</legend>
          {#each SEVERIDADES as sev (sev)}
            <label class="check">
              <input type="checkbox" checked={reglas.severidades.includes(sev)} onchange={(e) => marcar(sev, e.currentTarget.checked)} />
              <span><strong>{SEVERIDAD[sev].texto}</strong> <span class="faint">{SEVERIDAD[sev].que}</span></span>
            </label>
          {/each}
          <label class="check"><input type="checkbox" bind:checked={reglas.resumen_diario} /><span><strong>Resumen diario</strong></span></label>
          <label class="check"><input type="checkbox" bind:checked={reglas.resumen_semanal} /><span><strong>Resumen semanal</strong> <span class="faint">Cómo están las copias de cada equipo.</span></span></label>
        </fieldset>
        {#if delServidor}
          <fieldset class="grupo">
            <legend>De qué clientes</legend>
            <label class="check"><input type="radio" name="cn-clientes" checked={!soloAlgunos} onchange={() => (soloAlgunos = false)} /><span>De todos (también los que se creen después)</span></label>
            <label class="check"><input type="radio" name="cn-clientes" checked={soloAlgunos} onchange={() => (soloAlgunos = true)} /><span>Solo de estos</span></label>
            {#if soloAlgunos}
              <div class="clientes">
                {#each app.clientes as cl (cl.id)}
                  <label class="check"><input type="checkbox" checked={reglas.clientes?.includes(cl.id) ?? false} onchange={(e) => marcarCliente(cl.id, e.currentTarget.checked)} /><span>{cl.nombre}</span></label>
                {/each}
              </div>
            {/if}
          </fieldset>
        {/if}
        <fieldset class="grupo">
          <legend>Horas de silencio</legend>
          <label class="switch-row">
            <input class="switch" type="checkbox" bind:checked={conSilencio} onchange={() => conSilencio && !reglas.silencio && (reglas.silencio = { ...SILENCIO })} />
            <span><strong>Guardar lo que llegue de noche</strong><span class="faint">Sale agrupado al terminar (hora del servidor).</span></span>
          </label>
          {#if conSilencio && reglas.silencio}
            <div class="horas">
              <label>De <input class="input" type="time" bind:value={reglas.silencio.desde} required /></label>
              <label>a <input class="input" type="time" bind:value={reglas.silencio.hasta} required /></label>
              <label class="check"><input type="checkbox" bind:checked={reglas.silencio.salvo_criticos} /><span>Los críticos, al momento</span></label>
            </div>
          {/if}
        </fieldset>
      {/if}

      {#if sensible || pedirCodigo}
        <div class="codigo">
          <p class="faint pequeno">
            {previo ? "Cambias a dónde va o un secreto:" : "Para añadir un canal,"} escribe un código de <strong>tu</strong> aplicación de verificación.
          </p>
          <CampoCodigo bind:value={codigo} error={errorCodigo} />
        </div>
      {/if}
      {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if faltan.length}<span class="faint pequeno falta">Falta {faltan.join(", ")}.</span>{/if}
        <button type="button" class="btn btn-ghost" onclick={cerrar}>Cancelar</button>
        <BotonCargando class="btn btn-primary" type="submit" cargando={guardando} textoCargando="Guardando…" disabled={faltan.length > 0}>{previo ? "Guardar" : "Añadir"}</BotonCargando>
      </footer>
    </form>
  </Modal>
{/if}

{#if quitar}
  <Modal labelledby="t-quitar-canal" onclose={() => (quitar = null)} width={440}>
    <div class="dlg-title">
      <span class="ticon danger"><Trash2 size={18} /></span>
      <div>
        <h2 id="t-quitar-canal">¿Quitar «{quitar.nombre}»?</h2>
        <p>Deja de mandar avisos por aquí y se borran sus secretos. Lo que esté en cola para este canal no saldrá.</p>
      </div>
    </div>
    <footer>
      <button class="btn btn-ghost" onclick={() => (quitar = null)}>Cancelar</button>
      <button class="btn btn-danger" onclick={confirmarQuitar}>Quitar</button>
    </footer>
  </Modal>
{/if}

<style>
  .canales {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .card.lista {
    overflow: visible;
  }
  .ico {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    color: var(--accent-text);
    background: var(--accent-soft);
    border-radius: var(--radius);
  }
  .ico.apagado {
    color: var(--text-3);
    background: var(--surface-3);
  }
  .fila-titulo {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .tipo {
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--text-3);
  }
  .silencio {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .vacio {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .anadir {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }
  .dos {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--sp-3);
  }
  .mono {
    font-family: var(--mono);
  }
  .grupo {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: var(--sp-3) var(--sp-4);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .grupo legend {
    padding: 0 4px;
    font-size: var(--fs-sm);
    font-weight: 600;
  }
  .check {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  .check input {
    margin-top: 3px;
  }
  .check .faint {
    font-size: var(--fs-xs);
  }
  .clientes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 4px 12px;
    max-height: 180px;
    overflow: auto;
    padding: 4px 0 0 24px;
  }
  .horas {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3);
    font-size: var(--fs-sm);
  }
  .horas > label:not(.check) {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .horas .input {
    width: 120px;
  }
  .quitar {
    margin-top: -6px;
  }
  .codigo {
    padding: var(--sp-3) var(--sp-4);
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .pequeno {
    margin: 0 0 6px;
    font-size: var(--fs-xs);
  }
  .falta {
    margin: 0 auto 0 0;
  }
  @media (max-width: 560px) {
    .dos {
      grid-template-columns: 1fr;
    }
    .fila {
      flex-wrap: wrap;
    }
  }
</style>
