<script lang="ts">
  // «Nuevo destino» (tarea 7a, docs/copias-en-cadena.md): crear un destino sin
  // crear todavía un repositorio.
  //
  // - Otra zona de un almacén (otro disco): orden al almacén (NuevaZona).
  // - Una nube o un servidor (B2, S3, Dropbox, un NAS, SFTP, WebDAV o un servidor
  //   de copias de fuera), todos al mismo nivel: el mismo formulario que el paso
  //   «Adónde» del espejo (ConectarDestino, plan 0.8.0 B2), con el bucket y la
  //   carpeta base aparte. Se deja solo en la lista (las credenciales se piden al
  //   crear un repositorio y van selladas solo para ese equipo) o se conecta ya en
  //   un almacén para sus espejos.
  import { Cloud, HardDrive } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import NuevaZona from "./NuevaZona.svelte";
  import ConectarDestino from "./ConectarDestino.svelte";
  import Ayuda from "./Ayuda.svelte";
  import { admiteZonas, almacenesDe } from "$lib/destinos";
  import type { Cliente, Equipo } from "$lib/tipos";

  let { cliente, equipos, onclose, alCambiar }: { cliente: Cliente; equipos: Equipo[]; onclose: () => void; alCambiar?: () => void } = $props();

  type Clase = "zona" | "nube";
  const almacenes = $derived(almacenesDe(equipos));
  const conZonas = $derived(almacenes.filter(admiteZonas));
  let clase = $state<Clase>("nube");
  const almacenInicial = () => almacenesDe(equipos).find(admiteZonas)?.id ?? "";
  let almacenId = $state(almacenInicial());
  const almacen = $derived(conZonas.find((a) => a.id === almacenId) ?? conZonas[0]);
  let zonaEn = $state<Equipo | null>(null);
  let formulario = $state(false);
</script>

{#if zonaEn}
  <NuevaZona {cliente} equipo={zonaEn} onclose={() => ((zonaEn = null), alCambiar?.(), onclose())} alTerminar={() => alCambiar?.()} />
{:else if formulario}
  <ConectarDestino {cliente} onclose={() => ((formulario = false), alCambiar?.(), onclose())} alConectar={() => alCambiar?.()} />
{:else}
  <Modal labelledby="t-nuevo-destino" {onclose} width={560}>
    <div class="dlg-title">
      <span class="ticon"><Cloud size={18} /></span>
      <div>
        <h2 id="t-nuevo-destino">Nuevo destino</h2>
        <p>Dónde se guardarán repositorios o espejos. Se crea ahora y se usa después. <Ayuda id="catalogo-destinos" /></p>
      </div>
    </div>
    <div class="form">
      <div class="clases" role="radiogroup" aria-label="Qué destino">
        <label class="clase" class:on={clase === "nube"}>
          <input type="radio" bind:group={clase} value="nube" />
          <Cloud size={16} /><span><strong>Una nube o un servidor</strong><span class="faint">B2, S3, Dropbox, un NAS, SFTP, WebDAV u otro servidor de copias</span></span>
        </label>
        <label class="clase" class:on={clase === "zona"}>
          <input type="radio" bind:group={clase} value="zona" />
          <HardDrive size={16} /><span><strong>Otro disco de un almacén</strong><span class="faint">Una zona más, con su puerto</span></span>
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
      {:else}
        <p class="faint">Con su bucket y su carpeta base aparte: el destino los recuerda y no te los vuelve a pedir.</p>
        <footer>
          <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
          <button type="button" class="btn btn-primary" onclick={() => (formulario = true)}>Seguir</button>
        </footer>
      {/if}
    </div>
  </Modal>
{/if}

<style>
  .clases {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
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
  @media (max-width: 560px) {
    .clases {
      grid-template-columns: 1fr;
    }
  }
</style>
