<script lang="ts">
  // Restauraciones (plan 0.8.0, E2: 13d, 14l): cada restauración de los equipos
  // del cliente (o de uno, `?equipo=`), con quién, cuándo, qué versión, desde
  // dónde, cuánto y cuánto tardó (el RTO real), su motivo y su ficha. Las anota
  // el propio equipo (sin rutas) y llegan a todas sus consolas; el motivo que se
  // añade aquí se queda en esta consola.
  import { untrack } from "svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { ArchiveRestore, CircleCheck, ClipboardCheck, Clock, History, ListChecks, LoaderCircle, MessageSquareText, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import Cifra from "$lib/componentes/Cifra.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import Vacio from "$lib/componentes/Vacio.svelte";
  import Cargando from "$lib/componentes/Cargando.svelte";
  import GraficaBarras from "$lib/componentes/repo/GraficaBarras.svelte";
  import * as api from "$lib/api";
  import { actual, puede, reloj } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { bytes, fechaLarga, numero, plural, relativo } from "$lib/formato";
  import { estadisticas, juntar, MOTIVOS, textoDesde, textoDestino, textoMotivo, textoQuien, textoTardo, type ConMotivo } from "$lib/restauraciones";
  import type { MotivoRestauracion, Restauraciones } from "$lib/tipos";

  let datos = $state<Restauraciones | null>(null);
  let error = $state("");
  const equipoFiltro = $derived(page.url.searchParams.get("equipo") ?? "");
  let periodo = $state<"30" | "90" | "365" | "todo">("365");
  let soloResultado = $state<"" | "ok" | "fallo">("");

  let vuelta = 0;
  async function cargar(c: string, e: string) {
    const mia = ++vuelta;
    error = "";
    try {
      const r = await api.restauraciones(c, e || undefined);
      if (mia === vuelta) datos = r;
    } catch (x) {
      if (mia === vuelta) {
        error = (x as Error).message;
        datos = { ajustes: { motivo_obligatorio: false }, entradas: [], notas: [] };
      }
    }
  }
  $effect(() => {
    const [c, e] = [actual.id, equipoFiltro];
    if (!c) return;
    untrack(() => void cargar(c, e));
  });

  const nombreEquipo = (id: string) => actual.equipos.find((e) => e.id === id)?.nombre ?? "Equipo quitado";
  const nombreRepo = (eq: string, repo: string) => actual.equipos.find((e) => e.id === eq)?.resumen?.repositorios?.find((r) => r.id === repo)?.nombre ?? repo;

  const todas = $derived(datos ? juntar(datos) : []);
  const desdeT = $derived(periodo === "todo" ? 0 : reloj.ahora - Number(periodo) * 86_400_000);
  const lista = $derived(todas.filter((r) => Date.parse(r.hora) >= desdeT && (!soloResultado || r.resultado === soloResultado)));
  const cifras = $derived(estadisticas(lista));
  const conDuracion = $derived(lista.filter((r) => r.resultado === "ok" && typeof r.duracion_s === "number"));

  function filtrarEquipo(id: string) {
    const u = new URL(page.url);
    if (id) u.searchParams.set("equipo", id);
    else u.searchParams.delete("equipo");
    void goto(u, { replaceState: true, keepFocus: true, noScroll: true });
  }

  // ---------- Ficha y motivo ----------
  let ficha = $state<ConMotivo | null>(null);
  let motivo = $state<MotivoRestauracion | "">("");
  let notas = $state("");
  let guardando = $state(false);
  function abrir(r: ConMotivo) {
    ficha = r;
    motivo = r.motivoFinal?.tipo ?? "";
    notas = r.motivoFinal?.notas ?? "";
  }
  async function guardarMotivo() {
    if (!ficha || !motivo || !actual.id) return;
    guardando = true;
    try {
      await api.ponerMotivoRestauracion(actual.id, ficha.equipo, ficha.id, motivo, notas.trim() || null);
      avisar("Motivo guardado.");
      ficha = null;
      await cargar(actual.id, equipoFiltro);
    } catch (x) {
      avisar((x as Error).message, "bad");
    } finally {
      guardando = false;
    }
  }

  // ---------- Ajuste del cliente ----------
  let cambiandoAjuste = $state(false);
  async function obligatorio(v: boolean) {
    if (!actual.id || !datos) return;
    cambiandoAjuste = true;
    try {
      datos.ajustes = await api.ponerAjustesRestauraciones(actual.id, v);
      avisar(v ? "El motivo será obligatorio al restaurar." : "El motivo vuelve a ser opcional.");
    } catch (x) {
      avisar((x as Error).message, "bad");
    } finally {
      cambiandoAjuste = false;
    }
  }
