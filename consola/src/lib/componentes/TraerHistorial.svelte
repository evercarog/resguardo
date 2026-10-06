<script lang="ts">
  // «Traer historial de otro repositorio» (copiar_historial, v1.14): el equipo
  // copia (`restic copy`) a este repositorio las versiones de otro, p. ej. el
  // de la app de escritorio. Solo añade: no borra nada aquí ni en el origen,
  // y no repite lo ya traído. Sigue en el equipo aunque se cierre la ventana.
  import { onDestroy, untrack } from "svelte";
  import { CircleCheck, FlaskConical, History, KeyRound, LoaderCircle, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { TEXTO_TROCEADO, origenCuerpo, origenRecordado, probarRepositorio, pruebaParaEquipo, repoExistenteCompleto, repoExistenteVacio, type Prueba } from "$lib/adoptar";
  import { borrar } from "$lib/cripto/bytes";
  import { ErrorLlavesCambiadas, mandarOrden } from "$lib/ordenar";
  import { fechaLarga } from "$lib/formato";
  import type { Cliente, Equipo, Orden, RepositorioResumen } from "$lib/tipos";
  import Ayuda from "./Ayuda.svelte";
  import AlertaLlaves from "./AlertaLlaves.svelte";
  import CampoClave from "./CampoClave.svelte";
  import FormRepoExistente from "./FormRepoExistente.svelte";
  import ElegirCarpetas from "./ElegirCarpetas.svelte";
  import { ADMITE, admite, errorFiltro, filtroParaOrden } from "$lib/cadenas";

  let { cliente, equipo, repo, onclose }: { cliente: Cliente; equipo: Equipo; repo: RepositorioResumen; onclose: () => void } = $props();

  const otros = $derived((equipo.resumen?.repositorios ?? []).filter((r) => r.id !== repo.id));
  let modo = $state<"otro" | "equipo">("otro");
  let origenId = $state("");
  // Si se creó «para traer el historial» de otro (en este navegador): ese origen, ya puesto.
  const recordado = untrack(() => origenRecordado(cliente.id, equipo.id, repo.id));
  let origen = $state(recordado ?? repoExistenteVacio());
  let claveAdmin = $state("");
  /** «Explorar…»: elegir en el equipo la carpeta del repositorio de origen. */
  let explorar = $state(false);
  let ocupado = $state(false);
  let pasoTxt = $state("");
  let error = $state("");
  let cambiadas = $state<Equipo | null>(null);
  let probado = $state<Prueba | null>(null);
  /** Equipos elegidos (de los que copiaron en el origen); vacío: todos. */
  let elegidos = $state<string[]>([]);
  let orden = $state<Orden | null>(null);
  /** Tarea 4c (agente con `admite: "filtros"`): qué versiones traer, por etiqueta y fecha. */
  const conFiltros = $derived(admite(equipo, ADMITE.filtros));
  let filtroTxt = $state({ etiquetas: "", desde: "", ultimos_dias: "" as number | string });
  const filtro = $derived.by(() => {
    const f = conFiltros ? filtroParaOrden(filtroTxt) : null;
    const eq = modo === "otro" && elegidos.length ? { equipos: elegidos } : {};
    return f || Object.keys(eq).length ? { ...(f ?? {}), ...eq } : null;
  });
  const errorF = $derived(conFiltros ? errorFiltro(filtroTxt) : null);

  let prueba: { clave: string; bytes: Uint8Array } | null = null;
  let vivo = true;
  onDestroy(() => {
    vivo = false;
    if (prueba) borrar(prueba.bytes);
    claveAdmin = origen.contrasena = origen.secreto = "";
  });

  const huella = $derived(JSON.stringify([origen.tipo, origen.direccion, origen.usuario, origen.secreto, origen.ca, origen.contrasena]));
  let probadoCon = "";
  $effect(() => {
    if (huella !== probadoCon) {
      probado = null;
      elegidos = [];
    }
  });

  const listo = $derived(!!claveAdmin && !errorF && (modo === "equipo" ? !!origenId : !!probado));

  async function laPrueba(): Promise<Uint8Array> {
    if (prueba && prueba.clave === claveAdmin) return prueba.bytes;
    if (prueba) borrar(prueba.bytes);
    pasoTxt = "Comprobando la clave y las llaves del equipo…";
    prueba = { clave: claveAdmin, bytes: await pruebaParaEquipo(cliente, equipo, claveAdmin) };
    return prueba.bytes;
  }

  async function probar() {
    error = "";
    ocupado = true;
    try {
      const p = await laPrueba();
      const h = huella;
      probado = await probarRepositorio({ cliente, equipo, prueba: p, repo: origen, alPaso: (t) => (pasoTxt = t) });
      probadoCon = h;
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) cambiadas = equipo;
      else error = (err as Error).message;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }

  async function seguir(o: Orden) {
    // Mientras la ventana siga abierta: el progreso que manda el equipo (cada minuto).
    while (vivo) {
      await new Promise((r) => setTimeout(r, 3000));
      if (!vivo) return;
      try {
        const x = (await api.ordenesEquipo(cliente.id, equipo.id, 20)).find((y) => y.id === o.id);
        if (x) orden = x;
        if (x && !["pendiente", "entregada", "en_marcha"].includes(x.estado)) return;
      } catch {
        // Sin red un momento: se vuelve a intentar.
      }
    }
  }

  async function traer(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    ocupado = true;
    try {
      const p = await laPrueba();
      const o = await mandarOrden({
        cliente,
        equipo,
        tipo: "copiar_historial",
        cuerpo: {
          repo: repo.id,
          origen: modo === "equipo" ? { repo: origenId } : origenCuerpo(origen),
          ...(filtro ? { filtro } : {}),
        },
        secretos: { prueba: p },
        alPaso: (t) => (pasoTxt = t),
      });
      if (prueba) borrar(prueba.bytes);
      prueba = null;
      claveAdmin = origen.contrasena = origen.secreto = "";
      orden = o;
      void seguir(o);
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) cambiadas = equipo;
      else error = (err as Error).message;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }

  const enCurso = $derived(!!orden && ["pendiente", "entregada", "en_marcha"].includes(orden.estado));
</script>

<Modal labelledby="t-historial" {onclose} width={600} dismissible={false}>
  <div class="dlg-title">
    <span class="ticon"><History size={18} /></span>
    <div>
      <h2 id="t-historial">Traer historial a «{repo.nombre}»</h2>
      <p>{equipo.nombre} copiará aquí las versiones de otro repositorio (por ejemplo, el de la app de escritorio). Solo añade: no borra nada, ni aquí ni en el otro, y lo que ya se trajo no se repite.</p>
    </div>
  </div>

  {#if cambiadas}
    <AlertaLlaves equipo={cambiadas} cliente={cliente.id} />
    <footer><button class="btn btn-primary" onclick={onclose}>Entendido</button></footer>
  {:else if orden}
    <div class="form">
      {#if enCurso}
        <div class="notice notice-info" role="status"><LoaderCircle size={16} class="spin" /><p>{orden.mensaje ?? (orden.estado === "pendiente" ? `Esperando a ${equipo.nombre}…` : "Trayendo el historial…")}</p></div>
        <p class="faint">Puede tardar horas si el historial es grande o el origen está lejos. Puedes cerrar esta ventana: sigue en {equipo.nombre}, y el resultado saldrá en sus órdenes.</p>
      {:else if orden.estado === "hecha"}
        <div class="notice notice-success" role="status"><CircleCheck size={16} /><p>{orden.mensaje ?? "Historial traído."}</p></div>
        <p class="faint">Las versiones traídas aparecerán en el repositorio con el próximo informe de {equipo.nombre}.</p>
      {:else}
        <div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{orden.mensaje ?? "No se pudo traer el historial."}</p></div>
      {/if}
      <footer><button class="btn btn-primary" onclick={onclose}>Cerrar</button></footer>
    </div>
  {:else}
    <form class="form" onsubmit={traer}>
      {#if otros.length}
        <div class="segmented" role="group" aria-label="Origen">
          <button type="button" class:on={modo === "otro"} aria-pressed={modo === "otro"} onclick={() => (modo = "otro")}>Otro repositorio</button>
          <button type="button" class:on={modo === "equipo"} aria-pressed={modo === "equipo"} onclick={() => (modo = "equipo")}>Uno de {equipo.nombre}</button>
        </div>
      {/if}

      {#if modo === "equipo"}
        <div class="field">
          <label class="field-label" for="h-origen">Repositorio de origen</label>
          <select id="h-origen" class="input" bind:value={origenId}>
            <option value="" disabled>Elige uno</option>
            {#each otros as r (r.id)}<option value={r.id}>{r.nombre}{r.solo_lectura ? " (importado)" : ""}</option>{/each}
          </select>
        </div>
      {:else}
        <FormRepoExistente bind:repo={origen} id="h" nombreEquipo={equipo.nombre} alExplorar={() => (explorar = true)} />
      {/if}

      <CampoClave requerido id="h-admin" etiqueta="Clave de administración" bind:value={claveAdmin} error={error && error.includes("clave de administración") ? error : ""}>
        {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
      </CampoClave>

      {#if probado}
        <div class="notice notice-success" role="status">
          <CircleCheck size={16} />
          <div>
            <p>{probado.mensaje}</p>
            {#if probado.ultima}<p class="faint pequeno">Última versión: {fechaLarga(probado.ultima)}</p>{/if}
          </div>
        </div>
        {#if probado.equipos.length > 1}
          <fieldset class="equipos">
            <legend class="field-label">¿De qué equipos? <span class="faint">(sin marcar ninguno, de todos)</span></legend>
            {#each probado.equipos as h (h)}
              <label class="switch-row"><input type="checkbox" value={h} bind:group={elegidos} /><span>{h}</span></label>
            {/each}
          </fieldset>
        {/if}
      {/if}

      {#if conFiltros}
        <details class="filtro-versiones">
          <summary class="faint">Solo algunas versiones</summary>
          <div class="field">
            <label class="field-label" for="h-etiquetas">Con la etiqueta</label>
            <input id="h-etiquetas" class="input" bind:value={filtroTxt.etiquetas} placeholder="diaria, semanal" />
          </div>
          <div class="field">
            <label class="field-label" for="h-dias">De los últimos (días)</label>
            <input id="h-dias" class="input num" type="number" min="1" max="3650" bind:value={filtroTxt.ultimos_dias} />
          </div>
          <div class="field">
            <label class="field-label" for="h-desde">Desde el</label>
            <input id="h-desde" class="input" type="date" bind:value={filtroTxt.desde} />
          </div>
          {#if errorF}<p class="error-campo">{errorF}</p>{/if}
        </details>
      {/if}

      {#if recordado && modo === "otro"}
        <p class="faint nota">Este repositorio se creó para traer el historial de <code>{recordado.direccion}</code>: {TEXTO_TROCEADO} Escribe la contraseña de ese repositorio y pulsa «Probar».</p>
      {:else}
      <p class="faint nota">
        Si el otro repositorio no se creó a partir de este (o este a partir de él), cada uno trocea los archivos a su manera: lo traído no se aprovecha del todo de lo que ya hay y ocupará algo más. Para evitarlo, crea el repositorio nuevo con «Nuevo repositorio → Para traer el historial de otro».
      </p>
      {/if}

      {#if error && !error.includes("clave de administración")}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}

      <footer>
        {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
        {#if modo === "otro"}<button type="button" class="btn" disabled={!repoExistenteCompleto(origen) || !claveAdmin || ocupado} onclick={probar}><FlaskConical size={15} />Probar</button>{/if}
        <button class="btn btn-primary" disabled={!listo || ocupado}><KeyRound size={15} />Traer el historial</button>
      </footer>
    </form>
  {/if}
</Modal>

{#if explorar}
  <ElegirCarpetas
    {cliente}
    {equipo}
    claveAdmin={claveAdmin || undefined}
    unica
    buscarRepos
    titulo="La carpeta del repositorio de origen en {equipo.nombre}"
    iniciales={origen.direccion.trim() ? [origen.direccion.trim()] : []}
    onclose={() => (explorar = false)}
    alElegir={(rutas) => {
      if (rutas[0]) origen.direccion = rutas[0];
      explorar = false;
    }}
  />
{/if}

<style>
  .pequeno {
    font-size: var(--fs-xs);
    margin: 4px 0 0;
  }
  .nota {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .filtro-versiones[open] {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .filtro-versiones summary {
    cursor: pointer;
  }
  .equipos {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    border: none;
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
</style>
