<script lang="ts">
  // Crear un repositorio desde la consola (orden «crear_repositorio», con la
  // clave de administración). La contraseña del repositorio la genera este
  // navegador, se muestra en el kit de recuperación (para imprimir) y viaja
  // sellada solo para el equipo, igual que las credenciales del destino.
  import CampoObservaciones from "./notas/CampoObservaciones.svelte";
  import { errorTextoNota, guardarObservacion, objetoDe } from "$lib/notas.svelte";
  import { onDestroy, untrack } from "svelte";
  import { Check, Database, FolderOpen, KeyRound, LoaderCircle, Printer, TriangleAlert } from "@lucide/svelte";
  import { errorCarpetaDestino } from "$lib/ganchos";
  import ElegirCarpetas from "./ElegirCarpetas.svelte";
  import BotonCargando from "./BotonCargando.svelte";
  import CopiarEnAlmacen from "./CopiarEnAlmacen.svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { aleatorio } from "$lib/cripto/bytes";
  import { ErrorEtiqueta, mandarOrden } from "$lib/ordenar";
  import { fechaLarga } from "$lib/formato";
  import { avisar } from "$lib/avisos.svelte";
  import { cargarCliente } from "$lib/estado.svelte";
  import type { Cliente, DestinoResumen, Equipo } from "$lib/tipos";
  import Ayuda from "./Ayuda.svelte";
  import CampoClave from "./CampoClave.svelte";
  // v1.14: «Usar uno que ya existe» y crear uno «para traer el historial de otro».
  import AdoptarRepositorio from "./AdoptarRepositorio.svelte";
  import FormRepoExistente from "./FormRepoExistente.svelte";
  import { TEXTO_TROCEADO, origenCuerpo, recordarOrigen, repoExistenteCompleto, repoExistenteVacio } from "$lib/adoptar";
  import { admiteAlmacenPropio, TEXTO_ALMACEN_PROPIO } from "$lib/retencion";

  let { cliente, equipos, destinos, equipoInicial, onclose }: { cliente: Cliente; equipos: Equipo[]; destinos: DestinoResumen[]; equipoInicial?: string; onclose: () => void } = $props();

  type Paso = "datos" | "kit" | "clave";
  let paso = $state<Paso>("datos");
  // Valores iniciales del formulario (después manda lo que se elija aquí).
  const inicial = () => ({
    equipo: equipoInicial ?? equipos.find((e) => e.confirmado && e.rol !== "almacenamiento")?.id ?? equipos[0]?.id ?? "",
    destino: destinos[0]?.id ?? "nuevo",
  });
  let equipoId = $state(inicial().equipo);
  /** v1.3x: observaciones del repositorio (se guardan en el servidor al crearlo). */
  let observaciones = $state("");
  let nombre = $state("");
  let destinoId = $state<string>(inicial().destino);
  let tipo = $state<"rest" | "b2" | "s3" | "local">("rest");
  let donde = $state("");
  let nombreDestino = $state("");
  let usuario = $state("");
  let secretoDestino = $state("");
  let contrasena = $state("");
  let impreso = $state(false);
  let claveAdmin = $state("");
  let ocupado = $state(false);
  let pasoTxt = $state("");
  let error = $state("");
  /** Elegir la carpeta del destino local en el propio equipo. */
  let explorar = $state(false);
  let existente = $state(false);
  let paraHistorial = $state(false);
  let origen = $state(repoExistenteVacio());

  const equipo = $derived(equipos.find((e) => e.id === equipoId));
  // Solo los destinos que ya tiene ESTE equipo (los de otros no los conoce).
  const destinosEquipo = $derived(equipo?.resumen?.destinos?.length ? destinos.filter((d) => equipo!.resumen!.destinos!.some((x) => x.id === d.id)) : destinos.filter(() => !equipo?.resumen));
  // Los equipos del cliente que guardan copias: el destino recomendado. Elegir
  // uno hace lo de «Copiar en …» (pide acceso al almacén y crea el repositorio),
  // sin escribir dirección, usuario ni contraseña. Si ya está conectado, es un destino más.
  // v1.28: también el propio equipo, si es un almacén y su agente lo admite (su propio almacén, por localhost).
  const almacenes = $derived(
    equipos.filter((e) => (e.id !== equipoId || admiteAlmacenPropio(e)) && e.confirmado && e.modo !== "trasladado" && e.resumen?.guarda_copias?.activo),
  );
  /** ¿Es este destino el almacén `a`? (por el equipo que guarda, o por el id que le pone «Copiar en …»). */
  const esDe = (d: DestinoResumen, a: Equipo) => d.equipo_almacen === a.id || d.id === `almacen-${a.id.slice(0, 8)}` || (!d.equipo_almacen && d.tipo === "rest" && d.nombre === a.nombre);
  /** Los almacenes a los que este equipo aún no copia. */
  const almacenesNuevos = $derived(almacenes.filter((a) => !destinosEquipo.some((d) => esDe(d, a))).sort((a, b) => Number(a.id === equipoId) - Number(b.id === equipoId)));
  const almacenElegido = $derived(destinoId.startsWith("almacen:") ? almacenes.find((a) => `almacen:${a.id}` === destinoId) : undefined);
  let copiarEn = $state<Equipo | null>(null);
  const destino = $derived(destinos.find((d) => d.id === destinoId));
  const datosOk = $derived(
    !!equipo && nombre.trim().length > 0 && (destinoId !== "nuevo" || (nombreDestino.trim() && donde.trim())) && (!paraHistorial || repoExistenteCompleto(origen)),
  );
  // Al elegir equipo: el almacén de la oficina al que aún no copia (lo recomendado),
  // o uno de sus destinos. Después manda lo que se elija aquí.
  let equipoVisto = "";
  $effect(() => {
    if (equipoId === equipoVisto) return;
    equipoVisto = equipoId;
    // Su propio almacén no es lo recomendado (sigue en el mismo equipo): solo si se elige.
    const otro = untrack(() => almacenesNuevos.find((a) => a.id !== equipoId));
    destinoId = untrack(() => (otro ? `almacen:${otro.id}` : (destinosEquipo[0]?.id ?? "nuevo")));
  });
  const ubicacion = $derived(destino ? `${destino.nombre}${destino.donde ? ` (${destino.donde})` : ""}` : `${nombreDestino} (${donde})`);
  const ETIQUETA_TIPO = { rest: "Servidor de copias (rest-server)", b2: "Backblaze B2", s3: "S3 compatible", local: "Disco o carpeta del equipo" };

  /** 32 bytes aleatorios en base64url: la contraseña del repositorio. */
  function generar() {
    const b = aleatorio(32);
    contrasena = btoa(String.fromCharCode(...b)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
    b.fill(0);
  }

  /** Id del repositorio (va en el kit: hace falta para restaurar sin el equipo). */
  let repoId = $state("");
  function aKit() {
    generar();
    repoId = `${nombre
      .trim()
      .toLowerCase()
      .normalize("NFD")
      .replace(/[̀-ͯ]/g, "")
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "")
      .slice(0, 40) || "repositorio"}-${crypto.randomUUID().slice(0, 4)}`;
    paso = "kit";
  }

  function olvidar() {
    contrasena = secretoDestino = claveAdmin = usuario = origen.contrasena = origen.secreto = "";
  }
  onDestroy(olvidar);

  async function crear(e: SubmitEvent) {
    e.preventDefault();
    if (!equipo) return;
    error = "";
    ocupado = true;
    try {
      const id = repoId;
      const o = await mandarOrden({
        cliente,
        equipo,
        tipo: "crear_repositorio",
        cuerpo: {
          id,
          nombre: nombre.trim(),
          destino:
            destinoId === "nuevo"
              ? { id: `destino-${crypto.randomUUID().slice(0, 8)}`, nombre: nombreDestino.trim(), tipo, donde: donde.trim(), usuario: usuario || null, secreto: secretoDestino || null }
              : { id: destinoId },
          contrasena,
          ...(paraHistorial ? { parametros_de: origenCuerpo(origen) } : {}),
        },
        secretos: { claveAdmin },
        alPaso: (t) => (pasoTxt = t),
      });
      if (paraHistorial) recordarOrigen(cliente.id, equipo.id, id, origen);
      if (observaciones.trim())
        await guardarObservacion(cliente.id, "repositorio", objetoDe(equipo.id, id), observaciones).catch((err) => avisar(`El repositorio se pidió, pero las observaciones no se guardaron: ${(err as Error).message}`, "warn"));
      olvidar();
      avisar(
        paraHistorial
          ? `Pedido al equipo (orden n.º ${o.seq}): creará «${nombre.trim()}». Después, en su página, usa «Traer historial».`
          : `Pedido al equipo (orden n.º ${o.seq}): creará «${nombre.trim()}».`,
      );
      void cargarCliente(cliente.id, { silencioso: true });
      onclose();
    } catch (err) {
      error = err instanceof ErrorEtiqueta ? err.message : (err as Error).message;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }
</script>

{#if existente}
  <AdoptarRepositorio {cliente} {equipos} equipoInicial={equipoId} {onclose} onvolver={() => (existente = false)} />
{:else if !copiarEn}
<Modal labelledby="t-repo" onclose={() => (olvidar(), onclose())} width={560} dismissible={false}>
  <div class="dlg-title">
    <span class="ticon"><Database size={18} /></span>
    <div>
      <h2 id="t-repo">Nuevo repositorio</h2>
      <p>Donde se guardan, cifradas, las versiones de una o varias copias.</p>
    </div>
  </div>

  {#if paso === "datos"}
    <div class="form">
      <div class="segmented" role="group" aria-label="Repositorio nuevo o existente">
        <button type="button" class="on" aria-pressed="true">Crear uno nuevo</button>
        <button type="button" aria-pressed="false" onclick={() => (olvidar(), (existente = true))}>Usar uno que ya existe</button>
      </div>
      <div class="field">
        <label class="field-label" for="r-equipo">Equipo</label>
        <select id="r-equipo" class="input" bind:value={equipoId}>
          {#each equipos.filter((e) => e.confirmado && e.modo !== "trasladado") as e (e.id)}<option value={e.id}>{e.nombre}</option>{/each}
        </select>
      </div>
      <div class="field">
        <label class="field-label" for="r-nombre">Nombre</label>
        <input id="r-nombre" class="input" bind:value={nombre} placeholder="Por ejemplo: Documentos de recepción" />
      </div>
      <div class="field">
        <label class="field-label" for="r-destino">Destino</label>
        <select id="r-destino" class="input" bind:value={destinoId}>
          {#each almacenesNuevos as a (a.id)}<option value="almacen:{a.id}">{a.id === equipoId ? `Su propio almacén (${a.nombre})` : `Almacén ${a.nombre} (recomendado)`}</option>{/each}
          {#each destinosEquipo as d (d.id)}<option value={d.id}>{d.nombre}{d.inmutable ? " · inmutable" : ""}{almacenes.some((a) => esDe(d, a)) ? " · almacén de la oficina" : ""}</option>{/each}
          <option value="nuevo">Un destino nuevo…</option>
        </select>
      </div>
      {#if destinoId === "nuevo"}
        <div class="nuevo">
          <div class="field">
            <label class="field-label" for="d-tipo">Tipo</label>
            <select id="d-tipo" class="input" bind:value={tipo}>
              {#each Object.entries(ETIQUETA_TIPO) as [k, t] (k)}<option value={k}>{t}</option>{/each}
            </select>
          </div>
          <div class="field">
            <label class="field-label" for="d-nombre">Nombre del destino</label>
            <input id="d-nombre" class="input" bind:value={nombreDestino} placeholder={tipo === "b2" ? "Backblaze B2" : "Servidor de la oficina"} />
          </div>
          <div class="field">
            <label class="field-label" for="d-donde">{tipo === "rest" ? "Dirección (https://servidor:puerto)" : tipo === "local" ? "Carpeta o disco" : "Bucket"}</label>
            {#if tipo === "local"}
              <div class="con-boton">
                <input id="d-donde" class="input mono" bind:value={donde} spellcheck="false" placeholder={/windows/i.test(equipo?.so ?? "") ? "E:\\Resguardo" : "/mnt/disco2/resguardo"} />
                <button type="button" class="btn" disabled={!equipo} onclick={() => (explorar = true)}><FolderOpen size={15} />Explorar…</button>
              </div>
              <span class="field-hint">Un disco o carpeta de {equipo?.nombre ?? "el equipo"}, o una carpeta compartida de la red (\\servidor\copias).</span>
            {:else}
              <input id="d-donde" class="input mono" bind:value={donde} spellcheck="false" />
            {/if}
          </div>
          {#if tipo !== "local"}
            <div class="fila-campos">
              <div class="field">
                <label class="field-label" for="d-usuario">{tipo === "rest" ? "Usuario" : "Id de la clave"}</label>
                <input id="d-usuario" class="input mono" bind:value={usuario} autocomplete="off" spellcheck="false" />
              </div>
              <CampoClave id="d-secreto" etiqueta={tipo === "rest" ? "Contraseña" : "Clave secreta"} bind:value={secretoDestino} />
            </div>
            <p class="faint nota">Las credenciales van selladas solo para {equipo?.nombre ?? "el equipo"}: el servidor no las ve ni las guarda.</p>
          {/if}
        </div>
      {:else if almacenElegido}
        {#if almacenElegido.id === equipoId}
          <p class="faint nota">{TEXTO_ALMACEN_PROPIO}</p>
        {:else}
          <p class="faint nota">{almacenElegido.nombre} guarda copias de los equipos de {cliente.nombre}: {equipo?.nombre ?? "el equipo"} tendrá allí su propio usuario y no podrá borrar lo ya copiado. No hace falta escribir dirección ni contraseñas.</p>
        {/if}
      {/if}
      {#if !almacenElegido}<CampoObservaciones id="r-observaciones" bind:valor={observaciones} filas={2} />{/if}
      <!-- También en un almacén: «Copiar en …» lo pasa a crear_repositorio (parametros_de). -->
      <details class="avanzado" bind:open={paraHistorial}>
        <summary>Para traer el historial de otro repositorio (avanzado)</summary>
        <p class="faint nota">{TEXTO_TROCEADO} Después, en la página del repositorio, «Traer historial» ya tendrá puesto este origen: solo faltará su contraseña.</p>
        {#if paraHistorial}<FormRepoExistente bind:repo={origen} id="p" nombreEquipo={equipo?.nombre} />{/if}
      </details>
      <footer>
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
        <button type="button" class="btn btn-primary" disabled={!datosOk || !!errorTextoNota(observaciones)} onclick={() => (almacenElegido ? (copiarEn = almacenElegido) : aKit())}>Seguir</button>
      </footer>
    </div>
  {:else if paso === "kit"}
    <div class="form">
      <p>Este es el <strong>kit de recuperación</strong> del repositorio. Sin esta contraseña nadie, ni tú, podrá leer sus copias: imprímelo o guárdalo en PDF ahora.</p>
      <article class="kit" id="kit-imprimible">
        <h3>Kit de recuperación · Resguardo</h3>
        <dl>
          <dt>Cliente</dt><dd>{cliente.nombre}</dd>
          <dt>Equipo</dt><dd>{equipo?.nombre}</dd>
          <dt>Repositorio</dt><dd>{nombre} · <span class="pastilla mono selectable">{repoId}</span></dd>
          <dt>Destino</dt><dd>{ubicacion}</dd>
          <dt>Contraseña</dt><dd><code class="selectable pw">{contrasena}</code></dd>
          <dt>Creado</dt><dd>{fechaLarga(new Date().toISOString())}</dd>
        </dl>
        <p class="faint">Guárdalo fuera del equipo. Con esta contraseña y el destino se pueden restaurar las copias con restic, incluso sin Resguardo.</p>
      </article>
      <div class="acciones">
        <button type="button" class="btn" onclick={() => window.print()}><Printer size={15} />Imprimir o guardar en PDF</button>
      </div>
      <label class="switch-row"><input type="checkbox" bind:checked={impreso} /><span>He guardado el kit en un sitio seguro</span></label>
      <footer>
        <button type="button" class="btn btn-ghost" onclick={() => (paso = "datos")}>Atrás</button>
        <button type="button" class="btn btn-primary" disabled={!impreso} onclick={() => (paso = "clave")}><Check size={15} />Seguir</button>
      </footer>
    </div>
  {:else}
    <form class="form" onsubmit={crear}>
      <p>{equipo?.nombre} creará «{nombre}» en {ubicacion}. Confírmalo con la clave de administración de {cliente.nombre}.</p>
      <CampoClave requerido id="r-admin" etiqueta="Clave de administración" bind:value={claveAdmin} autofocus error={error && error.includes("clave") ? error : ""}>
        {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
      </CampoClave>
      {#if error && !error.includes("clave")}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={() => (paso = "kit")}>Atrás</button>
        <BotonCargando class="btn btn-primary" disabled={!claveAdmin} cargando={ocupado} textoCargando="Enviando…"><KeyRound size={15} />Confirmar con la clave de administración</BotonCargando>
      </footer>
    </form>
  {/if}
</Modal>
{:else if equipo}
  <!-- Un almacén del cliente: lo mismo que «Copiar en …» de la ficha del equipo. -->
  <CopiarEnAlmacen {cliente} {equipo} almacen={copiarEn} nombreInicial={nombre.trim()} origen={paraHistorial ? $state.snapshot(origen) : undefined} {onclose} />
{/if}

{#if explorar && equipo}
  <ElegirCarpetas
    {cliente}
    {equipo}
    unica
    titulo="Dónde guardar «{nombre.trim() || 'el repositorio'}» en {equipo.nombre}"
    iniciales={donde.trim() ? [donde.trim()] : []}
    validar={(r) => errorCarpetaDestino(r, /windows/i.test(equipo.so), true)}
    onclose={() => (explorar = false)}
    alElegir={(rutas) => {
      donde = rutas[0] ?? donde;
      explorar = false;
    }}
  />
{/if}

<style>
  .nuevo {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-4);
    background: var(--surface-2);
    border-radius: var(--radius);
  }
  .nota {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .avanzado {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .avanzado summary {
    cursor: pointer;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .avanzado[open] summary {
    margin-bottom: var(--sp-3);
  }
  .kit {
    padding: var(--sp-5);
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
  }
  .kit h3 {
    margin-bottom: var(--sp-3);
    font-size: var(--fs-h2);
  }
  .kit dl {
    display: grid;
    grid-template-columns: 110px minmax(0, 1fr);
    gap: 6px 12px;
    margin: 0 0 var(--sp-3);
    font-size: var(--fs-sm);
  }
  .kit dt {
    color: var(--text-3);
  }
  .kit dd {
    margin: 0;
  }
  .pw {
    font-size: 14px;
    word-break: break-all;
  }
  .kit p {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .acciones {
    display: flex;
    gap: 6px;
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  /* Al imprimir, solo el kit. */
  @media print {
    :global(body *) {
      visibility: hidden;
    }
    :global(#kit-imprimible),
    :global(#kit-imprimible *) {
      visibility: visible;
    }
    :global(#kit-imprimible) {
      position: fixed;
      inset: 0 auto auto 0;
      width: 100%;
      border: none;
      color: #000;
    }
  }
</style>
