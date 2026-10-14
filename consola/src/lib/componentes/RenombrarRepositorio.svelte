<script lang="ts">
  // Cambiar el nombre que se ve de un repositorio (plan 0.8.0, B1): lo guarda el
  // equipo dueño (`nombre_repositorio`, inofensiva) y lo ven igual todas sus
  // consolas. No cambia su carpeta ni su id: el kit sigue valiendo.
  import { Pencil, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { pedirAlEquipo } from "$lib/pedirAlEquipo";
  import { avisar } from "$lib/avisos.svelte";
  import { cargarCliente } from "$lib/estado.svelte";
  import type { Cliente, Equipo, RepositorioResumen } from "$lib/tipos";
  import BotonFalta from "./BotonFalta.svelte";

  let { cliente, equipo, repo, alCambiar, onclose }: { cliente: Cliente; equipo: Equipo; repo: RepositorioResumen; alCambiar?: () => void; onclose: () => void } = $props();

  const inicial = () => repo.nombre;
  let nombre = $state(inicial());
  let ocupado = $state(false);
  let error = $state("");
  const limpio = $derived(nombre.trim());
  const falta = $derived(
    !limpio ? { texto: "Falta el nombre", campo: "rr-nombre" } : limpio.length > 80 ? { texto: "Hasta 80 caracteres", campo: "rr-nombre" } : limpio === repo.nombre ? { texto: "Escribe otro nombre", campo: "rr-nombre" } : null,
  );

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    if (falta) return;
    ocupado = true;
    error = "";
    try {
      const r = await pedirAlEquipo(cliente, equipo, "nombre_repositorio", { repo: repo.id, nombre: limpio });
      avisar(r.hecha ? `Ahora se llama «${limpio}» en todas las consolas de ${equipo.nombre}.` : r.texto);
      void cargarCliente(cliente.id, { silencioso: true });
      alCambiar?.();
      onclose();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }
</script>

<Modal labelledby="t-renombrar" {onclose} width={480}>
  <div class="dlg-title">
    <span class="ticon"><Pencil size={18} /></span>
    <div>
      <h2 id="t-renombrar">Cambiar el nombre del repositorio</h2>
      <p>Lo guarda {equipo.nombre} y lo verán igual todas sus consolas. Su carpeta no cambia: el kit de recuperación sigue valiendo.</p>
    </div>
  </div>
  <form class="form" onsubmit={guardar}>
    <div class="field">
      <label class="field-label" for="rr-nombre">Nombre</label>
      <!-- svelte-ignore a11y_autofocus -->
      <input id="rr-nombre" class="input" bind:value={nombre} maxlength="80" autofocus disabled={ocupado} />
    </div>
    {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
    <footer>
      <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
      <BotonFalta disabled={ocupado} {falta}>{ocupado ? "Guardando…" : "Cambiar el nombre"}</BotonFalta>
    </footer>
  </form>
</Modal>
