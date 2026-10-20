<script lang="ts" module>
  import type { TrabajoEspejo } from "$lib/espejoTrabajos";
  /** Qué se renombra: una copia, una copia derivada o un espejo (con la lista de quien lo hace). */
  export type QueRenombrar =
    | { tipo: "copia"; id: string; nombre: string }
    | { tipo: "derivada"; repo: string; id: string; nombre: string }
    | { tipo: "espejo"; trabajo: TrabajoEspejo; quien: "almacen" | "equipo"; lista: TrabajoEspejo[] };
</script>

<script lang="ts">
  // 0.8.1 (15t): cambiar el nombre de una copia, de una copia derivada o de un espejo con
  // un diálogo pequeño: el nombre nuevo y la clave de administración (para que nadie
  // confunda las copias). Sin espera: no reduce la protección. Lo guarda el equipo que la
  // hace y lo ven igual todas sus consolas, también en el historial («"X" ahora se llama "Y"»).
  //
  // - Copia y derivada: orden `nombre_copia` (agente con `admite: "nombre_copia"`). Con un
  //   agente anterior, la copia se renombra en el editor de copias (enlace) y la derivada no.
  // - Espejo: la lista de espejos de quien lo hace con el nombre nuevo (`guarda_copias`).
  import { Pencil, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { pedirAlEquipo } from "$lib/pedirAlEquipo";
  import { avisar } from "$lib/avisos.svelte";
  import { cargarCliente } from "$lib/estado.svelte";
  import { ErrorLlavesCambiadas } from "$lib/ordenar";
  import { conTrabajo, cuerpoAlmacen, cuerpoEquipo, paraOrden } from "$lib/espejoTrabajos";
  import type { Cliente, Equipo } from "$lib/tipos";
  import BotonFalta from "../BotonFalta.svelte";
  import CampoClave from "../CampoClave.svelte";
  import AlertaLlaves from "../AlertaLlaves.svelte";

  let { cliente, equipo, que, alCambiar, onclose }: { cliente: Cliente; equipo: Equipo; que: QueRenombrar; alCambiar?: () => void; onclose: () => void } = $props();

  /** Como el agente (`MAX_NOMBRE_COPIA`) y el editor de espejos. */
  const MAX = 60;
  const ADMITE = "nombre_copia";
  const actualNombre = () => (que.tipo === "espejo" ? que.trabajo.nombre : que.nombre);
  // svelte-ignore state_referenced_locally
  let nombre = $state(actualNombre());
  let clave = $state("");
  let ocupado = $state(false);
  let error = $state("");
  let llaves = $state(false);
  const limpio = $derived(nombre.trim());
  const admite = $derived(que.tipo === "espejo" || !!equipo.resumen?.admite?.includes(ADMITE));
  const QUE = { copia: "la copia", derivada: "la copia derivada", espejo: "el espejo" } as const;
  const falta = $derived(
    !limpio
      ? { texto: "Falta el nombre", campo: "rc-nombre" }
      : [...limpio].length > MAX
        ? { texto: `Hasta ${MAX} caracteres`, campo: "rc-nombre" }
        : limpio === actualNombre().trim()
          ? { texto: "Escribe otro nombre", campo: "rc-nombre" }
          : !clave
            ? { texto: "Falta la clave de administración", campo: "rc-clave" }
            : null,
  );

  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    if (falta || ocupado) return;
    ocupado = true;
    error = "";
    try {
      let tipo = "nombre_copia";
      let cuerpo: Record<string, unknown>;
      if (que.tipo === "copia") cuerpo = { copia: que.id, nombre: limpio };
      else if (que.tipo === "derivada") cuerpo = { derivada: { repo: que.repo, id: que.id }, nombre: limpio };
      else {
        tipo = "guarda_copias";
        const nuevos = conTrabajo(que.lista.map(paraOrden), { ...paraOrden(que.trabajo), nombre: limpio });
        cuerpo = que.quien === "almacen" ? cuerpoAlmacen(nuevos) : cuerpoEquipo(nuevos);
      }
      const r = await pedirAlEquipo(cliente, equipo, tipo, cuerpo, 20, { claveAdmin: clave });
      clave = "";
      avisar(r.hecha ? `Ahora se llama «${limpio}» en todas las consolas de ${equipo.nombre}.` : r.texto);
      void cargarCliente(cliente.id, { silencioso: true });
      alCambiar?.();
      onclose();
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) llaves = true;
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }
</script>

<Modal labelledby="t-renombrar-copia" {onclose} width={480}>
  <div class="dlg-title">
    <span class="ticon"><Pencil size={18} /></span>
    <div>
      <h2 id="t-renombrar-copia">Cambiar el nombre de {QUE[que.tipo]}</h2>
      <p>Lo guarda {equipo.nombre} y lo verán igual todas sus consolas, también en el historial. No cambia nada más: ni sus versiones ni lo que copia.</p>
    </div>
  </div>
  {#if llaves}
    <AlertaLlaves {equipo} cliente={cliente.id} />
  {:else if !admite}
    {#if que.tipo === "copia"}
      <p class="faint">El agente de {equipo.nombre} aún no cambia solo el nombre: hazlo en el editor de copias (también con la clave) o actualiza el agente.</p>
      <footer>
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cerrar</button>
        <a class="btn btn-primary" href="/c/{cliente.id}/equipos/{equipo.id}/copias?copia={encodeURIComponent(que.id)}">Abrir el editor de copias</a>
      </footer>
    {:else}
      <p class="faint">Actualiza el agente de {equipo.nombre} para poner nombre a sus copias derivadas.</p>
      <footer><button type="button" class="btn btn-primary" onclick={onclose}>Cerrar</button></footer>
    {/if}
  {:else}
    <form class="form" onsubmit={guardar}>
      <div class="field">
        <label class="field-label" for="rc-nombre">Nombre</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input id="rc-nombre" class="input" bind:value={nombre} maxlength={MAX} autofocus disabled={ocupado} autocomplete="off" />
      </div>
      <CampoClave id="rc-clave" etiqueta="Clave de administración" bind:value={clave} requerido ayuda="Para que nadie confunda las copias. Va cifrada solo para el equipo." />
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
        <BotonFalta disabled={ocupado} {falta}>{ocupado ? "Guardando…" : "Cambiar el nombre"}</BotonFalta>
      </footer>
    </form>
  {/if}
</Modal>
