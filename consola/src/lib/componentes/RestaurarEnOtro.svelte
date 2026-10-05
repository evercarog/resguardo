<script lang="ts">
  // «Restaurar en otro equipo» (api-servidor.md §10). La consola no tiene las
  // credenciales del destino del equipo A: solo A las conoce.
  //  - Con A vivo: con la contraseña del repositorio y la clave de
  //    administración, A sella y firma su acceso para B (compartir_acceso;
  //    antes se comprueban aquí las llaves de B con su etiqueta) y B lo
  //    importa solo de lectura (importar_repositorio, con la sign_pub de A
  //    comprobada). El sobre va de A a B: este navegador solo lo pasa.
  //  - «El equipo original ya no existe»: se escriben los datos del kit de
  //    recuperación y viajan sellados solo para B.
  // Después se restaura en B desde ese repositorio, como cualquier otro.
  import { onDestroy } from "svelte";
  import { ArrowRight, FolderOpen, KeyRound, LoaderCircle, MonitorSmartphone, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { borrar } from "$lib/cripto/bytes";
  import { ErrorLlavesCambiadas, kcfgComprobada, mandarOrden } from "$lib/ordenar";
  import { cargarCliente } from "$lib/estado.svelte";
  import type { Cliente, Equipo, Orden } from "$lib/tipos";
  import Ayuda from "./Ayuda.svelte";
  import AlertaLlaves from "./AlertaLlaves.svelte";
  import CampoClave from "./CampoClave.svelte";
  import ElegirCarpetas from "./ElegirCarpetas.svelte";

  let {
    cliente,
    equipos,
    origenInicial,
    onclose,
  }: { cliente: Cliente; equipos: Equipo[]; origenInicial?: { equipo: string; repo: string }; onclose: () => void } = $props();

  const activos = $derived(equipos.filter((e) => e.confirmado && e.modo !== "trasladado"));
  const opciones = $derived(activos.flatMap((e) => (e.resumen?.repositorios ?? []).filter((r) => !r.solo_lectura).map((r) => ({ equipo: e, repo: r }))));

  const inicial = () => origenInicial;
  let modo = $state<"vivo" | "kit">("vivo");
  let origen = $state(inicial() ? `${inicial()!.equipo}|${inicial()!.repo}` : "");
  let destinoId = $state("");
  let nombre = $state("");
  let contrasena = $state("");
  let claveAdmin = $state("");
  // Kit
  let kitTipo = $state<"local" | "rest" | "sftp" | "s3" | "b2">("rest");
  let kitNombreDestino = $state("");
  let kitDonde = $state("");
  let kitUsuario = $state("");
  let kitSecreto = $state("");
  let kitCa = $state("");
  let kitRepo = $state("");
  /** «Explorar…» de la carpeta del kit, en el equipo donde se restaura. */
  let explorar = $state(false);

  let ocupado = $state(false);
  let pasoTxt = $state("");
  let error = $state("");
  let cambiadas = $state<Equipo | null>(null);
  let hecho = $state<{ equipo: Equipo; repo: string; nombre: string; mensaje: string } | null>(null);

  const elegido = $derived(opciones.find((o) => `${o.equipo.id}|${o.repo.id}` === origen));
  const destinosPosibles = $derived(activos.filter((e) => modo === "kit" || e.id !== elegido?.equipo.id));
  const destino = $derived(destinosPosibles.find((e) => e.id === destinoId));
  $effect(() => {
    if (!nombre && elegido) nombre = `${elegido.repo.nombre} (de ${elegido.equipo.nombre})`;
  });
  const listo = $derived(
    !!destino &&
      !!claveAdmin &&
      !!nombre.trim() &&
      (modo === "vivo" ? !!elegido && !!contrasena : !!kitDonde.trim() && !!kitRepo.trim() && !!contrasena),
  );

  onDestroy(() => {
    contrasena = claveAdmin = kitSecreto = "";
  });

  /** Id para el repositorio importado en B: letras, números y guiones, como pide el agente. */
  const idPara = (base: string) =>
    `${base
      .normalize("NFD")
      .replace(/[̀-ͯ]/g, "")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "")
      .slice(0, 40) || "importado"}-${crypto.randomUUID().slice(0, 4)}`;

  async function respuesta(eq: Equipo, o: Orden): Promise<Orden> {
    for (let i = 0; i < 120; i++) {
      const x = (await api.ordenesEquipo(cliente.id, eq.id, 10)).find((y) => y.id === o.id);
      if (x && ["hecha", "fallida", "rechazada", "cancelada", "caducada"].includes(x.estado)) return x;
      await new Promise((r) => setTimeout(r, 1500));
    }
    throw new Error(`${eq.nombre} no ha respondido todavía. Mira sus órdenes en un momento.`);
  }

  async function importar(e: SubmitEvent) {
    e.preventDefault();
    if (!destino) return;
    error = "";
    ocupado = true;
    let paraLlaves: Equipo = destino;
    try {
      const id = idPara(modo === "vivo" ? elegido!.repo.id : kitRepo);
      let cuerpo: Record<string, unknown>;
      if (modo === "vivo") {
        const a = elegido!.equipo;
        // 1) Las llaves de B, comprobadas con su etiqueta: A sellará solo para esa box_pub.
        pasoTxt = `Comprobando las llaves de ${destino.nombre}…`;
        borrar(await kcfgComprobada(cliente, destino, claveAdmin));
        // 2) A sella y firma su acceso para B (la mandarOrden comprueba también las llaves de A).
        paraLlaves = a;
        const oa = await mandarOrden({
          cliente,
          equipo: a,
          tipo: "compartir_acceso",
          cuerpo: { repo: elegido!.repo.id, para: { equipo: destino.id, box_pub: destino.box_pub }, incluir_contrasena: false },
          secretos: { claveAdmin, repo: { repo: elegido!.repo.id, contrasena } },
          alPaso: (t) => (pasoTxt = `${a.nombre}: ${t.toLowerCase()}`),
        });
        pasoTxt = `Esperando a ${a.nombre}…`;
        const ra = await respuesta(a, oa);
        if (ra.estado !== "hecha" || !ra.detalle) throw new Error(ra.mensaje ?? `${a.nombre} no pudo compartir el acceso.`);
        const sellado = (JSON.parse(ra.detalle) as { acceso_sellado?: string }).acceso_sellado;
        if (!sellado) throw new Error("La respuesta no trae el acceso sellado.");
        // La contraseña va aparte, sellada solo para B (A no la reenvía).
        cuerpo = { id, nombre: nombre.trim(), acceso_sellado: sellado, sign_pub_origen: a.sign_pub, contrasena };
      } else {
        cuerpo = {
          id,
          nombre: nombre.trim(),
          acceso: {
            destino: {
              tipo: kitTipo,
              ...(kitNombreDestino.trim() ? { nombre: kitNombreDestino.trim() } : {}),
              donde: kitDonde.trim(),
              ...(kitUsuario ? { usuario: kitUsuario } : {}),
              ...(kitSecreto ? { secreto: kitSecreto } : {}),
              ...(kitCa.trim() ? { ca_pem: kitCa.trim() } : {}),
            },
            repo: kitRepo.trim(),
            contrasena,
          },
        };
      }
      // 3) B lo abre, comprueba la firma (o los datos del kit) y lo añade solo de lectura.
      paraLlaves = destino;
      const ob = await mandarOrden({ cliente, equipo: destino, tipo: "importar_repositorio", cuerpo, secretos: { claveAdmin }, alPaso: (t) => (pasoTxt = `${destino.nombre}: ${t.toLowerCase()}`) });
      pasoTxt = `Esperando a ${destino.nombre}…`;
      const rb = await respuesta(destino, ob);
      if (rb.estado !== "hecha") throw new Error(rb.mensaje ?? `${destino.nombre} no pudo importar el repositorio.`);
      contrasena = claveAdmin = kitSecreto = "";
      hecho = { equipo: destino, repo: id, nombre: nombre.trim(), mensaje: rb.mensaje ?? "Repositorio importado." };
      void cargarCliente(cliente.id, { silencioso: true });
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) cambiadas = paraLlaves;
      else error = (err as Error).message;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }

  function irARestaurar() {
    if (!hecho) return;
    const q = new URLSearchParams({ equipo: hecho.equipo.id, repo: hecho.repo, nombre: hecho.nombre });
    // Carga de nuevo la página de restaurar con ese equipo y repositorio (también si ya estaba en ella).
    location.assign(`/c/${cliente.id}/restaurar?${q}`);
  }
</script>

<Modal labelledby="t-otro" {onclose} width={600} dismissible={false}>
  <div class="dlg-title">
    <span class="ticon"><MonitorSmartphone size={18} /></span>
    <div>
      <h2 id="t-otro">Restaurar en otro equipo</h2>
      <p>El otro equipo añade el repositorio <strong>solo de lectura</strong> <Ayuda id="solo-lectura" />: podrá explorarlo y restaurar, pero ninguna copia escribirá en él.</p>
    </div>
  </div>

  {#if hecho}
    <div class="form">
      <div class="notice notice-success" role="status"><p>{hecho.mensaje}</p></div>
      <p class="faint">«{hecho.nombre}» ya está en {hecho.equipo.nombre}. Ahora elige la versión y los archivos.</p>
      <footer>
        <button class="btn btn-ghost" onclick={onclose}>Cerrar</button>
        <button class="btn btn-primary" onclick={irARestaurar}>Restaurar en {hecho.equipo.nombre}<ArrowRight size={15} /></button>
      </footer>
    </div>
  {:else}
    <form class="form" onsubmit={importar}>
      <div class="segmented" role="group" aria-label="Equipo original">
        <button type="button" class:on={modo === "vivo"} aria-pressed={modo === "vivo"} onclick={() => (modo = "vivo")}>El equipo original sigue aquí</button>
        <button type="button" class:on={modo === "kit"} aria-pressed={modo === "kit"} onclick={() => (modo = "kit")}>El equipo original ya no existe</button>
      </div>

      {#if modo === "vivo"}
        <div class="field">
          <label class="field-label" for="o-origen">Repositorio</label>
          <select id="o-origen" class="input" bind:value={origen}>
            <option value="" disabled>Elige uno</option>
            {#each opciones as o (o.equipo.id + o.repo.id)}<option value="{o.equipo.id}|{o.repo.id}">{o.repo.nombre} · en {o.equipo.nombre}</option>{/each}
          </select>
          {#if elegido && !elegido.equipo.conectado}<span class="field-hint">{elegido.equipo.nombre} no está conectado: compartirá el acceso cuando vuelva.</span>{/if}
        </div>
      {:else}
        <p class="faint pequeno">Escribe los datos del kit de recuperación del repositorio. Viajan sellados solo para el equipo que lo importa.</p>
        <div class="dos">
          <div class="field">
            <label class="field-label" for="k-tipo">Tipo de destino</label>
            <select id="k-tipo" class="input" bind:value={kitTipo}>
              <option value="rest">Servidor de copias (rest-server)</option>
              <option value="local">Disco o carpeta</option>
              <option value="sftp">SFTP</option>
              <option value="s3">S3 compatible</option>
              <option value="b2">Backblaze B2</option>
            </select>
          </div>
          <div class="field">
            <label class="field-label" for="k-nombre">Nombre del destino</label>
            <input id="k-nombre" class="input" bind:value={kitNombreDestino} placeholder="Servidor de la oficina" />
          </div>
        </div>
        <div class="field">
          <label class="field-label" for="k-donde">{kitTipo === "rest" ? "Dirección (https://servidor:puerto/…)" : kitTipo === "local" ? "Carpeta" : kitTipo === "sftp" ? "Servidor y ruta" : "Bucket"}</label>
          {#if kitTipo === "local"}
            <div class="con-boton">
              <input id="k-donde" class="input mono" bind:value={kitDonde} spellcheck="false" />
              <button type="button" class="btn" disabled={!destino} onclick={() => (explorar = true)}><FolderOpen size={15} />Explorar…</button>
            </div>
            <span class="field-hint">{destino ? `La carpeta en ${destino.nombre}, el equipo donde se restaura.` : "Elige antes el equipo donde restaurar: la carpeta se busca en él."}</span>
          {:else}
            <input id="k-donde" class="input mono" bind:value={kitDonde} spellcheck="false" />
          {/if}
        </div>
        {#if kitTipo !== "local"}
          <div class="dos">
            <div class="field">
              <label class="field-label" for="k-usuario">{kitTipo === "s3" || kitTipo === "b2" ? "Id de la clave" : "Usuario"}</label>
              <input id="k-usuario" class="input mono" bind:value={kitUsuario} autocomplete="off" spellcheck="false" />
            </div>
            <CampoClave id="k-secreto" etiqueta={kitTipo === "s3" || kitTipo === "b2" ? "Clave secreta" : "Contraseña del destino"} bind:value={kitSecreto} />
          </div>
        {/if}
        {#if kitTipo === "rest"}
          <div class="field">
            <label class="field-label" for="k-ca">Autoridad TLS (PEM, si el kit la trae)</label>
            <textarea id="k-ca" class="input mono" rows="3" bind:value={kitCa} placeholder="-----BEGIN CERTIFICATE-----"></textarea>
          </div>
        {/if}
        <div class="field">
          <label class="field-label" for="k-repo">Id del repositorio</label>
          <input id="k-repo" class="input mono" bind:value={kitRepo} spellcheck="false" placeholder="documentos-recepcion-1a2b" />
          <span class="field-hint">Está en el kit, junto al nombre del repositorio.</span>
        </div>
      {/if}

      <div class="dos">
        <div class="field">
          <label class="field-label" for="o-destino">Restaurar en</label>
          <select id="o-destino" class="input" bind:value={destinoId}>
            <option value="" disabled>Elige un equipo</option>
            {#each destinosPosibles as e (e.id)}<option value={e.id}>{e.nombre}</option>{/each}
          </select>
        </div>
        <div class="field">
          <label class="field-label" for="o-nombre">Nombre en ese equipo</label>
          <input id="o-nombre" class="input" bind:value={nombre} placeholder="Copias del equipo perdido" />
        </div>
      </div>

      <CampoClave requerido id="o-contrasena" etiqueta="Contraseña del repositorio" bind:value={contrasena} ayuda="Está en su kit de recuperación.">
        {#snippet extra()}<Ayuda id="contrasena-repo" />{/snippet}
      </CampoClave>
      <CampoClave requerido id="o-clave" etiqueta="Clave de administración" bind:value={claveAdmin}>
        {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
      </CampoClave>

      {#if cambiadas}<AlertaLlaves equipo={cambiadas} cliente={cliente.id} />{/if}
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
        <button class="btn btn-primary" disabled={!listo || ocupado}><KeyRound size={15} />Confirmar con las dos claves</button>
      </footer>
    </form>
  {/if}
</Modal>

{#if explorar && destino}
  <ElegirCarpetas
    {cliente}
    equipo={destino}
    claveAdmin={claveAdmin || undefined}
    unica
    buscarRepos
    titulo="La carpeta del kit en {destino.nombre}"
    iniciales={kitDonde.trim() ? [kitDonde.trim()] : []}
    onclose={() => (explorar = false)}
    alElegir={(rutas) => {
      if (rutas[0]) kitDonde = rutas[0];
      explorar = false;
    }}
  />
{/if}

<style>
  .segmented {
    align-self: flex-start;
  }
  .dos {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--sp-3);
  }
  .mono {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 12px;
  }
  .pequeno {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  @media (max-width: 560px) {
    .dos {
      grid-template-columns: 1fr;
    }
  }
</style>
