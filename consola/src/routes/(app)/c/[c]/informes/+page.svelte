<script lang="ts">
  // Informe mensual del cliente, para enseñar o imprimir (o guardar en PDF):
  // portada con la marca del cliente (v1.32), un resumen en una frase, cuatro
  // cifras, las copias de cada día del periodo, una tabla por equipo y, de
  // cada equipo, sus repositorios. Sale del historial que guarda cada equipo
  // (v1.23) o, con un servidor anterior, de sus últimos informes. Sin rutas ni
  // nombres de archivos.
  import { onMount, untrack } from "svelte";
  import { seguirCambios } from "$lib/vivo.svelte";
  import { tip } from "$lib/tooltip";
  import { Archive, CalendarRange, CloudUpload, Database, FileBarChart, Monitor, Palette, Printer, ShieldCheck } from "@lucide/svelte";
  import Logo from "$ui/componentes/Logo.svelte";
  import * as api from "$lib/api";
  import { actual, app, puede, reloj } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { bytes, fechaLarga, numero, plural, relativo } from "$lib/formato";
  import { saludEquipo } from "$lib/salud";
  import { bytesRepo, claveDia, informeDe, nVersiones, proteccion, pruebaRestauracion, verificacion, type Dia } from "$lib/repo";
  import { cargarInformes, ultimos } from "$lib/informes.svelte";
  import type { Equipo, EntradaHistorial, Informe } from "$lib/tipos";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Cifra from "$lib/componentes/Cifra.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import DiasCuadros from "$lib/componentes/repo/DiasCuadros.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import EditorMarca from "$lib/componentes/EditorMarca.svelte";
  import { dataUrlAArchivo, logoAPng } from "$lib/marca";
  import EtiquetaChip from "$lib/componentes/EtiquetaChip.svelte";
  import { etiquetasDe, gruposPorEtiqueta, mismaEtiqueta, pasaFiltro } from "$lib/etiquetas.svelte";
  import { cuentaRegla, fraseRegla, PARTES, reglasDelCliente } from "$lib/regla321";
  import { catalogoDe, cargarCatalogo } from "$lib/catalogoDestinos.svelte";

  type Periodo = "mes" | "anterior" | "30";
  // A principios de mes, lo que se quiere enseñar suele ser el mes que acaba de terminar.
  let periodo = $state<Periodo>(new Date(reloj.ahora).getDate() <= 5 ? "anterior" : "mes");

  /** El principio y el fin (sin incluir) del periodo, y cómo se llama. */
  const rango = $derived.by(() => {
    const hoy = new Date(reloj.ahora);
    if (periodo === "30") {
      const fin = new Date(hoy.getFullYear(), hoy.getMonth(), hoy.getDate() + 1);
      return { desde: new Date(fin.getFullYear(), fin.getMonth(), fin.getDate() - 30), hasta: fin, nombre: "los últimos 30 días" };
    }
    const m = periodo === "mes" ? hoy.getMonth() : hoy.getMonth() - 1;
    const desde = new Date(hoy.getFullYear(), m, 1);
    const hasta = new Date(desde.getFullYear(), desde.getMonth() + 1, 1);
    return { desde, hasta, nombre: new Intl.DateTimeFormat("es", { month: "long", year: "numeric" }).format(desde) };
  });
  /** Hasta hoy (no se pintan días que aún no han llegado). */
  const finVisible = $derived(new Date(Math.min(rango.hasta.getTime(), new Date(new Date(reloj.ahora).setHours(24, 0, 0, 0)).getTime())));

  interface Vuelta {
    t: number;
    r: "ok" | "aviso" | "fallo" | "sin_cambios";
    bytes: number;
  }
  let datos = $state<{ equipo: Equipo; vueltas: Vuelta[] }[] | null>(null);
  let vuelta = 0;

  /** Las copias del historial desde `desde`, página a página (v1.26; un servidor anterior repite la primera: se para). */
  async function copiasDesde(equipo: Equipo, desde: Date): Promise<EntradaHistorial[]> {
    const todas: EntradaHistorial[] = [];
    const vistas = new Set<string>();
    let antes: string | undefined;
    for (let i = 0; i < 20; i++) {
      const pag = await api.historialEquipo(actual.id, equipo.id, { desde: desde.toISOString(), tipo: ["copia"], limite: 2000, antes }).catch(() => [] as EntradaHistorial[]);
      const nuevas = pag.filter((x) => !vistas.has(x.id));
      for (const x of nuevas) vistas.add(x.id);
      todas.push(...nuevas);
      if (pag.length < 2000 || !nuevas.length) break;
      antes = pag[pag.length - 1].id;
    }
    return todas;
  }

  /** Las vueltas del historial del equipo (v1.23) o, si no lo hay, de sus informes (como antes). */
  async function vueltasDe(equipo: Equipo, desde: Date): Promise<Vuelta[]> {
    const h = await copiasDesde(equipo, desde);
    const copias = h.filter((x) => x.tipo === "copia" && x.resultado);
    if (copias.length) return copias.map((x) => ({ t: Date.parse(x.hora), r: x.resultado!, bytes: x.resultado === "fallo" || x.resultado === "sin_cambios" ? 0 : (x.anadido ?? 0) }));
    const informes: Informe[] = await api.informes(actual.id, equipo.id, 30).catch(() => []);
    const repos = informes[0]?.datos.repos;
    if (repos?.length) {
      return repos.flatMap((r) => {
        const anadido = new Map(r.versiones.map((v) => [`${v.copia}|${v.hora}`, v.anadido_empaquetado ?? v.anadido ?? 0]));
        return r.ejecuciones.map((e) => ({ t: Date.parse(e.hora), r: e.resultado, bytes: e.resultado === "sin_cambios" || e.resultado === "fallo" ? 0 : (e.anadido ?? anadido.get(`${e.copia}|${e.hora}`) ?? 0) }));
      });
    }
    const vistas = new Map<string, Vuelta>();
    for (const i of informes)
      for (const c of i.datos.copias ?? []) if (c.cuando) vistas.set(`${c.id}|${c.cuando}`, { t: Date.parse(c.cuando), r: c.estado === "fallo" ? "fallo" : c.estado === "aviso" ? "aviso" : "ok", bytes: c.bytes ?? 0 });
    return [...vistas.values()];
  }

  // Solo cuando cambian los equipos o el periodo (no con cada refresco del cliente cada 15 s).
  const idsEquipos = $derived(actual.equipos.filter((e) => e.confirmado && e.modo !== "trasladado").map((e) => e.id).join(","));
  const desdeMs = $derived(rango.desde.getTime());
  let periodoCargado = -1;
  /** Sube cuando un equipo sube historial nuevo (canal en vivo): se vuelven a contar sin vaciar las gráficas. */
  let recarga = $state(0);
  onMount(() => seguirCambios(() => recarga++, { ms: 0, toca: (x) => x.t === "historial" }));
  $effect(() => {
    void recarga;
    const ids = idsEquipos ? idsEquipos.split(",") : [];
    const desde = new Date(desdeMs);
    const n = ++vuelta;
    // Sin seguir lo que lean las peticiones (el contador de actividad cambiaría y volvería a empezar).
    untrack(() => {
      if (periodoCargado !== desdeMs) datos = null;
      const eqs = actual.equipos.filter((e) => ids.includes(e.id));
      void Promise.all(eqs.map(async (equipo) => ({ equipo, vueltas: await vueltasDe(equipo, desde) }))).then((d) => {
        if (n !== vuelta) return;
        datos = d;
        periodoCargado = desdeMs;
      });
    });
  });
  $effect(() => {
    // Las cargas, sin seguir lo que leen (si no, cada respuesta podría volver a lanzar el efecto).
    const [cc, ids] = [actual.id, actual.equipos.map((e) => e.id)];
    untrack(() => void cargarInformes(cc, ids));
  });

  // Tarea 8: la regla 3-2-1-1-0 de cada copia (con lo que dice el catálogo de destinos).
  $effect(() => {
    const cc = actual.id;
    if (cc) untrack(() => void cargarCatalogo(cc));
  });

  // v1.52: un informe por etiqueta («Contabilidad»): solo sus equipos.
  let deEtiqueta = $state("");
  const reglas = $derived(reglasDelCliente(actual.equipos.filter((e) => pasaFiltro(e, deEtiqueta)), ultimos.porEquipo, catalogoDe(actual.id), reloj.ahora, actual.equipos));
  const cuentaR = $derived(cuentaRegla(reglas));
  const etiquetas = $derived(etiquetasDe(actual.equipos));
  $effect.pre(() => {
    // Una etiqueta que ya nadie lleva (u otro cliente): vuelve al cliente entero.
    if (deEtiqueta && actual.cargado && !etiquetas.some((t) => mismaEtiqueta(t.nombre, deEtiqueta))) deEtiqueta = "";
  });
  const enRango = (v: Vuelta) => v.t >= rango.desde.getTime() && v.t < rango.hasta.getTime();
  const delPeriodo = $derived(
    (datos ?? []).map((d) => ({ equipo: actual.equipos.find((e) => e.id === d.equipo.id) ?? d.equipo, vueltas: d.vueltas.filter(enRango) })).filter((d) => pasaFiltro(d.equipo, deEtiqueta)),
  );
  /** Sin etiqueta elegida: una fila por etiqueta (y los que no tienen). */
  const porEtiqueta = $derived(deEtiqueta || !etiquetas.length ? [] : gruposPorEtiqueta(delPeriodo.map((d) => ({ ...d, etiquetas: d.equipo.etiquetas }))));
  const todas = $derived(delPeriodo.flatMap((d) => d.vueltas));
  const cuenta = (vs: Vuelta[], r: Vuelta["r"] | "bien") => vs.filter((v) => (r === "bien" ? v.r === "ok" || v.r === "sin_cambios" : v.r === r)).length;
  const nuevos = (vs: Vuelta[]) => vs.reduce((n, v) => n + v.bytes, 0);
  const protegido = $derived(
    actual.equipos.filter((e) => pasaFiltro(e, deEtiqueta)).flatMap((e) => (e.resumen?.repositorios ?? []).map((r) => bytesRepo(r, informeDe(ultimos.porEquipo[e.id], r.id)) ?? 0)).reduce((n, b) => n + b, 0),
  );
  const saludes = $derived(delPeriodo.map((d) => saludEquipo(d.equipo, reloj.ahora)));
  const alDia = $derived(saludes.filter((s) => s.tono === "ok").length);

  /** Los días del periodo (hasta hoy), con el peor resultado de cada uno: para los cuadros y la gráfica. */
  function diasDe(vs: Vuelta[]): (Dia & { copias: number; fallos: number })[] {
    const por = new Map<string, Vuelta[]>();
    for (const v of vs) {
      const k = claveDia(new Date(v.t));
      por.set(k, [...(por.get(k) ?? []), v]);
    }
    const out: (Dia & { copias: number; fallos: number })[] = [];
    const fmt = new Intl.DateTimeFormat("es", { weekday: "long", day: "numeric", month: "long" });
    for (let f = new Date(rango.desde); f < finVisible; f = new Date(f.getFullYear(), f.getMonth(), f.getDate() + 1)) {
      const clave = claveDia(f);
      const xs = por.get(clave) ?? [];
      const fallos = cuenta(xs, "fallo");
      const bien = xs.some((x) => x.r === "ok" || x.r === "sin_cambios");
      const datosNuevos = xs.some((x) => x.r === "ok" && x.bytes > 0) || xs.some((x) => x.r === "ok");
      const estado = !xs.length ? "nada" : fallos && !bien ? "mal" : fallos || xs.some((x) => x.r === "aviso") ? "aviso" : datosNuevos ? "datos" : "igual";
      const texto = !xs.length ? "sin copia" : `${plural(xs.length, "copia", "copias")}${fallos ? `, ${plural(fallos, "fallida", "fallidas")}` : ""}`;
      out.push({ clave, fecha: new Date(f), estado, n: xs.length, titulo: `${fmt.format(f)}: ${texto}`, copias: xs.length, fallos });
    }
    return out;
  }
  const diasTodos = $derived(diasDe(todas));
  const maxDia = $derived(Math.max(1, ...diasTodos.map((d) => d.copias)));

  /** La frase del resumen: qué pasó en el periodo y cómo está ahora. */
  const frase = $derived.by(() => {
    if (!delPeriodo.length) return "";
    const n = todas.length;
    const partes = [`En ${rango.nombre}, ${plural(delPeriodo.length, "equipo hizo", "equipos hicieron")} ${plural(n, "copia", "copias")}`];
    if (n) partes[0] += `: ${numero(cuenta(todas, "bien"))} correctas${cuenta(todas, "aviso") ? `, ${numero(cuenta(todas, "aviso"))} con avisos` : ""}${cuenta(todas, "fallo") ? ` y ${plural(cuenta(todas, "fallo"), "fallida", "fallidas")}` : ""}`;
    const nuevosTxt = nuevos(todas) ? ` Se guardaron ${bytes(nuevos(todas))} nuevos.` : "";
    const mal = saludes.filter((s) => s.tono === "bad" || s.tono === "warn").length;
    const ahora = mal ? ` Hoy ${plural(mal, "equipo necesita", "equipos necesitan")} atención.` : " Hoy todo está protegido.";
    return `${partes.join("")}.${nuevosTxt}${ahora}`;
  });

  // --- Marca del cliente (v1.32): logo y color, guardados en el servidor ----
  let editarMarca = $state(false);
  const marca = $derived(actual.cliente?.marca);
  // Antes, el logo se guardaba solo en este navegador («resguardo.informe.logo»).
  // Si queda uno, se ofrece pasarlo al cliente (o quitarlo); mientras, sale en
  // la portada de los clientes sin logo propio, como antes.
  const CLAVE_LOGO = "resguardo.informe.logo";
  let logoLocal = $state<string | null>(null);
  $effect(() => {
    try {
      logoLocal = localStorage.getItem(CLAVE_LOGO);
    } catch {
      logoLocal = null;
    }
  });
  const logo = $derived(marca?.logo ?? logoLocal);
  function olvidarLogoLocal() {
    logoLocal = null;
    try {
      localStorage.removeItem(CLAVE_LOGO);
    } catch {
      /* sin almacenamiento */
    }
  }
  let pasando = $state(false);
  async function pasarLogoAlCliente() {
    if (!logoLocal || !actual.cliente) return;
    pasando = true;
    try {
      const png = await logoAPng(dataUrlAArchivo(logoLocal));
      const m = await api.cambiarMarca(actual.id, { acento: marca?.acento ?? null, logo: png.base64 });
      actual.cliente.marca = m;
      const x = app.clientes.find((k) => k.id === actual.id);
      if (x) x.marca = m;
      olvidarLogoLocal();
      avisar(`Logo guardado como el de ${actual.cliente.nombre}. Ya no depende de este navegador.`);
    } catch (e) {
      avisar((e as Error).message, "warn");
    } finally {
      pasando = false;
    }
  }
