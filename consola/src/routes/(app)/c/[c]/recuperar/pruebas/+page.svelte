<script lang="ts">
  // Pruebas funcionales (plan 0.8.0, E3, 14k): los simulacros de recuperación del
  // cliente, con su plazo (cada 6 meses por defecto, se puede apagar, y por
  // repositorio), los objetivos de RTO y RPO y si se cumplen, y el historial con la
  // ficha de cada una. La última correcta de cada repositorio cuenta en el «0» de la
  // regla 3-2-1-1-0; la lista sale en Informes y en el informe ejecutivo en PDF.
  //
  // ?registrar=1 abre el formulario; ?restauracion=<equipo>|<id> lo abre con los
  // datos de esa restauración registrada.
  import { untrack } from "svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { CalendarClock, ClipboardCheck, Clock, FileText, History, Pencil, Plus, Settings2, Target, Trash2, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Cifra from "$lib/componentes/Cifra.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import ErrorCarga from "$lib/componentes/ErrorCarga.svelte";
  import GraficaBarras from "$lib/componentes/repo/GraficaBarras.svelte";
  import FormPruebaFuncional from "$lib/componentes/pruebas/FormPruebaFuncional.svelte";
  import * as api from "$lib/api";
  import { actual, puede, reloj } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { bytes, fechaLarga, numero, plural } from "$lib/formato";
  import { cargarFuncionales, errorFuncionales, funcionalesDe, ponerFuncionales } from "$lib/funcionales.svelte";
  import { aMinutos, cifrasPruebas, deMinutos, deRestauracion, diaLargo, resultadoPrueba, textoAlcance, textoMinutos, textoMotivoPrueba, textoPlazo, DESDE_PRUEBA } from "$lib/pruebasFuncionales";
  import { juntar, type ConMotivo } from "$lib/restauraciones";
  import { ultimos } from "$lib/informes.svelte";
  import { informeDe } from "$lib/repo";
  import type { EntradaPruebaFuncional, PoliticaPruebas, PruebaFuncional } from "$lib/tiposPruebas";

  const c = $derived(actual.id);
  const datos = $derived(funcionalesDe(c));
  const pruebas = $derived(datos?.pruebas ?? []);
  const cifras = $derived(cifrasPruebas(pruebas));
  const ajustes = $derived(datos?.ajustes ?? null);

  // Las restauraciones registradas (para enlazar una al registrar).
  let restauraciones = $state<ConMotivo[]>([]);
  $effect(() => {
    const cc = c;
    if (!cc) return;
    untrack(() => {
      void cargarFuncionales(cc, true);
      api
        .restauraciones(cc)
        .then((r) => cc === actual.id && (restauraciones = juntar(r)))
        .catch(() => {});
    });
  });

  const nombreEquipo = (id: string) => actual.equipos.find((e) => e.id === id)?.nombre ?? "Equipo quitado";
  const nombreRepo = (eq: string, repo: string) => actual.equipos.find((e) => e.id === eq)?.resumen?.repositorios?.find((r) => r.id === repo)?.nombre ?? repo;
  const nombre = (eq: string, repo: string) => `«${nombreRepo(eq, repo)}» de ${nombreEquipo(eq)}`;

  // ---------- Formulario ----------
  let form = $state<{ editando: PruebaFuncional | null; inicial: EntradaPruebaFuncional | null } | null>(null);
  const puedeRegistrar = $derived(puede.ordenar(actual.cliente?.rol));
  // ?registrar=1 o ?restauracion=<equipo>|<id> (desde la ficha de una restauración).
  let abiertoDeUrl = "";
  $effect(() => {
    const q = page.url.searchParams;
    const clave = `${q.get("registrar") ?? ""}|${q.get("restauracion") ?? ""}`;
    const rs = restauraciones;
    if (clave === "|" || !puedeRegistrar) return;
    untrack(() => {
      const rid = q.get("restauracion");
      if (rid && !rs.length) return;
      if (abiertoDeUrl === clave) return;
      abiertoDeUrl = clave;
      const r = rid ? rs.find((x) => `${x.equipo}|${x.id}` === rid) : null;
      const version = r?.version ? (informeDe(ultimos.porEquipo[r.equipo] ?? null, r.repo)?.versiones.find((v) => v.id.startsWith(r.version!))?.hora ?? null) : null;
      form = { editando: null, inicial: r ? deRestauracion(r, version) : null };
      const u = new URL(page.url);
      u.searchParams.delete("registrar");
      u.searchParams.delete("restauracion");
      void goto(u, { replaceState: true, keepFocus: true, noScroll: true });
    });
  });

  // ---------- Ficha ----------
  let ficha = $state<PruebaFuncional | null>(null);
  const fichaAlDia = $derived(ficha ? (pruebas.find((p) => p.id === ficha!.id) ?? null) : null);
  let borrando = $state(false);
  let confirmarBorrar = $state(false);
  async function borrar() {
    if (!ficha || !c) return;
    borrando = true;
    try {
      ponerFuncionales(c, await api.borrarPruebaFuncional(c, ficha.id));
      avisar("Prueba funcional borrada.");
      ficha = null;
      confirmarBorrar = false;
    } catch (e) {
      avisar((e as Error).message, "bad");
    } finally {
      borrando = false;
    }
  }

  // ---------- Política ----------
  let editarPolitica = $state(false);
  let pol = $state<PoliticaPruebas>({ activo: true, meses: 6, aviso_dias: 30, rto_objetivo_min: null, rpo_objetivo_min: null, repos: [] });
  let rtoObj = $state({ texto: "", unidad: "h" as "min" | "h" | "dias" });
  let rpoObj = $state({ texto: "", unidad: "h" as "min" | "h" | "dias" });
  let nuevoRepo = $state("");
  let nuevoMeses = $state(3);
  let guardandoPol = $state(false);
  function abrirPolitica() {
    if (!ajustes) return;
    pol = { activo: ajustes.activo, meses: ajustes.meses, aviso_dias: ajustes.aviso_dias, rto_objetivo_min: ajustes.rto_objetivo_min ?? null, rpo_objetivo_min: ajustes.rpo_objetivo_min ?? null, repos: [...(ajustes.repos ?? [])] };
    rtoObj = deMinutos(ajustes.rto_objetivo_min);
    rpoObj = deMinutos(ajustes.rpo_objetivo_min);
    editarPolitica = true;
  }
  const todosRepos = $derived(actual.equipos.filter((e) => e.confirmado !== false && e.modo !== "trasladado").flatMap((e) => (e.resumen?.repositorios ?? []).map((r) => ({ equipo: e.id, repo: r.id, texto: `${r.nombre} (${e.nombre})` }))));
  const errorPol = $derived.by(() => {
    const a = aMinutos(rtoObj.texto, rtoObj.unidad);
    const b = aMinutos(rpoObj.texto, rpoObj.unidad);
    if ((a != null && (Number.isNaN(a) || a < 1)) || (b != null && (Number.isNaN(b) || b < 1))) return "Escribe los objetivos con números mayores que cero (o déjalos en blanco).";
    if (!(pol.meses >= 1 && pol.meses <= 36)) return "El plazo, de 1 a 36 meses.";
    if (!(pol.aviso_dias >= 1 && pol.aviso_dias <= 90)) return "El aviso, de 1 a 90 días antes.";
    return null;
  });
  async function guardarPolitica() {
    if (!c || errorPol) return;
    guardandoPol = true;
    try {
      const x = await api.ponerPoliticaPruebas(c, { ...pol, rto_objetivo_min: aMinutos(rtoObj.texto, rtoObj.unidad), rpo_objetivo_min: aMinutos(rpoObj.texto, rpoObj.unidad) });
      ponerFuncionales(c, x);
      avisar(pol.activo ? "Política guardada." : "Avisos de las pruebas funcionales apagados.");
      editarPolitica = false;
    } catch (e) {
      avisar((e as Error).message, "bad");
    } finally {
      guardandoPol = false;
    }
  }
  function anadirPlazoRepo() {
    const [equipo, repo] = nuevoRepo.split("|");
    if (!equipo || !repo) return;
    pol.repos = [...pol.repos.filter((r) => !(r.equipo === equipo && r.repo === repo)), { equipo, repo, meses: nuevoMeses }];
    nuevoRepo = "";
  }

  const conTiempos = $derived(pruebas.map((p) => ({ ...p, hora: `${p.fecha}T12:00:00` })));
  const tono = (r: string) => resultadoPrueba(r).tono;
  const cumpleTexto = (x: boolean | null | undefined) => (x === false ? " (no cumple el objetivo)" : "");
