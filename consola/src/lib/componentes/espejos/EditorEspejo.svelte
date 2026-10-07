<script lang="ts">
  // Editor de un espejo (plan 0.7.26, bloque 4; docs/espejo.md «Trabajos de espejo»).
  //
  // Guiado (por defecto): Qué repositorios → Adónde → Cuándo → Retención → Resumen,
  // un paso cada vez, con lo ya elegido arriba en una línea que se toca para volver.
  // Lo poco habitual de cada paso, bajo «Más opciones». «Avanzado» lo enseña todo de
  // golpe (la consola lo recuerda para cada persona). Intro avanza; Esc cierra.
  //
  // No manda nada: al «Guardar…» devuelve el espejo y quien lo abrió manda la lista
  // entera con la clave de administración (OrdenDialog). El almacén nunca recibe
  // contraseñas de repositorios: copia archivos cifrados sin abrirlos.
  import { untrack } from "svelte";
  import { ArrowLeft, ArrowRight, Check, ChevronDown, Cloud, Pencil, Save, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import ElegirDestinoPaso, { type OpcionDestino } from "../ElegirDestinoPaso.svelte";
  import EditorHorario from "../EditorHorario.svelte";
  import ConectarNube from "../ConectarNube.svelte";
  import ConectarDestino from "../ConectarDestino.svelte";
  import Ayuda from "../Ayuda.svelte";
  import { app, puede } from "$lib/estado.svelte";
  import { catalogoDe, cargarCatalogo } from "$lib/catalogoDestinos.svelte";
  import { destinosParaPasos, detalleDestino } from "$lib/cadenas";
  import { nombreZonaPorDefecto, PRINCIPAL, zonasDe } from "$lib/destinos";
  import { TIPOS_NUBE } from "$lib/espejo";
  import { errorCarpetaEspejo } from "$lib/ganchos";
  import { clasificacionDeVista } from "$lib/regla321";
  import { corta } from "$lib/tipoDestino";
  import {
    AVISO_IGUAL,
    conTrabajo,
    equiposDelAlmacen,
    errorTrabajos,
    FRENO_DEFECTO,
    horarioDiario,
    nombrePorDefecto,
    opcionesRepos,
    RETRASO_DIAS_DEFECTO,
    textoCuando,
    textoFreno,
    textoQue,
    textoRetencion,
    type TrabajoEspejo,
  } from "$lib/espejoTrabajos";
  import type { Cliente, Equipo, Horario } from "$lib/tipos";

  interface Props {
    cliente: Cliente;
    /** Quién lo hace: el almacén o el propio equipo. */
    hace: Equipo;
    quien: "almacen" | "equipo";
    equipos: Equipo[];
    /** El que se edita (uno nuevo ya viene con lo de por defecto). */
    trabajo: TrabajoEspejo;
    /** Los que ya tiene (para «después de otro» y para comprobarlo todo junto). */
    todos: TrabajoEspejo[];
    nuevo: boolean;
    onguardar: (t: TrabajoEspejo) => void;
    onclose: () => void;
  }
  let { cliente, hace, quien, equipos, trabajo, todos, nuevo, onguardar, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  let t = $state<TrabajoEspejo>(structuredClone($state.snapshot(trabajo)));
  const PASOS = [
    { id: "que", texto: "Qué repositorios" },
    { id: "adonde", texto: "Adónde" },
    { id: "cuando", texto: "Cuándo" },
    { id: "retencion", texto: "Retención" },
    { id: "resumen", texto: "Resumen" },
  ] as const;
  type IdPaso = (typeof PASOS)[number]["id"];
  // svelte-ignore state_referenced_locally
  let paso = $state<IdPaso>(nuevo ? "que" : "resumen");
  /** Se abrió un paso desde el resumen: al terminar, se vuelve a él. */
  // svelte-ignore state_referenced_locally
  let desdeResumen = $state(!nuevo);
  const i = $derived(PASOS.findIndex((p) => p.id === paso));

  // «Avanzado»: la preferencia de cada persona en este navegador.
  const claveAvanzado = $derived(`resguardo.editor.avanzado.${app.cuenta?.id ?? "?"}`);
  let avanzado = $state(untrack(() => leerAvanzado()));
  function leerAvanzado(): boolean {
    try {
      return localStorage.getItem(`resguardo.editor.avanzado.${app.cuenta?.id ?? "?"}`) === "1";
    } catch {
      return false;
    }
  }
  function cambiarAvanzado(v: boolean) {
    avanzado = v;
    try {
      localStorage.setItem(claveAvanzado, v ? "1" : "0");
    } catch {
      /* sin almacenamiento: solo esta vez */
    }
  }
  let mas = $state<Record<string, boolean>>({});

  // --- Qué ---
  const zonas = $derived(quien === "almacen" ? zonasDe(hace) : []);
  const zona = $derived(t.zona || PRINCIPAL);
  const repos = $derived(opcionesRepos(hace, quien, equipos, zona));
  const usuarios = $derived(quien === "almacen" ? equiposDelAlmacen(hace, equipos, zona) : []);
  const nombreRepo = (r: string) => {
    const o = repos.find((x) => x.valor === r);
    return o ? (o.equipo ? `${o.nombre} (${o.equipo})` : o.nombre) : r;
  };
  const nombreEquipo = (u: string) => usuarios.find((x) => x.usuario === u)?.nombre ?? u;
  function elegirQue(tipo: "todos" | "equipos" | "repos") {
    if (tipo === "todos") t.que = { tipo: "todos" };
    else if (tipo === "equipos") t.que = { tipo: "equipos", equipos: t.que.tipo === "equipos" ? t.que.equipos : [] };
    else t.que = { tipo: "repos", repos: t.que.tipo === "repos" ? t.que.repos : [] };
  }
  function alternar(lista: string[], v: string): string[] {
    return lista.includes(v) ? lista.filter((x) => x !== v) : [...lista, v];
  }

  // --- Adónde ---
  // svelte-ignore state_referenced_locally
  void cargarCatalogo(cliente.id);
  const vistas = $derived(destinosParaPasos(equipos, catalogoDe(cliente.id)));
  const nubesHace = $derived([...(hace.resumen?.nubes ?? []), ...(hace.resumen?.guarda_copias?.nubes ?? [])].filter((n, j, l) => l.findIndex((x) => x.nombre === n.nombre) === j));
  const opciones = $derived.by<OpcionDestino[]>(() => {
    const l: OpcionDestino[] = [];
    for (const z of zonas) {
      if (z.id === zona) continue;
      const v = vistas.find((x) => x.zona && x.zona.almacen.id === hace.id && x.zona.id === z.id);
      l.push({ valor: `zona:${z.id}`, nombre: v?.nombre ?? nombreZonaPorDefecto(z), detalle: z.principal ? "Zona principal" : "Otra zona de este almacén", clase: "zona", uso: { ok: true }, tipoDestino: v ? corta(clasificacionDeVista(v, equipos)) : undefined });
    }
    for (const n of nubesHace) {
      const v = vistas.find((x) => x.nube && x.nube.equipo.id === hace.id && x.nube.nombre === n.nombre);
      const inmutable = TIPOS_NUBE[n.tipo]?.inmutable;
      l.push({ valor: `nube:${n.nombre}`, nombre: v?.nombre ?? n.nombre, detalle: v ? detalleDestino(v) : TIPOS_NUBE[n.tipo]?.nombre, clase: "nube", uso: { ok: true, motivo: inmutable === false ? "No es inmutable" : undefined }, tipoDestino: v ? corta(clasificacionDeVista(v, equipos)) : undefined });
    }
    l.push({ valor: "carpeta", nombre: `Otra carpeta de ${hace.nombre}…`, detalle: "Mejor en otro disco", clase: "carpeta", uso: { ok: true } });
    return l;
  });
  const valorAdonde = $derived(t.adonde.tipo === "zona" ? `zona:${t.adonde.carpeta}` : t.adonde.tipo === "nube" ? `nube:${t.adonde.nube ?? ""}` : "carpeta");
  /** Al elegir otro destino: su carpeta y su bloqueo de objetos (si lo dice el catálogo). */
  function elegirAdonde(v: string) {
    if (v === valorAdonde) return;
    if (v.startsWith("zona:")) t.adonde = { tipo: "zona", carpeta: v.slice(5) };
    else if (v.startsWith("nube:")) t.adonde = { tipo: "nube", nube: v.slice(5), carpeta: t.adonde.tipo === "nube" ? t.adonde.carpeta : "Resguardo" };
    else t.adonde = { tipo: "carpeta", carpeta: "" };
    const vista = v.startsWith("nube:") ? vistas.find((x) => x.nube && x.nube.equipo.id === hace.id && x.nube.nombre === v.slice(5)) : v.startsWith("zona:") ? vistas.find((x) => x.zona && x.zona.almacen.id === hace.id && x.zona.id === v.slice(5)) : undefined;
    const dias = vista ? clasificacionDeVista(vista, equipos).bloqueoDias : null;
    t.bloqueo_dias = dias ?? null;
    if (dias && t.retencion.modo === "igual") t.retencion = { modo: "retraso", dias: dias + 1 };
    if (dias && t.retencion.modo === "retraso" && t.retencion.dias <= dias) t.retencion = { modo: "retraso", dias: dias + 1 };
  }
  const win = $derived(/windows/i.test(hace.so ?? ""));
  const errorCarpeta = $derived(t.adonde.tipo === "carpeta" && t.adonde.carpeta.trim() ? errorCarpetaEspejo(t.adonde.carpeta, win) : null);
  const tipoNube = $derived(t.adonde.tipo === "nube" ? nubesHace.find((n) => n.nombre === t.adonde.nube)?.tipo : undefined);
  let conectar = $state<"dropbox" | "otro" | null>(null);
  const textoAdonde = $derived(opciones.find((o) => o.valor === valorAdonde && o.valor !== "carpeta")?.nombre ?? (t.adonde.carpeta.trim() || "Sin elegir"));

  // --- Cuándo ---
  const otros = $derived(todos.filter((x) => x.id !== t.id));
  const conHorario = $derived(!!t.cuando.horario && (!!t.cuando.horario.horas?.length || !!t.cuando.horario.reglas?.length));
  let horarioGuardado: Horario = untrack(() => t.cuando.horario ?? horarioDiario("02:00"));
  function ponerHorario(si: boolean) {
    if (si) t.cuando.horario = horarioGuardado;
    else {
      if (t.cuando.horario) horarioGuardado = t.cuando.horario;
      t.cuando.horario = null;
    }
  }
  const tras = $derived(t.cuando.cadena ? "cadena" : t.cuando.despues ? "despues" : "no");
  function ponerTras(modo: "no" | "cadena" | "despues", id?: string) {
    const otro = id ?? t.cuando.cadena ?? t.cuando.despues ?? otros[0]?.id ?? null;
    t.cuando.cadena = modo === "cadena" ? otro : null;
    t.cuando.despues = modo === "despues" ? otro : null;
  }
  const nombreDe = (id: string) => {
    const x = todos.find((y) => y.id === id);
    return x ? x.nombre.trim() || nombrePorDefecto(x.adonde) : "otro espejo";
  };

  // --- Retención ---
  const bloqueoDias = $derived(t.bloqueo_dias && t.bloqueo_dias > 0 ? t.bloqueo_dias : null);
  const minDias = $derived(bloqueoDias ? bloqueoDias + 1 : 1);
  let diasRetraso = $state(untrack(() => (t.retencion.modo === "retraso" ? t.retencion.dias : RETRASO_DIAS_DEFECTO)));
  function elegirRetencion(modo: "nunca" | "retraso" | "igual") {
    if (modo === "retraso") t.retencion = { modo, dias: Math.max(diasRetraso, minDias) };
    else t.retencion = { modo };
  }
  $effect(() => {
    // Los días escritos van a la retención (sin leer la retención que se escribe).
    const d = diasRetraso;
    untrack(() => {
      if (t.retencion.modo === "retraso" && Number.isFinite(d)) t.retencion = { modo: "retraso", dias: Math.round(d) };
    });
  });

  // --- Todo junto ---
  const lista = $derived(conTrabajo(todos, { ...t, nombre: t.nombre.trim() }));
  const error = $derived(errorTrabajos(lista, quien) ?? errorCarpeta);
  /** El error del paso (o null): no se avanza con algo a medias. */
  const errorPaso = $derived.by(() => {
    if (paso === "que") return t.que.tipo === "equipos" && !t.que.equipos.length ? "Elige al menos un equipo." : t.que.tipo === "repos" && !t.que.repos.length ? "Elige al menos un repositorio." : null;
    if (paso === "adonde") {
      if (t.adonde.tipo === "carpeta") return t.adonde.carpeta.trim() ? errorCarpeta : "Escribe la carpeta.";
      if (t.adonde.tipo === "nube" && !t.adonde.carpeta.trim()) return "Escribe la carpeta dentro de la nube.";
      return null;
    }
    if (paso === "cuando") return !conHorario && !t.cuando.tras_copia && tras === "no" ? "Elige al menos uno." : tras !== "no" && !(t.cuando.cadena || t.cuando.despues) ? "Elige después de cuál." : null;
    if (paso === "retencion") return t.retencion.modo === "retraso" && (t.retencion.dias < minDias || t.retencion.dias > 3650) ? `Entre ${minDias} y 3650 días.` : null;
    return null;
  });
  function siguiente(e?: Event) {
    e?.preventDefault();
    if (avanzado) return guardar();
    if (errorPaso) return;
    if (paso === "resumen") return guardar();
    paso = desdeResumen ? "resumen" : PASOS[i + 1].id;
  }
  function atras() {
    paso = desdeResumen ? "resumen" : PASOS[Math.max(0, i - 1)].id;
  }
  function irA(p: IdPaso) {
    paso = p;
    desdeResumen = true;
  }
  function guardar() {
    if (error) return;
    onguardar({ ...t, nombre: t.nombre.trim() || nombrePorDefecto(t.adonde) });
  }
  /** La línea de resumen de cada paso. */
  const lineas = $derived<Record<IdPaso, string>>({
    que: `${textoQue(t, nombreRepo, nombreEquipo)}${quien === "almacen" && zonas.length > 1 ? ` · desde ${zonas.find((z) => z.id === zona) ? nombreZonaPorDefecto(zonas.find((z) => z.id === zona)!) : "la zona principal"}` : ""}`,
    adonde: `${textoAdonde}${t.adonde.tipo === "nube" ? ` · carpeta ${t.adonde.carpeta || "—"}` : ""}${bloqueoDias ? ` · bloqueo de ${bloqueoDias} días` : t.bloqueo ? " · con bloqueo" : ""}`,
    cuando: textoCuando(t, nombreDe),
    retencion: textoRetencion(t),
    resumen: "",
  });
  const verPaso = (p: IdPaso) => avanzado || paso === p;
</script>

<Modal labelledby="t-editor-espejo" {onclose} width={640} dismissible={false}>
  <form class="form editor" onsubmit={siguiente}>
    <div class="dlg-title cab">
      <span class="ticon"><Cloud size={18} /></span>
      <div>
        <h2 id="t-editor-espejo">{nuevo ? "Nuevo espejo" : `Cambiar «${trabajo.nombre || nombrePorDefecto(trabajo.adonde)}»`}</h2>
        <p>{quien === "almacen" ? `Lo hace ${hace.nombre}, sin contraseñas: copia los archivos cifrados tal cual.` : `Lo hace ${hace.nombre} con los repositorios de sus discos.`}</p>
      </div>
      <label class="avanzado"><input type="checkbox" class="switch" checked={avanzado} onchange={(e) => cambiarAvanzado(e.currentTarget.checked)} /><span>Avanzado</span></label>
    </div>

    {#if !avanzado}
      <!-- Lo ya elegido, una línea por paso: se toca para volver a él. -->
      <ol class="hechos">
        {#each PASOS.slice(0, paso === "resumen" ? 4 : i) as p (p.id)}
          {#if paso !== "resumen"}
            <li><button type="button" class="linea" onclick={() => irA(p.id)}><Check size={13} /><span class="et">{p.texto}</span><span class="val">{lineas[p.id]}</span><Pencil size={12} /></button></li>
          {/if}
        {/each}
      </ol>
      {#if paso !== "resumen"}<p class="paso-n faint">Paso {i + 1} de 4 · <strong>{PASOS[i].texto}</strong></p>{/if}
    {/if}

    {#if verPaso("que")}
      <fieldset class="paso">
        <legend>Qué repositorios</legend>
        <div class="opc" role="radiogroup" aria-label="Qué repositorios">
          <label class="radio"><input type="radio" name="que" checked={t.que.tipo === "todos"} onchange={() => elegirQue("todos")} /><span>{quien === "almacen" ? "Todos" : "Todos los de sus discos"}<span class="faint">También los que lleguen después.</span></span></label>
          {#if quien === "almacen"}
            <label class="radio"><input type="radio" name="que" checked={t.que.tipo === "equipos"} onchange={() => elegirQue("equipos")} /><span>Los de ciertos equipos</span></label>
          {/if}
          <label class="radio"><input type="radio" name="que" checked={t.que.tipo === "repos"} onchange={() => elegirQue("repos")} /><span>Algunos repositorios</span></label>
        </div>
        {#if t.que.tipo === "equipos"}
          {@const elegidos = t.que.equipos}
          <div class="checks">
            {#each usuarios as u (u.usuario)}
              <label class="check"><input type="checkbox" checked={elegidos.includes(u.usuario)} onchange={() => t.que.tipo === "equipos" && (t.que = { tipo: "equipos", equipos: alternar(elegidos, u.usuario) })} /><span>{u.nombre}</span></label>
            {:else}
              <p class="faint">Todavía no guarda copias de ningún equipo.</p>
            {/each}
          </div>
        {:else if t.que.tipo === "repos"}
          {@const elegidos = t.que.repos}
          <div class="checks">
            {#each repos as r (r.valor)}
              <label class="check"><input type="checkbox" checked={elegidos.includes(r.valor)} onchange={() => t.que.tipo === "repos" && (t.que = { tipo: "repos", repos: alternar(elegidos, r.valor) })} /><span>{r.nombre}{#if r.equipo}<span class="faint"> · {r.equipo}</span>{/if}</span></label>
            {:else}
              <p class="faint">{quien === "almacen" ? "Todavía no guarda ningún repositorio." : "No tiene repositorios en sus discos."}</p>
            {/each}
          </div>
        {/if}
        {#if quien === "almacen" && zonas.length > 1}
          <details class="mas" bind:open={mas.que}>
            <summary><ChevronDown size={14} />Más opciones</summary>
            <div class="field">
              <label class="field-label" for="ee-zona">Copiar desde</label>
              <select id="ee-zona" class="input" value={zona} onchange={(e) => ((t.zona = e.currentTarget.value === PRINCIPAL ? null : e.currentTarget.value), t.que.tipo !== "todos" && (t.que = { tipo: "todos" }))}>
                {#each zonas as z (z.id)}<option value={z.id}>{nombreZonaPorDefecto(z)}</option>{/each}
              </select>
            </div>
          </details>
        {/if}
      </fieldset>
    {/if}

    {#if verPaso("adonde")}
      <fieldset class="paso">
        <legend class="sr-only">Adónde</legend>
        <ElegirDestinoPaso id="ee-adonde" etiqueta="Adónde" {opciones} bind:value={() => valorAdonde, elegirAdonde} />
        {#if t.adonde.tipo === "carpeta"}
          <div class="field">
            <label class="field-label" for="ee-carpeta">Carpeta</label>
            <input id="ee-carpeta" class="input mono" bind:value={t.adonde.carpeta} placeholder={win ? "E:\\Resguardo-espejo" : "/mnt/disco2/espejo"} spellcheck="false" />
            {#if errorCarpeta}<p class="error-campo">{errorCarpeta}</p>{:else}<span class="field-hint">Si falla el disco del original, el espejo sigue ahí.</span>{/if}
          </div>
        {:else if t.adonde.tipo === "nube"}
          <div class="field">
            <label class="field-label" for="ee-nube">Carpeta dentro de la nube</label>
            <input id="ee-nube" class="input mono" bind:value={t.adonde.carpeta} spellcheck="false" />
          </div>
          {#if tipoNube && TIPOS_NUBE[tipoNube] && !TIPOS_NUBE[tipoNube].inmutable}
            <div class="notice notice-warn"><TriangleAlert size={16} /><p>{TIPOS_NUBE[tipoNube].nombre} no es inmutable: quien entre en la cuenta podría borrar lo de allí.</p></div>
          {/if}
        {/if}
        {#if puede.administrar(cliente.rol)}
          <div class="conectar">
            <button type="button" class="btn btn-sm btn-ghost" onclick={() => (conectar = "dropbox")}><Cloud size={13} />Conectar Dropbox en {hace.nombre}</button>
            <button type="button" class="btn btn-sm btn-ghost" onclick={() => (conectar = "otro")}>Otra nube o servidor…</button>
          </div>
        {/if}
        <details class="mas" bind:open={mas.adonde}>
          <summary><ChevronDown size={14} />Más opciones</summary>
          <label class="switch-row"><input type="checkbox" class="switch" checked={!!bloqueoDias || !!t.bloqueo} onchange={(e) => (e.currentTarget.checked ? (t.bloqueo_dias = 30) : ((t.bloqueo_dias = null), (t.bloqueo = false)))} /><span>Este destino tiene bloqueo de objetos<span class="faint">Object Lock: lo escrito no se puede borrar durante N días. Nunca se intenta borrar algo bloqueado.</span></span></label>
          {#if bloqueoDias || t.bloqueo}
            <div class="field">
              <label class="field-label" for="ee-bloqueo">Días del bloqueo</label>
              <input id="ee-bloqueo" class="input num corto" type="number" min="1" max="36500" value={bloqueoDias ?? ""} oninput={(e) => ((t.bloqueo_dias = Number(e.currentTarget.value) || null), (t.bloqueo = false))} />
            </div>
          {/if}
          <div class="field">
            <label class="field-label" for="ee-ver">Comprobar cada día (%)</label>
            <input id="ee-ver" class="input num corto" type="number" min="0" max="100" placeholder={t.adonde.tipo === "nube" ? "0" : "5"} value={t.verificar_pct ?? ""} oninput={(e) => (t.verificar_pct = e.currentTarget.value === "" ? null : Number(e.currentTarget.value))} />
            <span class="field-hint">Sin contraseña: cada archivo se llama como su huella. {t.adonde.tipo === "nube" ? "En una nube, comprobar es descargar." : ""}</span>
          </div>
          {#if t.adonde.tipo === "nube"}
            <div class="field">
              <label class="field-label" for="ee-vel">Velocidad máxima (KiB/s)</label>
              <input id="ee-vel" class="input num corto" type="number" min="1" placeholder="sin límite" value={t.limite_kib ?? ""} oninput={(e) => (t.limite_kib = e.currentTarget.value === "" ? null : Number(e.currentTarget.value))} />
            </div>
          {/if}
        </details>
      </fieldset>
    {/if}

    {#if verPaso("cuando")}
      <fieldset class="paso">
        <legend>Cuándo</legend>
        <label class="switch-row"><input type="checkbox" class="switch" bind:checked={t.cuando.tras_copia} /><span>Después de cada copia nueva<span class="faint">Cuando llega una versión nueva de sus repositorios (agrupa las que llegan seguidas).</span></span></label>
        <label class="switch-row"><input type="checkbox" class="switch" checked={conHorario} onchange={(e) => ponerHorario(e.currentTarget.checked)} /><span>Con horario</span></label>
        {#if conHorario && t.cuando.horario}
          <EditorHorario id="ee-horario" bind:horario={t.cuando.horario} admiteReglas={true} />
        {/if}
        {#if otros.length}
          <label class="switch-row"><input type="checkbox" class="switch" checked={tras !== "no"} onchange={(e) => ponerTras(e.currentTarget.checked ? "cadena" : "no")} /><span>Después de otro espejo</span></label>
          {#if tras !== "no"}
            <div class="fila">
              <select class="input" aria-label="Después de cuál" value={t.cuando.cadena ?? t.cuando.despues ?? ""} onchange={(e) => ponerTras(tras === "despues" ? "despues" : "cadena", e.currentTarget.value)}>
                {#each otros as o (o.id)}<option value={o.id}>{o.nombre.trim() || nombrePorDefecto(o.adonde)}</option>{/each}
              </select>
              <div class="opc fila" role="radiogroup" aria-label="Si el anterior falla">
                <label class="radio"><input type="radio" name="tras" checked={tras === "cadena"} onchange={() => ponerTras("cadena")} /><span>Solo si sale bien</span></label>
                <label class="radio"><input type="radio" name="tras" checked={tras === "despues"} onchange={() => ponerTras("despues")} /><span>Siempre</span></label>
              </div>
            </div>
          {/if}
        {/if}
        {#if t.cuando.tras_copia || tras !== "no"}
          <div class="field">
            <label class="field-label" for="ee-retraso">Con retraso (minutos)</label>
            <input id="ee-retraso" class="input num corto" type="number" min="0" max="1440" placeholder="0" value={t.cuando.retraso_min || ""} oninput={(e) => (t.cuando.retraso_min = Number(e.currentTarget.value) || 0)} />
            {#if t.cuando.tras_copia}<span class="field-hint">Tras una copia nueva espera al menos 12 minutos.</span>{/if}
          </div>
        {/if}
      </fieldset>
    {/if}

    {#if verPaso("retencion")}
      <fieldset class="paso">
        <legend>Retención <Ayuda id="espejo" /></legend>
        <div class="tarjetas" role="radiogroup" aria-label="Retención">
          <label class="tarjeta" class:on={t.retencion.modo === "nunca"}><input type="radio" name="ret" checked={t.retencion.modo === "nunca"} onchange={() => elegirRetencion("nunca")} /><span><strong>Nunca borra</strong><span class="faint">Guarda todo lo que copia. Crece sin fin.</span></span></label>
          <label class="tarjeta" class:on={t.retencion.modo === "retraso"} class:no={!!t.bloqueo}>
            <input type="radio" name="ret" disabled={!!t.bloqueo} checked={t.retencion.modo === "retraso"} onchange={() => elegirRetencion("retraso")} />
            <span><strong>Sigue al original con retraso</strong><span class="faint">Lo que se quita del original se borra aquí pasados unos días.</span>
              {#if t.retencion.modo === "retraso"}
                <span class="dias"><input class="input num corto" type="number" aria-label="Días" min={minDias} max="3650" bind:value={diasRetraso} /> días{#if bloqueoDias}<span class="faint"> (más de {bloqueoDias}, por el bloqueo)</span>{/if}</span>
              {/if}
            </span>
          </label>
          <label class="tarjeta" class:on={t.retencion.modo === "igual"} class:no={!!bloqueoDias || !!t.bloqueo}>
            <input type="radio" name="ret" disabled={!!bloqueoDias || !!t.bloqueo} checked={t.retencion.modo === "igual"} onchange={() => elegirRetencion("igual")} />
            <span><strong>Igual que el origen</strong><span class="faint">{bloqueoDias || t.bloqueo ? "No se puede con bloqueo de objetos." : "Se pone al día la vez siguiente."}</span></span>
          </label>
        </div>
        {#if t.retencion.modo === "igual"}
          <div class="notice notice-warn"><TriangleAlert size={16} /><p><strong>{AVISO_IGUAL}</strong> Deja al menos otro destino que nunca borre.</p></div>
        {/if}
        <details class="mas" bind:open={mas.retencion}>
          <summary><ChevronDown size={14} />Más opciones: freno</summary>
          <p class="faint nota">No se puede apagar. Si de golpe falta mucho en el original, no borra.</p>
          <div class="fila">
            <div class="field">
              <label class="field-label" for="ee-pct">Si falta más del (%)</label>
              <input id="ee-pct" class="input num corto" type="number" min="1" max="50" bind:value={t.freno.pct} />
            </div>
            <div class="field">
              <label class="field-label" for="ee-mina">Con más de (archivos)</label>
              <input id="ee-mina" class="input num corto" type="number" min="0" bind:value={t.freno.min_archivos} />
            </div>
            <div class="field">
              <label class="field-label" for="ee-minf">Y faltan al menos</label>
              <input id="ee-minf" class="input num corto" type="number" min="0" bind:value={t.freno.min_faltan} />
            </div>
          </div>
          <p class="faint nota">Por debajo, solo frena si desaparece un repositorio entero.</p>
          <div class="opc" role="radiogroup" aria-label="Qué hace el freno">
            <label class="radio"><input type="radio" name="freno" checked={t.freno.accion === "confirmar"} onchange={() => (t.freno.accion = "confirmar")} /><span>Pide confirmación con la clave<span class="faint">No borra nada más hasta entonces.</span></span></label>
            <label class="radio"><input type="radio" name="freno" checked={t.freno.accion === "avisar"} onchange={() => (t.freno.accion = "avisar")} /><span>Conserva lo que falta y avisa<span class="faint">Lo demás sigue al día.</span></span></label>
          </div>
          {#if JSON.stringify(t.freno) !== JSON.stringify(FRENO_DEFECTO)}<button type="button" class="btn btn-sm btn-ghost" onclick={() => (t.freno = { ...FRENO_DEFECTO })}>Volver a lo de por defecto</button>{/if}
        </details>
      </fieldset>
    {/if}

    {#if !avanzado && paso === "resumen"}
      <ul class="resumen">
        {#each PASOS.slice(0, 4) as p (p.id)}
          <li><span class="et">{p.texto}</span><span class="val">{lineas[p.id]}</span><button type="button" class="btn btn-sm btn-ghost" onclick={() => irA(p.id)}>Cambiar</button></li>
        {/each}
        <li><span class="et">Freno</span><span class="val faint">{textoFreno(t.freno)}</span><button type="button" class="btn btn-sm btn-ghost" onclick={() => ((mas.retencion = true), irA("retencion"))}>Cambiar</button></li>
      </ul>
      {#if t.retencion.modo === "igual"}<div class="notice notice-warn"><TriangleAlert size={16} /><p>{AVISO_IGUAL}</p></div>{/if}
    {/if}

    {#if avanzado || paso === "resumen"}
      <div class="fila final">
        <div class="field crece">
          <label class="field-label" for="ee-nombre">Nombre</label>
          <input id="ee-nombre" class="input" bind:value={t.nombre} placeholder={nombrePorDefecto(t.adonde)} maxlength="80" />
        </div>
        <label class="switch-row activo"><input type="checkbox" class="switch" bind:checked={t.activo} /><span>Activo</span></label>
      </div>
    {/if}

    {#if (avanzado || paso === "resumen") && error}<p class="error-campo" role="alert">{error}</p>{:else if !avanzado && errorPaso}<p class="error-campo" role="alert">{errorPaso}</p>{/if}

    <footer>
      {#if !avanzado && paso !== "que" && !(paso === "resumen" && !nuevo)}
        <button type="button" class="btn btn-ghost" onclick={atras}><ArrowLeft size={15} />{desdeResumen && paso !== "resumen" ? "Volver al resumen" : "Atrás"}</button>
      {/if}
      <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
      {#if avanzado || paso === "resumen"}
        <button type="submit" class="btn btn-primary" disabled={!!error}><Save size={15} />Guardar…</button>
      {:else}
        <button type="submit" class="btn btn-primary" disabled={!!errorPaso}>{desdeResumen ? "Listo" : "Siguiente"}<ArrowRight size={15} /></button>
      {/if}
    </footer>
  </form>
</Modal>

{#if conectar === "dropbox"}
  <ConectarNube {cliente} equipo={hace} onclose={() => (conectar = null)} />
{:else if conectar === "otro"}
  <ConectarDestino {cliente} equipo={hace} onclose={() => (conectar = null)} />
{/if}

<style>
  .editor {
    gap: var(--sp-3, 12px);
  }
  .cab {
    align-items: flex-start;
  }
  .avanzado {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
    white-space: nowrap;
  }
  .hechos {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  .linea {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm, 8px);
    background: var(--surface-2, transparent);
    color: inherit;
    font: inherit;
    font-size: var(--fs-sm);
    text-align: left;
    cursor: pointer;
  }
  .linea:hover {
    border-color: var(--accent);
  }
  .linea .et,
  .resumen .et {
    color: var(--text-muted);
    min-width: 8.5em;
  }
  .linea .val,
  .resumen .val {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .paso-n {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .paso {
    border: 0;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 10px;
    min-width: 0;
  }
  .paso legend {
    font-weight: 600;
    margin-bottom: 4px;
    display: inline-flex;
    gap: 6px;
    align-items: center;
  }
  .opc {
    display: grid;
    gap: 6px;
  }
  .opc.fila {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }
  .radio,
  .check {
    display: flex;
    gap: 8px;
    align-items: flex-start;
  }
  .radio > span,
  .check > span,
  .tarjeta > span {
    display: grid;
    gap: 2px;
  }
  .checks {
    display: grid;
    gap: 6px;
    max-height: 220px;
    overflow: auto;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm, 8px);
  }
  .tarjetas {
    display: grid;
    gap: 8px;
  }
  .tarjeta {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm, 8px);
    cursor: pointer;
  }
  .tarjeta.on {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent) inset;
  }
  .tarjeta.no {
    opacity: 0.6;
    cursor: not-allowed;
  }
  .dias {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
  }
  .mas summary {
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--fs-sm);
    color: var(--text-muted);
    list-style: none;
  }
  .mas summary::-webkit-details-marker {
    display: none;
  }
  .mas[open] summary :global(svg) {
    transform: rotate(180deg);
  }
  .mas > :global(*:not(summary)) {
    margin-top: 8px;
  }
  .fila {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    align-items: flex-end;
  }
  .crece {
    flex: 1 1 220px;
  }
  .activo {
    padding-bottom: 8px;
  }
  .conectar {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .resumen {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 2px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm, 8px);
  }
  .resumen li {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 8px 10px;
    font-size: var(--fs-sm);
  }
  .resumen li + li {
    border-top: 1px solid var(--border);
  }
  .nota {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .corto {
    max-width: 120px;
  }
  footer {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
  }
  .check > span {
    display: block;
  }
  @media (max-width: 520px) {
    .cab {
      flex-wrap: wrap;
    }
    .avanzado {
      margin-left: 48px;
    }
    .linea {
      flex-wrap: wrap;
    }
    .linea .et,
    .resumen .et {
      min-width: 0;
    }
    .linea .val {
      flex-basis: 100%;
      order: 3;
    }
    .resumen li {
      display: grid;
      grid-template-columns: 1fr auto;
      gap: 2px 8px;
    }
    .resumen li .val {
      grid-column: 1 / -1;
      grid-row: 2;
    }
  }
</style>
