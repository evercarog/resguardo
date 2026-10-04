<script lang="ts">
  // «Recibir un cliente» (superusuario, en el servidor nuevo): se pega el
  // bloque que da el servidor antiguo (nombre, sal y espera), se crea el
  // cliente con la MISMA sal (así la clave de administración sigue valiendo)
  // y se muestra, una sola vez, el bloque con la ficha, la identidad y la
  // autoridad TLS de este servidor para pegarlo en el antiguo.
  import { goto } from "$app/navigation";
  import { ArrowRightLeft, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { cargarClientes } from "$lib/estado.svelte";
  import { deB64 } from "$lib/cripto/bytes";
  import { bloqueServidor, huellaCorta } from "$lib/servidores";
  import type { ClienteRecibido } from "$lib/tipos";
  import BloqueCopiable from "./BloqueCopiable.svelte";

  let { onclose, alTambien }: { onclose: () => void; alTambien?: () => void } = $props();

  let pegado = $state("");
  let usos = $state(100);
  let dias = $state(7);
  let ocupado = $state(false);
  let error = $state("");
  let hecho = $state<ClienteRecibido | null>(null);

  const datos = $derived.by(() => {
    if (!pegado.trim()) return null;
    try {
      const x = JSON.parse(pegado.trim()) as { nombre?: unknown; sal_cliente?: unknown; espera_min_horas?: unknown };
      if (typeof x.nombre !== "string" || !x.nombre.trim()) return "Falta el nombre del cliente.";
      if (typeof x.sal_cliente !== "string" || deB64(x.sal_cliente).length < 16) return "Falta la sal del cliente.";
      const espera = typeof x.espera_min_horas === "number" ? x.espera_min_horas : undefined;
      return { nombre: x.nombre.trim(), sal_cliente: x.sal_cliente, espera_min_horas: espera };
    } catch {
      return "No es un bloque válido: cópialo entero desde «Mover este cliente a otro servidor» en el servidor antiguo.";
    }
  });

  async function recibir(e: SubmitEvent) {
    e.preventDefault();
    if (!datos || typeof datos === "string") return;
    ocupado = true;
    error = "";
    try {
      hecho = await api.recibirCliente({ ...datos, usos, dias });
      await cargarClientes();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }
</script>

<Modal labelledby="t-recibir" {onclose} width={560} dismissible={false}>
  {#if !hecho}
    <form class="form" onsubmit={recibir}>
      <div class="dlg-title">
        <span class="ticon"><ArrowRightLeft size={18} /></span>
        <div>
          <h2 id="t-recibir">Recibir un cliente</h2>
          <p>Para traer a este servidor un cliente que hoy está en otro. En el antiguo, abre el cliente y elige «Mover este cliente a otro servidor»: te dará un bloque para pegar aquí.</p>
        </div>
      </div>
      {#if alTambien}
        <div class="notice notice-info tambien">
          <p><strong>¿Sin dejar la otra consola?</strong> Para gestionar sus equipos desde las dos a la vez (por ejemplo, la del cliente en su oficina y esta en línea), crea un código de conexión.</p>
          <button type="button" class="btn btn-sm" onclick={alTambien}>Gestionarlo también desde aquí…</button>
        </div>
      {/if}
      <div class="field">
        <label class="field-label" for="bloque-cliente">Bloque del cliente</label>
        <textarea id="bloque-cliente" class="input mono" rows="5" bind:value={pegado} placeholder={'{ "nombre": "…", "sal_cliente": "…", "espera_min_horas": 24 }'}></textarea>
        {#if typeof datos === "string"}<p class="error-campo">{datos}</p>
        {:else if datos}<span class="field-hint">Se creará «{datos.nombre}» con la misma sal: su clave de administración seguirá valiendo.</span>{/if}
      </div>
      <div class="dos">
        <div class="field">
          <label class="field-label" for="usos">Equipos que pueden llegar</label>
          <input id="usos" class="input" type="number" min="1" max="1000" bind:value={usos} />
        </div>
        <div class="field">
          <label class="field-label" for="dias">La ficha vale</label>
          <select id="dias" class="input" bind:value={dias}>
            {#each [1, 3, 7, 14, 30] as d (d)}<option value={d}>{d === 1 ? "1 día" : `${d} días`}</option>{/each}
          </select>
        </div>
      </div>
      {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
      <footer>
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
        <button class="btn btn-primary" disabled={ocupado || !datos || typeof datos === "string"}>{ocupado ? "Creando…" : "Recibir el cliente"}</button>
      </footer>
    </form>
  {:else}
    <div class="form">
      <div class="dlg-title">
        <span class="ticon"><ArrowRightLeft size={18} /></span>
        <div>
          <h2 id="t-recibir">«{hecho.cliente.nombre}» está listo para llegar</h2>
          <p>Copia este bloque y pégalo en el servidor antiguo, en «Mover este cliente a otro servidor». Desde allí se manda a cada equipo, sellado.</p>
        </div>
      </div>
      <BloqueCopiable texto={bloqueServidor(hecho)} etiqueta="Copiar el bloque" />
      <p class="faint pequeno">Identidad de este servidor: <code>{huellaCorta(hecho.servidor.identidad)}</code>. Compruébala con quien está en el antiguo. La ficha vale para {hecho.usos} equipos hasta el {new Date(hecho.caduca).toLocaleDateString("es")}.</p>
      <div class="notice notice-warn"><TriangleAlert size={16} /><p>La ficha solo se muestra ahora. Si cierras sin copiarla, pide otra desde el cliente («Servidor» → «Dar una ficha»).</p></div>
      <footer>
        <button class="btn btn-primary" onclick={() => (onclose(), goto(`/c/${hecho!.cliente.id}/servidor`))}>Hecho</button>
      </footer>
    </div>
  {/if}
</Modal>

<style>
  .tambien {
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }
  .tambien p {
    margin: 0;
  }
  .mono {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 12px;
  }
  .dos {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--sp-3);
  }
  .pequeno {
    margin: 0;
    font-size: var(--fs-xs);
  }
  @media (max-width: 520px) {
    .dos {
      grid-template-columns: 1fr;
    }
  }
</style>
