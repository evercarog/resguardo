<script lang="ts">
  // Plan 0.8.1, C4 (plan-mejoras 15d; docs/infraestructura.md): infraestructura
  // para cualquier entorno. Dónde están de verdad los datos y las copias: los
  // servidores físicos, sus discos y su hipervisor, qué equipos (máquinas
  // virtuales, contenedores o el propio servidor) y qué destinos viven en cada
  // uno. **La interfaz propone** con lo que ven los equipos («El almacén guarda
  // en ZFS «tank» (1,1 TB, 65 % usado). ¿En qué servidor está?») **y la persona
  // confirma**, con pocos campos. Todo opcional: sin nada anotado, todo funciona
  // como antes. Sin conectar ningún hipervisor (15e, aparcada).
  //
  // Plan 0.8.2: en Ajustes (A1; `?equipo=<id>` o `?destino=<clave>` preseleccionan
  // su fila), los discos físicos uno a uno y cada conjunto con los suyos (A2),
  // cada volumen de un equipo en su disco o conjunto (A3) y, como dato común del
  // cliente, igual en todas sus consolas (A5).
  import { tick, untrack } from "svelte";
  import { page } from "$app/state";
  import { Check, ChevronDown, ChevronRight, Cloud, HardDrive, Layers, Lightbulb, Monitor, Pencil, Plus, Server, ServerCog, Share2, Trash2, TriangleAlert, X } from "@lucide/svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import EditorServidor from "$lib/componentes/infra/EditorServidor.svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { actual, app, puede } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { guardarInfra } from "$lib/infra.svelte";
  import {
    destinoAnotable,
    detalleDisco,
    formaConjunto,
    HIPERVISORES,
    MAX_BYTES_INFRA,
    miembrosDe,
    propuestas,
    TIPOS_CONJUNTO,
    type Infraestructura,
    type Propuesta,
    type ServidorFisico,
    type Sitio,
    type VolumenSitio,
  } from "$lib/infra";
  import { datosBasicos } from "$lib/datosBasicos";
  import { destinosParaPasos, detalleDestino } from "$lib/cadenas";
  import { catalogoDe, cargarCatalogo } from "$lib/catalogoDestinos.svelte";
  import { textoTipo, tipoEquipo } from "$lib/tipoEquipo";
  import { sedePorId } from "$lib/sedes";
  import { bytes, plural } from "$lib/formato";
  import { guardar as guardarTexto, leerTexto } from "$lib/recordar";
  import { hrefDestino } from "$lib/fichaDestino";

  const c = $derived(actual.id);
  $effect(() => {
    const cc = c;
    if (cc) untrack(() => void cargarCatalogo(cc));
  });
  const infra = $derived(actual.infra);
  const admin = $derived(!!actual.cliente && puede.administrar(actual.cliente.rol));
  const equipos = $derived(actual.equipos.filter((e) => e.modo !== "trasladado" && e.confirmado));
  const destinos = $derived(destinosParaPasos(actual.equipos, catalogoDe(c)));

  // Lo que la persona dijo «Ahora no» se recuerda por persona (en este navegador).
  const claveDescartes = $derived(`infra.descartes.${app.cuenta?.id ?? ""}.${c}`);
  let descartes = $state<string[]>([]);
  $effect(() => {
    const k = claveDescartes;
    untrack(() => {
      try {
        descartes = JSON.parse(leerTexto(k) || "[]");
      } catch {
        descartes = [];
      }
    });
  });
  function descartar(id: string) {
    descartes = [...descartes, id].slice(-300);
    guardarTexto(claveDescartes, JSON.stringify(descartes));
  }
  const sugerencias = $derived(propuestas(equipos, infra).filter((p) => !descartes.includes(p.id)));

  // A1: un equipo o un destino que llega preseleccionado (desde «Anotar» en su ficha).
  const preEquipo = $derived(page.url.searchParams.get("equipo") ?? "");
  const preDestino = $derived(page.url.searchParams.get("destino") ?? "");
  let volumenesAbiertos = $state<Set<string>>(new Set());
  $effect(() => {
    const [e, d, listo] = [preEquipo, preDestino, actual.cargado && infra.servidores.length > 0];
    if (!listo || (!e && !d)) return;
    untrack(() => {
      if (e) volumenesAbiertos = new Set([...volumenesAbiertos, e]);
      void tick().then(() => document.getElementById(e ? `fila-e-${e}` : `fila-d-${d}`)?.scrollIntoView({ block: "center", behavior: "smooth" }));
    });
  });

  let guardando = $state(false);
  async function guardar(nueva: Infraestructura, hecho: string) {
    guardando = true;
    try {
      await guardarInfra(c, nueva);
      avisar(hecho);
    } catch (e) {
      fallo(e);
      throw e;
    } finally {
      guardando = false;
    }
  }

  // --- Servidores ---
  let editando = $state<{ s: ServidorFisico; nuevo: boolean; asignar?: string } | null>(null);
  const vacio = (): ServidorFisico => ({ id: "", nombre: "", marca: null, modelo: null, sede: null, hipervisor: null, notas: null, discos: [], conjuntos: [] });
  /** Un sitio cuyo conjunto o disco ya no existe se queda solo con el servidor (y sus volúmenes, con lo que siga existiendo). */
  function ajustarSitio(s: ServidorFisico, v: Sitio): Sitio {
    if (v.servidor !== s.id) return v;
    const okC = (x?: string | null) => !!x && s.conjuntos.some((y) => y.id === x);
    const okD = (x?: string | null) => !!x && s.discos.some((y) => y.id === x);
    const volumenes = (v.volumenes ?? []).filter((w) => okC(w.conjunto) || okD(w.disco));
    return { servidor: s.id, ...(okC(v.conjunto) ? { conjunto: v.conjunto } : okD(v.disco) ? { disco: v.disco } : {}), ...(volumenes.length ? { volumenes } : {}) };
  }
  async function guardarServidor(s: ServidorFisico, asignar?: string) {
    const otros = infra.servidores.filter((x) => x.id !== s.id);
    const equiposN = { ...infra.equipos };
    if (asignar) equiposN[asignar] = { servidor: s.id };
    const ajustar = (m: Infraestructura["equipos"]) => Object.fromEntries(Object.entries(m).map(([k, v]) => [k, ajustarSitio(s, v)]));
    const pos = infra.servidores.findIndex((x) => x.id === s.id);
    const servidores = pos >= 0 ? infra.servidores.map((x) => (x.id === s.id ? s : x)) : [...otros, s];
    await guardar({ ...infra, servidores, equipos: ajustar(equiposN), destinos: ajustar(infra.destinos) }, `Servidor «${s.nombre}» guardado.`);
  }
  let quitando = $state<ServidorFisico | null>(null);
  async function quitarServidor(s: ServidorFisico) {
    const sin = (m: Infraestructura["equipos"]) => Object.fromEntries(Object.entries(m).filter(([, v]) => v.servidor !== s.id));
    await guardar({ ...infra, servidores: infra.servidores.filter((x) => x.id !== s.id), equipos: sin(infra.equipos), destinos: sin(infra.destinos) }, `Servidor «${s.nombre}» quitado.`).catch(() => {});
    quitando = null;
  }

  // --- Asignar un equipo o un destino a un servidor (y conjunto o disco) ---
  /** `donde`: «c:<conjunto>», «d:<disco>» o «» (sin decir). */
  type Asignacion = { servidor: string; donde: string };
  const dondeDe = (s?: Pick<Sitio, "conjunto" | "disco"> | null) => (s?.conjunto ? `c:${s.conjunto}` : s?.disco ? `d:${s.disco}` : "");
  const deDonde = (v: string) => (v.startsWith("c:") ? { conjunto: v.slice(2) } : v.startsWith("d:") ? { disco: v.slice(2) } : {});
  let eleccion = $state<Record<string, Asignacion>>({});
  const elegido = (k: string, actualS?: Sitio): Asignacion => eleccion[k] ?? { servidor: actualS?.servidor ?? "", donde: dondeDe(actualS) };
  function elegir(k: string, x: Partial<Asignacion>, actualS?: Sitio) {
    const prev = elegido(k, actualS);
    const sig = { ...prev, ...x };
    if (x.servidor !== undefined && x.servidor !== prev.servidor) sig.donde = "";
    eleccion = { ...eleccion, [k]: sig };
  }
  async function asignar(clase: "equipos" | "destinos", k: string, a: Asignacion, nombre: string) {
    const m = { ...infra[clase] };
    const previo = m[k];
    if (!a.servidor) delete m[k];
    else m[k] = { servidor: a.servidor, ...deDonde(a.donde), ...(clase === "equipos" && previo?.servidor === a.servidor && previo.volumenes?.length ? { volumenes: previo.volumenes } : {}) };
    await guardar({ ...infra, [clase]: m }, a.servidor ? `${nombre}: anotado en «${infra.servidores.find((s) => s.id === a.servidor)?.nombre}».` : `${nombre}: sin servidor.`).catch(() => {});
    const { [`${clase}:${k}`]: _q, ...resto } = eleccion;
    eleccion = resto;
  }
  /** A3: el disco o conjunto de un volumen del equipo (`""`: como el equipo). */
  async function asignarVolumen(e: { id: string; nombre: string }, punto: string, donde: string) {
    const s = infra.equipos[e.id];
    if (!s) return;
    const resto = (s.volumenes ?? []).filter((v) => v.punto !== punto);
    const volumenes: VolumenSitio[] = donde ? [...resto, { punto, ...deDonde(donde) }] : resto;
    await guardar({ ...infra, equipos: { ...infra.equipos, [e.id]: { ...s, volumenes } } }, `${e.nombre} · ${punto}: ${donde ? "anotado" : "como el equipo"}.`).catch(() => {});
  }
  function alternarVolumenes(id: string) {
    const s = new Set(volumenesAbiertos);
    if (s.has(id)) s.delete(id);
    else s.add(id);
    volumenesAbiertos = s;
  }

  /** Aceptar una propuesta «es un servidor físico»: el servidor con lo que vio el agente, para revisarlo. */
  function aceptarServidor(p: Propuesta) {
    if (p.servidor) editando = { s: p.servidor, nuevo: true, asignar: p.equipo.id };
  }
  /** A2: añadir al servidor los discos que ve su equipo (para revisarlos y escribir el número de serie). */
  function aceptarDiscos(p: Propuesta) {
    const s = infra.servidores.find((x) => x.id === p.enServidor);
    if (s && p.discosNuevos) editando = { s: { ...s, discos: [...s.discos, ...p.discosNuevos] }, nuevo: false };
  }

  const nombreHiper = (id: string | null | undefined) => HIPERVISORES.find((h) => h.id === id)?.nombre ?? null;
  const enServidor = (id: string) => ({
    equipos: equipos.filter((e) => infra.equipos[e.id]?.servidor === id),
    destinos: destinos.filter((d) => infra.destinos[d.clave]?.servidor === id),
  });
  const textoDonde = (s: ServidorFisico, x?: Pick<Sitio, "conjunto" | "disco"> | null) => (x?.conjunto ? s.conjuntos.find((y) => y.id === x.conjunto)?.nombre : x?.disco ? s.discos.find((y) => y.id === x.disco)?.nombre : null) ?? null;
  const minusc = (t: string) => t.charAt(0).toLowerCase() + t.slice(1);
  const ICONO_ESTADO = { bien: Check, degradado: TriangleAlert, reconstruyendo: Layers, sin_revisar: Layers } as const;
  const destinosAnotables = $derived(destinos.filter(destinoAnotable));
  const ocupa = $derived(infra.bytes_comunes ?? 0);
