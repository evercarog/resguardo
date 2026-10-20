<script lang="ts">
  // 15g: la página de una copia derivada (como la de cada espejo): de qué repositorio viene,
  // dónde está (destino y carpeta), sus versiones, su última subida, sus comprobaciones, su
  // retención y, si va a un almacén, «Retención en el almacén…» (sin ella, crece sin límite).
  // Llegan aquí los enlaces del camino de la copia y de su línea en «Repositorios» del equipo.
  // `d` es el id de la derivada (`externa`: la de cada día de antes).
  import { page } from "$app/state";
  import { untrack } from "svelte";
  import { ArrowRight, CalendarClock, CloudUpload, Database, FolderOpen, Gauge, History, Info, Play, Settings2, ShieldAlert, ShieldCheck, Trash2, TriangleAlert } from "@lucide/svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import Cifra from "$lib/componentes/Cifra.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";
  import EnMarcha from "$lib/componentes/EnMarcha.svelte";
  import OrdenDialog from "$lib/componentes/OrdenDialog.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import RetencionAlmacen from "$lib/componentes/RetencionAlmacen.svelte";
  import * as api from "$lib/api";
  import { actual, puede, reloj } from "$lib/estado.svelte";
  import { seguirCambios, tocaEquipo } from "$lib/vivo.svelte";
  import { fechaLarga, numero, relativo } from "$lib/formato";
  import { informeDe, TEXTO_TAREA, TONO_TAREA } from "$lib/repo";
  import { cuandoEnFrase, filtroEnFrase } from "$lib/cadenas";
  import { detallesExterna, ordenQuitarExterna } from "$lib/copiaExterna";
  import { franjasEnFrase } from "$lib/franjas";
  import { textoDestinoAusente } from "$lib/datosEquipo";
  import { almacenDeDerivada, textoHorario, textoRegla } from "$lib/retencion";
  import { hrefDestino } from "$lib/fichaDestino";
  import type { EquipoDetalle, TareaInforme } from "$lib/tipos";

  const c = $derived(page.params.c ?? "");
  const e = $derived(page.params.e ?? "");
  const rid = $derived(page.params.r ?? "");
  const did = $derived(page.params.d ?? "");

  let equipo = $state<EquipoDetalle | null>(null);
  let error = $state("");
  $effect(() => {
    const [cc, ee] = [c, e];
    untrack(() => cargar(cc, ee));
  });
  function cargar(cc = c, ee = e) {
    api.equipo(cc, ee).then((x) => ee === e && ((equipo = x), (error = "")), (x) => (error = (x as Error).message));
  }
  $effect(() => {
    const [cc, ee] = [c, e];
    return untrack(() =>
      seguirCambios(() => api.equipo(cc, ee).then((x) => ee === e && (equipo = x), () => {}), {
        ms: 0,
        toca: (x) => (x.t === "informe" || x.t === "config" || x.t === "equipo" || x.t === "orden") && tocaEquipo(x, ee),
      }),
    );
  });

  const repo = $derived(equipo?.resumen?.repositorios?.find((r) => r.id === rid) ?? null);
  const destinos = $derived(equipo?.resumen?.destinos ?? []);
  const inf = $derived(informeDe(equipo?.ultimo_informe, rid));
  /** La derivada, como la de cada día de antes (`externa`) o una de la lista. */
  const dv = $derived.by(() => {
    if (!repo) return null;
    if (did === "externa") {
      const x = repo.externa;
      return x ? { id: "externa", destino: x.destino ?? null, destino_id: x.destino_id ?? null, cuando: null, hora: x.hora, existente: x.existente, bloqueo_dias: x.bloqueo_dias, solo_anadir: x.solo_anadir, filtro: null, limite: null, con_retencion: x.con_retencion ?? null, ruta: null } : null;
    }
    const x = (repo.derivadas ?? []).find((y) => y.id === did);
    return x ? { ...x, hora: null as string | null } : null;
  });
  type Ultima = Partial<TareaInforme> & { versiones?: number | null; ultima_bien?: string | null; verificacion?: TareaInforme | null };
  const ultima = $derived<Ultima | null>(did === "externa" ? ((inf?.externa as Ultima | null | undefined) ?? null) : (inf?.derivadas?.find((x) => x.id === did) ?? null));
  const nombreDestino = $derived(dv?.destino ?? "otro destino");
  const titulo = $derived(dv ? `Copia derivada a «${nombreDestino}»` : "Copia derivada");
  const rol = $derived(equipo?.modo === "trasladado" ? "lectura" : actual.cliente?.rol);
  const carpeta = $derived(dv ? dv.ruta || (did === "externa" ? rid : `${rid}-${did}`) : "");
  const enAlm = $derived(repo && dv ? almacenDeDerivada(repo, { id: did, destino_id: dv.destino_id, ruta: dv.ruta }, destinos, actual.equipos) : null);
  const admiteRetDerivada = $derived(!!equipo?.resumen?.admite?.includes("retencion_derivada"));
  const estado = $derived.by(() => {
    if (!ultima?.resultado) return { tono: "neutral" as const, texto: "Todavía sin ninguna subida" };
    if (ultima.pausada) return { tono: "paused" as const, texto: "En pausa" };
    return { tono: TONO_TAREA[ultima.resultado], texto: ultima.resultado === "ok" ? "Al día" : TEXTO_TAREA[ultima.resultado] };
  });
  const cuando = $derived(dv ? (dv.hora ? `Cada día a las ${dv.hora}` : cuandoEnFrase(dv.cuando)) : "");

  let dialogo = $state<{ tipo: string; cuerpo: Record<string, unknown>; titulo?: string; descripcion: string; accion?: string } | null>(null);
  let retAlm = $state(page.url.searchParams.get("retencion") === "1");
  function subirAhora() {
    if (!repo) return;
    dialogo = { tipo: "subir_ahora", cuerpo: { repo: rid, ...(did === "externa" ? {} : { derivada: did }) }, descripcion: `Se subirán ahora las versiones nuevas de «${repo.nombre}» a «${nombreDestino}».`, accion: "Subir ahora" };
  }
  /** 14nn: también aquí, sobre todo si su destino ya no existe. */
  function quitar() {
    if (!repo) return;
    dialogo =
      did === "externa"
        ? ordenQuitarExterna(repo, destinos)
        : { tipo: "quitar_derivada", cuerpo: { repo: rid, id: did }, titulo: "Quitar la copia derivada", descripcion: `«${repo.nombre}» dejará de copiarse a «${nombreDestino}». Lo ya copiado allí se queda.` };
  }
