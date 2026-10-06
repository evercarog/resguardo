<script lang="ts">
  // «Nuevo destino» (tarea 7a, docs/copias-en-cadena.md): crear un destino sin
  // crear todavía un repositorio.
  //
  // - Otra zona de un almacén (otro disco): orden al almacén (NuevaZona).
  // - Nube o servidor para los repositorios (B2, S3, un rest-server de fuera):
  //   solo el catálogo del cliente, con nombre y dirección. Las credenciales
  //   se piden al crear el primer repositorio allí y van selladas solo para
  //   ese equipo: el servidor nunca las ve.
  // - Dropbox, Drive, NAS o WebDAV: se conectan en un almacén (para su espejo).
  import { Cloud, HardDrive, Server, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import BotonCargando from "./BotonCargando.svelte";
  import NuevaZona from "./NuevaZona.svelte";
  import Ayuda from "./Ayuda.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { guardarEnCatalogo } from "$lib/catalogoDestinos.svelte";
  import { admiteZonas, almacenesDe, errorDondeCatalogo, errorNombreDestino, idDestinoNuevo, type TipoSuelto } from "$lib/destinos";
  import type { Cliente, Equipo } from "$lib/tipos";

  let { cliente, equipos, onclose, alCambiar }: { cliente: Cliente; equipos: Equipo[]; onclose: () => void; alCambiar?: () => void } = $props();

  type Clase = "zona" | "red" | "nube";
  const almacenes = $derived(almacenesDe(equipos));
  const conZonas = $derived(almacenes.filter(admiteZonas));
  // Lo que se ofrece primero (después manda lo que se elija aquí).
  const claseInicial = (): Clase => (almacenesDe(equipos).some(admiteZonas) ? "zona" : "red");
  let clase = $state<Clase>(claseInicial());
  const almacenInicial = () => almacenesDe(equipos).find(admiteZonas)?.id ?? "";
  let almacenId = $state(almacenInicial());
  const almacen = $derived(conZonas.find((a) => a.id === almacenId) ?? conZonas[0]);
  let zonaEn = $state<Equipo | null>(null);

  const TIPOS: { tipo: Exclude<TipoSuelto, "sftp">; texto: string; donde: string; ejemplo: string }[] = [
    { tipo: "b2", texto: "Backblaze B2", donde: "Bucket", ejemplo: "copias-oficina" },
    { tipo: "s3", texto: "S3 compatible (Wasabi, R2, MinIO…)", donde: "Servidor y bucket", ejemplo: "s3.wasabisys.com/copias-oficina" },
    { tipo: "rest", texto: "Servidor de copias de fuera", donde: "Dirección", ejemplo: "https://copias.ejemplo.com:8000" },
  ];
  let tipo = $state<Exclude<TipoSuelto, "sftp">>("b2");
  let nombre = $state("");
  let donde = $state("");
  let ocupado = $state(false);
  let error = $state("");
  const t = $derived(TIPOS.find((x) => x.tipo === tipo)!);
  const errorNombre = $derived(nombre ? errorNombreDestino(nombre) : null);
  const errorDonde = $derived(donde.trim() ? errorDondeCatalogo(tipo, donde) : null);
  const valido = $derived(!errorNombreDestino(nombre) && !errorDondeCatalogo(tipo, donde));

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    if (!valido) return;
    ocupado = true;
    error = "";
    try {
      await guardarEnCatalogo(cliente.id, idDestinoNuevo(), { nombre, tipo, donde });
      avisar(`«${nombre.trim()}» ya está en la lista de destinos. Elígelo al crear un repositorio.`);
      alCambiar?.();
      onclose();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }
</script>

{#if zonaEn}
  <NuevaZona {cliente} equipo={zonaEn} onclose={() => ((zonaEn = null), alCambiar?.(), onclose())} alTerminar={() => alCambiar?.()} />
{:else}
  <Modal labelledby="t-nuevo-destino" {onclose} width={560}>
    <div class="dlg-title">
      <span class="ticon"><Cloud size={18} /></span>
      <div>
        <h2 id="t-nuevo-destino">Nuevo destino</h2>
        <p>Dónde se guardarán repositorios. Se crea ahora y se usa al crear el primero. <Ayuda id="catalogo-destinos" /></p>
      </div>
    </div>
    <div class="form">
      <div class="clases" role="radiogroup" aria-label="Qué destino">
        <label class="clase" class:on={clase === "zona"}>
          <input type="radio" bind:group={clase} value="zona" />
          <HardDrive size={16} /><span><strong>Otro disco de un almacén</strong><span class="faint">Una zona más, con su puerto</span></span>
        </label>
        <label class="clase" class:on={clase === "red"}>
          <input type="radio" bind:group={clase} value="red" />
          <Server size={16} /><span><strong>Nube o servidor</strong><span class="faint">B2, S3 o un servidor de copias de fuera</span></span>
        </label>
        <label class="clase" class:on={clase === "nube"}>
          <input type="radio" bind:group={clase} value="nube" />
          <Cloud size={16} /><span><strong>Dropbox, Drive, NAS…</strong><span class="faint">Para el espejo de un almacén</span></span>
        </label>
      </div>

      {#if clase === "zona"}
        {#if conZonas.length}
          <div class="field">
            <label class="field-label" for="nd-almacen">Almacén</label>
            <select id="nd-almacen" class="input" bind:value={almacenId}>
              {#each conZonas as a (a.id)}<option value={a.id}>{a.nombre}</option>{/each}
            </select>
            <span class="field-hint">Otra carpeta, mejor en otro disco, con su propio servidor de solo añadir. Dos discos del mismo equipo no protegen de un robo o un incendio: pon también un destino fuera. <Ayuda id="zona" /></span>
          </div>
          <footer>
            <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
            <button type="button" class="btn btn-primary" disabled={!almacen} onclick={() => (zonaEn = almacen ?? null)}>Seguir</button>
          </footer>
        {:else}
          <p class="faint">{almacenes.length ? "Los almacenes de este cliente aún no pueden servir otro disco: actualiza su agente." : "Este cliente no tiene ningún almacén: primero, «Este equipo guarda copias» en la ficha de un equipo."}</p>
          <footer><button type="button" class="btn btn-ghost" onclick={onclose}>Cerrar</button></footer>
        {/if}
      {:else if clase === "red"}
        <form class="form" onsubmit={guardar}>
          <div class="field">
            <label class="field-label" for="nd-tipo">Tipo</label>
            <select id="nd-tipo" class="input" bind:value={tipo}>
              {#each TIPOS as x (x.tipo)}<option value={x.tipo}>{x.texto}</option>{/each}
            </select>
          </div>
          <div class="field">
            <label class="field-label" for="nd-nombre">Nombre</label>
            <input id="nd-nombre" class="input" bind:value={nombre} maxlength="80" placeholder="{t.texto} de la oficina" />
            {#if errorNombre}<p class="error-campo">{errorNombre}</p>{/if}
          </div>
          <div class="field">
            <label class="field-label" for="nd-donde">{t.donde}</label>
            <input id="nd-donde" class="input mono" bind:value={donde} spellcheck="false" placeholder={t.ejemplo} />
            {#if errorDonde}<p class="error-campo">{errorDonde}</p>{:else}<span class="field-hint">Sin usuario ni contraseña: se piden al crear el primer repositorio y van selladas solo para ese equipo. El servidor guarda el nombre, el tipo y esta dirección.</span>{/if}
          </div>
          {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
          <footer>
            <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
            <BotonCargando class="btn btn-primary" disabled={!valido} cargando={ocupado} textoCargando="Guardando…">Crear el destino</BotonCargando>
          </footer>
        </form>
      {:else}
        <p>Dropbox, Google Drive, un NAS (SMB), WebDAV, y también B2, S3 o SFTP para el espejo, se conectan <strong>en un almacén</strong>, con su permiso cifrado solo para él. Desde allí se copian al espejo. No son inmutables (salvo B2 y S3 con bloqueo de objetos): conviene que otro destino sí lo sea.</p>
        {#if almacenes.length}
          <ul class="ir">
            {#each almacenes as a (a.id)}<li><a class="btn btn-sm" href="/c/{cliente.id}/equipos/{a.id}" onclick={onclose}>Conectar en {a.nombre}</a></li>{/each}
          </ul>
        {:else}
          <p class="faint">Este cliente no tiene ningún almacén todavía.</p>
        {/if}
        <footer><button type="button" class="btn btn-ghost" onclick={onclose}>Cerrar</button></footer>
      {/if}
    </div>
  </Modal>
{/if}

<style>
  .clases {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-2);
  }
  .clase {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: var(--sp-3);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    font-size: var(--fs-sm);
  }
  .clase input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .clase:focus-within {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .clase.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .clase span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .ir {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }
  @media (max-width: 560px) {
    .clases {
      grid-template-columns: 1fr;
    }
  }
</style>