</script>

<svelte:head><title>Informe · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page informe">
  <div class="no-imprimir">
    <CabeceraPagina
      titulo="Informes"
      icono={FileBarChart}
      migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${actual.id}` }, { texto: "Informes" }]}
      resumen="Un informe para el cliente: listo para imprimir o guardar en PDF. Sin rutas ni nombres de archivos."
    >
      {#snippet acciones()}
        {#if actual.cliente && puede.administrar(actual.cliente.rol)}<button class="btn btn-ghost" onclick={() => (editarMarca = true)}><Palette size={16} />Marca del cliente</button>{/if}
        <button class="btn btn-primary" onclick={() => window.print()} disabled={!datos?.length}><Printer size={16} />Imprimir o PDF</button>
      {/snippet}
    </CabeceraPagina>
    {#if logoLocal && !marca?.logo}
      <div class="notice notice-info logo-antiguo">
        <p>
          Este navegador guarda un logo de antes, solo para ti. Ahora el logo es de cada cliente y lo ven todas sus personas.
          {#if actual.cliente && puede.administrar(actual.cliente.rol)}
            <button class="notice-action" disabled={pasando} onclick={pasarLogoAlCliente}>{pasando ? "Guardando…" : `Usarlo para ${actual.cliente.nombre}`}</button> ·
          {/if}
          <button class="notice-action" onclick={olvidarLogoLocal}>Quitarlo de este navegador</button>
        </p>
      </div>
    {/if}
    <div class="segmented inline periodo" role="group" aria-label="Periodo">
      <button class:on={periodo === "mes"} aria-pressed={periodo === "mes"} onclick={() => (periodo = "mes")}>Este mes</button>
      <button class:on={periodo === "anterior"} aria-pressed={periodo === "anterior"} onclick={() => (periodo = "anterior")}>Mes pasado</button>
      <button class:on={periodo === "30"} aria-pressed={periodo === "30"} onclick={() => (periodo = "30")}>Últimos 30 días</button>
    </div>
    {#if etiquetas.length}
      <div class="de-etiqueta">
        <label class="field-label" for="inf-etiqueta">De</label>
        <select id="inf-etiqueta" class="input" bind:value={deEtiqueta}>
          <option value="">Todo el cliente</option>
          {#each etiquetas as t (t.nombre)}<option value={t.nombre}>Los equipos con «{t.nombre}» ({t.n})</option>{/each}
        </select>
      </div>
    {/if}
  </div>

  <article class="hoja card" aria-label="Informe de {actual.cliente?.nombre ?? ''}">
    <header class="portada marca-cliente" data-acento={marca?.acento ?? undefined} class:con-acento={!!marca?.acento}>
      <div class="marca">
        {#if logo}<img class="logo-propio" src={logo} alt="Logo de {marca?.logo ? (actual.cliente?.nombre ?? 'el cliente') : 'quien lo prepara'}" />{:else}<span class="logo-r"><Logo size={34} /></span>{/if}
      </div>
      <div class="titulo">
        <p class="sobre">Informe de copias de seguridad</p>
        <h2>{actual.cliente?.nombre ?? ""}</h2>
        {#if deEtiqueta}<p class="solo-et">Equipos con <EtiquetaChip nombre={deEtiqueta} /></p>{/if}
        <p class="periodo-txt"><CalendarRange size={14} /><span class="primera-mayuscula">{rango.nombre}</span></p>
      </div>
      <p class="preparado">Preparado {app.cuenta?.nombre ? `por ${app.cuenta.nombre}` : ""}<br />el {fechaLarga(new Date(reloj.ahora).toISOString())}</p>
    </header>

    {#if datos === null}
      <Esqueleto forma="cifras" n={4} />
      <Esqueleto forma="tabla" n={4} />
    {:else if !delPeriodo.length}
      <Vacio icono={FileBarChart} titulo="Aún no hay nada que contar" texto="Cuando los equipos hagan sus primeras copias, aquí saldrá su informe.">
        <a class="btn btn-primary no-imprimir" href="/c/{actual.id}/equipos">Ver los equipos</a>
      </Vacio>
    {:else}
      <p class="frase">{frase}</p>

      <div class="cifras" role="list" aria-label="Cifras del periodo">
        <Cifra icono={ShieldCheck} etiqueta="Equipos al día" valor={numero(alDia)} de="de {delPeriodo.length}" sub={alDia === delPeriodo.length ? "todos bien" : plural(delPeriodo.length - alDia, "necesita atención", "necesitan atención")} mal={alDia < delPeriodo.length} />
        <Cifra icono={Archive} etiqueta="Copias" valor={numero(todas.length)} sub={cuenta(todas, "fallo") ? plural(cuenta(todas, "fallo"), "fallida", "fallidas") : "ninguna fallida"} mal={cuenta(todas, "fallo") > 0} />
        <Cifra icono={CloudUpload} etiqueta="Datos nuevos" valor={nuevos(todas) ? bytes(nuevos(todas)) : "—"} sub="en el periodo" />
        <Cifra icono={Database} etiqueta="Protegido" valor={protegido ? bytes(protegido) : "—"} sub="en total, hoy" />
      </div>

      <section class="bloque" aria-labelledby="t-dias">
        <h3 id="t-dias">Copias de cada día</h3>
        <div class="grafica" role="img" aria-label="Copias por día en {rango.nombre}: {numero(todas.length)} en total">
          <span class="g-max num" aria-hidden="true">{maxDia}</span>
          <div class="g-barras">
            {#each diasTodos as d (d.clave)}
              <span class="g-col" use:tip={d.titulo}>
                <span class="g-pila" style:height="{(d.copias / maxDia) * 100}%">
                  {#if d.fallos}<span class="g-fallo" style:flex={d.fallos}></span>{/if}
                  {#if d.copias - d.fallos}<span class="g-barra" style:flex={d.copias - d.fallos}></span>{/if}
                </span>
              </span>
            {/each}
          </div>
          <div class="g-eje num" aria-hidden="true">
            <span>{diasTodos[0]?.fecha.getDate()}</span>
            <span>{diasTodos[Math.floor(diasTodos.length / 2)]?.fecha.getDate()}</span>
            <span>{diasTodos.at(-1)?.fecha.getDate()}</span>
          </div>
        </div>
        <p class="leyenda"><span class="m bien"></span>copias correctas o con avisos <span class="m mal"></span>fallidas</p>
      </section>

      {#if porEtiqueta.length > 1}
        <section class="bloque" aria-labelledby="t-etiquetas">
          <h3 id="t-etiquetas">Por etiqueta</h3>
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div class="desliza" role="region" aria-label="Por etiqueta (se desliza a los lados)" tabindex="0">
          <table class="tabla" aria-labelledby="t-etiquetas">
            <thead><tr><th scope="col">Etiqueta</th><th scope="col" class="der">Equipos</th><th scope="col" class="der">Al día</th><th scope="col" class="der">Copias</th><th scope="col" class="der">Fallidas</th><th scope="col" class="der">Datos nuevos</th></tr></thead>
            <tbody>
              {#each porEtiqueta as g (g.etiqueta ?? "")}
                {@const vs = g.equipos.flatMap((d) => d.vueltas)}
                {@const ok = g.equipos.filter((d) => saludEquipo(d.equipo, reloj.ahora).tono === "ok").length}
                <tr>
                  <td>{#if g.etiqueta}<EtiquetaChip nombre={g.etiqueta} />{:else}<span class="sub">Sin etiqueta</span>{/if}</td>
                  <td class="num der">{numero(g.equipos.length)}</td>
                  <td class="num der" class:mal={ok < g.equipos.length}>{numero(ok)}</td>
                  <td class="num der">{numero(vs.length)}</td>
                  <td class="num der" class:mal={cuenta(vs, "fallo") > 0}>{numero(cuenta(vs, "fallo"))}</td>
                  <td class="num der">{bytes(nuevos(vs))}</td>
                </tr>
              {/each}
            </tbody>
          </table></div>
          <p class="leyenda">Un equipo con varias etiquetas cuenta en cada una.</p>
        </section>
      {/if}

      <section class="bloque" aria-labelledby="t-equipos">
        <h3 id="t-equipos">Equipos</h3>
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <div class="desliza" role="region" aria-label="Equipos (se desliza a los lados)" tabindex="0">
        <table class="tabla t-equipos" aria-labelledby="t-equipos">
          <thead><tr><th scope="col">Equipo</th><th scope="col">Estado hoy</th><th scope="col" class="der">Copias</th><th scope="col" class="der">Fallidas</th><th scope="col" class="der">Datos nuevos</th><th scope="col">Cada día</th></tr></thead>
          <tbody>
            {#each delPeriodo as d, i (d.equipo.id)}
              <tr>
                <td><span class="eq"><Monitor size={14} /><strong>{d.equipo.nombre}</strong></span><span class="sub">{d.equipo.so}</span></td>
                <td><Chip pequeno tono={saludes[i].tono} texto={saludes[i].texto} /></td>
                <td class="num der">{numero(d.vueltas.length)}</td>
                <td class="num der" class:mal={cuenta(d.vueltas, "fallo") > 0}>{numero(cuenta(d.vueltas, "fallo"))}</td>
                <td class="num der">{bytes(nuevos(d.vueltas))}</td>
                <td><DiasCuadros dias={diasDe(d.vueltas)} tamano="mini" etiqueta="Copias de {d.equipo.nombre} cada día" /></td>
              </tr>
            {/each}
          </tbody>
        </table></div>
        <p class="leyenda">Cuadros: <span class="m datos"></span>con versión nueva <span class="m igual"></span>sin cambios <span class="m aviso"></span>con avisos <span class="m mal"></span>falló <span class="m nada"></span>sin copia</p>
      </section>

      {#if reglas.length}
        <!-- Tarea 8: la regla 3-2-1-1-0 por copia (para enseñarla al cliente). -->
        <section class="bloque" aria-labelledby="t-regla">
          <h3 id="t-regla">Regla 3-2-1-1-0 <span class="sub-h">· {cuentaR.cumplen} de {plural(cuentaR.total, "copia la cumple", "copias la cumplen")}</span></h3>
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div class="desliza" role="region" aria-label="Regla 3-2-1-1-0 (se desliza a los lados)" tabindex="0">
          <table class="tabla t-regla">
            <caption class="sr-only">Cómo cumple cada copia la regla 3-2-1-1-0</caption>
            <thead>
              <tr>
                <th scope="col">Copia</th>
                {#each reglas[0].regla.partes as p (p.id)}<th scope="col" class="centro" title={PARTES[p.id].titulo}>{PARTES[p.id].cifra}</th>{/each}
                <th scope="col">Qué le falta</th>
              </tr>
            </thead>
            <tbody>
              {#each reglas as r (r.equipo.id + r.copia.id)}
                <tr>
                  <td><strong>{r.copia.nombre}</strong><span class="sub">{r.equipo.nombre}</span></td>
                  {#each r.regla.partes as p (p.id)}
                    <td class="centro" class:mal={!p.cumple && !p.cumple_config} class:atrasado={!p.cumple && p.cumple_config}>{p.cumple ? "✓" : p.cumple_config ? "◷" : "✗"}<span class="sr-only">{p.cumple ? "cumple" : p.cumple_config ? "no está al día" : "falta"}</span></td>
                  {/each}
                  <td>{r.regla.cumple ? "Nada" : fraseRegla(r.regla).replace(/^(Le falta|Dejó de cumplir): /, "")}</td>
                </tr>
              {/each}
            </tbody>
          </table></div>
          <p class="leyenda">3 copias (con los originales) · 2 soportes · 1 fuera del sitio · 1 inmutable o aislado · 0 errores al verificar y probar la restauración. ✓ cumple, ◷ configurada pero no está al día, ✗ falta. Es una guía.</p>
        </section>
      {/if}

      <section class="bloque" aria-labelledby="t-repos">
        <h3 id="t-repos">Repositorios</h3>
        {#each delPeriodo as d (d.equipo.id)}
          {@const repos = d.equipo.resumen?.repositorios ?? []}
          {#if repos.length}
            <div class="por-equipo">
              <h4>{d.equipo.nombre}</h4>
              <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
              <div class="desliza" role="region" aria-label="Repositorios de {d.equipo.nombre} (se desliza a los lados)" tabindex="0">
              <table class="tabla">
                <caption class="sr-only">Repositorios de {d.equipo.nombre}</caption>
                <thead><tr><th scope="col">Repositorio</th><th scope="col" class="der">Versiones</th><th scope="col" class="der">Tamaño</th><th scope="col">Verificado</th><th scope="col">Restauración probada</th><th scope="col">Protección</th></tr></thead>
                <tbody>
                  {#each repos as r (r.id)}
                    {@const inf = informeDe(ultimos.porEquipo[d.equipo.id], r.id)}
                    {@const p = proteccion(inf)}
                    <tr>
                      <td><strong>{r.nombre}</strong></td>
                      <td class="num der">{numero(nVersiones(r, inf))}</td>
                      <td class="num der">{bytes(bytesRepo(r, inf))}</td>
                      <td>{verificacion(r, inf)?.ultima ? relativo(verificacion(r, inf)?.ultima, reloj.ahora) : "nunca"}</td>
                      <td>{pruebaRestauracion(r, inf)?.ultima ? relativo(pruebaRestauracion(r, inf)?.ultima, reloj.ahora) : "nunca"}</td>
                      <td class="num">{p ? `${p.puntuacion} de ${p.total}` : "—"}</td>
                    </tr>
                  {/each}
                </tbody>
              </table></div>
            </div>
          {/if}
        {/each}
      </section>

      <footer class="pie-informe">
        <span><Logo size={14} />Resguardo</span>
        <span>Las copias van cifradas de punta a punta: este informe no contiene rutas ni nombres de archivos.</span>
      </footer>
    {/if}
  </article>
</div>

{#if editarMarca && actual.cliente}
  <EditorMarca cliente={actual.id} nombre={actual.cliente.nombre} marca={actual.cliente.marca} onclose={() => (editarMarca = false)} />
{/if}

<style>
  .no-imprimir {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-5);
  }
  .no-imprimir > :global(nav),
  .no-imprimir > :global(.cabecera-pagina) {
    align-self: stretch;
  }
  .de-etiqueta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .de-etiqueta .input {
    width: auto;
    max-width: 100%;
  }
  .solo-et {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 2px 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .hoja {
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
    padding: var(--sp-8);
    border-radius: var(--radius-xl);
  }
  .portada {
    display: flex;
    align-items: center;
    gap: var(--sp-5);
    padding-bottom: var(--sp-5);
    border-bottom: 1px solid var(--border);
  }
  /* Con el color del cliente: la raya de la portada, más marcada. */
  .portada.con-acento {
    border-bottom: 2px solid var(--marca);
  }
  .logo-antiguo {
    margin-top: var(--sp-3);
  }
  /* Logos apaisados: hasta 160 px de ancho; los cuadrados, 72. */
  .marca {
    display: grid;
    flex: none;
    place-items: center;
    min-width: 72px;
    max-width: 160px;
    height: 72px;
  }
  .logo-propio {
    max-width: 160px;
    max-height: 64px;
    object-fit: contain;
  }
  .titulo {
    flex: 1;
    min-width: 0;
  }
  .sobre {
    margin: 0;
    font-size: var(--fs-overline);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .titulo h2 {
    margin: 2px 0;
    font-size: var(--fs-display);
    line-height: var(--lh-display);
    font-weight: 650;
    letter-spacing: -0.022em;
  }
  .periodo-txt {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    color: var(--text-2);
  }
  .primera-mayuscula::first-letter {
    text-transform: uppercase;
  }
  .preparado {
    margin: 0;
    font-size: var(--fs-xs);
    color: var(--text-3);
    text-align: right;
  }
  .frase {
    margin: 0;
    font-size: 16px;
    line-height: 24px;
    color: var(--text-1);
  }
  .bloque {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    break-inside: avoid;
  }
  .bloque h3 {
    margin: 0;
    font-size: var(--fs-h2);
    line-height: var(--lh-h2);
    font-weight: 600;
  }
  .por-equipo {
    break-inside: avoid;
  }
  .por-equipo + .por-equipo {
    margin-top: var(--sp-4);
  }
  .por-equipo h4 {
    margin: 0 0 6px;
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--text-2);
  }
  .tabla {
    width: 100%;
  }
  .der {
    text-align: right;
  }
  .mal {
    color: var(--bad);
  }
  /* Tarea 8: la tabla de la regla 3-2-1-1-0. */
  .centro {
    text-align: center;
    width: 2.2em;
    font-weight: 600;
  }
  td.centro {
    color: var(--ok);
  }
  td.centro.mal {
    color: var(--bad);
  }
  td.centro.atrasado {
    color: var(--warn);
  }
  .sub-h {
    font-weight: 400;
    color: var(--text-3);
  }
  @media (max-width: 480px) {
    .t-regla th,
    .t-regla td {
      padding-left: 3px;
      padding-right: 3px;
    }
    .t-regla .centro {
      width: 1.4em;
    }
  }
  .eq {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .eq :global(svg) {
    color: var(--text-3);
  }
  .sub {
    display: block;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  /* Gráfica de copias por día: barras finas en tinta neutra; las de días con fallos, en rojo. */
  .grafica {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    grid-template-rows: 120px auto;
    column-gap: 8px;
  }
  .g-max {
    align-self: start;
    font-size: 10.5px;
    color: var(--text-3);
  }
  .g-barras {
    display: flex;
    align-items: flex-end;
    gap: 3px;
    height: 120px;
    border-bottom: 1px solid var(--border-strong);
    background: linear-gradient(to bottom, var(--border) 1px, transparent 1px) 0 0 / 100% 50%;
  }
  .g-col {
    display: flex;
    flex: 1;
    align-items: flex-end;
    justify-content: center;
    height: 100%;
    min-width: 0;
  }
  /* Cada día, una pila fina: las correctas en tinta neutra y, encima, las fallidas en rojo. */
  .g-pila {
    display: flex;
    flex-direction: column;
    gap: 1px;
    width: min(10px, 100%);
    overflow: hidden;
    border-radius: 2px 2px 0 0;
    transition: height var(--dur-slow) var(--ease-out);
  }
  .g-barra {
    min-height: 2px;
    background: color-mix(in srgb, var(--text-3) 55%, transparent);
  }
  .g-fallo {
    min-height: 2px;
    background: var(--bad);
  }
  .g-col:hover .g-barra {
    background: var(--accent);
  }
  .g-eje {
    grid-column: 2;
    display: flex;
    justify-content: space-between;
    padding-top: 4px;
    font-size: 10.5px;
    color: var(--text-3);
  }
  .leyenda {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 6px;
    margin: 0;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .m {
    display: inline-block;
    width: 10px;
    height: 10px;
    margin-left: 8px;
    border-radius: 2px;
  }
  .m.bien {
    background: color-mix(in srgb, var(--text-3) 55%, transparent);
  }
  .m.mal {
    background: var(--bad);
  }
  .m.datos {
    background: color-mix(in srgb, var(--ok) 85%, transparent);
  }
  .m.igual {
    background: color-mix(in srgb, var(--ok) 45%, transparent);
  }
  .m.aviso {
    background: var(--warn);
  }
  .m.nada {
    background: var(--surface-3);
  }
  .pie-informe {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: 6px 16px;
    padding-top: var(--sp-4);
    font-size: var(--fs-xs);
    color: var(--text-3);
    border-top: 1px solid var(--border);
  }
  .pie-informe span:first-child {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
  }
  @media (max-width: 640px) {
    .hoja {
      padding: var(--sp-5);
    }
    .portada {
      flex-wrap: wrap;
      align-items: flex-start;
      gap: var(--sp-3) var(--sp-4);
    }
    .marca {
      min-width: 48px;
      max-width: 120px;
      height: 48px;
    }
    .logo-propio {
      max-width: 120px;
      max-height: 44px;
    }
    .titulo {
      flex: 1 1 calc(100% - 64px);
    }
    .preparado {
      flex-basis: 100%;
      text-align: left;
    }
    .t-equipos th:nth-child(4),
    .t-equipos td:nth-child(4),
    .t-equipos th:nth-child(6),
    .t-equipos td:nth-child(6) {
      display: none;
    }
  }
  /* En pantallas estrechas, las tablas se deslizan dentro de la hoja (la página no). */
  .desliza {
    overflow-x: auto;
    max-width: 100%;
    border-radius: var(--radius-sm, 6px);
  }
  .desliza:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .desliza > .tabla {
    min-width: 100%;
  }

  /* Impresión: solo la hoja, en claro, a toda página y sin cortar tablas ni bloques. */
  @media print {
    @page {
      size: A4;
      margin: 14mm 12mm;
    }
    :global(html),
    :global(body) {
      background: #fff !important;
    }
    :global(:root) {
      --bg: #fff;
      --surface: #fff;
      --surface-2: #f4f4f5;
      --surface-3: #ececee;
      --border: #e4e4e7;
      --border-strong: #d4d4d8;
      --text-1: #18181b;
      --text-2: #3f3f46;
      --text-3: #52525b;
      color-scheme: light;
    }
    .portada {
      --marca: var(--marca-l);
    }
    :global(.lateral),
    :global(.movil),
    :global(.toaster),
    :global(.barra-progreso),
    .no-imprimir {
      display: none !important;
    }
    :global(.contenido) {
      padding: 0 !important;
    }
    .informe {
      max-width: none;
    }
    .hoja {
      padding: 0;
      border: none;
      box-shadow: none;
    }
    :global(.cifra) {
      animation: none !important;
      break-inside: avoid;
    }
    .g-pila {
      transition: none;
    }
    .g-barra,
    .g-fallo {
      print-color-adjust: exact;
      -webkit-print-color-adjust: exact;
    }
    :global(.cuadros .c),
    .m,
    :global(.badge) {
      print-color-adjust: exact;
      -webkit-print-color-adjust: exact;
    }
    .tabla :global(tr) {
      break-inside: avoid;
    }
  }
</style>