</script>

<svelte:head><title>{titulo} · {repo?.nombre ?? "Repositorio"} · Resguardo Server</title></svelte:head>

<div class="page">
  {#if !equipo || !actual.cliente}
    {#if error}<div class="notice notice-danger"><p>{error}</p></div>{:else}<Cargando forma="ficha" filas={3} />{/if}
  {:else if !repo || !dv}
    <CabeceraPagina titulo="Esta copia derivada ya no está" icono={CloudUpload} migas={[{ texto: "Equipos", href: `/c/${c}/equipos` }, { texto: equipo.nombre, href: `/c/${c}/equipos/${e}` }]} />
    <div class="notice notice-info"><Info size={16} /><p>Puede que se quitara. Las demás están en <a href="/c/{c}/equipos/{e}">la ficha de {equipo.nombre}</a>.</p></div>
  {:else}
    <CabeceraPagina
      {titulo}
      icono={CloudUpload}
      migas={[
        { texto: "Equipos", href: `/c/${c}/equipos` },
        { texto: equipo.nombre, href: `/c/${c}/equipos/${e}` },
        { texto: repo.nombre, href: `/c/${c}/equipos/${e}/repositorios/${encodeURIComponent(rid)}` },
        { texto: `Copia derivada a «${nombreDestino}»` },
      ]}
    >
      {#snippet detalle()}
        <p class="linea">
          <Chip tono={estado.tono} texto={estado.texto} pequeno />
          <a class="link-suave" href="/c/{c}/equipos/{e}/repositorios/{encodeURIComponent(rid)}">{repo.nombre}</a>
          <ArrowRight size={14} class="faint" aria-hidden="true" />
          {#if dv.destino_id}<a class="link-suave" href={hrefDestino(c, dv.destino_id)}>{nombreDestino}</a>{:else}<span>{nombreDestino}</span>{/if}
          <span class="faint">· {cuando.toLowerCase()}</span>
        </p>
      {/snippet}
      {#snippet acciones()}
        {#if puede.ordenar(rol) && !repo.solo_lectura}
          <button class="btn btn-primary" onclick={subirAhora}><Play size={15} />Subir ahora</button>
          <a class="btn" href="/c/{c}/equipos/{e}{did === 'externa' ? `?externa=${encodeURIComponent(rid)}` : ''}"><Settings2 size={15} />Cambiar</a>
          <button class="btn btn-ghost" onclick={quitar}><Trash2 size={15} />Quitar</button>
        {/if}
      {/snippet}
    </CabeceraPagina>

    <EnMarcha equipo={e} repo={rid} tipos={["copia_externa", "verificar_externa"]} marco alTerminar={() => cargar()} />

    {#if ultima?.motivo === "destino_ausente"}
      <!-- 14nn: su destino ya no existe: quitarla (aquí también) o volver a crearla desde cero. -->
      <div class="notice notice-danger" role="note">
        <TriangleAlert size={16} />
        <p>
          {textoDestinoAusente(ultima)}
          {#if puede.ordenar(rol)}<button type="button" class="btn btn-sm" onclick={quitar}>Quitar esta copia</button><a class="btn btn-sm btn-ghost" href="/c/{c}/equipos/{e}">Volver a crearla desde cero…</a>{/if}
        </p>
      </div>
    {:else if ultima?.resultado === "fallo" && ultima.mensaje_corto}
      <div class="notice notice-danger"><ShieldAlert size={16} /><p>{ultima.mensaje_corto}</p></div>
    {/if}
    {#if dv.activa === false}
      <div class="notice notice-warn"><Info size={16} /><p>Ahora no se hace: «{repo.nombre}» no tiene ninguna copia activa. Vuelve a hacerse en cuanto la tenga.</p></div>
    {/if}

    <div class="cifras" role="list" aria-label="Cifras de la copia derivada">
      <Cifra icono={History} etiqueta="Última subida" valor={ultima?.ultima ? relativo(ultima.ultima, reloj.ahora) : "Todavía no"} detalle={ultima?.ultima ? fechaLarga(ultima.ultima) : undefined} sub={ultima?.ultima_bien && ultima.ultima_bien !== ultima.ultima ? `la última bien, ${relativo(ultima.ultima_bien, reloj.ahora)}` : (estado.texto.toLowerCase())} mal={ultima?.resultado === "fallo"} />
      <Cifra icono={CalendarClock} etiqueta="Cuándo" valor={dv.hora ?? dv.cuando?.hora ?? (dv.cuando?.tras_copia ? "Tras cada copia" : "Con horario")} sub={cuando} />
      <Cifra icono={Database} etiqueta="Versiones" valor={ultima?.versiones != null ? numero(ultima.versiones) : "—"} sub={ultima?.versiones != null ? "en su repositorio, tras la última subida" : "se sabrán tras la próxima subida"} />
      <Cifra
        icono={ShieldCheck}
        etiqueta="Comprobación"
        valor={ultima?.verificacion?.ultima ? TEXTO_TAREA[ultima.verificacion.resultado] : "—"}
        detalle={ultima?.verificacion?.ultima ? fechaLarga(ultima.verificacion.ultima) : undefined}
        sub={ultima?.verificacion?.ultima ? relativo(ultima.verificacion.ultima, reloj.ahora) : dv.verificacion ? "programada" : "sin comprobación automática"}
        mal={ultima?.verificacion?.resultado === "fallo"}
      />
    </div>

    <section class="card p" aria-labelledby="t-donde">
      <h2 class="section-title" id="t-donde"><FolderOpen size={16} />Dónde está</h2>
      <dl class="datos">
        <div><dt>De qué repositorio viene</dt><dd><a class="link-suave" href="/c/{c}/equipos/{e}/repositorios/{encodeURIComponent(rid)}">{repo.nombre}</a>, de {equipo.nombre}{#if filtroEnFrase(dv.filtro)}<span class="faint sub">Solo las versiones {filtroEnFrase(dv.filtro)}</span>{/if}</dd></div>
        <div><dt>Destino</dt><dd>{#if dv.destino_id}<a class="link-suave" href={hrefDestino(c, dv.destino_id)}>{nombreDestino}</a>{:else}{nombreDestino}{/if}{#each detallesExterna(dv) as x (x)}<span class="faint sub">{x}</span>{/each}</dd></div>
        <div><dt>Su carpeta en el destino</dt><dd class="mono">{carpeta}</dd></div>
        {#if dv.limite}<div><dt><Gauge size={13} />Velocidad</dt><dd>{franjasEnFrase(dv.limite.franjas, dv.limite.kib)}</dd></div>{/if}
      </dl>
    </section>

    <section class="card p" aria-labelledby="t-ret">
      <h2 class="section-title" id="t-ret">Retención <Ayuda id="retencion-almacen" /></h2>
      {#if enAlm}
        {#if enAlm.retencion}
          {@const ra = enAlm.retencion}
          <p>{ra.texto ?? textoRegla(ra.retencion)}</p>
          <p class="faint">La aplica {enAlm.almacen.nombre} {ra.horario_texto ?? textoHorario(ra.horario)}{ra.verificar ? " y después comprueba el repositorio" : ""}.{#if ra.ultima}{" "}La última vez, <Tiempo iso={ra.ultima} />.{/if}</p>
          {#if ra.clave === "pendiente"}<p class="aviso">Su clave aún no abre el repositorio: vuelve a guardarla con {equipo.nombre} encendido.</p>{/if}
        {:else}
          <p class="aviso"><TriangleAlert size={14} />Va a {enAlm.almacen.nombre}, que es de solo añadir, y no tiene retención: crece sin límite.</p>
        {/if}
        {#if puede.administrar(rol) && enAlm.admite}
          {#if admiteRetDerivada}
            <button class="btn btn-sm" onclick={() => (retAlm = true)}>{enAlm.retencion ? "Cambiar…" : "Retención en el almacén…"}</button>
          {:else}
            <p class="faint">Para ponerla, actualiza el agente de {equipo.nombre}.</p>
          {/if}
        {/if}
      {:else if dv.con_retencion}
        <p>La aplica {equipo.nombre} en el repositorio de la copia derivada, tras cada subida.</p>
      {:else if dv.bloqueo_dias || dv.solo_anadir}
        <p class="faint">Su destino es de solo añadir o tiene bloqueo: desde {equipo.nombre} no se borra nada.</p>
      {:else}
        <p class="faint">Sin retención: guarda todas las versiones que sube. Se pone en «Cambiar».</p>
      {/if}
    </section>
  {/if}
</div>

{#if dialogo && equipo && actual.cliente && repo}
  <OrdenDialog
    cliente={actual.cliente}
    {equipo}
    tipo={dialogo.tipo}
    cuerpo={dialogo.cuerpo}
    titulo={dialogo.titulo}
    descripcion={dialogo.descripcion}
    accion={dialogo.accion}
    repo={{ id: repo.id, nombre: repo.nombre }}
    onclose={() => ((dialogo = null), cargar())}
  />
{/if}

{#if retAlm && enAlm && equipo && actual.cliente && repo && dv}
  <RetencionAlmacen cliente={actual.cliente} {equipo} {repo} en={enAlm} derivada={{ id: did, nombre: nombreDestino }} onclose={() => ((retAlm = false), cargar())} />
{/if}

<style>
  .linea {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
  }
  .cifras {
    margin-top: var(--sp-4);
  }
  section {
    margin-top: var(--sp-5);
  }
  .datos {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: var(--sp-3) var(--sp-6);
    margin: 0;
  }
  dt {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--text-3);
  }
  dd {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
    overflow-wrap: anywhere;
  }
  .sub {
    display: block;
    font-size: var(--fs-xs);
  }
  .mono {
    font-family: var(--mono);
  }
  .aviso {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--warn);
  }
  section p {
    margin: 0 0 var(--sp-2);
  }
  .notice p {
    margin: 0;
  }
  .notice .btn {
    margin-left: var(--sp-2);
  }
</style>
