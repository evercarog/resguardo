<script lang="ts">
  import { untrack } from "svelte";
  import { tip } from "$lib/tooltip";
  // Estado del cliente (docs/diseno.md §5, «Inicio»): el resumen grande con
  // lo urgente y su acción, cuatro cifras, las órdenes que esperan, la salud
  // de cada equipo con sus 14 días, los repositorios y cuánto se guarda en
  // cada destino con su tendencia.
  import { Archive, CalendarClock, ChevronRight, CircleAlert, CircleCheck, Cloud, Database, HardDrive, Lock, LockKeyhole, Monitor, Plus, Server, ShieldCheck, TriangleAlert } from "@lucide/svelte";
  import Anuncio from "$lib/componentes/Anuncio.svelte";
  import { actual, puede, reloj } from "$lib/estado.svelte";
  import { bytes, cuandoFrase, numero, plural, relativo } from "$lib/formato";
  import { almacenes as almacenesDe, proximaDeTodos, ultimas24h } from "$lib/panel";
  import { cargarInformes, ultimos } from "$lib/informes.svelte";
  import { copiaAtrasada, PESO, saludEquipo, type Tono } from "$lib/salud";
  import type { Equipo } from "$lib/tipos";
  import { bytesRepo, informeDe } from "$lib/repo";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import TarjetaRepo from "$lib/componentes/repo/TarjetaRepo.svelte";
  import Esqueleto from "$lib/componentes/Esqueleto.svelte";
  import FiltroEtiquetas from "$lib/componentes/FiltroEtiquetas.svelte";
  import { agruparPorEtiqueta, filtroEtiqueta, gruposPorEtiqueta, pasaFiltro } from "$lib/etiquetas.svelte";
  import EtiquetaChip from "$lib/componentes/EtiquetaChip.svelte";
  import TarjetaEquipo from "$lib/componentes/TarjetaEquipo.svelte";
  import Sparkline from "$lib/componentes/Sparkline.svelte";
  import Pendientes from "$lib/componentes/Pendientes.svelte";
  import AvisoConsolas from "$lib/componentes/AvisoConsolas.svelte";
  import PrimerosPasos from "$lib/componentes/PrimerosPasos.svelte";
  import OrdenDialog from "$lib/componentes/OrdenDialog.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import MapaProteccion from "$lib/componentes/mapa/MapaProteccion.svelte";
  import AlmacenEnOtraConsola from "$lib/componentes/AlmacenEnOtraConsola.svelte";
  import CuandoSeLlena from "$lib/componentes/llenado/CuandoSeLlena.svelte";
  import Observaciones from "$lib/componentes/notas/Observaciones.svelte";
  import Comentarios from "$lib/componentes/notas/Comentarios.svelte";
  import { riesgosDelCliente } from "$lib/dondeGuarda";
  // Tarea 8: cuántas copias cumplen la regla 3-2-1-1-0 (y las que dejaron de cumplir, sin urgencia).
  import ReglaCliente from "$lib/componentes/regla/ReglaCliente.svelte";

  interface Urgente {
    tono: Tono;
    texto: string;
    /** Quién (el equipo) y qué le pasa, por separado. */
    titulo?: string;
    detalle?: string;
    accion?: { texto: string; href?: string; orden?: { equipo: Equipo; repo: string; copia: string; nombre: string } };
  }

  const c = $derived(actual.id);
  // Con una etiqueta elegida (Equipos, Avisos y Estado comparten el filtro), solo sus equipos.
  const delFiltro = $derived(actual.equipos.filter((e) => pasaFiltro(e, filtroEtiqueta.valor)));
  const equipos = $derived([...delFiltro].sort((a, b) => PESO[saludEquipo(a, reloj.ahora).tono] - PESO[saludEquipo(b, reloj.ahora).tono] || a.nombre.localeCompare(b.nombre)));
  const saludes = $derived(delFiltro.map((e) => saludEquipo(e, reloj.ahora)));
  // v1.4x: agrupados por etiqueta (como en Equipos; la misma preferencia).
  const hayEtiquetas = $derived(actual.equipos.some((e) => e.etiquetas?.length));
  const grupos = $derived(agruparPorEtiqueta.valor && hayEtiquetas && !filtroEtiqueta.valor ? gruposPorEtiqueta(equipos) : null);
  /** Cuántos de un grupo están bien, necesitan atención o fallan (con texto, no solo color). */
  function cuentaGrupo(es: Equipo[]) {
    const t = es.map((e) => saludEquipo(e, reloj.ahora).tono);
    return { mal: t.filter((x) => x === "bad").length, atencion: t.filter((x) => x === "warn").length, bien: t.filter((x) => x === "ok").length };
  }
  const urgentes = $derived.by(() => {
    const out: Urgente[] = [];
    for (const e of delFiltro) {
      const s = saludEquipo(e, reloj.ahora);
      if (s.tono !== "bad" && s.tono !== "warn") continue;
      const copia = (e.resumen?.copias ?? []).find((x) => x.ultima?.estado === "fallo" || copiaAtrasada(x, reloj.ahora));
      const sinContacto = s.texto === "Sin contacto" || s.texto === "Detenido";
      out.push({
        tono: s.tono,
        texto: `${e.nombre}: ${s.detalle}`,
        titulo: `${e.nombre} · ${s.texto.toLowerCase()}`,
        detalle: s.detalle,
        accion:
          copia && !sinContacto && puede.ordenar(actual.cliente?.rol)
            ? { texto: "Copiar ahora", orden: { equipo: e, repo: copia.repo, copia: copia.id, nombre: copia.nombre } }
            : { texto: "Ver equipo", href: `/c/${c}/equipos/${e.id}` },
      });
    }
    // v1.41: copias que se quedan en el mismo equipo que protegen.
    for (const { equipo: e, repo: r, riesgo } of riesgosDelCliente(delFiltro, actual.equipos))
      out.push({
        tono: "warn",
        texto: riesgo.texto,
        titulo: `${e.nombre} · copias en el mismo equipo`,
        detalle: `«${r.nombre}»: ${riesgo.texto}`,
        accion: puede.administrar(actual.cliente?.rol)
          ? { texto: "Mover a un almacén…", href: `/c/${c}/equipos/${e.id}/repositorios/${encodeURIComponent(r.id)}?mover=1` }
          : { texto: "Ver el repositorio", href: `/c/${c}/equipos/${e.id}/repositorios/${encodeURIComponent(r.id)}` },
      });
    if (actual.avisosAbiertos) out.push({ tono: "warn", texto: `${plural(actual.avisosAbiertos, "aviso sin revisar", "avisos sin revisar")}.`, accion: { texto: "Ver avisos", href: `/c/${c}/avisos` } });
    return out.sort((a, b) => PESO[a.tono] - PESO[b.tono]);
  });
  const ultima = $derived(
    delFiltro
      .flatMap((e) => [...(e.resumen?.copias ?? []).map((x) => x.ultima?.cuando), ...(informes[e.id]?.datos.copias ?? []).map((x) => x.cuando)])
      .filter((x): x is string => !!x)
      .sort()
      .at(-1),
  );
  const repos = $derived(delFiltro.flatMap((e) => e.resumen?.repositorios ?? []));
  const protegido = $derived(
    delFiltro.flatMap((e) => (e.resumen?.repositorios ?? []).map((r) => bytesRepo(r, informeDe(informes[e.id], r.id)) ?? 0)).reduce((n, b) => n + b, 0),
  );
  /** Equipos que aún no han hecho su primera copia: sin eso no se dice «Todo protegido». */
  const sinPrimera = $derived(saludes.filter((x) => x.texto === "Sin copias todavía" || x.texto === "Sin copias").length);
  const titular = $derived(
    !delFiltro.length
      ? "Todavía no hay equipos"
      : urgentes.length
        ? urgentes.length === 1
          ? "1 cosa necesita atención"
          : `${urgentes.length} cosas necesitan atención`
        : sinPrimera === delFiltro.length
          ? "Sin copias todavía"
          : sinPrimera
            ? `${plural(sinPrimera, "equipo aún sin copias", "equipos aún sin copias")}; el resto, protegido`
            : "Todo protegido",
  );
  const tonoTitular = $derived<Tono>(
    !delFiltro.length ? "neutral" : urgentes.length ? (urgentes.some((u) => u.tono === "bad") ? "bad" : "warn") : sinPrimera === delFiltro.length ? "neutral" : sinPrimera ? "info" : "ok",
  );
  /** Con un solo equipo sin copias, cuándo será la primera. */
  const pistaPrimera = $derived(sinPrimera && sinPrimera === delFiltro.length ? saludes.find((x) => x.texto === "Sin copias todavía")?.detalle : undefined);
  const cuenta = (t: Tono | "sin") => (t === "sin" ? saludes.filter((s) => s.texto === "Sin contacto").length : saludes.filter((s) => s.tono === t).length);

  // El último informe de cada equipo (para los cuadros, la protección de cada
  // repositorio y lo que ocupa cada destino): compartido con otras pantallas.
  $effect(() => {
    // Las cargas, sin seguir lo que leen (si no, cada respuesta podría volver a lanzar el efecto).
    const [cc, ids] = [c, actual.equipos.map((e) => e.id)];
    untrack(() => void cargarInformes(cc, ids));
  });
  const informes = $derived(ultimos.cliente === c ? ultimos.porEquipo : {});

  // Las cuatro cifras de arriba.
  const alDia = $derived(saludes.filter((x) => x.tono === "ok").length);
  const recientes = $derived(ultimas24h(delFiltro, informes, reloj.ahora));
  const proxima = $derived(proximaDeTodos(delFiltro, reloj.ahora));
  /** Dónde se guarda y cuánto, con la tendencia de 30 días. */
  const destinos = $derived(almacenesDe(delFiltro, informes, 30, reloj.ahora));
  const ICONO_DESTINO = { almacen: Server, rest: Server, local: HardDrive, s3: Cloud, b2: Cloud, sftp: Server, nube: Cloud, otro: Database };
  const TIPO_DESTINO = { almacen: "Almacén", rest: "Servidor de copias", local: "Disco del equipo", s3: "S3", b2: "Backblaze B2", sftp: "SFTP", nube: "Nube", otro: "Otro destino" };
  const fmtDiaCorto = new Intl.DateTimeFormat("es", { day: "numeric", month: "short" });
  const etiquetaDia = (k: string) => fmtDiaCorto.format(new Date(`${k}T12:00:00`));
  /** Cuánto creció en el periodo, en texto («+1,2 GB en 30 días»). */
  function crecimiento(serie: { bytes: number }[]) {
    const d = (serie.at(-1)?.bytes ?? 0) - (serie[0]?.bytes ?? 0);
    if (!serie.length || Math.abs(d) < 1) return "sin cambios en 30 días";
    return `${d > 0 ? "+" : "−"}${bytes(Math.abs(d))} en 30 días`;
  }

  /** Repositorios de todos los equipos (no los trasladados): lo que falla, primero. */
  const tarjetas = $derived(
    delFiltro
      .filter((e) => e.modo !== "trasladado")
      .flatMap((e) => (e.resumen?.repositorios ?? []).map((r) => ({ equipo: e, repo: r, peso: PESO[saludEquipo(e, reloj.ahora).tono] })))
      .sort((a, b) => a.peso - b.peso || a.repo.nombre.localeCompare(b.repo.nombre)),
  );

  /** v1.4x: el almacén de otra consola que sale en el mapa («Conectar también…»). */
  let fuera = $state<{ almacen: string; consolas: string[] } | null>(null);
  let copiar = $state<{ equipo: Equipo; repo: string; copia: string; nombre: string } | null>(null);
