<script lang="ts">
  // Ajustes de una etiqueta (v1.4x, tarea 6): su color (de la misma paleta,
  // siempre con su nombre escrito), la plantilla de copia que se propone a un
  // equipo nuevo con ella y cómo se avisa de sus equipos. Los cambian
  // administradores y propietarios; los avisos, solo el propietario (como el
  // resto de las notificaciones del cliente).
  //
  // Las plantillas van cifradas con la clave de administración: para elegir
  // una hay que escribirla (aquí solo se guarda su id, nunca su contenido).
  import { BellRing, KeyRound, LayoutTemplate, Palette, RotateCcw, Tag } from "@lucide/svelte";
  import { untrack } from "svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { actual, puede } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { borrar } from "$lib/cripto/bytes";
  import { kcfgDelCliente } from "$lib/ordenar";
  import { cargarPlantillas, type Plantilla } from "$lib/plantillas";
  import { ajusteDe, colorPorNombre, conAvisos, cuerpoAjuste, N_COLORES, NOMBRES_COLOR } from "$lib/etiquetasGrupos";
  import { plural } from "$lib/formato";
  import { TIPO_CANAL } from "$lib/notificaciones";
  import type { CanalRef, NotifCliente, Severidad, TipoCanal } from "$lib/tipos";
  import EtiquetaChip from "./EtiquetaChip.svelte";
  import BotonCargando from "./BotonCargando.svelte";
  import CampoClave from "./CampoClave.svelte";

  let { nombre, n = 0, onclose }: { nombre: string; n?: number; onclose: () => void } = $props();

  const previo = untrack(() => ajusteDe(nombre, actual.etiquetas));
  const rol = $derived(actual.cliente?.rol);
  const esPropietario = $derived(puede.propietario(rol));
  const automatico = untrack(() => colorPorNombre(nombre));

  // --- Color ------------------------------------------------------------------
  /** -1: el automático (por el nombre). */
  let color = $state<number>(untrack(() => previo?.color ?? -1));
  const colorFinal = $derived(color >= 0 ? color : automatico);

  // --- Plantilla por defecto -------------------------------------------------
  let plantilla = $state<string>(untrack(() => previo?.plantilla ?? ""));
  let plantillas = $state<Plantilla[] | null>(null);
  let pidiendoClave = $state(false);
  let clave = $state("");
  let errorClave = $state("");
  let abriendo = $state(false);
  const nombrePlantilla = $derived(plantillas?.find((p) => p.id === plantilla)?.nombre);
  async function abrirPlantillas() {
    if (!actual.cliente || abriendo) return;
    if (!clave) return void (errorClave = "Escribe la clave de administración.");
    errorClave = "";
    abriendo = true;
    let kcfg: Uint8Array | null = null;
    try {
      kcfg = await kcfgDelCliente(actual.cliente, clave);
      const r = await cargarPlantillas(actual.id, kcfg);
      if (!r.lista.length && r.ilegibles) throw new Error("La clave de administración no es correcta: no abre ninguna plantilla.");
      plantillas = r.lista;
      pidiendoClave = false;
      clave = "";
    } catch (err) {
      errorClave = (err as Error).message;
    } finally {
      borrar(kcfg);
      abriendo = false;
    }
  }

  // --- Avisos (solo el propietario) -------------------------------------------
  let critico = $state(untrack(() => previo?.avisos?.importancia === "critico"));
  let canales = $state<CanalRef[]>(untrack(() => [...(previo?.avisos?.canales ?? [])]));
  let notif = $state<NotifCliente | null>(null);
  $effect(() => {
    const c = actual.id;
    if (!esPropietario || !c) return;
    untrack(() => api.notifCliente(c).then((x) => (notif = x), () => (notif = null)));
  });
  /** Los canales compartidos que pueden recibir sus avisos (webhook, ntfy, Telegram). */
  const posibles = $derived<{ ref: CanalRef; nombre: string; tipo: TipoCanal; de: string }[]>([
    ...(notif?.canales ?? []).filter((k) => k.tipo !== "correo" && k.activo).map((k) => ({ ref: { ambito: "cliente" as const, id: k.id }, nombre: k.nombre, tipo: k.tipo, de: "del cliente" })),
    ...(notif?.servidor.canales ?? []).filter((k) => !!k.id && k.tipo !== "correo").map((k) => ({ ref: { ambito: "servidor" as const, id: k.id! }, nombre: k.nombre, tipo: k.tipo, de: "del servidor" })),
  ]);
  const marcado = (r: CanalRef) => canales.some((x) => x.ambito === r.ambito && x.id === r.id);
  function marcar(r: CanalRef, si: boolean) {
    canales = si ? [...canales.filter((x) => !(x.ambito === r.ambito && x.id === r.id)), r] : canales.filter((x) => !(x.ambito === r.ambito && x.id === r.id));
  }
  const avisos = $derived(esPropietario ? { importancia: (critico ? "critico" : null) as Severidad | null, canales } : (previo?.avisos ?? null));

  // --- Guardar ----------------------------------------------------------------
  let guardando = $state(false);
  let error = $state("");
  async function guardar(e: SubmitEvent | null, todoPorDefecto = false) {
    e?.preventDefault();
    guardando = true;
    error = "";
    try {
      const cuerpo = todoPorDefecto ? cuerpoAjuste(nombre, null, null, esPropietario ? null : previo?.avisos) : cuerpoAjuste(nombre, color >= 0 ? color : null, plantilla || null, avisos);
      actual.etiquetas = await api.ponerAjusteEtiqueta(actual.id, cuerpo);
      avisar(todoPorDefecto ? `«${nombre}» vuelve a lo de siempre.` : `Ajustes de «${nombre}» guardados.`);
      onclose();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      guardando = false;
    }
  }