</script>

<svelte:head><title>Pruebas funcionales · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <CabeceraPagina
    titulo="Pruebas funcionales"
    icono={ClipboardCheck}
    migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${actual.id}` }, { texto: "Pruebas funcionales" }]}
    resumen="Simulacros de recuperación: recuperar datos como el día de un problema y apuntar cuánto se tardó (RTO) y de cuándo eran los datos (RPO). La última correcta de cada repositorio cuenta en el «0» de la regla 3-2-1-1-0."
  >
    {#snippet acciones()}
      <a class="btn" href="/c/{actual.id}/restauraciones"><History size={16} />Restauraciones</a>
      {#if puedeRegistrar && datos}<button class="btn btn-primary" onclick={() => (form = { editando: null, inicial: null })}><Plus size={16} />Registrar una prueba</button>{/if}
    {/snippet}
  </CabeceraPagina>

  {#if datos === undefined && errorFuncionales(c)}
    <ErrorCarga que="las pruebas funcionales" error={errorFuncionales(c)} reintentar={() => cargarFuncionales(c, true)} />
  {:else if datos === undefined}
    <Cargando forma="lineas" />
  {:else if datos === null}
    <div class="notice notice-info"><p>Este servidor aún no guarda las pruebas funcionales: actualiza Resguardo Server para registrarlas.</p></div>
  {:else}
    <section class="card p estado" aria-labelledby="t-plazo">
      <div class="estado-cab">
        <h2 class="section-title" id="t-plazo"><CalendarClock size={16} />Plazo</h2>
        {#if puede.administrar(actual.cliente?.rol)}<button class="btn btn-sm" onclick={abrirPolitica}><Settings2 size={14} />Cambiar la política</button>{/if}
      </div>
      {#if !datos.estado}
        <p class="faint">Sin plazo: los avisos de las pruebas funcionales están apagados en este cliente.</p>
      {:else}
        {@const t = textoPlazo(datos.estado.cliente)}
        <p class="plazo"><Chip tono={t.tono} texto={t.tono === "ok" ? "Al día" : t.tono === "warn" ? "Pronto" : "Vencida"} /> {t.texto}</p>
        <p class="faint">
          {datos.estado.cliente.ultima ? `La última que cuenta (correcta o con incidencias): ${diaLargo(datos.estado.cliente.ultima.slice(0, 10))}.` : "Aún no hay ninguna que cuente."} Se avisa en la consola y por los canales {plural(ajustes?.aviso_dias ?? 30, "día", "días")} antes y al vencer.
        </p>
        {#if datos.estado.repos.length}
          <ul class="plazos-repo">
            {#each datos.estado.repos as r (`${r.equipo}|${r.repo}`)}
              {@const tr = textoPlazo(r.plazo)}
              <li><Chip pequeno tono={tr.tono} texto={tr.tono === "ok" ? "Al día" : tr.tono === "warn" ? "Pronto" : "Vencida"} /> {nombre(r.equipo, r.repo)}: {tr.texto}</li>
            {/each}
          </ul>
        {/if}
      {/if}
      {#if ajustes?.rto_objetivo_min || ajustes?.rpo_objetivo_min}
        <p class="objetivos"><Target size={14} />Objetivos: {[ajustes.rto_objetivo_min ? `datos de vuelta en ${textoMinutos(ajustes.rto_objetivo_min)} como mucho (RTO)` : "", ajustes.rpo_objetivo_min ? `perder ${textoMinutos(ajustes.rpo_objetivo_min)} como mucho (RPO)` : ""].filter(Boolean).join(" · ")}.</p>
      {/if}
    </section>

    {#if !pruebas.length}
      <div class="card">
        <Vacio icono={ClipboardCheck} ilustracion="sin-versiones" titulo="Aún no hay pruebas funcionales" texto="Haz un simulacro (restaura unos datos en una máquina de pruebas, por ejemplo) y regístralo aquí con lo que tardó. Puedes partir de una restauración ya registrada.">
          {#if puedeRegistrar}<button class="btn btn-primary" onclick={() => (form = { editando: null, inicial: null })}><Plus size={16} />Registrar una prueba</button>{/if}
        </Vacio>
      </div>
    {:else}
      <div class="cifras" role="list" aria-label="Cifras de las pruebas funcionales">
        <Cifra icono={ClipboardCheck} etiqueta="Pruebas" valor={numero(cifras.total)} sub={cifras.fallidas || cifras.conIncidencias ? [cifras.conIncidencias ? plural(cifras.conIncidencias, "con incidencias", "con incidencias") : "", cifras.fallidas ? plural(cifras.fallidas, "fallida", "fallidas") : ""].filter(Boolean).join(" · ") : "todas correctas"} mal={cifras.fallidas > 0} />
        <Cifra icono={Clock} etiqueta="Datos de vuelta (RTO)" valor={textoMinutos(cifras.rtoMedio)} sub={cifras.rtoPeor != null ? `de media; el peor, ${textoMinutos(cifras.rtoPeor)}` : "sin datos"} />
        <Cifra icono={History} etiqueta="Antigüedad (RPO)" valor={textoMinutos(cifras.rpoMedio)} sub={cifras.rpoPeor != null ? `de media; la peor, ${textoMinutos(cifras.rpoPeor)}` : "sin datos"} />
        <Cifra
          icono={Target}
          etiqueta="Cumplen los objetivos"
          valor={cifras.rtoConObjetivo + cifras.rpoConObjetivo ? `${cifras.rtoCumplen + cifras.rpoCumplen} de ${cifras.rtoConObjetivo + cifras.rpoConObjetivo}` : "—"}
          sub={ajustes?.rto_objetivo_min || ajustes?.rpo_objetivo_min ? "RTO y RPO" : "sin objetivos"}
          mal={cifras.rtoCumplen + cifras.rpoCumplen < cifras.rtoConObjetivo + cifras.rpoConObjetivo}
        />
      </div>

      {#if conTiempos.filter((p) => p.rto_min != null).length >= 2}
        <div class="graficas">
          <div class="card p"><GraficaBarras titulo="Tiempo hasta tener los datos de vuelta (RTO)" datos={conTiempos} valor={(p) => p.rto_min} formato={(x) => textoMinutos(x)} alElegir={(p) => (ficha = p)} clave={(p) => p.id} elegido={ficha?.id ?? null} /></div>
          <div class="card p"><GraficaBarras titulo="Antigüedad de los datos recuperados (RPO)" datos={conTiempos} valor={(p) => p.rpo_min} formato={(x) => textoMinutos(x)} alElegir={(p) => (ficha = p)} clave={(p) => p.id} elegido={ficha?.id ?? null} /></div>
        </div>
      {/if}

      <section class="card p-0" aria-labelledby="t-hist">
        <h2 class="sr-only" id="t-hist">Historial de pruebas funcionales</h2>
        <ul class="lista">
          {#each pruebas as p (p.id)}
            {@const r = resultadoPrueba(p.resultado)}
            <li>
              <button class="fila elegible-fila" onclick={() => (ficha = p)}>
                <span class="tile-ic"><ClipboardCheck size={16} /></span>
                <span class="fila-texto">
                  <span class="fila-titulo">{diaLargo(p.fecha)} · {textoAlcance(p, nombre)}</span>
                  <span class="fila-sub">{textoMotivoPrueba(p.motivo)}{p.quien ? ` · ${p.quien}` : ""}{p.donde ? ` · ${p.donde}` : ""}{p.adjuntos.length ? ` · ${plural(p.adjuntos.length, "adjunto", "adjuntos")}` : ""}</span>
                </span>
                <span class="fila-meta num">
                  <Chip pequeno tono={r.tono} texto={r.texto} />
                  <span class="bloque">
                    <span class:aviso={p.cumple_rto === false}>RTO {textoMinutos(p.rto_min)}</span> · <span class:aviso={p.cumple_rpo === false}>RPO {textoMinutos(p.rpo_min)}</span>
                  </span>
                </span>
              </button>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  {/if}
</div>

{#if form}
  <FormPruebaFuncional cliente={c} editando={form.editando} inicial={form.inicial} {restauraciones} onclose={() => (form = null)} />
{/if}

{#if fichaAlDia}
  {@const p = fichaAlDia}
  {@const r = resultadoPrueba(p.resultado)}
  <Modal labelledby="t-ficha-pf" onclose={() => ((ficha = null), (confirmarBorrar = false))} width={620}>
    <div class="dlg-title">
      <span class="ticon"><ClipboardCheck size={18} /></span>
      <div>
        <h2 id="t-ficha-pf">Prueba funcional del {diaLargo(p.fecha)}</h2>
        <p>{textoAlcance(p, nombre)}</p>
      </div>
    </div>
    <dl class="ficha">
      <dt>Resultado</dt>
      <dd><Chip pequeno tono={tono(p.resultado)} texto={r.texto} /></dd>
      {#if p.incidencias}<dt>Incidencias</dt><dd class="pre">{p.incidencias}</dd>{/if}
      <dt>Motivo</dt>
      <dd>{textoMotivoPrueba(p.motivo)}</dd>
      <dt>Quién</dt>
      <dd>{p.quien || p.por}</dd>
      {#if p.que}<dt>Qué se restauró</dt><dd class="pre">{p.que}</dd>{/if}
      {#if p.donde}<dt>Dónde</dt><dd>{p.donde}</dd>{/if}
      {#if p.desde}<dt>Desde</dt><dd>{DESDE_PRUEBA.find((d) => d.id === p.desde?.tipo)?.texto ?? "—"}{p.desde.nombre ? `: ${p.desde.nombre}` : ""}</dd>{/if}
      <dt>Datos de vuelta (RTO)</dt>
      <dd>{textoMinutos(p.rto_min)}{cumpleTexto(p.cumple_rto)}</dd>
      <dt>Antigüedad (RPO)</dt>
      <dd>{textoMinutos(p.rpo_min)}{cumpleTexto(p.cumple_rpo)}</dd>
      {#if p.restauracion}<dt>Restauración</dt><dd><a class="link-suave" href="/c/{actual.id}/restauraciones?equipo={encodeURIComponent(p.restauracion.equipo)}">La registrada en {nombreEquipo(p.restauracion.equipo)}</a></dd>{/if}
      {#if p.notas}<dt>Notas</dt><dd class="pre">{p.notas}</dd>{/if}
      {#if p.adjuntos.length}
        <dt>Adjuntos</dt>
        <dd>
          <ul class="adjuntos">
            {#each p.adjuntos as a (a.n)}<li><FileText size={14} /><a class="link-suave" href={api.urlAdjuntoPrueba(actual.id, p.id, a.n)} download>{a.nombre}</a> <span class="faint">{bytes(a.tamano)}</span></li>{/each}
          </ul>
        </dd>
      {/if}
      <dt>Registrada</dt>
      <dd class="faint">{fechaLarga(p.creada)} por {p.por}{p.actualizada !== p.creada ? ` · cambiada ${fechaLarga(p.actualizada)}` : ""}</dd>
      <dt>En la regla</dt>
      <dd>{p.resultado === "ok" ? "Cuenta como prueba de restauración correcta de los repositorios que incluye (45 días)." : "Deja el «0» de esos repositorios por revisar hasta que haya otra correcta."}</dd>
    </dl>
    <footer class="pie">
      {#if puede.administrar(actual.cliente?.rol)}
        {#if confirmarBorrar}
          <span class="faint">¿Borrarla con sus adjuntos?</span>
          <button class="btn btn-danger" disabled={borrando} onclick={borrar}><Trash2 size={15} />Sí, borrarla</button>
        {:else}
          <button class="btn btn-ghost" onclick={() => (confirmarBorrar = true)}><Trash2 size={15} />Borrar</button>
        {/if}
      {/if}
      <span class="hueco"></span>
      {#if puedeRegistrar}<button class="btn" onclick={() => ((form = { editando: p, inicial: null }), (ficha = null))}><Pencil size={15} />Cambiar</button>{/if}
      <button class="btn btn-primary" onclick={() => (ficha = null)}>Cerrar</button>
    </footer>
  </Modal>
{/if}

{#if editarPolitica}
  <Modal labelledby="t-pol" onclose={() => (editarPolitica = false)} width={600}>
    <div class="dlg-title">
      <span class="ticon"><Settings2 size={18} /></span>
      <div>
        <h2 id="t-pol">Política de pruebas funcionales</h2>
        <p>Cada cuánto toca un simulacro en este cliente, cuándo se avisa y qué se quiere conseguir.</p>
      </div>
    </div>
    <form
      class="form"
      onsubmit={(e) => {
        e.preventDefault();
        void guardarPolitica();
      }}
    >
      <label class="switch-row">
        <input type="checkbox" bind:checked={pol.activo} />
        <span><strong>Avisar cuando toque una prueba</strong><span class="faint">En la consola y por los canales: informativo al acercarse, importante al vencer.</span></span>
      </label>
      {#if pol.activo}
        <div class="dos">
          <div class="field">
            <label class="field-label" for="pol-meses">Cada cuántos meses</label>
            <input id="pol-meses" class="input num" type="number" min="1" max="36" bind:value={pol.meses} />
          </div>
          <div class="field">
            <label class="field-label" for="pol-aviso">Avisar antes (días)</label>
            <input id="pol-aviso" class="input num" type="number" min="1" max="90" bind:value={pol.aviso_dias} />
          </div>
        </div>
        <div class="field">
          <span class="field-label">Repositorios con su propio plazo <span class="faint">(opcional)</span></span>
          {#if pol.repos.length}
            <ul class="plazos-repo">
              {#each pol.repos as r, i (`${r.equipo}|${r.repo}`)}
                <li>
                  {nombre(r.equipo, r.repo)}: cada <input class="input num mini" type="number" min="1" max="36" bind:value={pol.repos[i].meses} aria-label="Meses" /> meses
                  <button type="button" class="btn btn-ghost btn-sm" aria-label="Quitar" onclick={() => (pol.repos = pol.repos.filter((_, j) => j !== i))}><Trash2 size={14} /></button>
                </li>
              {/each}
            </ul>
          {/if}
          <div class="fila-campo">
            <select class="input" bind:value={nuevoRepo} aria-label="Repositorio">
              <option value="">Elige un repositorio</option>
              {#each todosRepos as x (`${x.equipo}|${x.repo}`)}<option value={`${x.equipo}|${x.repo}`}>{x.texto}</option>{/each}
            </select>
            <input class="input num mini" type="number" min="1" max="36" bind:value={nuevoMeses} aria-label="Cada cuántos meses" />
            <button type="button" class="btn btn-sm" disabled={!nuevoRepo} onclick={anadirPlazoRepo}><Plus size={14} />Añadir</button>
          </div>
        </div>
      {/if}
      <div class="dos">
        <div class="field">
          <label class="field-label" for="pol-rto">Objetivo de RTO <span class="faint">(datos de vuelta en…)</span></label>
          <div class="fila-campo">
            <input id="pol-rto" class="input num mini" inputmode="decimal" bind:value={rtoObj.texto} placeholder="—" />
            <select class="input" bind:value={rtoObj.unidad} aria-label="Unidad"><option value="min">minutos</option><option value="h">horas</option><option value="dias">días</option></select>
          </div>
        </div>
        <div class="field">
          <label class="field-label" for="pol-rpo">Objetivo de RPO <span class="faint">(perder como mucho…)</span></label>
          <div class="fila-campo">
            <input id="pol-rpo" class="input num mini" inputmode="decimal" bind:value={rpoObj.texto} placeholder="—" />
            <select class="input" bind:value={rpoObj.unidad} aria-label="Unidad"><option value="min">minutos</option><option value="h">horas</option><option value="dias">días</option></select>
          </div>
        </div>
      </div>
      {#if errorPol}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{errorPol}</p></div>{/if}
      <footer class="pie">
        <span class="hueco"></span>
        <button type="button" class="btn btn-ghost" onclick={() => (editarPolitica = false)}>Cancelar</button>
        <button type="submit" class="btn btn-primary" disabled={guardandoPol || !!errorPol}>Guardar la política</button>
      </footer>
    </form>
  </Modal>
{/if}

<style>
  .estado-cab h2 {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .estado-cab {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
  }
  .estado p {
    margin: 0 0 var(--sp-2);
  }
  .plazo {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .objetivos {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .plazos-repo {
    margin: 0 0 var(--sp-2);
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .plazos-repo li {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .graficas {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 320px), 1fr));
    gap: var(--sp-4);
  }
  .lista {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .lista li + li {
    border-top: 1px solid var(--border);
  }
  .elegible-fila {
    width: 100%;
    font: inherit;
    text-align: left;
    background: none;
    border: 0;
    cursor: pointer;
  }
  .elegible-fila:hover,
  .elegible-fila:focus-visible {
    background: var(--surface-2);
  }
  .bloque {
    display: block;
  }
  .aviso {
    color: var(--warn);
  }
  .ficha {
    display: grid;
    /* G6: con rutas largas, la columna del valor encoge y parte (no desborda a 375 px). */
    grid-template-columns: minmax(0, max-content) minmax(0, 1fr);
    gap: 6px var(--sp-4);
    margin: 0 0 var(--sp-5);
    font-size: var(--fs-sm);
  }
  .ficha dt {
    color: var(--text-3);
  }
  .ficha dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .pre {
    white-space: pre-wrap;
  }
  .adjuntos {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .adjuntos li {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .pie {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .hueco {
    flex: 1;
  }
  .dos {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 220px), 1fr));
    gap: var(--sp-4);
  }
  .fila-campo {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .fila-campo .input:not(.mini) {
    flex: 1;
    min-width: 0;
  }
  .mini {
    width: 5.5rem;
  }
</style>