</script>

<svelte:head><title>Restauraciones · {actual.cliente?.nombre ?? ""} · Resguardo Server</title></svelte:head>

<div class="page">
  <CabeceraPagina
    titulo="Restauraciones"
    icono={ArchiveRestore}
    migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${actual.id}` }, { texto: "Restauraciones" }]}
    resumen="Cada restauración queda anotada sola: quién, cuándo, qué versión, desde dónde, cuánto y cuánto tardó. Sin nombres de archivos: no salen del equipo."
  >
    {#snippet acciones()}
      {#if puede.ordenar(actual.cliente?.rol)}<a class="btn btn-primary" href="/c/{actual.id}/restaurar{equipoFiltro ? `?equipo=${encodeURIComponent(equipoFiltro)}` : ''}"><History size={16} />Restaurar</a>{/if}
    {/snippet}
  </CabeceraPagina>

  <div class="filtros">
    <label class="field-inline">
      <span class="faint">Equipo</span>
      <select class="input" value={equipoFiltro} onchange={(e) => filtrarEquipo(e.currentTarget.value)}>
        <option value="">Todos</option>
        {#each actual.equipos.filter((e) => e.confirmado !== false) as e (e.id)}<option value={e.id}>{e.nombre}</option>{/each}
      </select>
    </label>
    <label class="field-inline">
      <span class="faint">Periodo</span>
      <select class="input" bind:value={periodo}>
        <option value="30">Últimos 30 días</option>
        <option value="90">Últimos 90 días</option>
        <option value="365">Último año</option>
        <option value="todo">Todo</option>
      </select>
    </label>
    <label class="field-inline">
      <span class="faint">Resultado</span>
      <select class="input" bind:value={soloResultado}>
        <option value="">Todos</option>
        <option value="ok">Correctas</option>
        <option value="fallo">Fallidas</option>
      </select>
    </label>
  </div>

  {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}

  {#if !datos}
    <Cargando forma="lineas" />
  {:else if !todas.length}
    <div class="card">
      <Vacio icono={ArchiveRestore} ilustracion="sin-versiones" titulo="Aún no hay restauraciones" texto="Cuando alguien restaure algo (desde aquí o desde la ventana del equipo), aparecerá aquí con su ficha.">
        {#if puede.ordenar(actual.cliente?.rol)}<a class="btn btn-primary" href="/c/{actual.id}/restaurar">Restaurar archivos</a>{/if}
      </Vacio>
    </div>
  {:else}
    <div class="cifras" role="list" aria-label="Cifras de las restauraciones">
      <Cifra icono={ArchiveRestore} etiqueta="Restauraciones" valor={numero(cifras.total)} sub={cifras.fallidas ? plural(cifras.fallidas, "fallida", "fallidas") : "todas correctas"} mal={cifras.fallidas > 0} />
      <Cifra icono={Clock} etiqueta="Tiempo medio (RTO real)" valor={textoTardo(cifras.medio_s)} sub={cifras.maximo_s != null ? `la más larga, ${textoTardo(cifras.maximo_s)}` : "sin correctas"} />
      <Cifra icono={CircleCheck} etiqueta="Éxito" valor={cifras.exito == null ? "—" : `${cifras.exito} %`} sub={`${bytes(cifras.bytes)} restaurados`} />
      <Cifra icono={MessageSquareText} etiqueta="Sin motivo" valor={numero(cifras.sinMotivo)} sub={datos.ajustes.motivo_obligatorio ? "el motivo es obligatorio" : "el motivo es opcional"} />
    </div>

    {#if conDuracion.length >= 2}
      <div class="graficas">
        <div class="card p"><GraficaBarras titulo="Cuánto tardó cada restauración" datos={conDuracion} valor={(r) => r.duracion_s} formato={(x) => textoTardo(x)} alElegir={(r) => abrir(r)} clave={(r) => r.id} elegido={ficha?.id ?? null} /></div>
        <div class="card p"><GraficaBarras titulo="Datos restaurados" datos={conDuracion} valor={(r) => r.bytes} formato={(x) => bytes(x)} alElegir={(r) => abrir(r)} clave={(r) => r.id} elegido={ficha?.id ?? null} /></div>
      </div>
    {/if}

    <section class="card p-0" aria-labelledby="t-lista">
      <h2 class="sr-only" id="t-lista">Lista de restauraciones</h2>
      {#if !lista.length}
        <p class="faint vacio-filtro">Ninguna con estos filtros.</p>
      {/if}
      <ul class="lista-rest">
        {#each lista as r (`${r.equipo}|${r.id}`)}
          <li>
            <button class="fila elegible-fila" onclick={() => abrir(r)}>
              <span class="tile-ic"><ArchiveRestore size={16} /></span>
              <span class="fila-texto">
                <span class="fila-titulo">«{nombreRepo(r.equipo, r.repo)}» de {nombreEquipo(r.equipo)}</span>
                <span class="fila-sub">
                  {plural(r.elementos ?? 0, "elemento", "elementos")}{r.archivos != null ? ` · ${plural(r.archivos, "archivo", "archivos")}` : ""}{r.bytes ? ` · ${bytes(r.bytes)}` : ""} · {textoDestino(r.destino)} · {textoQuien(r)}
                </span>
              </span>
              <span class="fila-meta num">
                <span class="chips-fila">
                  {#if r.motivoFinal}<span class="badge badge-sm tone-neutral">{textoMotivo(r.motivoFinal.tipo)}</span>{/if}
                  <Chip pequeno tono={r.resultado === "ok" ? "ok" : "bad"} texto={r.resultado === "ok" ? "Correcta" : "Falló"} />
                </span>
                <span class="bloque">{relativo(r.hora, reloj.ahora)} · tardó {textoTardo(r.duracion_s)}</span>
              </span>
            </button>
          </li>
        {/each}
      </ul>
    </section>

    {#if puede.administrar(actual.cliente?.rol)}
      <label class="switch-row ajuste">
        <input type="checkbox" checked={datos.ajustes.motivo_obligatorio} disabled={cambiandoAjuste} onchange={(e) => obligatorio(e.currentTarget.checked)} />
        <span><strong>Pedir siempre el motivo</strong> al restaurar en este cliente<span class="faint">Sin marcar, se pide con un clic pero se puede dejar en blanco. Lo cumple esta consola.</span></span>
      </label>
    {/if}
  {/if}
</div>

{#if ficha}
  {@const r = ficha}
  <Modal labelledby="t-ficha" onclose={() => (ficha = null)} width={600}>
    <div class="dlg-title">
      <span class="ticon"><ArchiveRestore size={18} /></span>
      <div>
        <h2 id="t-ficha">Restauración de «{nombreRepo(r.equipo, r.repo)}»</h2>
        <p>{nombreEquipo(r.equipo)} · {fechaLarga(r.hora)}</p>
      </div>
    </div>
    <dl class="ficha">
      <dt>Resultado</dt>
      <dd><Chip pequeno tono={r.resultado === "ok" ? "ok" : "bad"} texto={r.resultado === "ok" ? "Correcta" : "Falló"} />{#if r.mensaje} <span class="faint">{r.mensaje}</span>{/if}</dd>
      <dt>Quién</dt>
      <dd>{textoQuien(r)}</dd>
      <dt>Cuándo</dt>
      <dd>{r.inicio ? `${fechaLarga(r.inicio)} → ` : ""}{fechaLarga(r.hora)}</dd>
      <dt>Cuánto tardó</dt>
      <dd>{textoTardo(r.duracion_s)}</dd>
      <dt>Qué</dt>
      <dd>{plural(r.elementos ?? 0, "elemento elegido", "elementos elegidos")}{r.archivos != null ? `, ${plural(r.archivos, "archivo", "archivos")}` : ""}{r.bytes != null ? `, ${bytes(r.bytes)}` : ""}</dd>
      <dt>Versión</dt>
      <dd class="num">{r.version || "—"}</dd>
      <dt>Desde</dt>
      <dd>{textoDesde(r)}</dd>
      <dt>A dónde</dt>
      <dd>{textoDestino(r.destino)}{r.cifrado ? " (con contraseña, AES-256)" : ""}</dd>
      {#if r.resultado === "ok" && ((r.archivos ?? 0) > 0 || (r.bytes ?? 0) > 0) && r.desde !== "derivada" && r.desde !== "externa"}
        <dt>Prueba</dt>
        <dd>Cuenta como prueba de restauración real del repositorio (45 días).</dd>
      {/if}
    </dl>
    {#if puede.ordenar(actual.cliente?.rol)}
      <!-- Plan 0.8.0, E3 (14k): si fue un simulacro, se registra con sus datos ya puestos. -->
      <p class="como-prueba">
        <a class="btn btn-sm" href="/c/{actual.id}/pruebas?restauracion={encodeURIComponent(`${r.equipo}|${r.id}`)}"><ClipboardCheck size={14} />Registrar como prueba funcional</a>
        <span class="faint">Con el día, el repositorio y lo que tardó ya puestos.</span>
      </p>
    {/if}
    <div class="form motivo-ficha">
      <h3 class="section-title">Motivo</h3>
      {#if puede.ordenar(actual.cliente?.rol)}
        <div class="chips-motivo" role="radiogroup" aria-label="Motivo">
          {#each MOTIVOS as m (m.id)}
            <label class="chip-motivo" class:on={motivo === m.id}><input type="radio" name="motivo-ficha" value={m.id} bind:group={motivo} />{m.texto}</label>
          {/each}
        </div>
        <textarea class="input" rows="3" bind:value={notas} maxlength={500} placeholder="Notas (opcional). Sin contraseñas." aria-label="Notas"></textarea>
        {#if r.motivoFinal?.por}<p class="faint pequeno">Puesto por {r.motivoFinal.por}.</p>{/if}
        <footer>
          <button class="btn btn-ghost" onclick={() => (ficha = null)}>Cerrar</button>
          <button class="btn btn-primary" disabled={!motivo || guardando} onclick={guardarMotivo}>{#if guardando}<LoaderCircle size={15} class="spin" />{:else}<ListChecks size={15} />{/if}Guardar el motivo</button>
        </footer>
      {:else}
        <p>{r.motivoFinal ? `${textoMotivo(r.motivoFinal.tipo)}${r.motivoFinal.notas ? `: ${r.motivoFinal.notas}` : ""}` : "Sin motivo."}</p>
        <footer><button class="btn" onclick={() => (ficha = null)}>Cerrar</button></footer>
      {/if}
    </div>
  </Modal>
{/if}

<style>
  .filtros {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-3);
  }
  .field-inline {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .graficas {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 320px), 1fr));
    gap: var(--sp-4);
  }
  .lista-rest {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .lista-rest li + li {
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
  .chips-fila {
    display: inline-flex;
    gap: 6px;
    justify-content: flex-end;
  }
  .bloque {
    display: block;
  }
  .vacio-filtro {
    padding: var(--sp-4) var(--sp-5);
  }
  .ficha {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px var(--sp-4);
    margin: 0 0 var(--sp-5);
    font-size: var(--fs-sm);
  }
  .ficha dt {
    color: var(--text-3);
  }
  .ficha dd {
    margin: 0;
  }
  .motivo-ficha footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .chips-motivo {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip-motivo {
    display: inline-flex;
    align-items: center;
    padding: 4px 10px;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  .chip-motivo input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .chip-motivo.on {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .chip-motivo:focus-within {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .ajuste {
    max-width: 660px;
  }
  .como-prueba {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin: 0 0 var(--sp-4);
    font-size: var(--fs-sm);
  }
</style>