</script>

{#snippet opcionesDonde(srv: ServidorFisico, vacioTexto: string)}
  <option value="">{vacioTexto}</option>
  {#if srv.conjuntos.length}
    <optgroup label="Conjuntos">
      {#each srv.conjuntos as x (x.id)}<option value="c:{x.id}">{x.nombre} · {formaConjunto(x)}</option>{/each}
    </optgroup>
  {/if}
  {#if srv.discos.length}
    <optgroup label="Discos">
      {#each srv.discos as x (x.id)}<option value="d:{x.id}">{x.nombre}{detalleDisco(x) ? ` · ${detalleDisco(x)}` : ""}</option>{/each}
    </optgroup>
  {/if}
{/snippet}

<svelte:head><title>Infraestructura · {actual.cliente?.nombre ?? "Resguardo"}</title></svelte:head>

<div class="page">
  <CabeceraPagina
    titulo="Infraestructura"
    icono={ServerCog}
    migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${c}` }, { texto: "Ajustes" }, { texto: "Infraestructura" }]}
    resumen="Dónde están de verdad tus datos: los servidores físicos, sus discos y qué vive en cada uno. Es opcional: sirve para la regla 3-2-1-1-0 (los soportes reales y el aviso «servidor físico compartido»). Resguardo no gestiona los discos ni el hipervisor."
  >
    {#snippet acciones()}
      {#if admin}<button class="btn btn-primary" disabled={guardando} onclick={() => (editando = { s: vacio(), nuevo: true })}><Plus size={16} />Añadir un servidor</button>{/if}
    {/snippet}
  </CabeceraPagina>

  {#if !actual.cargado}
    <Esqueleto forma="filas" n={4} etiqueta="Cargando la infraestructura…" />
  {:else}
    {#if admin && sugerencias.length}
      <section class="card p" aria-labelledby="t-propuestas">
        <h2 class="section-title" id="t-propuestas"><Lightbulb size={16} />Lo que ven tus equipos <span class="faint cuenta">· {sugerencias.length}</span></h2>
        <p class="faint intro">Confírmalo con un clic. Desde dentro de una máquina virtual o un contenedor no se ven los discos del anfitrión: eso lo anotas tú.</p>
        <ul class="propuestas">
          {#each sugerencias as p (p.id)}
            {@const k = p.clase === "zona" ? `destinos:${p.claveDestino}` : `equipos:${p.equipo.id}`}
            {@const a = elegido(k)}
            {@const srv = infra.servidores.find((s) => s.id === a.servidor)}
            <li class="propuesta">
              <span class="p-ic" aria-hidden="true">{#if p.clase === "zona" || p.clase === "discos"}<HardDrive size={16} />{:else if p.clase === "servidor"}<Server size={16} />{:else}<Monitor size={16} />{/if}</span>
              <p class="p-frase">{p.frase}</p>
              <div class="p-acciones">
                {#if p.clase === "servidor"}
                  <button class="btn btn-sm btn-primary" disabled={guardando} onclick={() => aceptarServidor(p)}><Check size={14} />Anotarlo como servidor</button>
                  {#if infra.servidores.length}
                    <select class="input input-sm" aria-label="O es parte de" value={a.servidor} onchange={(ev) => elegir(k, { servidor: ev.currentTarget.value })}>
                      <option value="">O está en…</option>
                      {#each infra.servidores as s (s.id)}<option value={s.id}>{s.nombre}</option>{/each}
                    </select>
                    {#if a.servidor}<button class="btn btn-sm" disabled={guardando} onclick={() => asignar("equipos", p.equipo.id, a, p.equipo.nombre)}>Guardar</button>{/if}
                  {/if}
                {:else if p.clase === "discos"}
                  <button class="btn btn-sm btn-primary" disabled={guardando} onclick={() => aceptarDiscos(p)}><Plus size={14} />Añadirlos y revisarlos</button>
                {:else if infra.servidores.length}
                  <select class="input input-sm" aria-label="Servidor físico" value={a.servidor} onchange={(ev) => elegir(k, { servidor: ev.currentTarget.value })}>
                    <option value="">Elige el servidor…</option>
                    {#each infra.servidores as s (s.id)}<option value={s.id}>{s.nombre}</option>{/each}
                  </select>
                  {#if srv && (srv.conjuntos.length || srv.discos.length)}
                    <select class="input input-sm" aria-label="Conjunto o disco" value={a.donde} onchange={(ev) => elegir(k, { donde: ev.currentTarget.value })}>
                      {@render opcionesDonde(srv, "¿En qué discos? (opcional)")}
                    </select>
                  {/if}
                  <button class="btn btn-sm btn-primary" disabled={guardando || !a.servidor} onclick={() => (p.clase === "zona" ? asignar("destinos", p.claveDestino!, a, p.equipo.nombre) : asignar("equipos", p.equipo.id, a, p.equipo.nombre))}><Check size={14} />Guardar</button>
                {:else}
                  <button class="btn btn-sm" disabled={guardando} onclick={() => (editando = { s: vacio(), nuevo: true })}><Plus size={14} />Anota primero el servidor</button>
                {/if}
                <button class="btn btn-sm btn-ghost" onclick={() => descartar(p.id)}><X size={14} />Ahora no</button>
              </div>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    <section aria-labelledby="t-servidores">
      <h2 class="section-title" id="t-servidores">Servidores físicos {#if infra.servidores.length}<span class="faint cuenta">· {infra.servidores.length}</span>{/if}</h2>
      {#if !infra.servidores.length}
        <div class="card">
          <Vacio icono={ServerCog} titulo="Nada anotado todavía" texto="Sin infraestructura, todo funciona igual. Anótala si quieres saber dónde están de verdad las copias: p. ej. una máquina virtual y su almacén en el mismo servidor físico.">
            {#if admin}<button class="btn btn-primary" onclick={() => (editando = { s: vacio(), nuevo: true })}><Plus size={16} />Añadir un servidor</button>{/if}
          </Vacio>
        </div>
      {:else}
        <div class="servidores">
          {#each infra.servidores as s (s.id)}
            {@const dentro = enServidor(s.id)}
            {@const sede = sedePorId(actual.organizacion, s.sede)}
            <article class="card p servidor" aria-labelledby="srv-{s.id}">
              <header class="srv-cab">
                <span class="tile-ic" aria-hidden="true"><Server size={16} /></span>
                <div class="srv-t">
                  <h3 id="srv-{s.id}">{s.nombre}</h3>
                  <p class="faint">{[nombreHiper(s.hipervisor) ?? "Sin hipervisor", [s.marca, s.modelo].filter(Boolean).join(" "), sede?.nombre].filter(Boolean).join(" · ")}</p>
                </div>
                {#if admin}
                  <div class="srv-acc">
                    <button class="icon-btn" aria-label="Cambiar {s.nombre}" disabled={guardando} onclick={() => (editando = { s, nuevo: false })}><Pencil size={15} /></button>
                    <button class="icon-btn" aria-label="Quitar {s.nombre}" disabled={guardando} onclick={() => (quitando = s)}><Trash2 size={15} /></button>
                  </div>
                {/if}
              </header>
              {#if s.notas}<p class="notas">{s.notas}</p>{/if}
              {#if s.discos.length}
                <h4 class="etiqueta-seccion">Discos físicos</h4>
                <ul class="conjuntos">
                  {#each s.discos as d (d.id)}
                    <li>
                      <HardDrive size={14} aria-hidden="true" />
                      <span class="cj-n">{d.nombre}</span>
                      {#if detalleDisco(d)}<span class="faint">{detalleDisco(d)}</span>{/if}
                      {#if d.serie}<span class="faint mono serie">S/N {d.serie}</span>{/if}
                      {#if d.estado === "averiado"}<span class="badge tone-bad"><TriangleAlert size={12} />Averiado</span>{:else if d.estado === "aviso"}<span class="badge tone-warn"><TriangleAlert size={12} />Con avisos</span>{:else if d.estado === "bien"}<span class="badge tone-ok"><Check size={12} />Bien</span>{/if}
                    </li>
                  {/each}
                </ul>
              {/if}
              {#if s.conjuntos.length}
                <h4 class="etiqueta-seccion">Conjuntos de discos</h4>
                <ul class="conjuntos">
                  {#each s.conjuntos as x (x.id)}
                    {@const Ic = x.estado ? ICONO_ESTADO[x.estado] : Layers}
                    <li>
                      <Layers size={14} aria-hidden="true" />
                      <span class="cj-n">{x.nombre}</span>
                      <span class="faint">{TIPOS_CONJUNTO.find((t) => t.id === x.tipo)?.nombre ?? x.tipo}{x.miembros?.length ? `${x.tipo === "disco" ? ":" : " de"} ${miembrosDe(s, x)}` : x.discos ? ` · ${plural(x.discos, "disco", "discos")}` : ""}{x.capacidad ? ` · ${bytes(x.capacidad)}` : ""}</span>
                      {#if x.estado === "degradado"}<span class="badge tone-warn"><Ic size={12} />Degradado</span>{:else if x.estado === "reconstruyendo"}<span class="badge tone-info"><Ic size={12} />Reconstruyendo</span>{:else if x.estado === "bien"}<span class="badge tone-ok"><Ic size={12} />Bien</span>{/if}
                    </li>
                  {/each}
                </ul>
              {/if}
              <h4 class="etiqueta-seccion">Lo que vive aquí</h4>
              {#if dentro.equipos.length || dentro.destinos.length}
                <ul class="dentro">
                  {#each dentro.equipos as e (e.id)}
                    {@const donde = textoDonde(s, infra.equipos[e.id])}
                    {@const vols = infra.equipos[e.id]?.volumenes ?? []}
                    <li><Monitor size={14} aria-hidden="true" /><a class="link" href="/c/{c}/equipos/{e.id}">{e.nombre}</a><span class="faint">{minusc(textoTipo(tipoEquipo(e)))}{donde ? ` · en ${donde}` : ""}{vols.length ? ` · ${vols.map((v) => `${v.punto} en ${textoDonde(s, v)}`).join(", ")}` : ""}</span></li>
                  {/each}
                  {#each dentro.destinos as d (d.clave)}
                    {@const donde = textoDonde(s, infra.destinos[d.clave])}
                    <li><HardDrive size={14} aria-hidden="true" /><a class="link" href={hrefDestino(c, d.clave)}>{d.nombre}</a><span class="faint">{detalleDestino(d)}{donde ? ` · en ${donde}` : ""}</span></li>
                  {/each}
                </ul>
              {:else}
                <p class="faint">Nada asignado todavía: elige el servidor de cada equipo o destino abajo.</p>
              {/if}
            </article>
          {/each}
        </div>
      {/if}
    </section>

    {#if infra.servidores.length}
      <section class="card lista" id="equipos" aria-labelledby="t-equipos-srv">
        <h2 class="section-title fila-t" id="t-equipos-srv">Equipos</h2>
        {#each equipos as e (e.id)}
          {@const k = `equipos:${e.id}`}
          {@const ya = infra.equipos[e.id]}
          {@const a = elegido(k, ya)}
          {@const srv = infra.servidores.find((s) => s.id === a.servidor)}
          {@const cambia = a.servidor !== (ya?.servidor ?? "") || a.donde !== dondeDe(ya)}
          {@const vols = datosBasicos(e)?.discos ?? []}
          {@const srvYa = infra.servidores.find((s) => s.id === ya?.servidor)}
          {@const abierto = volumenesAbiertos.has(e.id)}
          <div class="fila asignar" class:resaltado={preEquipo === e.id} id="fila-e-{e.id}">
            <span class="as-n"><Monitor size={14} aria-hidden="true" /><a class="link" href="/c/{c}/equipos/{e.id}">{e.nombre}</a><span class="faint">{textoTipo(tipoEquipo(e))}</span></span>
            {#if admin}
              <span class="as-c">
                <select class="input input-sm" aria-label="Servidor físico de {e.nombre}" value={a.servidor} onchange={(ev) => elegir(k, { servidor: ev.currentTarget.value }, ya)}>
                  <option value="">Sin anotar</option>
                  {#each infra.servidores as s (s.id)}<option value={s.id}>{s.nombre}</option>{/each}
                </select>
                {#if srv && (srv.conjuntos.length || srv.discos.length)}
                  <select class="input input-sm" aria-label="Discos de {e.nombre}" value={a.donde} onchange={(ev) => elegir(k, { donde: ev.currentTarget.value }, ya)}>
                    {@render opcionesDonde(srv, "Discos sin decir")}
                  </select>
                {/if}
                {#if cambia}<button class="btn btn-sm btn-primary" disabled={guardando} onclick={() => asignar("equipos", e.id, a, e.nombre)}>Guardar</button>{/if}
                {#if !cambia && srvYa && vols.length && (srvYa.conjuntos.length || srvYa.discos.length)}
                  <button class="btn btn-sm btn-ghost" aria-expanded={abierto} onclick={() => alternarVolumenes(e.id)}>{#if abierto}<ChevronDown size={14} />{:else}<ChevronRight size={14} />{/if}Por volumen{#if ya?.volumenes?.length} · {ya.volumenes.length}{/if}</button>
                {/if}
              </span>
            {:else}
              <span class="faint">{srvYa?.nombre ?? "Sin anotar"}{srvYa && textoDonde(srvYa, ya) ? ` · ${textoDonde(srvYa, ya)}` : ""}</span>
            {/if}
          </div>
          {#if admin && abierto && srvYa && !cambia}
            <!-- A3 (15ww): cada volumen del equipo en su disco o conjunto (sin decir: como el equipo). -->
            <ul class="volumenes" aria-label="Volúmenes de {e.nombre}">
              {#each vols as v (v.punto)}
                {@const suyo = ya?.volumenes?.find((x) => x.punto === v.punto)}
                <li>
                  <span class="mono">{v.punto}</span>
                  <span class="faint">{[v.etiqueta ? `«${v.etiqueta}»` : null, v.sistema_archivos, bytes(v.total)].filter(Boolean).join(" · ")}</span>
                  <select class="input input-sm" aria-label="Dónde está {v.punto} de {e.nombre}" value={dondeDe(suyo)} disabled={guardando} onchange={(ev) => asignarVolumen(e, v.punto, ev.currentTarget.value)}>
                    {@render opcionesDonde(srvYa, "Como el equipo")}
                  </select>
                </li>
              {/each}
            </ul>
          {/if}
        {/each}
      </section>

      <section class="card lista" id="destinos" aria-labelledby="t-destinos-srv">
        <h2 class="section-title fila-t" id="t-destinos-srv">Destinos y zonas</h2>
        {#each destinosAnotables as d (d.clave)}
          {@const k = `destinos:${d.clave}`}
          {@const ya = infra.destinos[d.clave]}
          {@const a = elegido(k, ya)}
          {@const srv = infra.servidores.find((s) => s.id === a.servidor)}
          {@const cambia = a.servidor !== (ya?.servidor ?? "") || a.donde !== dondeDe(ya)}
          <div class="fila asignar" class:resaltado={preDestino === d.clave || d.ids.includes(preDestino)} id="fila-d-{d.clave}">
            <span class="as-n"><HardDrive size={14} aria-hidden="true" /><a class="link" href={hrefDestino(c, d.clave)}>{d.nombre}</a><span class="faint">{detalleDestino(d)}</span></span>
            {#if admin}
              <span class="as-c">
                <select class="input input-sm" aria-label="Servidor físico de {d.nombre}" value={a.servidor} onchange={(ev) => elegir(k, { servidor: ev.currentTarget.value }, ya)}>
                  <option value="">{d.zona ? "El de su almacén" : "Sin anotar"}</option>
                  {#each infra.servidores as s (s.id)}<option value={s.id}>{s.nombre}</option>{/each}
                </select>
                {#if srv && (srv.conjuntos.length || srv.discos.length)}
                  <select class="input input-sm" aria-label="Discos de {d.nombre}" value={a.donde} onchange={(ev) => elegir(k, { donde: ev.currentTarget.value }, ya)}>
                    {@render opcionesDonde(srv, "Discos sin decir")}
                  </select>
                {/if}
                {#if cambia}<button class="btn btn-sm btn-primary" disabled={guardando} onclick={() => asignar("destinos", d.clave, a, d.nombre)}>Guardar</button>{/if}
              </span>
            {:else}
              <span class="faint">{infra.servidores.find((s) => s.id === ya?.servidor)?.nombre ?? "Sin anotar"}</span>
            {/if}
          </div>
        {/each}
        <p class="faint pie"><Cloud size={13} aria-hidden="true" />La nube (Backblaze, S3, Dropbox…) no se anota: siempre está fuera.</p>
      </section>
    {/if}
    {#if infra.servidores.length || infra.actualizada}
      <div class="faint pie comun">
        <Share2 size={13} aria-hidden="true" />
        <p>
          {#if infra.comun}Igual en todas las consolas del cliente: viaja por sus equipos (ocupa el {Math.max(1, Math.round((ocupa / MAX_BYTES_INFRA) * 100))} % de lo que cabe en cada uno). Los servidores y sus discos se reparten solos; dónde está cada equipo o destino, con la clave de administración desde el aviso «Lo mismo en todas las consolas».{:else}De momento es de esta consola: se compartirá con las demás cuando sus equipos tengan un agente que la guarde.{/if}
          {#if infra.actualizada} Cambiado {infra.por ? `por ${infra.por} ` : ""}el {new Date(infra.actualizada).toLocaleString("es")}.{/if}
        </p>
      </div>
    {/if}
  {/if}
</div>

{#if editando}
  <EditorServidor
    servidor={editando.s}
    usados={infra.servidores.map((s) => s.id).filter((id) => id !== editando!.s.id)}
    sedes={actual.organizacion.sedes}
    alGuardar={(s) => guardarServidor(s, editando?.asignar)}
    onclose={() => (editando = null)}
  />
{/if}
{#if quitando}
  {@const q = quitando}
  <Modal labelledby="t-quitar-srv" onclose={() => (quitando = null)} width={440} ocupado={guardando}>
    <div class="dlg-title">
      <span class="ticon danger"><Trash2 size={18} /></span>
      <div>
        <h2 id="t-quitar-srv">¿Quitar «{q.nombre}»?</h2>
        <p>Se quitan también sus discos y lo que estaba anotado en él, en todas las consolas del cliente. No toca nada en los equipos ni en las copias.</p>
      </div>
    </div>
    <footer>
      <button type="button" class="btn btn-ghost" disabled={guardando} onclick={() => (quitando = null)}>Cancelar</button>
      <button type="button" class="btn btn-danger" disabled={guardando} onclick={() => quitarServidor(q)}>Quitar</button>
    </footer>
  </Modal>
{/if}

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
  }
  .section-title {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .cuenta {
    font-weight: 400;
  }
  .intro {
    margin: 4px 0 var(--sp-3);
  }
  .propuestas {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .propuesta {
    display: grid;
    grid-template-columns: 32px 1fr;
    gap: 4px var(--sp-3);
    align-items: start;
    padding: var(--sp-3);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .p-ic {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    color: var(--text-2);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  .p-frase {
    margin: 0;
    align-self: center;
    overflow-wrap: anywhere;
  }
  .p-acciones :global(.input),
  .as-c :global(.input),
  .volumenes :global(.input) {
    width: auto;
    max-width: 100%;
  }
  .p-acciones {
    grid-column: 2;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  .servidores {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: var(--sp-4);
    margin-top: var(--sp-3);
  }
  .srv-cab {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
  }
  .srv-t {
    flex: 1;
    min-width: 0;
  }
  .srv-t h3 {
    margin: 0;
    font-size: var(--fs-h2);
    overflow-wrap: anywhere;
  }
  .srv-t p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .srv-acc {
    display: flex;
    gap: 2px;
  }
  .notas {
    margin: var(--sp-3) 0 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
    white-space: pre-line;
  }
  .servidor h4 {
    margin: var(--sp-4) 0 var(--sp-2);
  }
  .conjuntos,
  .dentro {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    font-size: var(--fs-sm);
    list-style: none;
  }
  .conjuntos li,
  .dentro li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .conjuntos :global(svg),
  .dentro :global(svg) {
    color: var(--text-3);
  }
  .cj-n {
    font-weight: 500;
    overflow-wrap: anywhere;
  }
  .serie {
    font-size: var(--fs-xs);
  }
  .fila-t {
    padding: var(--sp-4) var(--sp-4) var(--sp-2);
    margin: 0;
  }
  .asignar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2) var(--sp-4);
    scroll-margin-top: 96px;
  }
  .asignar.resaltado {
    background: var(--accent-soft, var(--surface-2));
    box-shadow: inset 3px 0 0 var(--accent);
  }
  .as-n {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .as-n :global(svg) {
    color: var(--text-3);
  }
  .as-c {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
    max-width: 100%;
  }
  .as-c :global(select),
  .volumenes :global(select),
  .p-acciones :global(select) {
    min-width: 0;
    text-overflow: ellipsis;
  }
  .volumenes {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin: 0;
    padding: var(--sp-2) var(--sp-4) var(--sp-3) calc(var(--sp-4) + 20px);
    list-style: none;
    font-size: var(--fs-sm);
    border-bottom: 1px solid var(--border);
  }
  .volumenes li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px var(--sp-3);
  }
  .volumenes .mono {
    font-weight: 500;
  }
  .pie {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    padding: var(--sp-3) var(--sp-4);
    font-size: var(--fs-sm);
  }
  .comun {
    align-items: flex-start;
    padding: 0;
  }
  .comun p {
    margin: 0;
  }
  .comun :global(svg) {
    flex: none;
    margin-top: 3px;
  }
  @media (max-width: 640px) {
    .servidores {
      grid-template-columns: 1fr;
    }
    .p-acciones {
      grid-column: 1 / -1;
    }
    .volumenes {
      padding-left: var(--sp-4);
    }
  }
</style>
