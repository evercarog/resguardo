<script lang="ts">
  // «Conectar otro destino» en el equipo que guarda copias (docs/espejo.md §3c):
  // Backblaze B2, S3 compatible, SFTP, una carpeta de red (SMB) o WebDAV, por
  // el rclone que acompaña al agente. Los datos viajan SELLADOS para el equipo
  // en `conectar_nube` (clave de administración): el servidor no los ve. El
  // equipo prueba que entra antes de guardarlos, protegidos, solo allí.
  import { onDestroy } from "svelte";
  import { Check, LoaderCircle, Server, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { avisar } from "$lib/avisos.svelte";
  import { cargarCliente } from "$lib/estado.svelte";
  import { nombreNubeValido } from "$lib/nubes";
  import { CAMPOS_DESTINO, errorCampoDestino, TIPOS_NUBE } from "$lib/espejo";
  import { ErrorLlavesCambiadas, mandarOrden } from "$lib/ordenar";
  import type { Cliente, Equipo, Orden } from "$lib/tipos";
  import AlertaLlaves from "./AlertaLlaves.svelte";
  import Ayuda from "./Ayuda.svelte";
  import CampoClave from "./CampoClave.svelte";

  let { cliente, equipo, onclose }: { cliente: Cliente; equipo: Equipo; onclose: () => void } = $props();

  type Tipo = keyof typeof CAMPOS_DESTINO;
  const TIPOS: Tipo[] = ["b2", "s3", "sftp", "smb", "webdav"];
  const existentes = $derived((equipo.resumen?.guarda_copias?.nubes ?? []).map((n) => n.nombre.toLowerCase()));
  let tipo = $state<Tipo>("b2");
  let nombre = $state("");
  let valores = $state<Record<string, string>>({});
  let carpeta = $state("");
  let claveAdmin = $state("");
  let ocupado = $state(false);
  let pasoTxt = $state("");
  let error = $state("");
  let cambiadas = $state(false);
  let hecho = $state(false);

  const campos = $derived(CAMPOS_DESTINO[tipo]);
  const nombreFinal = $derived(nombre.trim() || `${TIPOS_NUBE[tipo].nombre} ${cliente.nombre}`.slice(0, 40));
  const nombreError = $derived(
    !nombreNubeValido(nombreFinal) ? "Usa letras, números, espacios, guiones o puntos (hasta 40)." : existentes.includes(nombreFinal.toLowerCase()) ? "Ya hay un destino con ese nombre en este equipo." : "",
  );
  const errores = $derived(Object.fromEntries(campos.map((c) => [c.clave, errorCampoDestino(tipo, c.clave, valores[c.clave] ?? "")])));
  const carpetaError = $derived(carpeta.trim() && (carpeta.includes(":") || carpeta.split("/").some((x) => x === ".." || x === ".")) ? "Una ruta con «/», sin «..» ni «:»." : null);
  const valido = $derived(!nombreError && !carpetaError && Object.values(errores).every((e) => !e) && !!claveAdmin);

  onDestroy(() => {
    claveAdmin = "";
    valores = {};
  });

  function cambiarTipo(t: Tipo) {
    tipo = t;
    // Los datos de un tipo no pasan a otro (sobre todo las contraseñas).
    valores = {};
    error = "";
  }

  async function respuesta(o: Orden): Promise<Orden> {
    for (let i = 0; i < 80; i++) {
      const x = (await api.ordenesEquipo(cliente.id, equipo.id, 10)).find((y) => y.id === o.id);
      if (x && ["hecha", "fallida", "rechazada", "cancelada", "caducada"].includes(x.estado)) return x;
      await new Promise((r) => setTimeout(r, 1500));
    }
    throw new Error(`${equipo.nombre} no ha respondido todavía. Mira sus órdenes en un momento.`);
  }

  async function conectar(e: SubmitEvent) {
    e.preventDefault();
    if (!valido) return;
    error = "";
    ocupado = true;
    try {
      const parametros: Record<string, string> = {};
      for (const c of campos) {
        const v = c.secreto ? (valores[c.clave] ?? "") : (valores[c.clave] ?? "").trim();
        if (v) parametros[c.clave] = v;
      }
      const o = await mandarOrden({
        cliente,
        equipo,
        tipo: "conectar_nube",
        cuerpo: { tipo, nombre: nombreFinal, parametros, ...(carpeta.trim() ? { carpeta_prueba: carpeta.trim().replace(/^\/+|\/+$/g, "") } : {}) },
        secretos: { claveAdmin },
        alPaso: (t) => (pasoTxt = t),
      });
      pasoTxt = `${equipo.nombre} está probando que entra…`;
      const r = await respuesta(o);
      if (r.estado !== "hecha") throw new Error(r.mensaje ?? `${equipo.nombre} no pudo conectar ese destino.`);
      claveAdmin = "";
      valores = {};
      hecho = true;
      avisar(`«${nombreFinal}» conectado en ${equipo.nombre}.`);
      void cargarCliente(cliente.id, { silencioso: true });
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) cambiadas = true;
      else error = (err as Error).message;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }
</script>

<Modal labelledby="t-destino" {onclose} width={560} dismissible={false}>
  <div class="dlg-title">
    <span class="ticon"><Server size={18} /></span>
    <div>
      <h2 id="t-destino">Conectar otro destino en {equipo.nombre}</h2>
      <p>Para el espejo de lo que guarda este equipo. Los datos viajan cifrados solo para el equipo y se guardan protegidos allí: el servidor no los ve.</p>
    </div>
  </div>

  {#if hecho}
    <div class="form">
      <div class="notice notice-success" role="status"><Check size={16} /><p>«{nombreFinal}» está conectado y el equipo ha podido entrar. Ya puedes añadirlo como destino del espejo.</p></div>
      <footer><button class="btn btn-primary" onclick={onclose}>Hecho</button></footer>
    </div>
  {:else}
    <form class="form" onsubmit={conectar}>
      <div class="field">
        <label class="field-label" for="cd-tipo">Tipo</label>
        <select id="cd-tipo" class="input" value={tipo} onchange={(e) => cambiarTipo(e.currentTarget.value as Tipo)}>
          {#each TIPOS as t (t)}<option value={t}>{TIPOS_NUBE[t].nombre}</option>{/each}
        </select>
        <span class="field-hint">
          {#if TIPOS_NUBE[tipo].inmutable}Si el bucket tiene bloqueo de objetos (Object Lock), márcalo al añadirlo al espejo: así nunca se intenta borrar allí.{:else}No es inmutable: quien tenga sus credenciales puede borrar lo copiado. Conviene que otro destino sí lo sea.{/if}
          <Ayuda id="espejo" />
        </span>
      </div>
      <div class="field">
        <label class="field-label" for="cd-nombre">Nombre</label>
        <input id="cd-nombre" class="input" bind:value={nombre} placeholder={nombreFinal} />
        {#if nombreError}<p class="error-campo">{nombreError}</p>{:else}<span class="field-hint">Así lo verás al elegir los destinos del espejo.</span>{/if}
      </div>
      {#each campos as c (tipo + c.clave)}
        {#if c.secreto}
          <CampoClave requerido id="cd-{c.clave}" etiqueta={c.etiqueta} bind:value={valores[c.clave]} ayuda={c.ayuda} />
        {:else}
          <div class="field">
            <label class="field-label" for="cd-{c.clave}">{c.etiqueta}{#if c.opcional}<span class="faint"> (opcional)</span>{/if}</label>
            {#if c.opciones}
              <select id="cd-{c.clave}" class="input" bind:value={valores[c.clave]}>
                <option value="">—</option>
                {#each c.opciones as op (op)}<option value={op}>{op}</option>{/each}
              </select>
            {:else if c.largo}
              <textarea id="cd-{c.clave}" class="input mono" rows="3" bind:value={valores[c.clave]} placeholder={c.ejemplo} spellcheck="false"></textarea>
            {:else}
              <input id="cd-{c.clave}" class="input" class:mono={c.clave !== "usuario"} bind:value={valores[c.clave]} placeholder={c.ejemplo} autocomplete="off" spellcheck="false" />
            {/if}
            {#if valores[c.clave] && errores[c.clave]}<p class="error-campo">{errores[c.clave]}</p>{:else if c.ayuda}<span class="field-hint">{c.ayuda}</span>{/if}
          </div>
        {/if}
      {/each}
      <div class="field">
        <label class="field-label" for="cd-carpeta">Carpeta del espejo <span class="faint">(opcional, para probar que entra)</span></label>
        <input id="cd-carpeta" class="input mono" bind:value={carpeta} placeholder={tipo === "b2" || tipo === "s3" ? "bucket/Resguardo" : tipo === "smb" ? "recurso/Resguardo" : "Resguardo"} spellcheck="false" />
        {#if carpetaError}<p class="error-campo">{carpetaError}</p>{:else}<span class="field-hint">{tipo === "b2" || tipo === "s3" ? "El bucket y, si quieres, una carpeta dentro." : tipo === "smb" ? "El recurso compartido y, si quieres, una carpeta dentro." : "Dentro del espacio de ese usuario."}</span>{/if}
      </div>
      <CampoClave requerido id="cd-clave" etiqueta="Clave de administración" bind:value={claveAdmin}>
        {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
      </CampoClave>
      {#if cambiadas}<AlertaLlaves {equipo} cliente={cliente.id} />{/if}
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
        <button class="btn btn-primary" disabled={ocupado || !valido}>Conectar y probar</button>
      </footer>
    </form>
  {/if}
</Modal>

<style>
  .mono {
    font-family: var(--mono, ui-monospace, monospace);
  }
  textarea.input {
    resize: vertical;
    min-height: 4.5rem;
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
