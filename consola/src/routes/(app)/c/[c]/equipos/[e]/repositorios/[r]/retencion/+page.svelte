<script lang="ts">
  // «Retención en detalle» de un repositorio: lo que se eliminó (cada vuelta de
  // la retención que anotó el equipo, su almacén o su copia externa, con las
  // versiones que quitó) y lo que se eliminará (simulado con la regla de ahora:
  // la próxima vuelta y los días que vienen), con un calendario de las dos
  // cosas y filtros por copia. Desde «Ver en detalle» en la página del
  // repositorio, la de la copia y Ctrl+K.
  import { page } from "$app/state";
  import { untrack } from "svelte";
  import { CalendarClock } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { actual, reloj } from "$lib/estado.svelte";
  import { bytes, numero, plural } from "$lib/formato";
  import { horasDelDia, reglasDe } from "$lib/horario";
  import { reglaEfectiva } from "$lib/lineaTiempo";
  import { destinoDe, informeDe, nVersiones, versionesDe } from "$lib/repo";
  import { almacenDe, resumenRegla, textoHorario } from "$lib/retencion";
  import { marcasPorDia, prever, totales, vueltasDelRepo, type EntradaRetencion } from "$lib/retencionDetalle";
  import { leerRetenciones } from "$lib/retencionHistorial";
  import type { EquipoDetalle, Horario } from "$lib/tipos";
  import { seguirCambios, tocaEquipo } from "$lib/vivo.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import IndicePagina from "$lib/componentes/IndicePagina.svelte";
  import Migas from "$lib/componentes/Migas.svelte";
  import CalendarioRetencion from "$lib/componentes/retencion/CalendarioRetencion.svelte";
  import PrevisionRetencion from "$lib/componentes/retencion/PrevisionRetencion.svelte";
  import VueltasRetencion from "$lib/componentes/retencion/VueltasRetencion.svelte";

  const c = $derived(page.params.c ?? "");
  const e = $derived(page.params.e ?? "");
  const rid = $derived(page.params.r ?? "");

  let equipo = $state<EquipoDetalle | null>(null);
  let error = $state("");
  $effect(() => {
    const [cc, ee] = [c, e];
    return untrack(() => {
      equipo = null;
      const leerEquipo = () =>
        api.equipo(cc, ee).then(
          (x) => {
            if (ee === e) [equipo, error] = [x, ""];
          },
          (x) => (error = (x as Error).message),
        );
      void leerEquipo();
      return seguirCambios(leerEquipo, {
        ms: 0,
        toca: (x) => (x.t === "informe" || x.t === "config" || x.t === "equipo" || (x.t === "progreso" && x.estado === "termina")) && tocaEquipo(x, ee),
      });
    });
  });

  const repo = $derived(equipo?.resumen?.repositorios?.find((r) => r.id === rid));
  const inf = $derived(informeDe(equipo?.ultimo_informe, rid));
  const copias = $derived((equipo?.resumen?.copias ?? []).filter((k) => k.repo === rid));
  const destino = $derived(destinoDe(equipo?.resumen?.destinos, repo));
  const enAlm = $derived(repo ? almacenDe(repo, destino, actual.equipos) : null);
  const efectiva = $derived(reglaEfectiva(repo, destino, actual.equipos));
  const versiones = $derived(versionesDe(inf).map((v) => ({ id: v.id, hora: v.hora, copia: v.copia, bytes: v.total_bytes })));
  const nombreCopia = (id: string | null) => (id ? (copias.find((k) => k.id === id)?.nombre ?? id) : null);

  // Las vueltas: las del equipo (y su copia externa) y las de su almacén.
  let propias = $state<EntradaRetencion[]>([]);
  let delAlmacen = $state<EntradaRetencion[]>([]);
  let leidas = $state(false);
  let errorVueltas = $state("");
  const almId = $derived(enAlm?.almacen.id ?? null);
  $effect(() => {
    const [cc, ee, aa] = [c, e, almId];
    return untrack(() => {
      leidas = false;
      const leer = () =>
        Promise.all([leerRetenciones(cc, ee), aa ? leerRetenciones(cc, aa) : Promise.resolve([] as EntradaRetencion[])]).then(
          ([p, a]) => {
            if (ee !== e) return;
            [propias, delAlmacen, leidas, errorVueltas] = [p, a, true, ""];
          },
          (x) => ([errorVueltas, leidas] = [(x as Error).message, true]),
        );
      void leer();
      return seguirCambios(leer, { ms: 0, toca: (x) => x.t === "historial" && (tocaEquipo(x, ee) || (!!aa && tocaEquipo(x, aa))) });
    });
  });
  const copiaDe = (id: string) => versiones.find((v) => v.id === id)?.copia ?? null;
  const vueltas = $derived(vueltasDelRepo({ propias, delAlmacen, repo: rid, enAlmacen: enAlm ? { usuario: enAlm.usuario, carpeta: enAlm.carpeta } : null, copiaDe }));
  const cifras = $derived(totales(vueltas));

  // La simulación.
  let dias = $state<7 | 30>(30);
  const proxima = $derived(enAlm?.retencion?.proxima ? Date.parse(enAlm.retencion.proxima) : null);
  const horasDe = (k: string | null) => {
    const copia = copias.find((x) => x.id === k && x.activa !== false && x.horario && typeof x.horario === "object");
    const reglas = copia ? reglasDe(copia.horario as Horario) : [];
    return reglas.length ? (f: Date) => horasDelDia(reglas, f) : null;
  };
  const prevision = $derived(
    efectiva && repo ? prever({ versiones, total: nVersiones(repo, inf), regla: efectiva.regla, ahora: reloj.ahora, proxima, dias, horasDe }) : null,
  );
  const marcas = $derived(marcasPorDia(vueltas, prevision, reloj.ahora));

  // Filtros: una copia y un día (del calendario).
  let copia = $state<string | null>(null);
  let dia = $state<string | null>(null);
  const copiasFiltro = $derived([...new Set([...copias.map((k) => k.id), ...versiones.map((v) => v.copia).filter((x): x is string => !!x)])]);
  const fmtCuando = new Intl.DateTimeFormat("es", { weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
</script>

<svelte:head><title>Retención en detalle · {repo?.nombre ?? "Repositorio"} · Resguardo Server</title></svelte:head>

<div class="page">
  <Migas
    items={[
      { texto: "Equipos", href: `/c/${c}/equipos` },
      { texto: equipo?.nombre ?? "Equipo", href: `/c/${c}/equipos/${e}` },
      { texto: repo?.nombre ?? "Repositorio", href: `/c/${c}/equipos/${e}/repositorios/${encodeURIComponent(rid)}` },
      { texto: "Retención en detalle" },
    ]}
  />

  {#if error && !equipo}
    <div class="notice notice-danger"><p>{error}</p></div>
  {:else if !equipo}
    <Cargando forma="ficha" filas={4} />
  {:else if !repo}
    <div class="notice notice-info"><p>{equipo.nombre} ya no tiene ese repositorio.</p></div>
  {:else}
    <header class="cab">
      <span class="page-icon"><CalendarClock size={22} /></span>
      <div class="cab-texto">
        <h1>Retención en detalle</h1>
        <p class="sub">
          De <a class="link-suave" href="/c/{c}/equipos/{e}/repositorios/{encodeURIComponent(rid)}">«{repo.nombre}»</a>, en {equipo.nombre}.
          {#if efectiva}
            {resumenRegla(efectiva.regla)}
            {#if enAlm?.retencion}La aplica el almacén {enAlm.almacen.nombre} {enAlm.retencion.horario_texto ?? textoHorario(enAlm.retencion.horario)}.{:else}Se aplica al pedirlo desde la consola o la ventana del equipo.{/if}
          {:else}
            No tiene retención: se guardan todas las versiones.
          {/if}
        </p>
      </div>
    </header>

    <IndicePagina
      items={[
        { id: "ret-calendario", texto: "Calendario" },
        { id: "ret-eliminado", texto: "Lo que se eliminó" },
        ...(prevision ? [{ id: "ret-proximo", texto: "Lo que se eliminará" }] : []),
      ]}
    />

    <div class="cifras">
      <div class="cifra">
        <span class="k">Veces aplicada</span>
        <strong class="num">{leidas ? numero(cifras.vueltas) : "…"}</strong>
        <span class="faint">{cifras.fallidas ? plural(cifras.fallidas, "fallida", "fallidas") : "anotadas por los equipos"}</span>
      </div>
      <div class="cifra">
        <span class="k">Versiones eliminadas</span>
        <strong class="num">{leidas ? numero(cifras.quitadas) : "…"}</strong>
        <span class="faint">en esas veces</span>
      </div>
      <div class="cifra">
        <span class="k">Espacio liberado</span>
        <strong class="num">{leidas ? bytes(cifras.liberado) : "…"}</strong>
        <span class="faint">{cifras.liberadoIncompleto ? "al menos (alguna vez no lo dijo)" : "lo que dijo restic al podar"}</span>
      </div>
      <div class="cifra">
        <span class="k">La próxima vez</span>
        <strong class="num">{prevision ? numero(prevision.proxima.ids.length) : "—"}</strong>
        <span class="faint">{!prevision ? "sin retención" : prevision.proxima.cuando ? `se eliminarían el ${fmtCuando.format(new Date(prevision.proxima.cuando))}` : "se eliminarían al aplicar la retención"}</span>
      </div>
    </div>

    <div class="filtros" role="group" aria-label="Filtros">
      {#if copiasFiltro.length > 1}
        <label class="campo">
          <span>Copia</span>
          <select bind:value={copia}>
            <option value={null}>Todas</option>
            {#each copiasFiltro as k (k)}<option value={k}>{nombreCopia(k)}</option>{/each}
          </select>
        </label>
      {/if}
      {#if prevision}
        <div class="campo">
          <span id="ret-dias">Mirar adelante</span>
          <div class="segmented inline" role="group" aria-labelledby="ret-dias">
            <button class:on={dias === 7} aria-pressed={dias === 7} onclick={() => (dias = 7)}>7 días</button>
            <button class:on={dias === 30} aria-pressed={dias === 30} onclick={() => (dias = 30)}>30 días</button>
          </div>
        </div>
      {/if}
      {#if dia}
        <button class="btn btn-sm btn-ghost" onclick={() => (dia = null)}>Quitar el día elegido</button>
      {/if}
    </div>

    <section class="card p" id="ret-calendario" aria-labelledby="ret-calendario-t">
      <h2 class="section-title" id="ret-calendario-t">Calendario</h2>
      <CalendarioRetencion {marcas} ahora={reloj.ahora} adelante={dias} {dia} alDia={(k) => (dia = k)} />
    </section>

    <section class="card p" id="ret-eliminado" aria-labelledby="ret-eliminado-t">
      <h2 class="section-title" id="ret-eliminado-t">Lo que se eliminó</h2>
      {#if errorVueltas}
        <div class="notice notice-danger"><p>{errorVueltas}</p></div>
      {:else if !leidas}
        <Cargando filas={3} />
      {:else}
        <VueltasRetencion {vueltas} nombres={{ equipo: equipo.nombre, almacen: enAlm?.almacen.nombre ?? null }} {nombreCopia} {copia} {dia} />
        <p class="faint pie">
          Lo anota cada equipo al aplicar la retención (desde la versión del agente que lo hace; las de antes no salen). Con la lista de versiones, las 50 veces más recientes; de las anteriores, sus cifras. Sin rutas ni nombres de archivos.
        </p>
      {/if}
    </section>

    {#if prevision && efectiva}
      <section class="card p previ" id="ret-proximo" aria-labelledby="ret-proximo-t">
        <h2 class="section-title" id="ret-proximo-t">Lo que se eliminará próximamente</h2>
        <PrevisionRetencion
          {prevision}
          {versiones}
          regla={efectiva.regla}
          {nombreCopia}
          {dias}
          total={nVersiones(repo, inf)}
          almacen={enAlm?.retencion ? enAlm.almacen.nombre : null}
          {copia}
          {dia}
          ahora={reloj.ahora}
        />
      </section>
    {/if}
  {/if}
</div>

<style>
  .cab {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-4);
  }
  .cab-texto {
    flex: 1;
    min-width: 0;
  }
  h1 {
    margin: 0;
    font-size: var(--fs-title);
    line-height: var(--lh-title);
    font-weight: 650;
    letter-spacing: -0.015em;
  }
  .sub {
    margin: 4px 0 0;
    max-width: 80ch;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .cifras {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--sp-3);
  }
  .cifra {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    padding: var(--sp-4);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }
  .cifra .k {
    font-size: var(--fs-xs);
    font-weight: 500;
    color: var(--text-3);
  }
  .cifra strong {
    font-size: var(--fs-stat, 24px);
    line-height: 1.15;
    font-weight: 600;
  }
  .cifra .faint {
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
  }
  .filtros {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--sp-3) var(--sp-4);
  }
  .campo {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: var(--fs-xs);
    color: var(--text-3);
  }
  .campo select {
    min-width: 14ch;
  }
  section.card {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .section-title {
    margin: 0;
  }
  .pie {
    margin: 0;
    font-size: var(--fs-xs);
  }
  @media (max-width: 760px) {
    .cifras {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .cifra strong {
      font-size: 20px;
    }
  }
</style>
