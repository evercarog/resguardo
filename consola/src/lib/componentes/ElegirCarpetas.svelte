<script module lang="ts">
  // 14zz: lo ya cargado de cada equipo se recuerda mientras dura la sesión del
  // navegador (en memoria, nunca en disco): al volver a abrir el explorador se
  // ve enseguida y solo se pide al equipo lo que falte.
  interface EntradaGuardada {
    nombre: string;
    tipo: "dir" | "archivo";
    sistema?: boolean;
    repositorio?: boolean;
  }
  const recordado = new Map<string, { hijos: Record<string, EntradaGuardada[]>; sonRepo: Record<string, boolean>; sugerencias: { id: string; nombre: string; detalle?: string; rutas: string[]; gancho?: string }[] | null }>();
</script>

<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Elegir carpetas en vivo: abre una sesión cifrada con el equipo
  // (orden «elegir_carpetas», con la prueba de administración) y recorre su
  // árbol. Arriba, las sugerencias que detecta el agente (carpetas de
  // usuarios, Ventas, SQL Server…). El servidor no ve ninguna ruta.
  // Con `unica`, se elige una sola carpeta (p. ej. dónde guardar un destino
  // local o las copias que recibe un equipo de almacenamiento) y, si el
  // agente lo admite (v1.15, `crear_carpeta`), se puede crear una nueva.
  // Las carpetas que parecen un repositorio de restic (con `config`, `data`,
  // `index`, `keys` y `snapshots`) llevan su marca: el agente lo dice al
  // listar (`repositorio`, los agentes posteriores a 0.7.21); con uno anterior, se sabe al abrir
  // la carpeta. Con `buscarRepos` («Usar uno que ya existe», «Traer
  // historial»), además, una línea lo explica.
  // 14zz: con varias carpetas, el árbol marca también lo de dentro; desmarcar
  // una subcarpeta la pasa a «No copiar» (ruta exacta) y la de arriba queda a
  // medias. Cada nivel se carga al abrirlo, con «Reintentar» si falla, y con
  // más paciencia para equipos lejanos (VPN, internet).
  import { onDestroy, onMount, tick } from "svelte";
  import { ChevronRight, Database, Folder, FolderOpen, FolderPlus, HardDrive, Info, LoaderCircle, RefreshCw, Sparkles, TriangleAlert, X } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { Sesion, type MensajeEquipo } from "$lib/sesion";
  import { errorNombreCarpeta } from "$lib/ganchos";
  import { alternarNodo, dentroDeExcluida, estadoNodo, exclusionesFinales, exclusionesVisibles, misma, quitarExclusion, seCopia, seleccionInicial, type Seleccion } from "$lib/arbolCarpetas";
  import type { Cliente, Equipo } from "$lib/tipos";
  import CampoClave from "./CampoClave.svelte";

  type Entrada = EntradaGuardada;
  interface Sugerencia {
    id: string;
    nombre: string;
    detalle?: string;
    rutas: string[];
    gancho?: string;
  }
  let {
    cliente,
    equipo,
    prueba,
    claveAdmin,
    unica = false,
    buscarRepos = false,
    validar,
    titulo,
    iniciales = [],
    exclusiones,
    onclose,
    alElegir,
  }: {
    cliente: Cliente;
    equipo: Equipo;
    /** La prueba de administración ya calculada, o la clave (y se calcula al abrir). */
    prueba?: Uint8Array;
    claveAdmin?: string;
    unica?: boolean;
    /** Se busca un repositorio que ya existe: se explica la marca «Repositorio de restic». */
    buscarRepos?: boolean;
    /** Con `unica`: por qué no vale la carpeta elegida (o null), para avisar antes de usarla. */
    validar?: (ruta: string) => string | null;
    titulo?: string;
    iniciales?: string[];
    /** 14zz: «No copiar» de la copia (con varias carpetas): las rutas exactas se ven y se cambian en el árbol. */
    exclusiones?: string[];
    onclose: () => void;
    /** Con `exclusiones`, también «No copiar» como queda (las reglas de antes y las rutas desmarcadas). */
    alElegir: (rutas: string[], gancho?: string, exclusiones?: string[]) => void;
  } = $props();

  /** Esperas más largas que las de siempre: los equipos lejanos tardan (14zz). */
  const ESPERA_SESION = 180_000;
  const ESPERA_NIVEL = 90_000;
  const win = $derived(/windows/i.test(equipo.so));
  const sep = $derived(win ? "\\" : "/");
  // svelte-ignore state_referenced_locally
  const claveRecuerdo = `${cliente.id}/${equipo.id}`;
  // svelte-ignore state_referenced_locally
  const previo = recordado.get(claveRecuerdo);
  let sesion: Sesion | null = null;
  let sesionLista: Promise<Sesion | null> | null = null;
  let estado = $state<"clave" | "abriendo" | "lista" | "error">("abriendo");
  let error = $state("");
  let paso = $state("");
  let hijos = $state<Record<string, Entrada[] | "cargando" | { error: string }>>({ ...(previo?.hijos ?? {}) });
  /** Las carpetas abiertas que resultaron ser un repositorio (con un agente que no da la pista). */
  let sonRepo = $state<Record<string, boolean>>({ ...(previo?.sonRepo ?? {}) });
  const PIEZAS = ["data", "index", "keys", "snapshots"];
  const pareceRepo = (es: Entrada[]) => es.some((e) => e.nombre === "config" && e.tipo === "archivo") && PIEZAS.every((p) => es.some((e) => e.nombre === p && e.tipo === "dir"));
  let abiertas = $state<Record<string, boolean>>({});
  // Las carpetas que ya tenía la copia (solo al abrir: después manda lo que se marque aquí).
  const inicio = () => new Set(iniciales);
  let elegidas = $state<Set<string>>(inicio());
  /** 14zz (varias carpetas): lo que se copia y lo que no, con las subcarpetas. */
  // svelte-ignore state_referenced_locally
  let sel = $state<Seleccion>(seleccionInicial(iniciales, exclusiones ?? [], /windows/i.test(equipo.so)));
  let sugerencias = $state<Sugerencia[]>(previo?.sugerencias ?? []);
  let gancho = $state<string | undefined>();
  /** ¿Puede el equipo crear carpetas? `null` mientras no se sabe. */
  let puedeCrear = $state<boolean | null>(null);
  let creando = $state<{ nombre: string; enviando: boolean; error: string } | null>(null);
  let campoNombre = $state<HTMLInputElement | null>(null);
  let hecho = $state("");
  /** Ya hay algo que enseñar (de esta vez o de antes): el árbol se ve mientras se conecta. */
  const conArbol = $derived(Array.isArray(hijos[""]));

  /** Guarda lo cargado para la próxima vez (solo lo que llegó bien). */
  function recordar() {
    const h: Record<string, Entrada[]> = {};
    for (const [k, v] of Object.entries($state.snapshot(hijos))) if (Array.isArray(v)) h[k] = v;
    recordado.set(claveRecuerdo, { hijos: h, sonRepo: $state.snapshot(sonRepo), sugerencias: $state.snapshot(sugerencias) });
  }

  // Raíz: el agente da las unidades («C:\») en Windows, o lo que hay en «/».
  const unir = (ruta: string, nombre: string) =>
    ruta === "" ? (win ? (nombre.endsWith(sep) ? nombre : nombre + sep) : `/${nombre}`) : ruta.endsWith(sep) ? ruta + nombre : ruta + sep + nombre;

  /** Carga un nivel al abrirlo (`otraVez`: aunque ya estuviera, p. ej. «Reintentar»). */
  async function cargar(ruta: string, otraVez = false) {
    const h = hijos[ruta];
    if (h === "cargando" || (Array.isArray(h) && !otraVez)) return;
    // Lo que ya se ve se queda mientras llega lo nuevo (sin parpadeos al refrescar).
    if (!Array.isArray(h)) hijos[ruta] = "cargando";
    try {
      const s = sesion ?? (await sesionLista);
      if (!s) throw new Error("Sin conexión con el equipo.");
      const r = await s.pedir<MensajeEquipo & { entradas: Entrada[] }>("carpetas", { ruta }, ESPERA_NIVEL);
      const todas = r.entradas ?? [];
      if (ruta && pareceRepo(todas)) sonRepo[ruta] = true;
      hijos[ruta] = todas.filter((e) => e.tipo === "dir");
      recordar();
    } catch (e) {
      if (!Array.isArray(hijos[ruta])) hijos[ruta] = { error: (e as Error).message };
    }
  }

  // Sin prueba ni clave: se pide aquí la clave de administración (elegir_carpetas la necesita).
  let clave = $state("");
  onMount(() => {
    if (prueba || claveAdmin) void abrirSesion();
    else estado = "clave";
  });
  onDestroy(() => (clave = ""));

  async function abrirSesion() {
    estado = "abriendo";
    error = "";
    let listo: (s: Sesion | null) => void = () => {};
    sesionLista = new Promise((ok) => (listo = ok));
    try {
      const s = await Sesion.abrir({ cliente, equipo, tipo: "elegir_carpetas", secretos: prueba ? { prueba } : { claveAdmin: claveAdmin || clave }, alPaso: (t) => (paso = t), espera: ESPERA_SESION });
      paso = "Esperando al equipo…";
      await s.lista;
      sesion = s;
      listo(s);
      estado = "lista";
      puedeCrear = s.admite("crear_carpeta");
      // Lo que se recordaba de antes se vuelve a pedir en silencio (por si cambió).
      void cargar("", true);
      for (const [r, a] of Object.entries(abiertas)) if (a && r) void cargar(r, true);
      // La carpeta que ya estaba escrita: se abren sus carpetas de arriba para verla marcada.
      if (unica && iniciales[0]) void desplegarHasta(iniciales[0]);
      try {
        if (!unica) {
          sugerencias = (await s.pedir<MensajeEquipo & { sugerencias: Sugerencia[] }>("sugerencias", {}, ESPERA_NIVEL)).sugerencias ?? [];
          recordar();
        }
      } catch {
        /* sin sugerencias */
      }
      clave = "";
    } catch (e) {
      listo(null);
      estado = "error";
      error = (e as Error).message;
      // Los niveles que esperaban a la sesión se pueden reintentar después.
      for (const [k, v] of Object.entries(hijos)) if (v === "cargando") hijos[k] = { error: error };
    }
  }
  onDestroy(() => void sesion?.cerrar());

  /** «Reintentar» tras un fallo al abrir: con la prueba o la clave que ya se tenían, o pidiéndola otra vez. */
  function reintentarSesion() {
    void sesion?.cerrar();
    sesion = null;
    if (prueba || claveAdmin) void abrirSesion();
    else estado = "clave";
  }

  function alternar(ruta: string) {
    abiertas[ruta] = !abiertas[ruta];
    if (abiertas[ruta]) void cargar(ruta);
  }
  const elegidaError = $derived(unica && validar && elegidas.size ? validar([...elegidas][0]) : null);
  function marcar(ruta: string, on: boolean) {
    const s = new Set(unica ? [] : elegidas);
    if (on) s.add(ruta);
    else s.delete(ruta);
    elegidas = s;
  }
  /** Las carpetas de arriba de una ruta («C:\», «C:\Users», …), en orden. */
  function antecesores(ruta: string): string[] {
    const out: string[] = [];
    if (win) {
      const m = /^([A-Za-z]:\\)(.*)$/.exec(ruta);
      if (!m) return out;
      let r = m[1];
      out.push(r);
      for (const p of m[2].split("\\").filter(Boolean).slice(0, -1)) out.push((r = unir(r, p)));
    } else {
      let r = "";
      for (const p of ruta.split("/").filter(Boolean).slice(0, -1)) out.push((r = `${r}/${p}`));
    }
    return out;
  }
  async function desplegarHasta(ruta: string) {
    for (const a of antecesores(ruta)) {
      abiertas[a] = true;
      await cargar(a);
    }
  }

  /** Dónde se crearía la carpeta nueva: dentro de la elegida. */
  const dondeCrear = $derived(unica ? ([...elegidas][0] ?? null) : null);
  const errorNombre = $derived(creando && creando.nombre ? errorNombreCarpeta(creando.nombre) : null);
  async function empezarACrear() {
    creando = { nombre: "", enviando: false, error: "" };
    await tick();
    campoNombre?.focus();
  }
  async function crearCarpeta(ev: SubmitEvent) {
    ev.preventDefault();
    if (!sesion || !creando || !dondeCrear || errorNombreCarpeta(creando.nombre)) return;
    const padre = dondeCrear;
    creando.enviando = true;
    creando.error = "";
    try {
      const r = await sesion.pedir<MensajeEquipo & { ruta: string; ya_existia?: boolean }>("crear_carpeta", { ruta: padre, nombre: creando.nombre });
      abiertas[padre] = true;
      await cargar(padre, true);
      marcar(r.ruta, true);
      hecho = r.ya_existia ? `Ya existía «${creando.nombre}»: queda elegida.` : `Carpeta «${creando.nombre}» creada y elegida.`;
      creando = null;
    } catch (e) {
      if (creando) {
        creando.enviando = false;
        creando.error = (e as Error).message;
      }
    }
  }

  function sugerir(s: Sugerencia) {
    for (const r of s.rutas) if (!seCopia(r, sel, win)) sel = alternarNodo(r, sel, win);
    if (s.gancho) gancho = s.gancho;
  }
  /** Las rutas exactas de «No copiar» que se ven (las de dentro de una carpeta elegida). */
  const noSeCopian = $derived(unica ? [] : exclusionesVisibles(sel, win));
  function usar() {
    if (unica) alElegir([...elegidas], gancho);
    else alElegir([...sel.elegidas], gancho, exclusiones ? exclusionesFinales(exclusiones, sel, win) : undefined);
  }
  /** Casilla a medias (–): `indeterminate` solo existe como propiedad. */
  function aMedias(nodo: HTMLInputElement, medio: boolean) {
    nodo.indeterminate = medio;
    return { update: (m: boolean) => void (nodo.indeterminate = m) };
  }