</script>

<svelte:head><title>Estado · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  {#if !actual.cargado || !actual.cliente}
    <Esqueleto forma="cifras" n={3} etiqueta="Cargando el estado…" />
    <Esqueleto forma="filas" n={4} />
  {:else}
    <Observaciones tipo="cliente" objeto={actual.cliente.id} />
    <section class="card resumen tone-{tonoTitular}" aria-labelledby="titular">
      <Anuncio texto={titular} />
      <div class="cab">
        <span class="icono">
          {#if tonoTitular === "ok"}<CircleCheck size={28} />{:else if tonoTitular === "bad"}<CircleAlert size={28} />{:else if tonoTitular === "warn"}<TriangleAlert size={28} />{:else}<Monitor size={28} />{/if}
        </span>
        <div>
          <h1 id="titular">{titular}</h1>
          <p>
            {#if delFiltro.length}
              {#if pistaPrimera}
                {pistaPrimera}
              {:else}
                {plural(delFiltro.length, "equipo", "equipos")} · {#if ultima}última copia <Tiempo iso={ultima} />{:else}todavía sin copias{/if} · {plural(repos.length, "repositorio", "repositorios")}{#if protegido}{" "}con {bytes(protegido)}{/if}
              {/if}
            {:else}
              Añade el primer equipo de {actual.cliente.nombre}: instala el agente y escribe el código que te daremos.
            {/if}
          </p>
        </div>
      </div>
      {#if urgentes.length}
        <ul class="urgentes">
          {#each urgentes.slice(0, 6) as u, i (i)}
            <li>
              <span class="u-ic tone-{u.tono}" aria-hidden="true">{#if u.tono === "bad"}<CircleAlert size={16} />{:else}<TriangleAlert size={16} />{/if}</span>
              <span class="texto">{#if u.titulo}<strong>{u.titulo}</strong><span class="det">{u.detalle}</span>{:else}{u.texto}{/if}</span>
              {#if u.accion?.href}
                <a class="btn btn-sm" href={u.accion.href}>{u.accion.texto}</a>
              {:else if u.accion?.orden}
                <button class="btn btn-sm" onclick={() => (copiar = u.accion!.orden!)}>{u.accion.texto}</button>
              {/if}
            </li>
          {/each}
        </ul>
      {:else if !actual.equipos.length && puede.administrar(actual.cliente.rol)}
        <a class="btn btn-primary" href="/c/{c}/emparejar"><Plus size={16} />Añadir equipo</a>
      {/if}
    </section>

    {#if delFiltro.length}
      <div class="cifras" role="list" aria-label="Cifras del cliente">
        <div class="cifra" role="listitem">
          <span class="c-et"><span class="c-ic" aria-hidden="true"><ShieldCheck size={14} /></span>Equipos al día</span>
          <span class="c-val num">{alDia}<small>{` de ${delFiltro.length}`}</small></span>
          <span class="barra" aria-hidden="true">
            {#each ["ok", "warn", "bad", "paused", "neutral"] as const as t (t)}
              {@const n = saludes.filter((x) => x.tono === t).length}
              {#if n}<span style:flex={n} style="--tone: var(--{t})"></span>{/if}
            {/each}
          </span>
          <span class="c-sub">{cuenta("bad") ? plural(cuenta("bad"), "con un problema", "con problemas") : cuenta("warn") ? plural(cuenta("warn"), "con avisos", "con avisos") : "ninguno con problemas"}</span>
        </div>
        <div class="cifra" role="listitem">
          <span class="c-et"><span class="c-ic" aria-hidden="true"><Database size={14} /></span>Protegido</span>
          <span class="c-val num">{protegido ? bytes(protegido) : "—"}</span>
          <span class="c-sub">en {plural(repos.length, "repositorio", "repositorios")}</span>
        </div>
        <div class="cifra" role="listitem">
          <span class="c-et"><span class="c-ic" aria-hidden="true"><Archive size={14} /></span>Versiones en 24 h</span>
          <span class="c-val num">{numero(recientes.versiones)}</span>
          <span class="c-sub">{#if recientes.fallos}<span class="mal">{plural(recientes.fallos, "copia fallida", "copias fallidas")}</span>{:else}sin copias fallidas{/if}</span>
        </div>
        <div class="cifra" role="listitem">
          <span class="c-et"><span class="c-ic" aria-hidden="true"><CalendarClock size={14} /></span>Próxima copia</span>
          <span class="c-val" use:tip={proxima ? cuandoFrase(proxima.cuando, reloj.ahora) : undefined}>{proxima ? relativo(proxima.cuando, reloj.ahora) : "—"}</span>
          <span class="c-sub">{proxima ? `${proxima.copia} · ${proxima.equipo.nombre}` : "nada programado"}</span>
        </div>
      </div>
    {/if}

    <PrimerosPasos cliente={actual.cliente} equipos={actual.equipos} {informes} />
    {#if delFiltro.length}<ReglaCliente cliente={c} equipos={delFiltro} todos={actual.equipos} {informes} ahora={reloj.ahora} />{/if}

    <Pendientes />

    <AvisoConsolas cliente={actual.cliente} equipos={actual.equipos} ahora={reloj.ahora} />

    <FiltroEtiquetas />

    {#if delFiltro.length}
      <MapaProteccion equipos={delFiltro} todos={actual.equipos} {informes} cliente={c} ahora={reloj.ahora} alConectarFuera={(n) => (fuera = n.fuera ?? null)} />

      <section>
        <div class="section-head">
          <h2>Equipos <span class="count">· {delFiltro.length}</span></h2>
          <span class="acciones-eq">
            {#if hayEtiquetas && !filtroEtiqueta.valor}<label class="agrupar"><input type="checkbox" checked={agruparPorEtiqueta.valor} onchange={(e) => agruparPorEtiqueta.poner(e.currentTarget.checked)} />Agrupar por etiqueta</label>{/if}
            {#if puede.administrar(actual.cliente.rol)}<a class="btn btn-sm btn-ghost" href="/c/{c}/emparejar"><Plus size={14} />Añadir equipo</a>{/if}
          </span>
        </div>
        {#if grupos}
          {#each grupos as g (g.etiqueta ?? "")}
            {@const n = cuentaGrupo(g.equipos)}
            <div class="cab-grupo">
              {#if g.etiqueta}<button type="button" class="sin-boton" onclick={() => filtroEtiqueta.poner(g.etiqueta!)} use:tip={`Ver solo los de «${g.etiqueta}»`}><EtiquetaChip nombre={g.etiqueta} /></button>{:else}<span class="sin-et">Sin etiqueta</span>{/if}
              <span class="faint">
                {plural(g.equipos.length, "equipo", "equipos")}{#if n.mal}{" · "}<span class="g-mal"><CircleAlert size={12} />{plural(n.mal, "con problemas", "con problemas")}</span>{/if}{#if n.atencion}{" · "}<span class="g-aviso"><TriangleAlert size={12} />{plural(n.atencion, "necesita atención", "necesitan atención")}</span>{/if}{#if n.bien === g.equipos.length}{" · "}<span class="g-ok"><CircleCheck size={12} />todos bien</span>{/if}
              </span>
            </div>
            <div class="rejilla equipos">
              {#each g.equipos as e (e.id)}<TarjetaEquipo equipo={e} informe={informes[e.id]} cliente={c} ahora={reloj.ahora} />{/each}
            </div>
          {/each}
        {:else}
          <div class="rejilla equipos">
            {#each equipos as e (e.id)}<TarjetaEquipo equipo={e} informe={informes[e.id]} cliente={c} ahora={reloj.ahora} />{/each}
          </div>
        {/if}
      </section>

      {#if tarjetas.length}
        <section>
          <div class="section-head">
            <h2>Repositorios <span class="count">· {tarjetas.length}</span> <Ayuda id="repositorio" /></h2>
            <a class="btn btn-sm btn-ghost" href="/c/{c}/repositorios">Ver todos</a>
          </div>
          <div class="rejilla repos">
            {#each tarjetas as t (t.equipo.id + t.repo.id)}
              <TarjetaRepo repo={t.repo} inf={informeDe(informes[t.equipo.id], t.repo.id)} equipo={t.equipo} cliente={c} ahora={reloj.ahora} />
            {/each}
          </div>
        </section>
      {/if}
    {:else}
      <div class="card">
        <Vacio icono={Database} ilustracion="bienvenida" titulo="Las copias aparecerán aquí" texto="Cuando un equipo se conecte, verás sus copias, su estado y lo que necesite atención." />
      </div>
    {/if}

    {#if destinos.length}
      <section>
        <div class="section-head">
          <h2>Dónde se guarda <span class="count">· {destinos.length}</span></h2>
          <a class="btn btn-sm btn-ghost" href="/c/{c}/repositorios">Repositorios y destinos<ChevronRight size={14} /></a>
        </div>
        <div class="card p-0 lista">
          {#each destinos as d (d.clave)}
            {@const Ic = ICONO_DESTINO[d.tipo]}
            <svelte:element this={d.equipo ? "a" : "div"} class="fila destino" href={d.equipo ? `/c/${c}/equipos/${d.equipo}` : undefined}>
              <span class="d-ic"><Ic size={16} /></span>
              <span class="fila-texto">
                <span class="fila-titulo">{d.nombre}</span>
                <span class="fila-sub">{#if d.nombre !== TIPO_DESTINO[d.tipo]}{TIPO_DESTINO[d.tipo]}{" · "}{/if}{#if d.donde && d.donde !== d.nombre}<span class="pastilla mono" title={d.donde}>{d.donde}</span>{" · "}{/if}{plural(d.repos, "repositorio", "repositorios")}{#if d.inmutable || d.tipo === "almacen"}<span class="candado">{" · "}<Lock size={11} />{d.tipo === "almacen" ? "solo añadir" : "inmutable"}</span>{/if}</span>
              </span>
              <span class="d-graf">
                <Sparkline valores={d.serie.map((x) => x.bytes)} etiquetas={d.serie.map((x) => etiquetaDia(x.dia))} formato={bytes} titulo="Datos protegidos en {d.nombre}, últimos 30 días" alto={30} />
              </span>
              <span class="d-num num">
                <strong>{bytes(d.protegido)}</strong>
                <span>{d.enDisco != null ? `${bytes(d.enDisco)} en disco · ` : ""}{crecimiento(d.serie)}</span>
              </span>
            </svelte:element>
          {/each}
        </div>
      </section>
    {/if}

    {#if delFiltro.length}<CuandoSeLlena equipos={delFiltro} todos={actual.equipos} {informes} cliente={c} ahora={reloj.ahora} />{/if}

    <Comentarios tipo="cliente" objeto={actual.cliente.id} titulo="Comentarios del cliente" />

    <p class="privacidad"><LockKeyhole size={12} />El servidor no puede leer tus archivos ni tus contraseñas: van cifrados entre este navegador y cada equipo.</p>
  {/if}
</div>

{#if fuera && actual.cliente}<AlmacenEnOtraConsola cliente={actual.cliente} almacen={fuera.almacen} consolas={fuera.consolas} onclose={() => (fuera = null)} />{/if}

{#if copiar && actual.cliente}
  {#key copiar}
  <OrdenDialog
    cliente={actual.cliente}
    equipo={copiar.equipo}
    tipo="copiar_ahora"
    cuerpo={{ repo: copiar.repo, copia: copiar.copia }}
    descripcion="Se hará ahora la copia «{copiar.nombre}», sin esperar a su hora. No borra nada."
    accion="Copiar ahora"
    onclose={() => (copiar = null)}
  />
  {/key}
{/if}

<style>
  .acciones-eq {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }
  .agrupar {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
    color: var(--text-2);
    cursor: pointer;
  }
  .cab-grupo {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    margin: var(--sp-4) 0 var(--sp-2);
    font-size: var(--fs-sm);
  }
  .cab-grupo:first-of-type {
    margin-top: 0;
  }
  .sin-boton {
    padding: 0;
    font: inherit;
    background: none;
    border: none;
    cursor: pointer;
  }
  .sin-et {
    font-weight: 500;
    color: var(--text-2);
  }
  .g-mal,
  .g-aviso,
  .g-ok {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .g-mal {
    color: var(--bad);
  }
  .g-aviso {
    color: var(--warn);
  }
  .g-ok {
    color: var(--ok);
  }
  .resumen {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    padding: var(--sp-6);
    border-radius: var(--radius-xl);
  }
  .cab {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-4);
  }
  .icono {
    display: grid;
    place-items: center;
    flex: none;
    width: 52px;
    height: 52px;
    color: var(--tone);
    background: color-mix(in srgb, var(--tone) var(--soft), transparent);
    border-radius: var(--radius-lg);
  }
  h1 {
    font-size: var(--fs-display);
    line-height: var(--lh-display);
    font-weight: 650;
    letter-spacing: -0.022em;
  }
  .cab p {
    margin: 6px 0 0;
    color: var(--text-2);
  }
  .urgentes {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .urgentes li {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: 10px 0;
    border-top: 1px solid var(--border);
  }
  .urgentes .texto {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .urgentes strong {
    font-weight: 500;
  }
  .urgentes .det {
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
    color: var(--text-2);
  }
  .urgentes li {
    align-items: flex-start;
  }
  .u-ic {
    display: grid;
    flex: none;
    margin-top: 2px;
    color: var(--tone);
  }
  .rejilla.repos {
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 340px), 1fr));
  }
  .rejilla.equipos {
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 250px), 1fr));
  }

  /* Las cifras (.cifras, .cifra, .c-et…) son globales: app.css. */
  /* Reparto de los equipos por estado: una barra fina (el texto de al lado lo dice en palabras). */
  .barra {
    display: flex;
    gap: 2px;
    height: 4px;
    margin: 6px 0 4px;
    overflow: hidden;
    border-radius: 999px;
  }
  .barra > span {
    background: var(--tone);
  }

  /* Dónde se guarda: nombre, minigráfica de 30 días y las cifras a la derecha. */
  .destino {
    gap: var(--sp-4);
    min-height: 60px;
  }
  .d-ic {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    color: var(--text-2);
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .candado :global(svg) {
    margin: 0 3px 0 1px;
    vertical-align: -1px;
  }
  .d-graf {
    flex: none;
    width: 160px;
  }
  .d-num {
    display: flex;
    flex: none;
    flex-direction: column;
    align-items: flex-end;
    min-width: 150px;
    text-align: right;
  }
  .d-num strong {
    font-weight: 600;
  }
  .d-num span {
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    color: var(--text-3);
  }
  @media (max-width: 900px) {
    .d-graf {
      width: 100px;
    }
  }
  @media (max-width: 640px) {
    .destino {
      flex-wrap: wrap;
      gap: var(--sp-2) var(--sp-3);
    }
    .destino .fila-texto {
      flex-basis: calc(100% - 48px);
    }
    .d-graf {
      flex: 1;
      margin-left: 44px;
    }
    .d-num {
      min-width: 0;
    }
    .resumen {
      padding: var(--sp-5);
    }
    .icono {
      width: 40px;
      height: 40px;
    }
    .icono :global(svg) {
      width: 22px;
      height: 22px;
    }
    .urgentes li {
      flex-wrap: wrap;
    }
    .urgentes .texto {
      flex-basis: calc(100% - 28px);
    }
    .urgentes .btn {
      margin-left: 28px;
    }
  }
</style>
