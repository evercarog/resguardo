<script lang="ts">
  import Ilustracion from "$ui/componentes/Ilustracion.svelte";
  import { tip } from "$lib/tooltip";
  // Restaurar, como un asistente tranquilo: equipo → repositorio →
  // contraseña del repositorio (abre una sesión cifrada «explorar») → versión
  // (una línea de tiempo por días) → archivos (árbol, búsqueda y tamaños) →
  // dónde (junto al original, lo recomendado; en su sitio; o descargar por el
  // relé) → progreso → listo, con dónde quedó. La contraseña solo vive en
  // esta pantalla y se borra al salir; el servidor no ve ni rutas ni archivos.
  import { bytesRepo, destinoDe, informeDe, nVersiones, ultimaVersion } from "$lib/repo";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Pasos from "$lib/componentes/Pasos.svelte";
  import Copiable from "$lib/componentes/Copiable.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import { saludEquipo } from "$lib/salud";
  import { cargarInformes, ultimos } from "$lib/informes.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import { onDestroy } from "svelte";
  import { page } from "$app/state";
  import {
    ArrowLeft,
    ArrowRight,
    Calendar,
    Check,
    ChevronRight,
    Database,
    Download,
    File,
    Folder,
    FolderOpen,
    FolderPlus,
    HardDrive,
    History,
    LoaderCircle,
    LockKeyhole,
    Monitor,
    MonitorSmartphone,
    Search,
    Server,
    ShieldCheck,
    TriangleAlert,
    X,
  } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { enFondo } from "$lib/actividad.svelte";
  import { actual, puede, reloj } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { aB64, aleatorio, borrar } from "$lib/cripto/bytes";
  import { ErrorFaltaAdmin, ErrorLlavesCambiadas, mandarOrden } from "$lib/ordenar";
  import { comprobarLlaves } from "$lib/fijadas";
  import AlertaLlaves from "$lib/componentes/AlertaLlaves.svelte";
  import { Sesion, type MensajeEquipo } from "$lib/sesion";
  import { bajarRelevo, guardar, type Progreso } from "$lib/descarga";
  import { bytes, dia, fechaCorta, hora, plural } from "$lib/formato";
  import { ESTADO_ORDEN } from "$lib/salud";
  import { resultadoFirmado } from "$lib/cripto/claves";
  import type { Equipo, Orden, RepositorioResumen } from "$lib/tipos";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import CampoClave from "$lib/componentes/CampoClave.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import RestaurarEnOtro from "$lib/componentes/RestaurarEnOtro.svelte";
  import LineaTiempoVersiones from "$lib/componentes/repo/LineaTiempoVersiones.svelte";
  import { reglaEfectiva } from "$lib/lineaTiempo";

  interface Version {
    id: string;
    cuando: string;
    archivos?: number;
    bytes?: number;
    etiquetas?: string[];
  }
  interface Entrada {
    nombre: string;
    tipo: "dir" | "archivo";
    bytes?: number;
    modificado?: string;
    ruta?: string;
  }

  type Paso = "origen" | "repo" | "clave" | "version" | "archivos" | "destino" | "progreso";
  let paso = $state<Paso>("origen");
  let equipoId = $state(page.url.searchParams.get("equipo") ?? "");
  $effect(() => void cargarInformes(actual.id, actual.equipos.map((e) => e.id)));
  let repoId = $state(page.url.searchParams.get("repo") ?? "");
  let contrasena = $state("");
  /** Primera vez que este navegador manda una contraseña a este equipo: también la clave de administración. */
  let claveAdmin = $state("");
  let pideAdmin = $state(false);
  let llavesCambiadas = $state(false);
  let error = $state("");
  let ocupado = $state(false);
  let pasoTxt = $state("");
  let sesion: Sesion | null = null;
  let versiones = $state<Version[]>([]);
  let version = $state<Version | null>(null);
  let hijos = $state<Record<string, Entrada[] | "cargando">>({});
  let abiertas = $state<Record<string, boolean>>({});
  let elegidas = $state<Map<string, Entrada>>(new Map());
  let buscar = $state("");
  let resultados = $state<Entrada[] | null>(null);
  let destino = $state<"junto" | "original" | "descargar">("junto");
  let reemplazar = $state(false);
  let orden = $state<Orden | null>(null);
  let progreso = $state<Progreso | null>(null);
  let terminado = $state(false);
  const parar = new AbortController();

  const opciones = $derived(actual.equipos.filter((e) => e.modo !== "trasladado").flatMap((e) => (e.resumen?.repositorios ?? []).map((r) => ({ equipo: e, repo: r }))));
  const equipo = $derived<Equipo | undefined>(actual.equipos.find((e) => e.id === equipoId));
  // Recién importado de otro equipo (§10): puede que su informe aún no lo traiga; basta con su id y nombre.
  const repo = $derived<RepositorioResumen | undefined>(
    equipo?.resumen?.repositorios?.find((r) => r.id === repoId) ??
      (equipo && repoId && page.url.searchParams.get("nombre") ? { id: repoId, nombre: page.url.searchParams.get("nombre")!, destino: "", solo_lectura: true } : undefined),
  );
  /** «Restaurar en otro equipo»: abierto, y con qué repositorio de partida. */
  let enOtro = $state<{ equipo: string; repo: string } | null | undefined>(undefined);
  /** Rutas de una versión: las de restic, siempre con «/» (en Windows, `/C/Users/…`). */
  const unir = (ruta: string, nombre: string) => (ruta.endsWith("/") ? ruta + nombre : `${ruta}/${nombre}`);
  const totalElegido = $derived([...elegidas.values()].reduce((n, e) => n + (e.bytes ?? 0), 0));
  const hayCarpetas = $derived([...elegidas.values()].some((e) => e.tipo === "dir"));
  const MAX_RELEVO = 500 * 1000 * 1000;

  // Si se llega con ?equipo=&repo= (o solo ?equipo=), se saltan los primeros pasos (una vez).
  let saltado = false;
  $effect(() => {
    if (saltado || paso !== "origen" || !equipo) return;
    if (page.url.searchParams.get("repo")) {
      if (repo) {
        paso = "clave";
        saltado = true;
      }
    } else if (page.url.searchParams.get("equipo")) {
      const rs = equipo.resumen?.repositorios ?? [];
      if (rs.length === 1) elegirRepo(rs[0].id);
      else paso = "repo";
      saltado = true;
    }
  });

  /** Los equipos que se pueden elegir: con algún repositorio y no trasladados. */
  const activos = $derived(actual.equipos.filter((e) => e.confirmado && e.modo !== "trasladado"));
  const conRepos = $derived(activos.filter((e) => (e.resumen?.repositorios ?? []).length));

  function elegirEquipo(e: Equipo) {
    equipoId = e.id;
    error = "";
    const rs = e.resumen?.repositorios ?? [];
    if (rs.length === 1) elegirRepo(rs[0].id);
    else paso = "repo";
  }
  function elegirRepo(id: string) {
    repoId = id;
    contrasena = "";
    paso = "clave";
  }
  /** Volver a un paso ya hecho (desde los pasos de arriba). Antes de la versión, se cierra la sesión abierta. */
  function volverA(id: string): boolean {
    if (ocupado || paso === "progreso") return false;
    const destinoPaso = id as Paso;
    if (indice(destinoPaso) >= indice(paso)) return false;
    if (indice(destinoPaso) <= indice("clave")) {
      void sesion?.cerrar();
      sesion = null;
      versiones = [];
      version = null;
      elegidas = new Map();
      claveAdmin = "";
    }
    if (destinoPaso === "origen") repoId = "";
    error = "";
    paso = destinoPaso;
    return true;
  }

  onDestroy(() => {
    parar.abort();
    void sesion?.cerrar();
    contrasena = claveAdmin = "";
  });
  // Al llegar a la contraseña se mira si las llaves del equipo ya están fijadas aquí.
  $effect(() => {
    if (paso === "clave" && equipo)
      void comprobarLlaves(actual.id, equipo).then((x) => {
        llavesCambiadas = x === "cambiada";
        pideAdmin = x === "sin_fijar";
      });
  });

  async function abrir(e: SubmitEvent) {
    e.preventDefault();
    if (!equipo || !repo || !actual.cliente) return;
    error = "";
    ocupado = true;
    try {
      sesion = await Sesion.abrir({ cliente: actual.cliente, equipo, tipo: "explorar", cuerpo: { repo: repo.id }, secretos: { repo: { repo: repo.id, contrasena }, claveAdmin: pideAdmin ? claveAdmin : undefined }, alPaso: (t) => (pasoTxt = t) });
      claveAdmin = "";
      pideAdmin = false;
      pasoTxt = "Esperando al equipo…";
      await sesion.lista;
      pasoTxt = "Leyendo las versiones…";
      const r = await sesion.pedir<MensajeEquipo & { versiones: Version[] }>("versiones", { repo: repo.id });
      versiones = (r.versiones ?? []).sort((a, b) => Date.parse(b.cuando) - Date.parse(a.cuando));
      paso = "version";
      // Desde la lista de versiones del repositorio: esa versión directamente
      // («Explorar»), o entera, lista para elegir dónde («Restaurar»).
      const pedida = page.url.searchParams.get("version");
      const v = pedida ? versiones.find((x) => x.id === pedida || x.id.startsWith(pedida)) : undefined;
      if (v) {
        elegirVersion(v);
        if (page.url.searchParams.get("todo") === "1") {
          pasoTxt = "Leyendo la versión…";
          await cargar("/");
          const raiz = hijos["/"];
          if (Array.isArray(raiz) && raiz.length) {
            elegidas = new Map(raiz.map((x) => [unir("/", x.nombre), x]));
            paso = "destino";
          }
        }
      }
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) llavesCambiadas = true;
      else if (err instanceof ErrorFaltaAdmin) pideAdmin = true;
      error = err instanceof ErrorLlavesCambiadas ? "" : (err as Error).message;
      void sesion?.cerrar();
      sesion = null;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }

  async function cargar(ruta: string) {
    if (!sesion || !version || hijos[ruta]) return;
    hijos[ruta] = "cargando";
    try {
      const r = await sesion.pedir<MensajeEquipo & { entradas: Entrada[] }>("listar", { version: version.id, ruta });
      hijos[ruta] = [...(r.entradas ?? [])].sort((a, b) => (a.tipo === b.tipo ? a.nombre.localeCompare(b.nombre) : a.tipo === "dir" ? -1 : 1));
    } catch (err) {
      delete hijos[ruta];
      error = (err as Error).message;
    }
  }

  function elegirVersion(v: Version) {
    version = v;
    hijos = {};
    abiertas = {};
    elegidas = new Map();
    resultados = null;
    paso = "archivos";
    void cargar("/");
  }

  function marcar(ruta: string, e: Entrada, on: boolean) {
    const m = new Map(elegidas);
    if (on) m.set(ruta, e);
    else m.delete(ruta);
    elegidas = m;
  }

  async function hacerBusqueda(ev: SubmitEvent) {
    ev.preventDefault();
    if (!sesion || !version || !buscar.trim()) return;
    resultados = null;
    try {
      const r = await sesion.pedir<MensajeEquipo & { resultados: Entrada[] }>("buscar", { version: version.id, texto: buscar.trim() });
      resultados = r.resultados ?? [];
    } catch (err) {
      error = (err as Error).message;
    }
  }

  // La línea de tiempo del paso «Versión»: la copia de cada versión sale del informe (sus ids son los 8 primeros).
  const versionesLinea = $derived.by(() => {
    const inf = equipo && repo ? informeDe(ultimos.porEquipo[equipo.id], repo.id) : null;
    return versiones.map((v) => {
      const x = inf?.versiones.find((i) => v.id.startsWith(i.id) || i.id.startsWith(v.id));
      return { id: v.id, hora: v.cuando, copia: x?.copia ?? null, bytes: v.bytes ?? x?.total_bytes ?? null, anadido: x ? (x.anadido_empaquetado ?? x.anadido) : null, archivos: v.archivos ?? null, etiquetas: v.etiquetas ?? [] };
    });
  });
  const retencionLinea = $derived(repo && equipo ? reglaEfectiva(repo, destinoDe(equipo.resumen?.destinos, repo), actual.equipos) : null);
  async function restaurar() {
    if (!equipo || !repo || !version || !actual.cliente) return;
    error = "";
    ocupado = true;
    const rutas = [...elegidas.keys()];
    carpetaFija = carpetaRestaurado;
    try {
      if (destino === "descargar") {
        const relevo = crypto.randomUUID();
        const clave = aleatorio(32);
        orden = await mandarOrden({
          cliente: actual.cliente,
          equipo,
          tipo: "descargar",
          cuerpo: { repo: repo.id, version: version.id, rutas, formato: rutas.length === 1 && !hayCarpetas ? "archivo" : "zip", relevo: { id: relevo, clave: aB64(clave) } },
          secretos: { repo: { repo: repo.id, contrasena } },
          relevo: { id: relevo, max_bytes: MAX_RELEVO },
          alPaso: (t) => (pasoTxt = t),
        });
        paso = "progreso";
        const blob = await bajarRelevo({ cliente: actual.cliente.id, relevo, clave, signal: parar.signal, alProgreso: (p) => (progreso = p) });
        borrar(clave);
        const nombre = rutas.length === 1 && !hayCarpetas ? rutas[0].split(/[\\/]/).pop() || "archivo" : `restauracion-${equipo.nombre}-${new Date(version.cuando).toISOString().slice(0, 10)}.zip`;
        await guardar(blob, nombre);
        terminado = true;
        avisar("Descarga lista.");
      } else {
        orden = await mandarOrden({
          cliente: actual.cliente,
          equipo,
          tipo: "restaurar",
          cuerpo: { repo: repo.id, version: version.id, rutas, destino: destino === "original" ? "original" : "junto", reemplazar: destino === "original" && reemplazar },
          secretos: { repo: { repo: repo.id, contrasena } },
          alPaso: (t) => (pasoTxt = t),
        });
        paso = "progreso";
        seguir(orden.id);
      }
    } catch (err) {
      error = err instanceof ErrorLlavesCambiadas ? err.message : (err as Error).message;
      if (paso !== "progreso") paso = "destino";
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }

  /** Tras un «en su sitio» rechazado: lo mismo, junto al original (un clic). */
  async function restaurarJunto() {
    destino = "junto";
    reemplazar = false;
    terminado = false;
    orden = null;
    await restaurar();
  }

  function seguir(id: string) {
    const t = setInterval(async () => {
      try {
        const o = (await enFondo(() => api.ordenesEquipo(actual.id, equipoId, 10))).find((x) => x.id === id);
        if (o) orden = o;
        if (o && ["hecha", "fallida", "rechazada", "cancelada", "caducada"].includes(o.estado)) {
          clearInterval(t);
          terminado = true;
        }
      } catch {
        /* se reintenta */
      }
    }, 1500);
    parar.signal.addEventListener("abort", () => clearInterval(t));
  }

  // «Junto al original», como lo hace el agente (sesiones_v2.rs, restaurar): en
  // la carpeta de cada elemento, una nueva «Restaurado AAAA-MM-DD HHMM» (su
  // hora local al restaurar) con el elemento dentro.
  const p2 = (n: number) => String(n).padStart(2, "0");
  const carpetaRestaurado = $derived.by(() => {
    const d = new Date(reloj.ahora);
    return `Restaurado ${d.getFullYear()}-${p2(d.getMonth() + 1)}-${p2(d.getDate())} ${p2(d.getHours())}${p2(d.getMinutes())}`;
  });
  /** Ruta de restic («/C/Users/…») a la del equipo («C:\Users\…» en Windows). */
  const rutaLocal = (r: string) => {
    if (!/windows/i.test(equipo?.so ?? "")) return r;
    const resto = r.replace(/^\/+/, "");
    const i = resto.indexOf("/");
    const unidad = i < 0 ? resto : resto.slice(0, i);
    return unidad.length === 1 ? `${unidad}:\\${i < 0 ? "" : resto.slice(i + 1).replaceAll("/", "\\")}` : r.replaceAll("/", "\\");
  };
  const esWindows = $derived(/windows/i.test(equipo?.so ?? ""));
  /** La carpeta de una ruta, como la ve el equipo («C:\Users\ana\Documentos»). */
  const carpetaDe = (r: string) => {
    const corte = r.replace(/\/+$/, "").lastIndexOf("/");
    return corte <= 0 ? "" : rutaLocal(r.slice(0, corte));
  };
  const ejemploJunto = $derived.by(() => {
    const primera = [...elegidas.keys()][0];
    if (!primera) return "";
    const corte = primera.replace(/\/+$/, "").lastIndexOf("/");
    if (corte <= 0) return "";
    const padre = primera.slice(0, corte);
    const nombre = primera.slice(corte + 1);
    const sep = /windows/i.test(equipo?.so ?? "") ? "\\" : "/";
    return `${rutaLocal(padre).replace(/[\\/]+$/, "")}${sep}${carpetaRestaurado}${sep}${nombre}`;
  });

  /** La carpeta «Restaurado …» junto al primer elemento, con la hora en que se pidió (para «Abrir la carpeta»). */
  let carpetaFija = $state("");
  const carpetaJunto = $derived.by(() => {
    const primera = [...elegidas.keys()][0];
    if (!primera || !carpetaFija) return "";
    const corte = primera.replace(/\/+$/, "").lastIndexOf("/");
    if (corte <= 0) return "";
    const sep = /windows/i.test(equipo?.so ?? "") ? "\\" : "/";
    return `${rutaLocal(primera.slice(0, corte)).replace(/[\\/]+$/, "")}${sep}${carpetaFija}`;
  });

  const PASOS: { id: Paso; texto: string }[] = [
    { id: "origen", texto: "Equipo" },
    { id: "repo", texto: "Repositorio" },
    { id: "clave", texto: "Contraseña" },
    { id: "version", texto: "Versión" },
    { id: "archivos", texto: "Archivos" },
    { id: "destino", texto: "Dónde" },
    { id: "progreso", texto: "Listo" },
  ];
  const indice = (p: Paso) => PASOS.findIndex((x) => x.id === p);

  /** Cómo va una restauración en el equipo (la orden): enviada, recogida, restaurando, hecha. */
  const ETAPAS = [
    { id: "enviada", texto: "Enviada" },
    { id: "recogida", texto: "El equipo la recoge" },
    { id: "restaurando", texto: "Restaurando" },
    { id: "hecha", texto: "Hecho" },
  ];
  const etapa = $derived(!orden ? 0 : orden.estado === "pendiente" ? 1 : orden.estado === "entregada" || orden.estado === "en_marcha" ? 2 : 3);
</script>

<svelte:head><title>Restaurar · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <CabeceraPagina
    titulo="Restaurar archivos"
    icono={History}
    migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${actual.id}` }, { texto: "Restaurar" }]}
    resumen="Recupera archivos de cualquier versión, paso a paso. Llegan cifrados desde el equipo: el servidor no ve ni nombres ni contenido."
  >
    {#snippet acciones()}
      {#if puede.administrar(actual.cliente?.rol) && activos.length > 1 && paso !== "progreso"}
        <button class="btn" onclick={() => (enOtro = equipoId && repoId ? { equipo: equipoId, repo: repoId } : null)}><MonitorSmartphone size={16} />Restaurar en otro equipo</button>
      {/if}
    {/snippet}
  </CabeceraPagina>

  {#if !puede.ordenar(actual.cliente?.rol) && actual.cliente}
    <div class="notice notice-info"><p>Tu papel en este cliente es de solo lectura: no puede restaurar.</p></div>
  {:else}
    <Pasos pasos={PASOS} actual={paso} completo={paso === "progreso" && terminado && (destino === "descargar" || orden?.estado === "hecha")} alElegir={paso === "progreso" || ocupado ? undefined : volverA} />

    {#if paso === "origen"}
      <section class="paso-card" aria-labelledby="t-origen">
        <header class="paso-cab">
          <h2 id="t-origen">¿De qué equipo son los archivos?</h2>
          <p>Elige el equipo donde estaban. Después, su repositorio y la versión.</p>
        </header>
        {#if !actual.cargado}
          <Esqueleto forma="tarjetas" n={3} />
        {:else if conRepos.length}
          <div class="rejilla equipos-r">
            {#each conRepos as e (e.id)}
              {@const s = saludEquipo(e, reloj.ahora)}
              {@const rs = e.resumen?.repositorios ?? []}
              <button class="card tile elegible" onclick={() => elegirEquipo(e)}>
                <span class="tile-cab">
                  <span class="tile-ic">{#if e.rol === "almacenamiento"}<Server size={16} />{:else}<Monitor size={16} />{/if}</span>
                  <span class="tile-nombre"><strong>{e.nombre}</strong><span>{#if e.conectado}<span class="dot" style="--tone: var(--ok)" aria-hidden="true"></span> Conectado{:else}Visto <Tiempo iso={e.ultimo_contacto} />{/if}</span></span>
                  <Chip pequeno tono={s.tono} texto={s.texto} />
                </span>
                <span class="tile-linea">{plural(rs.length, "repositorio", "repositorios")}: {rs.map((r) => r.nombre).join(", ")}</span>
              </button>
            {/each}
          </div>
        {:else}
          <div class="card">
            <Vacio icono={History} ilustracion="sin-versiones" titulo="Nada que restaurar todavía" texto="Cuando un equipo tenga su primera copia, podrás recuperar sus archivos desde aquí.">
              <a class="btn btn-primary" href="/c/{actual.id}/equipos">Ver los equipos</a>
            </Vacio>
          </div>
        {/if}
      </section>
    {:else if paso === "repo" && equipo}
      <section class="paso-card" aria-labelledby="t-repo">
        <header class="paso-cab">
          <h2 id="t-repo">¿De qué repositorio, en {equipo.nombre}?</h2>
          <p>Cada repositorio guarda las versiones de unas copias.</p>
        </header>
        <div class="card p-0 lista">
          {#each equipo.resumen?.repositorios ?? [] as r (r.id)}
            {@const inf = informeDe(ultimos.porEquipo[equipo.id], r.id)}
            {@const copiasDe = (equipo.resumen?.copias ?? []).filter((k) => k.repo === r.id)}
            {@const ult = ultimaVersion(r, inf)}
            <button class="fila elegible-fila" onclick={() => elegirRepo(r.id)}>
              <span class="tile-ic"><Database size={16} /></span>
              <span class="fila-texto">
                <span class="fila-titulo">{r.nombre}{#if r.solo_lectura} <span class="badge badge-sm tone-neutral" use:tip={"Importado de otro equipo: se puede explorar y restaurar, pero ninguna copia escribe en él."}>Solo lectura</span>{/if}</span>
                <span class="fila-sub">{copiasDe.length ? `Copias: ${copiasDe.map((k) => k.nombre).join(", ")}` : "Sin copias que escriban en él"}</span>
              </span>
              <span class="fila-meta num">
                {plural(nVersiones(r, inf), "versión", "versiones")}{#if bytesRepo(r, inf) != null}{" · "}{bytes(bytesRepo(r, inf))}{/if}
                {#if ult}<span class="bloque">la última, <Tiempo iso={ult} /></span>{/if}
              </span>
              <ChevronRight size={16} />
            </button>
          {/each}
        </div>
        <div class="fin izq"><button class="btn btn-ghost" onclick={() => volverA("origen")}><ArrowLeft size={15} />Otro equipo</button></div>
      </section>
    {:else if paso === "clave" && equipo && repo}
      <section class="card p estrecha">
        <form class="form" onsubmit={abrir}>
          <div class="dlg-title">
            <span class="ticon"><LockKeyhole size={18} /></span>
            <div>
              <h2>Abrir «{repo.nombre}» de {equipo.nombre}</h2>
              <p>Para ver sus versiones hace falta la contraseña del repositorio. Se usa solo en esta pantalla y se olvida al salir.</p>
            </div>
          </div>
          <CampoClave requerido id="clave-repo" etiqueta="Contraseña del repositorio" bind:value={contrasena} autofocus ayuda="Está en su kit de recuperación (el que se imprimió al crearlo).">
            {#snippet extra()}<Ayuda id="contrasena-repo" />{/snippet}
          </CampoClave>
          {#if pideAdmin}
            <p class="faint pequeno">Es la primera vez que este navegador manda una contraseña a {equipo.nombre}: con la clave de administración se comprueba que sus llaves son las auténticas y se recuerdan.</p>
            <CampoClave requerido id="clave-admin" etiqueta="Clave de administración" bind:value={claveAdmin}>
              {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
            </CampoClave>
          {/if}
          {#if llavesCambiadas}<AlertaLlaves {equipo} cliente={actual.id} />{/if}
          {#if !equipo.conectado}<div class="notice notice-warn"><TriangleAlert size={16} /><p>{equipo.nombre} no está conectado ahora: abrirá el repositorio cuando vuelva. Puedes esperar aquí.</p></div>{/if}
          {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
          <div class="fin">
            {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
            <button type="button" class="btn btn-ghost" onclick={() => volverA((equipo?.resumen?.repositorios?.length ?? 0) > 1 ? "repo" : "origen")} disabled={ocupado}><ArrowLeft size={15} />Atrás</button>
            <button class="btn btn-primary" disabled={!contrasena || (pideAdmin && !claveAdmin) || ocupado || llavesCambiadas}><LockKeyhole size={15} />{pideAdmin ? "Abrir con las dos claves" : "Abrir el repositorio"}</button>
          </div>
        </form>
      </section>
    {:else if paso === "version"}
      <section class="paso-card" aria-labelledby="t-version">
        <header class="paso-cab">
          <h2 id="t-version">¿De qué momento?</h2>
          <p>{plural(versiones.length, "versión guardada", "versiones guardadas")} de «{repo?.nombre}». Elige la de antes del problema (si se borró o estropeó algo, la anterior a ese día).</p>
        </header>
        {#if versiones.length}
          <div class="card p">
            <LineaTiempoVersiones
              versiones={versionesLinea}
              copias={equipo?.resumen?.copias?.filter((k) => k.repo === repo?.id) ?? []}
              regla={retencionLinea?.regla ?? null}
              quien={retencionLinea?.quien ?? null}
              ahora={reloj.ahora}
              seleccion={version?.id ?? null}
              alElegir={(id) => {
                const v = versiones.find((x) => x.id === id);
                if (v) elegirVersion(v);
              }}
              etiqueta="Versiones de «{repo?.nombre ?? ''}» en el tiempo; elige una para ver sus archivos"
            />
          </div>
        {:else}
          <div class="card"><Vacio icono={Calendar} ilustracion="sin-versiones" titulo="Este repositorio aún no tiene versiones" texto="Cuando se haga su primera copia, aparecerá aquí." /></div>
        {/if}
      </section>
    {:else if paso === "archivos" && version}
      <section class="card p-0 archivos" aria-labelledby="t-archivos">
        <div class="cab-paso">
          <FolderOpen size={16} />
          <span class="cab-texto"><strong id="t-archivos">¿Qué archivos?</strong><span class="faint">Versión del {dia(version.cuando)}, {hora(version.cuando)}</span></span>
          <button class="link" onclick={() => volverA("version")}>Otra versión</button>
        </div>
        <form class="buscar" onsubmit={hacerBusqueda} role="search">
          <Search size={15} />
          <input class="input" type="search" placeholder="Busca por nombre: «factura», «.xlsx»…" bind:value={buscar} aria-label="Buscar en la versión" />
          <button class="btn btn-sm" disabled={!buscar.trim()}>Buscar</button>
        </form>
        {#if resultados}
          <div class="resultados">
            <p class="faint pequeno res-cab">{resultados.length ? plural(resultados.length, "resultado", "resultados") : "Nada con ese nombre en esta versión"} · <button class="link" onclick={() => (resultados = null)}>Volver a las carpetas</button></p>
            {#each resultados as r (r.ruta)}
              <label class="nodo">
                <input type="checkbox" checked={elegidas.has(r.ruta!)} onchange={(e) => marcar(r.ruta!, r, e.currentTarget.checked)} />
                {#if r.tipo === "dir"}<Folder size={15} class="ic-dir" />{:else}<File size={15} />{/if}
                <span class="nombre res-nombre"><span>{r.ruta!.replace(/\/+$/, "").split("/").pop()}</span><span class="faint pequeno">{carpetaDe(r.ruta!)}</span></span>
                {#if r.tipo !== "dir"}{#if r.modificado}<span class="faint pequeno meta fecha num">{fechaCorta(r.modificado)}</span>{/if}<span class="faint pequeno meta num">{bytes(r.bytes)}</span>{/if}
              </label>
            {/each}
          </div>
        {:else}
          <div class="arbol" role="tree" aria-label="Archivos de la versión" aria-multiselectable="true">{@render nivel("/")}</div>
        {/if}
        <div class="pie-paso">
          <div class="elegidas">
            {#if elegidas.size}
              <strong class="num">{plural(elegidas.size, "elemento", "elementos")}{totalElegido ? ` · ${bytes(totalElegido)}` : ""}</strong>
              <span class="chips-elegidas">
                {#each [...elegidas.entries()].slice(0, 3) as [ruta, e] (ruta)}
                  <span class="chip-elegida">{#if e.tipo === "dir"}<Folder size={12} />{:else}<File size={12} />{/if}<span class="ce-nombre">{e.nombre}</span><button type="button" aria-label="Quitar {e.nombre}" onclick={() => marcar(ruta, e, false)}><X size={12} /></button></span>
                {/each}
                {#if elegidas.size > 3}<span class="faint pequeno">y {elegidas.size - 3} más</span>{/if}
              </span>
            {:else}
              <span class="faint">Marca los archivos o carpetas que quieras recuperar.</span>
            {/if}
          </div>
          <button class="btn btn-primary" disabled={!elegidas.size} onclick={() => (paso = "destino")}>Seguir<ArrowRight size={15} /></button>
        </div>
      </section>
    {:else if paso === "destino" && equipo && version}
      <section class="card p estrecha form" aria-labelledby="t-destino">
        <div class="resumen-rest">
          <span class="tile-ic"><History size={16} /></span>
          <p>
            <strong>{plural(elegidas.size, "elemento", "elementos")}{totalElegido ? ` · ${bytes(totalElegido)}` : ""}</strong> de «{repo?.nombre}», versión del {dia(version.cuando)}, {hora(version.cuando)}.
          </p>
        </div>
        <h2 class="section-title" id="t-destino">¿Dónde los quieres?</h2>
        <div class="opciones" role="radiogroup" aria-labelledby="t-destino">
          <label class="opcion" class:on={destino === "junto"}>
            <input type="radio" bind:group={destino} value="junto" />
            <FolderPlus size={18} />
            <span>
              <strong>En {equipo.nombre}, junto al original <span class="badge badge-sm tone-ok">Recomendado</span></strong>
              <span class="faint">Al lado de cada elemento se crea una carpeta «{carpetaRestaurado}» con lo restaurado dentro. No toca nada de lo que hay: compara y quédate con lo que quieras.</span>
              {#if ejemploJunto}<span class="ejemplo">Por ejemplo: <code>{ejemploJunto}</code></span>{/if}
            </span>
          </label>
          <label class="opcion" class:on={destino === "original"}>
            <input type="radio" bind:group={destino} value="original" />
            <HardDrive size={18} />
            <span><strong>En {equipo.nombre}, en su sitio</strong><span class="faint">Vuelve a donde estaba. Si ya existe, se deja como está salvo que marques «Reemplazar».</span></span>
          </label>
          <label class="opcion" class:on={destino === "descargar"}>
            <input type="radio" bind:group={destino} value="descargar" />
            <Download size={18} />
            <span><strong>Descargar en este navegador</strong><span class="faint">Hasta 500 MB. Llega cifrado por el servidor, que no puede leerlo. <Ayuda id="relevo" /></span></span>
          </label>
        </div>
        {#if destino === "original"}
          <label class="switch-row"><input type="checkbox" bind:checked={reemplazar} /><span><strong>Reemplazar</strong> los archivos que ya existan<span class="faint">Lo que haya ahora se perderá.</span></span></label>
        {/if}
        {#if destino === "descargar" && totalElegido > MAX_RELEVO}
          <div class="notice notice-warn"><TriangleAlert size={16} /><p>Son más de 500 MB: mejor restaurar en el equipo.</p></div>
        {/if}
        <p class="faint pequeno">¿Lo necesitas en otro equipo? <button class="link" onclick={() => (enOtro = { equipo: equipoId, repo: repoId })}>Restaurar en otro equipo</button></p>
        {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
        <div class="fin">
          {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
          <button class="btn btn-ghost" onclick={() => (paso = "archivos")} disabled={ocupado}><ArrowLeft size={15} />Atrás</button>
          <button class="btn {reemplazar && destino === 'original' ? 'btn-danger' : 'btn-primary'}" disabled={ocupado || (destino === "descargar" && totalElegido > MAX_RELEVO)} onclick={restaurar}>
            {#if destino === "descargar"}<Download size={15} />Descargar{:else}<History size={15} />Restaurar{/if} {plural(elegidas.size, "elemento", "elementos")}
          </button>
        </div>
      </section>
    {:else if paso === "progreso"}
      <section class="card p estrecha final" aria-live="polite">
        {#if destino === "descargar"}
          {#if terminado}
            <Ilustracion nombre="restaurado" ancho={150} />
            <h2 class="section-title">Descarga terminada</h2>
            <p class="faint">Está en la carpeta de descargas de este navegador. Ya se ha borrado del servidor.</p>
          {:else}
            <span class="gran-ic info"><Download size={26} /></span>
            <h2 class="section-title">{progreso?.fase === "bajando" ? "Descargando y descifrando…" : progreso?.fase === "subiendo" ? `${equipo?.nombre} está preparando los archivos…` : "Esperando al equipo…"}</h2>
            <div class="progress live barra"><div style:width="{progreso?.fase === 'bajando' ? Math.round((progreso.bajados / Math.max(progreso.bytes, 1)) * 100) : progreso?.trozos ? 35 : 8}%"></div></div>
            <p class="faint num">{progreso ? `${bytes(progreso.fase === "bajando" ? progreso.bajados : progreso.bytes)}${progreso.fase === "bajando" ? ` de ${bytes(progreso.bytes)}` : ""}` : "No cierres esta pestaña hasta que termine."}</p>
          {/if}
        {:else if orden}
          {#if terminado && orden.estado === "hecha"}
            <Ilustracion nombre="restaurado" ancho={150} />
            <h2 class="section-title">Listo: {plural(elegidas.size, "elemento restaurado", "elementos restaurados")} en {equipo?.nombre}</h2>
          {:else if terminado}
            <span class="gran-ic mal"><TriangleAlert size={28} /></span>
            <h2 class="section-title">No se pudo restaurar</h2>
          {:else}
            <span class="gran-ic info"><LoaderCircle size={28} class="spin" /></span>
            <h2 class="section-title">Restaurando en {equipo?.nombre}…</h2>
          {/if}
          <ol class="etapas" aria-label="Cómo va">
            {#each ETAPAS as et, j (et.id)}
              {@const hecha = etapa > j || (terminado && orden.estado === "hecha")}
              {@const enCurso = etapa === j && !terminado}
              <li class:hecha class:ahora={enCurso}>
                <span class="e-ic">{#if hecha}<Check size={13} strokeWidth={3} />{:else if enCurso}<LoaderCircle size={13} class="spin" />{/if}</span>{et.texto}
              </li>
            {/each}
          </ol>
          <p>{orden.mensaje ?? (equipo?.conectado ? "Puedes cerrar esta pantalla: el equipo sigue y lo verás en sus órdenes." : `${equipo?.nombre} lo hará en cuanto se conecte.`)}</p>
          {#if orden.firma_agente && equipo}<p class="faint pequeno firma"><ShieldCheck size={13} />{resultadoFirmado(equipo.sign_pub, orden) ? "Respuesta firmada por el equipo." : "La firma de la respuesta no es válida."}</p>{/if}
          {#if terminado && orden.estado === "hecha"}
            <div class="donde-quedo">
              <span class="tile-ic"><FolderOpen size={16} /></span>
              <div>
                {#if destino === "junto" && carpetaJunto}
                  <strong>Abre la carpeta en {equipo?.nombre}</strong>
                  <p>En el Explorador de archivos, pega esta ruta en la barra de direcciones (la hora del nombre es la del equipo, puede variar un minuto):</p>
                  <Copiable texto={carpetaJunto} que="la ruta" />
                {:else if destino === "junto"}
                  <strong>Junto al original</strong>
                  <p>Al lado de cada elemento hay una carpeta «Restaurado …» con la fecha de hoy.</p>
                {:else}
                  <strong>En su sitio de siempre</strong>
                  <p>{reemplazar ? "Los archivos se han reemplazado por los de la versión elegida." : "Lo que ya existía se ha dejado como estaba; lo que faltaba ha vuelto a su carpeta."}</p>
                {/if}
              </div>
            </div>
          {/if}
          {#if terminado && orden.estado !== "hecha" && destino === "original" && /junto/i.test(orden.mensaje ?? "")}
            <!-- v1.10: «en su sitio» solo dentro de las carpetas que copia el equipo; el agente sugiere «junto». -->
            <div class="notice notice-info sugerencia">
              <p>«En su sitio» solo vale para archivos de las carpetas que copia {equipo?.nombre}. Puedes restaurarlos junto al original, sin tocar nada de lo que hay.</p>
              <button class="btn btn-primary btn-sm" disabled={ocupado} onclick={restaurarJunto}>Restaurar junto al original</button>
            </div>
          {/if}
        {/if}
        {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
        <div class="fin centro-fin">
          <button
            class="btn"
            onclick={() => {
              paso = "archivos";
              terminado = false;
              orden = null;
              progreso = null;
              error = "";
            }}>Restaurar más de esta versión</button
          >
          <a class="btn btn-primary" href="/c/{actual.id}/equipos/{equipoId}?tab=ordenes">Ver en el equipo</a>
        </div>
      </section>
    {/if}
  {/if}
</div>

{#snippet nivel(ruta: string)}
  {@const h = hijos[ruta]}
  {#if h === "cargando"}
    <div class="cargando"><LoaderCircle size={14} class="spin" />Abriendo la carpeta…</div>
  {:else if h && !h.length}
    <div class="cargando">Carpeta vacía</div>
  {:else if h}
    <ul role="group">
      {#each h as e (e.nombre)}
        {@const r = unir(ruta, e.nombre)}
        <li role="treeitem" aria-selected={elegidas.has(r)} aria-expanded={e.tipo === "dir" ? !!abiertas[r] : undefined}>
          <div class="nodo" class:marcado={elegidas.has(r)}>
            {#if e.tipo === "dir"}
              <button
                type="button"
                class="icon-btn flecha"
                class:abierta={abiertas[r]}
                aria-label={abiertas[r] ? `Plegar ${e.nombre}` : `Abrir ${e.nombre}`}
                onclick={() => {
                  abiertas[r] = !abiertas[r];
                  if (abiertas[r]) void cargar(r);
                }}><ChevronRight size={14} /></button
              >
            {:else}<span class="hueco"></span>{/if}
            <input type="checkbox" checked={elegidas.has(r)} aria-label="Restaurar {r}" onchange={(ev) => marcar(r, e, ev.currentTarget.checked)} />
            {#if e.tipo === "dir"}<Folder size={15} class="ic-dir" />{:else}<File size={15} />{/if}
            <span class="nombre">{ruta === "/" && e.tipo === "dir" && /^[A-Za-z]$/.test(e.nombre) && esWindows ? `Disco ${e.nombre.toUpperCase()}:` : e.nombre}</span>
            {#if e.tipo === "archivo"}
              {#if e.modificado}<span class="faint pequeno meta fecha num">{fechaCorta(e.modificado)}</span>{/if}
              <span class="faint pequeno meta num">{bytes(e.bytes)}</span>
            {/if}
          </div>
          {#if abiertas[r]}{@render nivel(r)}{/if}
        </li>
      {/each}
    </ul>
  {/if}
{/snippet}

{#if enOtro !== undefined && actual.cliente}
  <RestaurarEnOtro cliente={actual.cliente} equipos={actual.equipos} origenInicial={enOtro ?? undefined} onclose={() => (enOtro = undefined)} />
{/if}

<style>
  .paso-card {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    animation: rise var(--dur-slow) var(--ease-out) both;
  }
  .paso-cab h2 {
    margin: 0;
    font-size: var(--fs-h2);
    line-height: var(--lh-h2);
    font-weight: 600;
  }
  .paso-cab p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .equipos-r {
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 280px), 1fr));
  }
  .elegible {
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .elegible:hover,
  .elegible:focus-visible {
    border-color: var(--border-strong);
    box-shadow: var(--shadow-sm);
  }
  .elegible-fila {
    gap: var(--sp-3);
  }
  .elegible-fila :global(> svg) {
    flex: none;
    color: var(--text-3);
  }
  .bloque {
    display: block;
  }
  .sugerencia {
    flex-direction: column;
    align-items: center;
    gap: var(--sp-3);
    text-align: center;
  }
  .ejemplo {
    display: block;
    margin-top: 4px;
    font-size: var(--fs-xs);
    color: var(--text-2);
    word-break: break-all;
  }
  .estrecha {
    width: 100%;
    max-width: 660px;
    animation: rise var(--dur-slow) var(--ease-out) both;
  }
  .fin {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    align-items: center;
    gap: 8px;
  }
  .fin.izq {
    justify-content: flex-start;
  }
  .centro-fin {
    justify-content: center;
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }

  /* Archivos. */
  .archivos {
    animation: rise var(--dur-slow) var(--ease-out) both;
  }
  .cab-paso {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: var(--sp-4) var(--sp-5);
    border-bottom: 1px solid var(--border);
  }
  .cab-paso > :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .cab-texto {
    display: flex;
    flex: 1;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 10px;
    min-width: 0;
  }
  .cab-texto .faint {
    font-size: var(--fs-sm);
  }
  .buscar {
    position: relative;
    display: flex;
    gap: 8px;
    padding: var(--sp-3) var(--sp-5) 0;
  }
  .buscar > :global(svg) {
    position: absolute;
    top: 22px;
    left: 31px;
    color: var(--text-3);
  }
  .buscar .input {
    flex: 1;
    padding-left: 34px;
  }
  .buscar .btn {
    align-self: center;
  }
  .arbol,
  .resultados {
    max-height: 50vh;
    overflow: auto;
    padding: var(--sp-3) var(--sp-4);
  }
  .res-nombre {
    display: flex;
    flex-direction: column;
    line-height: 1.25;
    padding: 3px 0;
  }
  .res-nombre span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .res-cab {
    margin: 0 0 6px 6px;
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
    min-height: 32px;
    padding-right: 6px;
    border-radius: var(--radius-sm);
    transition: background var(--dur-fast) var(--ease);
  }
  .nodo:hover {
    background: var(--surface-2);
  }
  .nodo.marcado {
    background: var(--accent-soft);
  }
  .nodo :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .nodo :global(.ic-dir) {
    color: var(--accent-text);
  }
  .hueco {
    width: 28px;
    flex: none;
  }
  .flecha :global(svg) {
    transition: transform var(--dur) var(--ease);
  }
  .flecha.abierta :global(svg) {
    transform: rotate(90deg);
  }
  .nombre {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    flex: none;
    min-width: 56px;
    text-align: right;
  }
  .meta.fecha {
    min-width: 96px;
  }
  .pequeno {
    font-size: var(--fs-xs);
  }
  .cargando {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 28px;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .pie-paso {
    position: sticky;
    bottom: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-5);
    font-size: var(--fs-sm);
    background: var(--surface);
    border-top: 1px solid var(--border);
  }
  .elegidas {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    min-width: 0;
  }
  .chips-elegidas {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    min-width: 0;
  }
  .chip-elegida {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    max-width: 200px;
    height: 24px;
    padding: 0 2px 0 8px;
    font-size: var(--fs-xs);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
  }
  .chip-elegida :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .ce-nombre {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chip-elegida button {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    padding: 0;
    color: var(--text-3);
    background: none;
    border: none;
    border-radius: 999px;
    cursor: pointer;
  }
  .chip-elegida button:hover {
    color: var(--text-1);
    background: var(--surface-3);
  }

  /* Dónde. */
  .resumen-rest {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3);
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .resumen-rest p {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .resumen-rest strong {
    color: var(--text-1);
    font-weight: 600;
  }
  .opciones {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .opcion {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    transition:
      border-color var(--dur-fast) var(--ease),
      box-shadow var(--dur-fast) var(--ease);
  }
  .opcion:hover {
    border-color: var(--border-strong);
  }
  .opcion.on {
    border-color: var(--accent);
    box-shadow: var(--focus);
  }
  .opcion :global(svg) {
    flex: none;
    margin-top: 1px;
    color: var(--text-2);
  }
  .opcion > span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .opcion strong .badge {
    margin-left: 4px;
    vertical-align: 1px;
  }
  .opcion .faint,
  .switch-row .faint {
    display: block;
    font-size: var(--fs-sm);
  }

  /* Progreso y final. */
  .final {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-3);
    text-align: center;
  }
  .final p {
    margin: 0;
  }
  .gran-ic {
    display: grid;
    place-items: center;
    width: 56px;
    height: 56px;
    border-radius: 999px;
  }
  .gran-ic.info {
    color: var(--info);
    background: var(--info-soft);
  }
  .gran-ic.mal {
    color: var(--bad);
    background: var(--bad-soft);
  }
  .etapas {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 6px 16px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-sm);
    color: var(--text-3);
  }
  .etapas li {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .e-ic {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border: 1.5px solid var(--border-strong);
    border-radius: 999px;
  }
  .etapas .hecha {
    color: var(--text-2);
  }
  .etapas .hecha .e-ic {
    color: var(--accent-contrast);
    background: var(--accent);
    border-color: var(--accent);
  }
  .etapas .ahora {
    color: var(--text-1);
    font-weight: 550;
  }
  .etapas .ahora .e-ic {
    color: var(--info);
    border-color: transparent;
  }
  .firma {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .donde-quedo {
    display: flex;
    gap: var(--sp-3);
    width: 100%;
    padding: var(--sp-4);
    text-align: left;
    background: var(--surface-2);
    border-radius: var(--radius-lg);
  }
  .donde-quedo > div {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .donde-quedo p {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .donde-quedo :global(code) {
    word-break: break-all;
  }
  .barra {
    width: 100%;
    max-width: 380px;
  }
  @media (max-width: 640px) {
    .meta.fecha {
      display: none;
    }
    .pie-paso {
      flex-wrap: wrap;
    }
    .pie-paso .btn {
      width: 100%;
    }
  }
</style>
