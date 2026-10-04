<script lang="ts">
  // «Usar uno que ya existe» (adoptar_repositorio, v1.14): un repositorio de
  // restic que ya existe (p. ej. de la app de escritorio) pasa a ser uno más
  // del equipo, de lectura y escritura, con todo su historial. Antes se
  // «prueba» (el equipo lo abre con esa contraseña y dice qué tiene); la
  // respuesta llega sellada solo para este navegador.
  import { onDestroy, untrack } from "svelte";
  import { ArrowLeft, CircleCheck, Database, FlaskConical, KeyRound, LoaderCircle, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import {
    destinoCuerpo,
    esperarRespuesta,
    idDe,
    partirDireccion,
    probarRepositorio,
    pruebaParaEquipo,
    repoExistenteCompleto,
    repoExistenteVacio,
    rutaEnAlmacen,
    usuarioEnAlmacen,
    type Prueba,
  } from "$lib/adoptar";
  import { borrar } from "$lib/cripto/bytes";
  import { ErrorLlavesCambiadas, mandarOrden } from "$lib/ordenar";
  import { avisar } from "$lib/avisos.svelte";
  import { cargarCliente } from "$lib/estado.svelte";
  import { fechaLarga } from "$lib/formato";
  import type { Cliente, DestinoResumen, Equipo } from "$lib/tipos";
  import Ayuda from "./Ayuda.svelte";
  import AlertaLlaves from "./AlertaLlaves.svelte";
  import CampoClave from "./CampoClave.svelte";
  import FormRepoExistente from "./FormRepoExistente.svelte";

  let { cliente, equipos, equipoInicial, onclose, onvolver }: { cliente: Cliente; equipos: Equipo[]; equipoInicial?: string; onclose: () => void; onvolver?: () => void } = $props();

  const posibles = $derived(equipos.filter((e) => e.confirmado && e.modo !== "trasladado"));
  const inicial = () => equipoInicial ?? equipos.find((e) => e.confirmado && e.rol !== "almacenamiento")?.id ?? equipos[0]?.id ?? "";
  let equipoId = $state(inicial());
  let repo = $state(repoExistenteVacio());
  let nombre = $state("");
  let claveAdmin = $state("");
  let ocupado = $state(false);
  let pasoTxt = $state("");
  let error = $state("");
  let cambiadas = $state<Equipo | null>(null);
  let probado = $state<Prueba | null>(null);
  let hecho = $state<{ id: string; mensaje: string } | null>(null);

  const equipo = $derived(posibles.find((e) => e.id === equipoId));

  // «Dónde está»: un destino que el equipo ya tiene (p. ej. el almacén donde ya
  // copia: basta el nombre de la carpeta) u «otro sitio» (la dirección completa).
  const destinosEquipo = $derived(equipo?.resumen?.destinos ?? []);
  /** ¿Es este destino el almacén `a`? (como en «Nuevo repositorio»). */
  const esDe = (d: DestinoResumen, a: Equipo) => d.equipo_almacen === a.id || d.id === `almacen-${a.id.slice(0, 8)}` || (!d.equipo_almacen && d.tipo === "rest" && d.nombre === a.nombre);
  const almacenDe = (d: DestinoResumen) => equipos.find((a) => a.id !== equipoId && a.resumen?.guarda_copias?.activo && esDe(d, a));
  let dondeSel = $state("otro");
  let ruta = $state("");
  const destinoSel = $derived(destinosEquipo.find((d) => d.id === dondeSel));
  const almacen = $derived(destinoSel ? almacenDe(destinoSel) : undefined);
  const usuarioAlmacen = $derived(almacen ? usuarioEnAlmacen(destinoSel?.donde) : "");
  const carpetaAlmacen = $derived(almacen?.resumen?.guarda_copias?.carpeta ?? "");
  /** Los repositorios que el almacén ve en la carpeta de este equipo y que el equipo aún no usa. */
  const encontrados = $derived.by(() => {
    if (!almacen || !usuarioAlmacen) return [];
    const suyos = new Set((equipo?.resumen?.repositorios ?? []).map((r) => r.id));
    const lista = almacen.resumen?.guarda_copias?.repositorios?.find((u) => u.usuario === usuarioAlmacen)?.repos ?? [];
    return lista.filter((r) => r !== "." && !suyos.has(r));
  });
  const rutaLimpia = $derived(ruta.trim().replace(/^\/+|\/+$/g, ""));
  // Al cambiar de equipo: su almacén, si copia en alguno; si no, «otro sitio».
  let equipoVisto = "";
  $effect(() => {
    if (equipoId === equipoVisto) return;
    equipoVisto = equipoId;
    dondeSel = untrack(() => destinosEquipo.find((d) => almacenDe(d))?.id ?? "otro");
  });
  /** La prueba de la clave de administración para ese equipo (se calcula una vez). */
  let prueba: { equipo: string; clave: string; bytes: Uint8Array } | null = null;
  function soltarPrueba() {
    if (prueba) borrar(prueba.bytes);
    prueba = null;
  }
  onDestroy(() => {
    soltarPrueba();
    claveAdmin = repo.contrasena = repo.secreto = "";
  });

  // Lo probado deja de valer si cambia el repositorio o el equipo.
  const huella = $derived(JSON.stringify([equipoId, dondeSel, rutaLimpia, repo.tipo, repo.direccion, repo.usuario, repo.secreto, repo.ca, repo.contrasena]));
  let probadoCon = "";
  $effect(() => {
    if (huella !== probadoCon) probado = null;
  });

  const listoParaProbar = $derived(!!equipo && !!claveAdmin && (destinoSel ? !!rutaLimpia && !!repo.contrasena : repoExistenteCompleto(repo)));
  /** En un destino del equipo: `{ destino: { id }, ruta }`. */
  const lugar = $derived(destinoSel ? { destino: { id: destinoSel.id }, ruta: rutaLimpia } : undefined);

  async function laPrueba(): Promise<Uint8Array> {
    if (!equipo) throw new Error("Elige el equipo.");
    if (prueba && prueba.equipo === equipo.id && prueba.clave === claveAdmin) return prueba.bytes;
    soltarPrueba();
    pasoTxt = "Comprobando la clave y las llaves del equipo…";
    const bytes = await pruebaParaEquipo(cliente, equipo, claveAdmin);
    prueba = { equipo: equipo.id, clave: claveAdmin, bytes };
    return bytes;
  }

  async function probar() {
    if (!equipo) return;
    error = "";
    ocupado = true;
    try {
      const p = await laPrueba();
      const h = huella;
      probado = await probarRepositorio({ cliente, equipo, prueba: p, repo, lugar, alPaso: (t) => (pasoTxt = t) });
      probadoCon = h;
      if (!nombre.trim()) nombre = (lugar ? lugar.ruta.split("/").pop() : partirDireccion(repo.tipo, repo.direccion).ruta) || "Repositorio existente";
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) cambiadas = equipo;
      else error = (err as Error).message;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }

  async function adoptar(e: SubmitEvent) {
    e.preventDefault();
    if (!equipo || !probado) return;
    error = "";
    ocupado = true;
    try {
      const p = await laPrueba();
      const id = idDe(nombre.trim());
      const o = await mandarOrden({
        cliente,
        equipo,
        tipo: "adoptar_repositorio",
        cuerpo: {
          id,
          nombre: nombre.trim(),
          ...(lugar ?? { destino: destinoCuerpo(repo, { id: `existente-${crypto.randomUUID().slice(0, 8)}` }), ruta: partirDireccion(repo.tipo, repo.direccion).ruta }),
          contrasena: repo.contrasena,
        },
        secretos: { prueba: p },
        alPaso: (t) => (pasoTxt = t),
      });
      pasoTxt = `Esperando a ${equipo.nombre}…`;
      const r = await esperarRespuesta(cliente.id, equipo, o);
      if (r.estado !== "hecha") throw new Error(r.mensaje ?? `${equipo.nombre} no pudo usar el repositorio.`);
      soltarPrueba();
      claveAdmin = repo.contrasena = repo.secreto = "";
      hecho = { id, mensaje: r.mensaje ?? "Repositorio listo." };
      avisar(`${equipo.nombre} ya puede copiar en «${nombre.trim()}».`);
      void cargarCliente(cliente.id, { silencioso: true });
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) cambiadas = equipo;
      else error = (err as Error).message;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }
</script>

<Modal labelledby="t-adoptar" {onclose} width={600} dismissible={false}>
  <div class="dlg-title">
    <span class="ticon"><Database size={18} /></span>
    <div>
      <h2 id="t-adoptar">Usar un repositorio que ya existe</h2>
      <p>Por ejemplo, el de la app de escritorio. <strong>Se conserva todo su historial</strong>: las versiones que ya tiene siguen ahí y las copias nuevas se añaden a ellas. No se borra ni se cambia nada.</p>
    </div>
  </div>

  {#if cambiadas}
    <AlertaLlaves equipo={cambiadas} cliente={cliente.id} />
    <footer><button class="btn btn-primary" onclick={onclose}>Entendido</button></footer>
  {:else if hecho}
    <div class="form">
      <div class="notice notice-success" role="status"><CircleCheck size={16} /><p>{hecho.mensaje}</p></div>
      <p class="faint">Ahora elige qué carpetas se copian en él y cuándo, en las copias de {equipo?.nombre}.</p>
      <footer>
        <button class="btn btn-ghost" onclick={onclose}>Cerrar</button>
        <a class="btn btn-primary" href="/c/{cliente.id}/equipos/{equipoId}/repositorios/{encodeURIComponent(hecho.id)}" onclick={onclose}>Ver el repositorio</a>
      </footer>
    </div>
  {:else}
    <form class="form" onsubmit={adoptar}>
      <div class="field">
        <label class="field-label" for="a-equipo">Equipo que copiará en él</label>
        <select id="a-equipo" class="input" bind:value={equipoId}>
          {#each posibles as e (e.id)}<option value={e.id}>{e.nombre}</option>{/each}
        </select>
      </div>

      <div class="field">
        <label class="field-label" for="a-donde">Dónde está</label>
        <select id="a-donde" class="input" bind:value={dondeSel}>
          {#each destinosEquipo as d (d.id)}
            {@const a = almacenDe(d)}
            <option value={d.id}>{a ? `Almacén ${a.nombre} (ya configurado)` : `${d.nombre} (destino de ${equipo?.nombre ?? "este equipo"})`}</option>
          {/each}
          <option value="otro">Otro sitio (escribir la dirección)…</option>
        </select>
      </div>

      {#if destinoSel}
        <div class="field">
          <label class="field-label" for="a-ruta">Carpeta del repositorio en {almacen ? almacen.nombre : destinoSel.nombre}</label>
          <input id="a-ruta" class="input mono" bind:value={ruta} spellcheck="false" autocomplete="off" placeholder="siigo" />
          {#if almacen}
            <span class="field-hint">
              {#if carpetaAlmacen && usuarioAlmacen}
                En {almacen.nombre}, la carpeta debe estar en <code class="selectable">{rutaEnAlmacen(carpetaAlmacen, usuarioAlmacen, rutaLimpia || "<nombre>")}</code>.
              {:else if usuarioAlmacen}
                En {almacen.nombre}, dentro de su carpeta de copias, en <code>{usuarioAlmacen}/{rutaLimpia || "<nombre>"}</code>.
              {:else}
                Solo el nombre de la carpeta, dentro de la de {equipo?.nombre} en {almacen.nombre}.
              {/if}
              Para moverlo allí sin ocupar más espacio, mira «Venir de la app de escritorio» en la guía del agente.
            </span>
            {#if encontrados.length}
              <div class="encontrados">
                <span class="faint">Repositorios encontrados en tu carpeta de {almacen.nombre}:</span>
                {#each encontrados as r (r)}<button type="button" class="chip" class:on={rutaLimpia === r} onclick={() => (ruta = r)}>{r}</button>{/each}
              </div>
            {:else if almacen.resumen?.guarda_copias?.repositorios}
              <p class="faint nota">{almacen.nombre} aún no ve ningún repositorio nuevo en la carpeta de {equipo?.nombre}. Si acabas de moverlo, aparecerá en unos minutos; también puedes escribir el nombre y «Probar».</p>
            {/if}
          {:else}
            <span class="field-hint">Solo el nombre de la carpeta dentro de {destinoSel.nombre}{destinoSel.donde ? ` (${destinoSel.donde})` : ""}.</span>
          {/if}
        </div>
        <CampoClave requerido id="a-contrasena" etiqueta="Contraseña del repositorio" ayuda="La que abre las copias: la del kit de recuperación o la que guardaba la app de escritorio." bind:value={repo.contrasena} />
        <p class="faint nota">Se usa el acceso que {equipo?.nombre ?? "el equipo"} ya tiene a ese destino. La contraseña va sellada solo para el equipo: el servidor no la ve ni la guarda.</p>
      {:else}
        <FormRepoExistente bind:repo id="a" nombreEquipo={equipo?.nombre} etiquetaTipo="Tipo" />
      {/if}

      <CampoClave requerido id="a-admin" etiqueta="Clave de administración" bind:value={claveAdmin} error={error && error.includes("clave de administración") ? error : ""}>
        {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
      </CampoClave>

      {#if probado}
        <div class="notice notice-success" role="status">
          <CircleCheck size={16} />
          <div>
            <p>{probado.mensaje}</p>
            {#if probado.ultima}<p class="faint pequeno">Última versión: {fechaLarga(probado.ultima)}{probado.equipos.length ? ` · copiaron en él: ${probado.equipos.join(", ")}` : ""}</p>{/if}
            {#if probado.solo_anadir}<p class="faint pequeno">Es un servidor de solo añadir: nadie puede borrar desde los equipos, y la retención (qué versiones se quitan) se aplica en el propio servidor.</p>{/if}
          </div>
        </div>
        {#if probado.en_uso}
          <div class="notice notice-warn" role="alert"><TriangleAlert size={16} /><p>{equipo?.nombre} ya usa este repositorio («{probado.en_uso}»): no hace falta añadirlo otra vez.</p></div>
        {:else}
          <div class="field">
            <label class="field-label" for="a-nombre">Nombre en la consola</label>
            <input id="a-nombre" class="input" bind:value={nombre} maxlength="80" />
          </div>
        {/if}
      {/if}

      {#if error && !error.includes("clave de administración")}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}

      <footer>
        {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
        {#if onvolver}<button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onvolver}><ArrowLeft size={15} />Crear uno nuevo</button>{/if}
        <button type="button" class="btn" disabled={!listoParaProbar || ocupado} onclick={probar}><FlaskConical size={15} />Probar</button>
        <button class="btn btn-primary" disabled={!probado || !!probado.en_uso || !nombre.trim() || ocupado}><KeyRound size={15} />Usar este repositorio</button>
      </footer>
    </form>
  {/if}
</Modal>

<style>
  .encontrados {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
    font-size: var(--fs-sm);
  }
  .chip {
    padding: 2px 10px;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-family: var(--font-mono, monospace);
    cursor: pointer;
  }
  .chip.on {
    border-color: var(--accent);
    color: var(--accent);
  }
  .nota {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .pequeno {
    font-size: var(--fs-xs);
    margin: 4px 0 0;
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
