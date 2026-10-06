<script lang="ts">
  // v1.56: un almacén que guarda copias de equipos de aquí, pero que no está en esta
  // consola (se gestiona desde otra): el mapa no puede enseñar lo que hace (su espejo a
  // otro disco o a la nube). Para verlo, ese almacén tiene que conectarse también a
  // esta consola: aquí se da el código de conexión y allí se pega en «Conectar también
  // a otra consola…» (el mismo flujo de «Equipos que no están en todas tus consolas»,
  // docs/consolas-multiples.md §2.5, visto desde el otro lado).
  import { Link2 } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { puede } from "$lib/estado.svelte";
  import { lista } from "$lib/formato";
  import type { Cliente } from "$lib/tipos";
  import DarCodigoConexion from "./DarCodigoConexion.svelte";

  let { cliente, almacen, consolas, onclose }: { cliente: Cliente; almacen: string; consolas: string[]; onclose: () => void } = $props();

  let codigo = $state(false);
  const donde = $derived(consolas.length ? lista(consolas.map((c) => `«${c}»`)) : "la otra consola");
  // Dar un código de conexión: quien administra el cliente aquí (propietario o administrador).
  const puedeDar = $derived(puede.administrar(cliente.rol));
</script>

{#if codigo}
  <DarCodigoConexion {cliente} onclose={() => ((codigo = false), onclose())} />
{:else}
  <Modal labelledby="t-otra-consola" {onclose} width={520}>
    <div class="form">
      <div class="dlg-title">
        <span class="ticon"><Link2 size={18} /></span>
        <div>
          <h2 id="t-otra-consola">«{almacen}» no está en esta consola</h2>
          <p>Guarda copias de equipos de aquí, pero se gestiona desde {donde}. Por eso este mapa no enseña sus espejos (a otro disco o a la nube) ni si van bien.</p>
        </div>
      </div>
      <ol class="pasos">
        <li><strong>Aquí:</strong> pide un código de conexión de esta consola.</li>
        <li><strong>En {donde}:</strong> abre ese almacén y usa «Conectar también a otra consola…» (o «Conectar también…» en el aviso de equipos que no están en todas tus consolas). Pega el código, comprueba la huella y escribe la clave de administración.</li>
        <li>En cuanto informe aquí, el mapa enseña todo su camino.</li>
      </ol>
      <p class="faint nota">Nada cambia en el almacén hasta que alguien con la clave de administración lo conecte desde allí. Si no quieres verlo aquí, no hace falta hacer nada: las copias siguen igual.</p>
      <footer>
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cerrar</button>
        {#if puedeDar}<button type="button" class="btn btn-primary" onclick={() => (codigo = true)}>Dar un código de conexión…</button>{/if}
      </footer>
    </div>
  </Modal>
{/if}

<style>
  .pasos {
    display: grid;
    gap: 6px;
    margin: 0;
    padding-left: 1.3em;
    font-size: var(--fs-sm);
  }
  .nota {
    margin: 0;
    font-size: var(--fs-xs);
  }
</style>