</script>

<Modal labelledby="t-carpetas" {onclose} width={640} dismissible={false}>
  <div class="dlg-title">
    <span class="ticon"><FolderOpen size={18} /></span>
    <div>
      <h2 id="t-carpetas">{titulo ?? (unica ? `Elegir una carpeta en ${equipo.nombre}` : `Elegir carpetas en ${equipo.nombre}`)}</h2>
      <p>Lo que ves llega cifrado desde el equipo: el servidor no ve las rutas.</p>
      {#if buscarRepos}<p class="pista-repos"><Database size={13} />Elige la carpeta del repositorio: las que lo parecen llevan la marca «Repositorio».</p>{/if}
    </div>
  </div>

  {#if estado === "clave"}
    <form
      class="form"
      onsubmit={(e) => {
        e.preventDefault();
        void abrirSesion();
      }}
    >
      <p class="faint">Para ver las carpetas del equipo hace falta la clave de administración.</p>
      <CampoClave requerido id="clave-carpetas" etiqueta="Clave de administración" bind:value={clave} autofocus />
      <div class="fin-clave"><button class="btn btn-primary" disabled={!clave}>Ver las carpetas</button></div>
    </form>
  {:else}
    {#if estado === "abriendo"}
      <p class="espera" role="status">
        <LoaderCircle size={16} class="spin" />{paso || "Abriendo la sesión…"}
        {#if conArbol}<span class="faint">Mientras, ves lo que ya se cargó antes.</span>{:else}<span class="faint">Con equipos lejanos (VPN o internet) puede tardar un par de minutos.</span>{/if}
      </p>
    {:else if estado === "error"}
      <div class="notice notice-danger" role="alert">
        <TriangleAlert size={16} />
        <div class="aviso-reintentar">
          <p>{error}{conArbol ? " Lo de abajo es lo que se cargó antes." : ""}</p>
          <button type="button" class="btn btn-sm" onclick={reintentarSesion}><RefreshCw size={14} />Reintentar</button>
        </div>
      </div>
    {/if}
    {#if estado === "lista" || conArbol}
      {#if sugerencias.length}
        <div class="sugerencias">
          <span class="faint"><Sparkles size={13} />Sugerencias</span>
          {#each sugerencias as s (s.id)}
            <button type="button" class="chip" use:tip={s.detalle} onclick={() => sugerir(s)}>{s.nombre}</button>
          {/each}
        </div>
      {/if}
      <div class="arbol" role="tree" aria-label="Carpetas del equipo" aria-multiselectable={!unica}>
        {@render nivel("")}
      </div>
      {#if !unica}
        <p class="faint pista-arbol">Al marcar una carpeta se copia todo lo de dentro, también lo que se cree después. Desmarca una subcarpeta para no copiarla.</p>
        {#if noSeCopian.length}
          <div class="no-copiar">
            <span class="field-label">No se copian <span class="faint">(van a «No copiar»)</span></span>
            <ul>
              {#each noSeCopian as x (x)}
                <li><span class="mono ajusta">{x}</span><button type="button" class="icon-btn" aria-label="Copiar también {x}" use:tip={"Volver a copiarla"} onclick={() => (sel = quitarExclusion(x, sel, win))}><X size={14} /></button></li>
              {/each}
            </ul>
          </div>
        {/if}
      {/if}
      {#if unica && !buscarRepos && estado === "lista"}
        {#if creando}
          <form class="crear" onsubmit={crearCarpeta}>
            <label class="field-label" for="nueva-carpeta">Carpeta nueva dentro de <span class="pastilla mono ajusta">{dondeCrear}</span></label>
            <div class="con-boton">
              <input id="nueva-carpeta" class="input" bind:this={campoNombre} bind:value={creando.nombre} placeholder="Por ejemplo: Resguardo" maxlength="100" autocomplete="off" spellcheck="false" disabled={creando.enviando} aria-required="true" aria-invalid={!!(errorNombre ?? creando.error)} aria-describedby={errorNombre || creando.error ? "nueva-carpeta-error" : undefined} />
              <button class="btn btn-primary" disabled={!creando.nombre || !!errorNombre || creando.enviando}>{#if creando.enviando}<LoaderCircle size={15} class="spin" />Creando…{:else}Crear{/if}</button>
              <button type="button" class="icon-btn" aria-label="No crear la carpeta" use:tip={"Cancelar"} onclick={() => (creando = null)} disabled={creando.enviando}><X size={15} /></button>
            </div>
            {#if errorNombre || creando.error}<p class="error-campo" id="nueva-carpeta-error" role="alert">{errorNombre ?? creando.error}</p>{/if}
          </form>
        {:else if puedeCrear}
          <div class="crear-fila">
            <button type="button" class="btn btn-sm" disabled={!dondeCrear} onclick={empezarACrear}><FolderPlus size={14} />Nueva carpeta</button>
            <span class="faint">{dondeCrear ? `Se creará dentro de ${dondeCrear}.` : "Marca primero dónde crearla (puede ser un disco)."}</span>
            {#if hecho}<span class="hecho" role="status">{hecho}</span>{/if}
          </div>
        {:else if puedeCrear === false}
          <p class="nota-crear faint"><Info size={14} />Este agente aún no crea carpetas desde aquí: elige una que ya exista, o escribe la ruta a mano y el equipo la creará al usarla. Actualiza el agente para usar esto.</p>
        {/if}
      {/if}
    {/if}
  {/if}

  <footer>
    {#if unica && elegidaError}<p class="error-campo eleccion" role="alert">{elegidaError}</p>{/if}
    <span class="cuenta faint"
      >{#if unica}{[...elegidas][0] ?? "Ninguna elegida"}{:else}{sel.elegidas.length} {sel.elegidas.length === 1 ? "carpeta elegida" : "carpetas elegidas"}{#if noSeCopian.length}{" "}· {noSeCopian.length} sin copiar{/if}{/if}</span
    >
    <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
    <button type="button" class="btn btn-primary" disabled={(unica ? !elegidas.size : !sel.elegidas.length) || !!elegidaError} onclick={usar}>{unica ? "Usar esta carpeta" : "Usar estas carpetas"}</button>
  </footer>
</Modal>

{#snippet nivel(ruta: string)}
  {@const h = hijos[ruta]}
  {#if h === "cargando"}
    <div class="cargando" role="status"><LoaderCircle size={14} class="spin" />Cargando…{#if ruta}<button type="button" class="btn btn-sm btn-ghost" onclick={() => void cargar(ruta, true)}>Reintentar</button>{/if}</div>
  {:else if h && !Array.isArray(h)}
    <div class="cargando mal" role="alert"><TriangleAlert size={14} />No se pudo cargar: {h.error}<button type="button" class="btn btn-sm" onclick={() => void cargar(ruta, true)}><RefreshCw size={13} />Reintentar</button></div>
  {:else if h}
    <ul role="group">
      {#if !h.length && ruta}<li class="vacia faint">Sin subcarpetas</li>{/if}
      {#each h as e (e.nombre)}
        {@const r = unir(ruta, e.nombre)}
        {@const est = unica ? "no" : estadoNodo(r, sel, win)}
        {@const bajoExcluida = !unica && dentroDeExcluida(r, sel, win)}
        {@const excluida = !unica && noSeCopian.some((x) => misma(x, r, win))}
        <li role="treeitem" aria-expanded={!!abiertas[r]} aria-selected={unica ? elegidas.has(r) : est !== "no"}>
          <div class="nodo" class:sistema={e.sistema} class:fuera={excluida || bajoExcluida}>
            <button type="button" class="icon-btn flecha" class:abierta={abiertas[r]} aria-label={abiertas[r] ? "Plegar" : "Desplegar"} onclick={() => alternar(r)}><ChevronRight size={14} /></button>
            {#if unica}
              <input type="radio" name="carpeta-unica" checked={elegidas.has(r)} aria-label="Elegir {r}" onchange={(ev) => marcar(r, ev.currentTarget.checked)} />
            {:else}
              <input
                type="checkbox"
                checked={est === "si"}
                use:aMedias={est === "medio"}
                disabled={(e.sistema && est === "no") || bajoExcluida}
                aria-checked={est === "medio" ? "mixed" : est === "si"}
                aria-label="Copiar {r}"
                onchange={() => (sel = alternarNodo(r, sel, win))}
              />
            {/if}
            {#if ruta === ""}<HardDrive size={15} />{:else if abiertas[r]}<FolderOpen size={15} />{:else}<Folder size={15} />{/if}
            <span class="nombre">{e.nombre}</span>
            {#if e.repositorio || sonRepo[r]}<span class="marca-repo"><Database size={12} />Repositorio</span>{/if}
            {#if excluida}<span class="marca-repo">No se copia</span>{/if}
            {#if e.sistema}<span class="faint sis">del sistema</span>{/if}
          </div>
          {#if abiertas[r]}{@render nivel(r)}{/if}
        </li>
      {/each}
    </ul>
  {/if}
{/snippet}

<style>
  .eleccion {
    flex-basis: 100%;
    margin: 0 0 4px;
  }
  .fin-clave {
    display: flex;
    justify-content: flex-end;
  }
  .espera {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-2);
  }
  .sugerencias {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-bottom: var(--sp-3);
  }
  .sugerencias .faint {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--fs-xs);
    margin-right: 4px;
  }
  .chip {
    height: 26px;
    padding: 0 10px;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--accent-text);
    background: var(--accent-soft);
    border: none;
    border-radius: 999px;
    cursor: pointer;
  }
  .arbol {
    max-height: 46vh;
    overflow: auto;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  ul {
    margin: 0;
    padding: 0 0 0 18px;
    list-style: none;
  }
  .arbol > ul {
    padding-left: 0;
  }
  .nodo {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    border-radius: var(--radius-sm);
  }
  .nodo:hover {
    background: var(--surface-2);
  }
  .nodo :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .nodo.sistema .nombre {
    color: var(--text-3);
  }
  .flecha :global(svg) {
    transition: transform var(--dur) var(--ease);
  }
  .flecha.abierta :global(svg) {
    transform: rotate(90deg);
  }
  .nombre {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sis {
    font-size: var(--fs-xs);
  }
  /* Una carpeta que parece un repositorio: pastilla neutra (no es un estado). */
  .marca-repo {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 4px;
    height: 20px;
    padding: 0 7px;
    font-size: 11.5px;
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
  }
  .nodo .marca-repo :global(svg) {
    color: var(--text-2);
  }
  .pista-repos {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .cargando {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 28px;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .cargando.mal {
    color: var(--bad);
    flex-wrap: wrap;
  }
  .cargando .btn {
    margin-left: 4px;
  }
  .vacia {
    padding: 4px 28px;
    font-size: var(--fs-sm);
  }
  .nodo.fuera .nombre {
    color: var(--text-3);
    text-decoration: line-through;
  }
  .aviso-reintentar {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
  }
  .espera .faint {
    font-size: var(--fs-sm);
  }
  .espera {
    flex-wrap: wrap;
    margin-bottom: var(--sp-3);
  }
  .pista-arbol {
    margin: var(--sp-2) 0 0;
    font-size: var(--fs-xs);
  }
  .no-copiar {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: var(--sp-3);
  }
  .no-copiar ul {
    padding: 0;
  }
  .no-copiar li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 2px 0 2px 8px;
    font-size: var(--fs-sm);
    border-left: 2px solid var(--border-strong);
  }
  .cuenta {
    margin-right: auto;
    font-size: var(--fs-sm);
    overflow-wrap: anywhere;
  }
  .crear {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: var(--sp-3);
  }
  .crear .con-boton {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .crear .con-boton .input {
    flex: 1;
    min-width: 0;
  }
  .crear-fila {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    margin-top: var(--sp-3);
    font-size: var(--fs-sm);
  }
  .crear-fila .faint {
    overflow-wrap: anywhere;
  }
  .hecho {
    color: var(--ok);
  }
  .nota-crear {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: var(--sp-3) 0 0;
    font-size: var(--fs-sm);
  }
  .nota-crear :global(svg) {
    flex: none;
    margin-top: 2px;
  }
</style>
