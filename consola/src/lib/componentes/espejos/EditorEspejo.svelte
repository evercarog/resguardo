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
  import { ArrowLeft, ArrowRight, Check, ChevronDown, Cloud, CloudUpload, Pencil, Save, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import BotonFalta from "../BotonFalta.svelte";
  import ElegirDestinoPaso, { type OpcionDestino } from "../ElegirDestinoPaso.svelte";
  import EditorHorario from "../EditorHorario.svelte";
  import ConectarNube from "../ConectarNube.svelte";
  import ConectarDestino from "../ConectarDestino.svelte";
  import Ayuda from "../Ayuda.svelte";
  import { app, puede } from "$lib/estado.svelte";
  import { catalogoDe, cargarCatalogo, guardarEnCatalogo } from "$lib/catalogoDestinos.svelte";
  import { destinosParaPasos, detalleDestino } from "$lib/cadenas";
  import { bucketDeNube, claveNube, errorBucket, errorSubcarpeta, nombreZonaPorDefecto, nubeConBucket, PRINCIPAL, subcarpetaDe, unirCarpeta, zonasDe, type DestinoVista } from "$lib/destinos";
  import { TIPOS_NUBE } from "$lib/espejo";
  import { errorCarpetaEspejo } from "$lib/ganchos";
  import { clasificacionDeVista } from "$lib/regla321";
  import { corta } from "$lib/tipoDestino";
  import { bytes, plural } from "$lib/formato";
  import { admitePrimeraDesde, empiezaDeCero, madrugadaSiguiente, primeraSubida } from "$lib/espejoPagina";
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
  import LimiteFranjas from "../LimiteFranjas.svelte";
  import { destinoEspejo, salenDelEspejo, textoSalen } from "$lib/espejosRepo";
  import { admiteFranjas, velocidadEnFrase } from "$lib/franjas";

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
  /** 15m (1): al cambiar uno que ya existía, los repositorios que dejan de estar en él. */
  const salen = $derived(nuevo ? [] : salenDelEspejo(trabajo, t, repos.map((o) => o.valor)));
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
      // Plan 0.8.0 (B2): con el bucket y la carpeta que recuerda.
      const bb = v ? bucketDeNube(v) : null;
      const donde = bb && (bb.bucket || bb.base) ? unirCarpeta(bb.bucket, bb.base) : null;
      l.push({ valor: `nube:${n.nombre}`, nombre: v?.nombre ?? n.nombre, detalle: [v ? detalleDestino(v) : TIPOS_NUBE[n.tipo]?.nombre, donde].filter(Boolean).join(" · "), clase: "nube", uso: { ok: true, motivo: inmutable === false ? "No es inmutable" : undefined }, tipoDestino: v ? corta(clasificacionDeVista(v, equipos)) : undefined });
    }
    // Plan 0.8.0 (B2, 14b): los destinos del cliente que aún no están conectados aquí (B2 y S3 del
    // catálogo, nubes conectadas en otros equipos): primero los que ya existen, y solo se piden sus credenciales.
    for (const v of porConectar) l.push({ valor: `conectar:${v.clave}`, nombre: v.nombre, detalle: detalleDestino(v), clase: "nube", uso: { ok: false, motivo: `Aún no está conectado en ${hace.nombre}`, accion: { tipo: "conectar_nube", equipo: hace, nube: v.clave, tipoNube: tipoDeVista(v), texto: `Conectar en ${hace.nombre}` } }, tipoDestino: corta(clasificacionDeVista(v, equipos)) });
    l.push({ valor: "carpeta", nombre: `Otra carpeta de ${hace.nombre}…`, detalle: "Mejor en otro disco", clase: "carpeta", uso: { ok: true } });
    return l;
  });
  /** El tipo de nube de un destino de la lista (B2, S3, Dropbox…). */
  const tipoDeVista = (v: DestinoVista) => v.nube?.tipo ?? (v.tipo === "b2" || v.tipo === "s3" ? v.tipo : (v.destino?.tipo ?? v.tipo));
  /** Los destinos de nube o de red que aún no están conectados en quien hace el espejo (no los suyos). */
  const porConectar = $derived(
    vistas.filter((v) => {
      if (v.zona || (v.nube && v.nube.equipo.id === hace.id)) return false;
      if (v.nube && nubesHace.some((n) => n.nombre === v.nube!.nombre)) return false;
      if (v.nube) return v.nube.tipo !== "drive";
      return v.clase === "suelto" && (v.tipo === "b2" || v.tipo === "s3");
    }),
  );
  /** «Conectar en …» de un destino que aún no está aquí: el formulario con lo que ya se sabe de él. */
  function conectarVista(clave: string) {
    const v = vistas.find((x) => x.clave === clave);
    if (!v) return;
    const bb = v.nube ? bucketDeNube(v) : { bucket: v.catalogo?.donde?.split(/[:/]/).pop() ?? null, base: v.catalogo?.carpeta ?? null };
    const servidor = v.tipo === "s3" && v.catalogo?.donde?.includes("/") ? v.catalogo.donde.split("/")[0] : null;
    conectarCon = { tipo: tipoDeVista(v), nombre: v.nombre.slice(0, 40), bucket: bb.bucket, base: bb.base ?? "Resguardo", servidor };
    conectar = "otro";
  }
  let conectarCon = $state<{ tipo: string; nombre: string; bucket: string | null; base: string | null; servidor: string | null } | null>(null);
  const valorAdonde = $derived(t.adonde.tipo === "zona" ? `zona:${t.adonde.carpeta}` : t.adonde.tipo === "nube" ? `nube:${t.adonde.nube ?? ""}` : "carpeta");
  /** Al elegir otro destino: su carpeta y su bloqueo de objetos (si lo dice el catálogo). */
  function elegirAdonde(v: string) {
    if (v === valorAdonde) return;
    if (v.startsWith("zona:")) t.adonde = { tipo: "zona", carpeta: v.slice(5) };
    else if (v.startsWith("nube:")) {
      // Plan 0.8.0 (B2): el bucket y la carpeta base que recuerda el destino; aquí, como mucho, una subcarpeta.
      t.adonde = { tipo: "nube", nube: v.slice(5), carpeta: "" };
      // Sin carpeta base en el destino, «Resguardo» (como antes); con ella, ninguna subcarpeta.
      sub = infoNube.base ? "" : "Resguardo";
      propia = false;
      bucketEscrito = "";
      rehacerCarpeta();
    } else t.adonde = { tipo: "carpeta", carpeta: "" };
    const vista = v.startsWith("nube:") ? vistas.find((x) => x.nube && x.nube.equipo.id === hace.id && x.nube.nombre === v.slice(5)) : v.startsWith("zona:") ? vistas.find((x) => x.zona && x.zona.almacen.id === hace.id && x.zona.id === v.slice(5)) : undefined;
    const dias = vista ? clasificacionDeVista(vista, equipos).bloqueoDias : null;
    t.bloqueo_dias = dias ?? null;
    if (dias && t.retencion.modo === "igual") t.retencion = { modo: "retraso", dias: dias + 1 };
    if (dias && t.retencion.modo === "retraso" && t.retencion.dias <= dias) t.retencion = { modo: "retraso", dias: dias + 1 };
  }
  const win = $derived(/windows/i.test(hace.so ?? ""));
  const errorCarpeta = $derived(t.adonde.tipo === "carpeta" && t.adonde.carpeta.trim() ? errorCarpetaEspejo(t.adonde.carpeta, win) : null);
  const tipoNube = $derived(t.adonde.tipo === "nube" ? nubesHace.find((n) => n.nombre === t.adonde.nube)?.tipo : undefined);
  // Plan 0.8.0 (B2, 14b): en una nube, el bucket y la carpeta base los pone el destino; aquí solo una subcarpeta
  // opcional, y se enseña la ruta final. Un espejo de antes con «bucket/carpeta» escrita a mano sigue igual.
  const vistaNube = $derived(t.adonde.tipo === "nube" ? vistas.find((x) => x.nube && x.nube.equipo.id === hace.id && x.nube.nombre === t.adonde.nube) : undefined);
  const infoNube = $derived(vistaNube ? bucketDeNube(vistaNube) : { bucket: null, base: null, deducido: false });
  /** Una nube con bucket (B2, S3, NAS) que aún no lo recuerda: se pide una vez y se guarda en el destino. */
  const pideBucket = $derived(t.adonde.tipo === "nube" && nubeConBucket(tipoNube) && !infoNube.bucket);
  let bucketEscrito = $state("");
  let sub = $state("");
  /** La carpeta de un espejo guardado que no cuelga del bucket y la carpeta base: se respeta tal cual. */
  let propia = $state(false);
  const bucketUsado = $derived(infoNube.bucket ?? (bucketEscrito.trim() || null));
  function rehacerCarpeta() {
    if (t.adonde.tipo !== "nube" || propia) return;
    t.adonde = { ...t.adonde, carpeta: unirCarpeta(nubeConBucket(tipoNube) ? bucketUsado : null, infoNube.base, sub) };
  }
  // Al abrir un espejo que ya va a una nube: lo que va detrás del bucket y la carpeta base (o «propia»).
  untrack(() => {
    if (t.adonde.tipo !== "nube") return;
    const s = subcarpetaDe(t.adonde.carpeta, nubeConBucket(tipoNube) ? infoNube.bucket : null, infoNube.base);
    if (s === null || (nubeConBucket(tipoNube) && !infoNube.bucket)) propia = !!t.adonde.carpeta.trim();
    else sub = s;
  });
  const errorBucketEscrito = $derived(pideBucket && !propia ? errorBucket(tipoNube ?? "", bucketEscrito) : null);
  const errorSub = $derived(propia ? null : errorSubcarpeta(sub));
  /** Un repositorio de ejemplo para la ruta final («caja-1/siigo»). */
  const ejemploRepo = $derived(t.que.tipo === "repos" && t.que.repos[0] ? t.que.repos[0] : quien === "equipo" ? (repos[0]?.valor ?? "<repositorio>") : (repos[0]?.valor ?? "<equipo>/<repositorio>"));
  const rutaFinal = $derived(t.adonde.tipo === "zona" ? null : t.adonde.carpeta.trim() ? `${t.adonde.carpeta.trim().replace(/[\\/]+$/, "")}${t.adonde.tipo === "carpeta" && win ? "\\" : "/"}${t.adonde.tipo === "carpeta" && win ? ejemploRepo.replace(/\//g, "\\") : ejemploRepo}` : null);
  /** Al guardar: el bucket escrito queda en el destino (catálogo del cliente), para no pedirlo más. */
  function recordarBucket() {
    if (!pideBucket || propia || !bucketEscrito.trim() || t.adonde.tipo !== "nube" || !t.adonde.nube || !puede.administrar(cliente.rol)) return;
    const nombre = vistaNube?.catalogo?.nombre?.trim() || t.adonde.nube;
    void guardarEnCatalogo(cliente.id, claveNube(hace.id, t.adonde.nube), { nombre, tipo: "nube", donde: bucketEscrito.trim() }).catch(() => {});
  }
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
  const error = $derived(errorTrabajos(lista, quien) ?? errorCarpeta ?? (t.adonde.tipo === "nube" ? (errorBucketEscrito ?? errorSub) : null));
  /** El error del paso (o null): no se avanza con algo a medias. */
  const errorPaso = $derived.by(() => {
    if (paso === "que") return t.que.tipo === "equipos" && !t.que.equipos.length ? "Elige al menos un equipo." : t.que.tipo === "repos" && !t.que.repos.length ? "Elige al menos un repositorio." : null;
    if (paso === "adonde") {
      if (t.adonde.tipo === "carpeta") return t.adonde.carpeta.trim() ? errorCarpeta : "Escribe la carpeta.";
      if (t.adonde.tipo === "nube") {
        if (errorBucketEscrito) return pideBucket && !bucketEscrito.trim() ? `Escribe el ${tipoNube === "smb" ? "recurso compartido" : "bucket"}.` : errorBucketEscrito;
        if (errorSub) return errorSub;
        if (!t.adonde.carpeta.trim()) return "Pon una subcarpeta (o una carpeta base en el destino).";
      }
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
  // --- 14s: la primera subida, avisada (con su tamaño y cuándo empieza) ---
  const deCero = $derived(t.activo && empiezaDeCero(t, nuevo ? null : trabajo, nuevo));
  const primera = $derived(deCero ? primeraSubida(t, hace, equipos) : null);
  const madrugada = $derived(admitePrimeraDesde(hace));
  // svelte-ignore state_referenced_locally
  let empezar = $state<"ahora" | "madrugada">(trabajo.cuando.primera_desde && Date.parse(trabajo.cuando.primera_desde) > Date.now() ? "madrugada" : "ahora");
  function guardar() {
    if (error) return;
    recordarBucket();
    const cuando = { ...t.cuando };
    if (primera && madrugada && empezar === "madrugada") cuando.primera_desde = madrugadaSiguiente(Date.now());
    else if (deCero) delete cuando.primera_desde;
    onguardar({ ...t, cuando, nombre: t.nombre.trim() || nombrePorDefecto(t.adonde) });
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
        <ElegirDestinoPaso id="ee-adonde" etiqueta="Adónde" {opciones} bind:value={() => valorAdonde, elegirAdonde} alConectar={(_e, clave) => conectarVista(clave)} />
        {#if t.adonde.tipo === "carpeta"}
          <div class="field">
            <label class="field-label" for="ee-carpeta">Carpeta</label>
            <input id="ee-carpeta" class="input mono" bind:value={t.adonde.carpeta} placeholder={win ? "E:\\Resguardo-espejo" : "/mnt/disco2/espejo"} spellcheck="false" />
            {#if errorCarpeta}<p class="error-campo">{errorCarpeta}</p>{:else}<span class="field-hint">Si falla el disco del original, el espejo sigue ahí.</span>{/if}
          </div>
        {:else if t.adonde.tipo === "nube"}
          {#if propia}
            <div class="field">
              <label class="field-label" for="ee-nube">Carpeta dentro de la nube</label>
              <input id="ee-nube" class="input mono" bind:value={t.adonde.carpeta} spellcheck="false" />
              <span class="field-hint">Este espejo usa su propia carpeta (con el bucket delante). Se queda así. {#if infoNube.bucket || infoNube.base}<button type="button" class="btn btn-sm btn-ghost enlinea" onclick={() => ((propia = false), (sub = ""), rehacerCarpeta())}>Usar la carpeta del destino</button>{/if}</span>
            </div>
          {:else}
            <div class="fila">
              {#if pideBucket}
                <div class="field">
                  <label class="field-label" for="ee-bucket">{tipoNube === "smb" ? "Recurso compartido" : "Contenedor (bucket)"}</label>
                  <input id="ee-bucket" class="input mono" bind:value={bucketEscrito} oninput={rehacerCarpeta} spellcheck="false" placeholder={tipoNube === "smb" ? "copias" : "mi-bucket"} />
                  {#if errorBucketEscrito && bucketEscrito}<p class="error-campo">{errorBucketEscrito}</p>{:else}<span class="field-hint">Una sola vez: el destino lo recordará.</span>{/if}
                </div>
              {/if}
              <div class="field crece">
                <label class="field-label" for="ee-sub">Subcarpeta <span class="faint">(opcional)</span></label>
                <input id="ee-sub" class="input mono" bind:value={sub} oninput={rehacerCarpeta} spellcheck="false" placeholder="p. ej. oficina" />
                {#if errorSub}<p class="error-campo">{errorSub}</p>{/if}
              </div>
            </div>
          {/if}
          {#if tipoNube && TIPOS_NUBE[tipoNube] && !TIPOS_NUBE[tipoNube].inmutable}
            <div class="notice notice-warn"><TriangleAlert size={16} /><p>{TIPOS_NUBE[tipoNube].nombre} no es inmutable: quien entre en la cuenta podría borrar lo de allí.</p></div>
          {/if}
        {/if}
        {#if rutaFinal}<p class="ruta faint">Quedará en <code>{rutaFinal}</code></p>{/if}
        {#if puede.administrar(cliente.rol)}
          <div class="conectar">
            <button type="button" class="btn btn-sm btn-ghost" onclick={() => ((conectarCon = null), (conectar = "otro"))}><Cloud size={13} />Conectar otra nube o servidor en {hace.nombre}…</button>
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
              <span class="field-hint">{t.limite_kib ? `${velocidadEnFrase(t.limite_kib)}. ` : ""}Fuera de las franjas de abajo (si las hay).</span>
            </div>
            <!-- Plan 0.8.0, F4 (14t): p. ej. 2 MB/s en horario de oficina y sin límite de noche. -->
            <LimiteFranjas id="ee-fr" bind:franjas={() => t.limite_franjas ?? [], (v) => (t.limite_franjas = v.length ? v : null)} porDefecto={t.limite_kib} admite={admiteFranjas(hace)} equipo={hace.nombre} />
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
    {#if (avanzado || paso === "resumen") && salen.length}
      <!-- 15m (1): los repositorios que dejan de estar y qué pasa con lo ya subido. -->
      <div class="notice notice-warn" role="status"><TriangleAlert size={16} /><p>{textoSalen(salen.map(nombreRepo), destinoEspejo(t, hace))}</p></div>
    {/if}

    {#if (avanzado || paso === "resumen") && primera}
      <!-- 14s: la primera subida, con su tamaño y cuándo empieza (y se ve en vivo). -->
      <div class="notice notice-info primera">
        <CloudUpload size={16} />
        <div>
          <p>
            Al guardar empezará la primera subida: {primera.bytes ? `unos ${bytes(primera.bytes)}` : "todo lo que guarda"}{primera.sinTamano ? ` (sin contar ${plural(primera.sinTamano, "repositorio", "repositorios")} de los que no se sabe el tamaño)` : ""}. Lo que ya esté en el destino no se vuelve a subir.
          </p>
          {#if madrugada}
            <div class="opciones-primera" role="radiogroup" aria-label="Cuándo empieza la primera subida">
              <label><input type="radio" name="ee-primera" value="ahora" bind:group={empezar} /> Empezar ahora</label>
              <label><input type="radio" name="ee-primera" value="madrugada" bind:group={empezar} /> Empezar en la próxima franja de la madrugada (desde las 2:00)</label>
            </div>
          {/if}
          <p class="faint">Se ve en vivo en la página del espejo y en todas las consolas.</p>
        </div>
      </div>
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
        <BotonFalta type="submit" falta={error ? { texto: error, yaVisible: true } : null}><Save size={15} />Guardar…</BotonFalta>
      {:else}
        <BotonFalta type="submit" falta={errorPaso ? { texto: errorPaso, yaVisible: true } : null}>{desdeResumen ? "Listo" : "Seguir"}<ArrowRight size={15} /></BotonFalta>
      {/if}
    </footer>
  </form>
</Modal>

{#if conectar === "dropbox"}
  <ConectarNube {cliente} equipo={hace} onclose={() => (conectar = null)} />
{:else if conectar === "otro"}
  <ConectarDestino {cliente} equipo={hace} inicial={conectarCon} onclose={() => (conectar = null)} />
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
    color: var(--text-3);
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
    color: var(--text-3);
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
  /* Plan 0.8.0 (B2): la ruta final que va a quedar. */
  .ruta {
    margin: 0;
    font-size: var(--fs-sm);
    overflow-wrap: anywhere;
  }
  .enlinea {
    margin-left: 4px;
  }
  .primera div {
    display: grid;
    gap: 6px;
  }
  .primera p {
    margin: 0;
  }
  .opciones-primera {
    display: grid;
    gap: 4px;
  }
  .opciones-primera label {
    display: inline-flex;
    gap: 6px;
    align-items: center;
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
