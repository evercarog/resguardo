<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Elegir carpetas en vivo: abre una sesión cifrada con el equipo
  // (orden «elegir_carpetas», con la prueba de administración) y recorre su
  // árbol. Arriba, las sugerencias que detecta el agente (carpetas de
  // usuarios, Siigo, SQL Server…). El servidor no ve ninguna ruta.
  // Con `unica`, se elige una sola carpeta (p. ej. dónde guardar un destino
  // local o las copias que recibe un equipo de almacenamiento) y, si el
  // agente lo admite (v1.15, `crear_carpeta`), se puede crear una nueva.
  // Las carpetas que parecen un repositorio de restic (con `config`, `data`,
  // `index`, `keys` y `snapshots`) llevan su marca: el agente lo dice al
  // listar (`repositorio`, los agentes posteriores a 0.7.21); con uno anterior, se sabe al abrir
  // la carpeta. Con `buscarRepos` («Usar uno que ya existe», «Traer
  // historial»), además, una línea lo explica.
  import { onDestroy, onMount, tick } from "svelte";
  import { ChevronRight, Database, Folder, FolderOpen, FolderPlus, HardDrive, Info, LoaderCircle, Sparkles, TriangleAlert, X } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { Sesion, type MensajeEquipo } from "$lib/sesion";
  import { errorNombreCarpeta } from "$lib/ganchos";
  import type { Cliente, Equipo } from "$lib/tipos";
  import CampoClave from "./CampoClave.svelte";

  interface Entrada {
    nombre: string;
    tipo: "dir" | "archivo";
    sistema?: boolean;
    /** La carpeta parece un repositorio de restic (agentes posteriores a 0.7.21). */
    repositorio?: boolean;
  }
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
    iniciales?: string[]; onclose: () => void; alElegir: (rutas: string[], gancho?: string) => void } = $props();

  const win = $derived(/windows/i.test(equipo.so));
  const sep = $derived(win ? "\\" : "/");
  let sesion: Sesion | null = null;
  let estado = $state<"clave" | "abriendo" | "lista" | "error">("abriendo");
  let error = $state("");
  let paso = $state("");
  let hijos = $state<Record<string, Entrada[] | "cargando">>({});
  /** Las carpetas abiertas que resultaron ser un repositorio (con un agente que no da la pista). */
  let sonRepo = $state<Record<string, boolean>>({});
  const PIEZAS = ["data", "index", "keys", "snapshots"];
  const pareceRepo = (es: Entrada[]) => es.some((e) => e.nombre === "config" && e.tipo === "archivo") && PIEZAS.every((p) => es.some((e) => e.nombre === p && e.tipo === "dir"));
  let abiertas = $state<Record<string, boolean>>({});
  // Las carpetas que ya tenía la copia (solo al abrir: después manda lo que se marque aquí).
  const inicio = () => new Set(iniciales);
  let elegidas = $state<Set<string>>(inicio());
  let sugerencias = $state<Sugerencia[]>([]);
  let gancho = $state<string | undefined>();
  /** ¿Puede el equipo crear carpetas? `null` mientras no se sabe. */
  let puedeCrear = $state<boolean | null>(null);
  let creando = $state<{ nombre: string; enviando: boolean; error: string } | null>(null);
  let campoNombre = $state<HTMLInputElement | null>(null);
  let hecho = $state("");

  // Raíz: el agente da las unidades («C:\») en Windows, o lo que hay en «/».
  const unir = (ruta: string, nombre: string) =>
    ruta === "" ? (win ? (nombre.endsWith(sep) ? nombre : nombre + sep) : `/${nombre}`) : ruta.endsWith(sep) ? ruta + nombre : ruta + sep + nombre;

  async function cargar(ruta: string) {
    if (!sesion || hijos[ruta]) return;
    hijos[ruta] = "cargando";
    try {
      const r = await sesion.pedir<MensajeEquipo & { entradas: Entrada[] }>("carpetas", { ruta });
      const todas = r.entradas ?? [];
      if (ruta && pareceRepo(todas)) sonRepo[ruta] = true;
      hijos[ruta] = todas.filter((e) => e.tipo === "dir");
    } catch (e) {
      delete hijos[ruta];
      error = (e as Error).message;
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
    try {
      sesion = await Sesion.abrir({ cliente, equipo, tipo: "elegir_carpetas", secretos: prueba ? { prueba } : { claveAdmin: claveAdmin || clave }, alPaso: (t) => (paso = t) });
      paso = "Esperando al equipo…";
      await sesion.lista;
      estado = "lista";
      puedeCrear = sesion.admite("crear_carpeta");
      void cargar("");
      // La carpeta que ya estaba escrita: se abren sus carpetas de arriba para verla marcada.
      if (unica && iniciales[0]) void desplegarHasta(iniciales[0]);
      try {
        if (!unica) sugerencias = (await sesion.pedir<MensajeEquipo & { sugerencias: Sugerencia[] }>("sugerencias")).sugerencias ?? [];
      } catch {
        /* sin sugerencias */
      }
      clave = "";
    } catch (e) {
      estado = "error";
      error = (e as Error).message;
    }
  }
  onDestroy(() => void sesion?.cerrar());

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
      delete hijos[padre];
      abiertas[padre] = true;
      await cargar(padre);
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
    const n = new Set(elegidas);
    for (const r of s.rutas) n.add(r);
    elegidas = n;
    if (s.gancho) gancho = s.gancho;
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
  {:else if estado === "abriendo"}
    <p class="espera" role="status"><LoaderCircle size={16} class="spin" />{paso || "Abriendo la sesión…"}</p>
  {:else if estado === "error"}
    <div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>
    {#if !prueba && !claveAdmin}<button type="button" class="btn btn-sm" onclick={() => (estado = "clave")}>Volver a intentarlo</button>{/if}
  {:else}
    {#if sugerencias.length}
      <div class="sugerencias">
        <span class="faint"><Sparkles size={13} />Sugerencias</span>
        {#each sugerencias as s (s.id)}
          <button type="button" class="chip" use:tip={s.detalle} onclick={() => sugerir(s)}>{s.nombre}</button>
        {/each}
      </div>
    {/if}
    <div class="arbol" role="tree" aria-label="Carpetas del equipo">
      {@render nivel("")}
    </div>
    {#if error}<p class="error-campo">{error}</p>{/if}
    {#if unica && !buscarRepos}
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

  <footer>
    {#if unica && elegidaError}<p class="error-campo eleccion" role="alert">{elegidaError}</p>{/if}
    <span class="cuenta faint">{#if unica}{[...elegidas][0] ?? "Ninguna elegida"}{:else}{elegidas.size} {elegidas.size === 1 ? "carpeta elegida" : "carpetas elegidas"}{/if}</span>
    <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
    <button type="button" class="btn btn-primary" disabled={!elegidas.size || !!elegidaError} onclick={() => alElegir([...elegidas], gancho)}>{unica ? "Usar esta carpeta" : "Usar estas carpetas"}</button>
  </footer>
</Modal>

{#snippet nivel(ruta: string)}
  {@const h = hijos[ruta]}
  {#if h === "cargando"}
    <div class="cargando"><LoaderCircle size={14} class="spin" />Cargando…</div>
  {:else if h}
    <ul role="group">
      {#each h as e (e.nombre)}
        {@const r = unir(ruta, e.nombre)}
        <li role="treeitem" aria-expanded={!!abiertas[r]} aria-selected={elegidas.has(r)}>
          <div class="nodo" class:sistema={e.sistema}>
            <button type="button" class="icon-btn flecha" class:abierta={abiertas[r]} aria-label={abiertas[r] ? "Plegar" : "Desplegar"} onclick={() => alternar(r)}><ChevronRight size={14} /></button>
            {#if unica}
              <input type="radio" name="carpeta-unica" checked={elegidas.has(r)} aria-label="Elegir {r}" onchange={(ev) => marcar(r, ev.currentTarget.checked)} />
            {:else}
              <input type="checkbox" checked={elegidas.has(r)} disabled={e.sistema} aria-label="Copiar {r}" onchange={(ev) => marcar(r, ev.currentTarget.checked)} />
            {/if}
            {#if ruta === ""}<HardDrive size={15} />{:else if abiertas[r]}<FolderOpen size={15} />{:else}<Folder size={15} />{/if}
            <span class="nombre">{e.nombre}</span>
            {#if e.repositorio || sonRepo[r]}<span class="marca-repo"><Database size={12} />Repositorio</span>{/if}
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