</script>

<Modal labelledby="t-ajustes-et" {onclose} width={540}>
  <form class="form" onsubmit={(e) => guardar(e)}>
    <div class="dlg-title">
      <span class="ticon"><Tag size={18} /></span>
      <div>
        <h2 id="t-ajustes-et">Etiqueta «{nombre}»</h2>
        <p>{n ? `${plural(n, "equipo la lleva", "equipos la llevan")}. ` : ""}Su color sale en todas las pantallas del cliente; la plantilla y los avisos valen para todos sus equipos.</p>
      </div>
    </div>

    <fieldset class="grupo">
      <legend><Palette size={14} />Color</legend>
      <div class="colores">
        <label class="op" style:--et="var(--et-{automatico})">
          <input type="radio" name="color-et" value={-1} bind:group={color} />
          <span class="punto" aria-hidden="true"></span>Automático <span class="faint">({NOMBRES_COLOR[automatico].toLowerCase()})</span>
        </label>
        {#each Array.from({ length: N_COLORES }, (_, i) => i) as i (i)}
          <label class="op" style:--et="var(--et-{i})">
            <input type="radio" name="color-et" value={i} bind:group={color} />
            <span class="punto" aria-hidden="true"></span>{NOMBRES_COLOR[i]}
          </label>
        {/each}
      </div>
      <p class="faint pequeno">Así se verá: <span class="muestra" style:--et-elegido="var(--et-{colorFinal})"><EtiquetaChip {nombre} /></span> El nombre siempre va escrito: el color solo ayuda a reconocerla.</p>
    </fieldset>

    <fieldset class="grupo">
      <legend><LayoutTemplate size={14} />Plantilla para los equipos nuevos</legend>
      <p class="faint pequeno">A un equipo con «{nombre}» que aún no tiene copias se le propone esta plantilla. Nunca se aplica sola: se revisa y se envía con la clave de administración.</p>
      {#if plantillas}
        <div class="field">
          <label class="field-label" for="et-plantilla">Plantilla</label>
          <select id="et-plantilla" class="input" bind:value={plantilla}>
            <option value="">Ninguna</option>
            {#each plantillas as p (p.id)}<option value={p.id}>{p.nombre}</option>{/each}
            {#if plantilla && !nombrePlantilla}<option value={plantilla}>La que tenía (ya no existe)</option>{/if}
          </select>
          {#if !plantillas.length}<span class="field-hint">No hay plantillas todavía: se guardan desde «Cambiar las copias» de un equipo («Guardar como plantilla…»).</span>{/if}
        </div>
      {:else if pidiendoClave}
        <!-- Intro aquí abre las plantillas (no guarda los ajustes). -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="clave-pl" onkeydown={(e) => e.key === "Enter" && (e.preventDefault(), void abrirPlantillas())}>
          <CampoClave requerido id="et-clave" etiqueta="Clave de administración" bind:value={clave} error={errorClave} ayuda="Las plantillas van cifradas: solo se ven con la clave del cliente." autofocus />
          <div class="fila-botones">
            <button type="button" class="btn btn-ghost btn-sm" onclick={() => ((pidiendoClave = false), (clave = ""))}>Cancelar</button>
            <BotonCargando type="button" class="btn btn-sm" cargando={abriendo} textoCargando="Abriendo…" onclick={abrirPlantillas}>Ver las plantillas</BotonCargando>
          </div>
        </div>
      {:else}
        <div class="fila-pl">
          <span>{plantilla ? "Tiene una plantilla elegida (escribe la clave para ver cuál)." : "Ninguna."}</span>
          {#if puede.administrar(rol)}<button type="button" class="btn btn-sm" onclick={() => (pidiendoClave = true)}><KeyRound size={14} />{plantilla ? "Cambiar…" : "Elegir…"}</button>{/if}
          {#if plantilla}<button type="button" class="btn btn-sm btn-ghost" onclick={() => (plantilla = "")}>Quitar</button>{/if}
        </div>
      {/if}
    </fieldset>

    <fieldset class="grupo">
      <legend><BellRing size={14} />Avisos de sus equipos</legend>
      {#if esPropietario}
        <label class="switch-row">
          <input class="switch" type="checkbox" bind:checked={critico} />
          <span><strong>Tratar sus avisos como críticos</strong><span class="faint">Lo importante (un equipo sin contacto, una copia atrasada…) llega a quien recibe los críticos. Lo informativo no cambia.</span></span>
        </label>
        {#if posibles.length}
          <p class="faint pequeno sub">Avisar siempre por estos canales (aunque solo reciban otras gravedades):</p>
          {#each posibles as k (k.ref.ambito + k.ref.id)}
            <label class="check">
              <input type="checkbox" checked={marcado(k.ref)} onchange={(e) => marcar(k.ref, e.currentTarget.checked)} />
              <span><strong>{k.nombre}</strong> <span class="faint">{TIPO_CANAL[k.tipo].texto} {k.de}</span></span>
            </label>
          {/each}
        {:else if notif}
          <p class="faint pequeno">Sin canales compartidos (webhook, ntfy, Telegram): se añaden en Ajustes → Notificaciones.</p>
        {/if}
        <p class="faint pequeno">Qué recibe cada persona por correo de los equipos con «{nombre}» se elige en sus avisos (Ajustes → Personas o «Mis notificaciones»).</p>
      {:else}
        <p class="faint pequeno">
          {#if conAvisos(previo?.avisos)}{previo?.avisos?.importancia === "critico" ? "Sus avisos cuentan como críticos" : "Como los demás"}{previo?.avisos?.canales?.length ? ` y ${plural(previo.avisos.canales.length, "canal avisa", "canales avisan")} siempre` : ""}.{:else}Como los de los demás equipos.{/if}
          Solo el propietario del cliente lo cambia.
        </p>
      {/if}
    </fieldset>

    {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
    <footer>
      {#if previo}<button type="button" class="btn btn-ghost izq" disabled={guardando} onclick={() => guardar(null, true)}><RotateCcw size={14} />Lo de siempre</button>{/if}
      <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
      <BotonCargando class="btn btn-primary" type="submit" cargando={guardando} textoCargando="Guardando…" disabled={pidiendoClave}>Guardar</BotonCargando>
    </footer>
  </form>
</Modal>

<style>
  .grupo {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    padding: 0;
    border: none;
  }
  .grupo legend {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 6px;
    font-weight: 600;
  }
  .colores {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .op {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 32px;
    padding: 0 10px;
    font-size: var(--fs-sm);
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
    cursor: pointer;
  }
  .op input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .op:has(input:checked) {
    color: var(--text-1);
    background: color-mix(in srgb, var(--et) 14%, var(--surface));
    border-color: color-mix(in srgb, var(--et) 70%, var(--border));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--et) 70%, var(--border));
  }
  .op:has(input:focus-visible) {
    box-shadow: var(--focus);
  }
  .punto {
    flex: none;
    width: 10px;
    height: 10px;
    background: var(--et);
    border-radius: 999px;
  }
  /* La muestra con el color elegido (sin guardar todavía). */
  .muestra :global(.et) {
    --et: var(--et-elegido) !important;
  }
  .pequeno {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .sub {
    margin-top: 4px;
  }
  .fila-pl {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .fila-pl span {
    flex: 1;
    min-width: 180px;
    color: var(--text-2);
  }
  .clave-pl {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .fila-botones {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .check {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .check input {
    margin-top: 3px;
  }
  .izq {
    margin-right: auto;
  }
</style>
