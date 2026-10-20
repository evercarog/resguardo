<script lang="ts">
  // 15hh: «Parar» (o «Pausar») lo que está en marcha desde la ventana del equipo. Siempre con
  // la clave de administración escrita en ese momento: parar deja datos sin proteger. Lo que
  // ya se copió se aprovecha la próxima vez; una pausa sigue sola a su hora. Queda en el
  // historial de todas las consolas, con la cuenta del equipo que lo pidió.
  import { CirclePause, CircleStop } from "@lucide/svelte";
  import { pedir } from "./puente.svelte";
  import CampoClave from "./ajustes/CampoClave.svelte";
  import type { Actividad } from "./tipos";

  let { a }: { a: Actividad } = $props();

  let abierto = $state(false);
  let como = $state<"parar" | "noche" | "horas" | "mano">("parar");
  let clave = $state("");
  let ocupado = $state(false);
  let error = $state("");
  let hecho = $state("");

  /** «Esta noche a las 22:00» (o mañana si ya pasó) y «en 2 horas». */
  function hasta(): string | null {
    const ahora = new Date();
    if (como === "noche") {
      const h = new Date(ahora);
      h.setHours(22, 0, 0, 0);
      if (h <= ahora) h.setDate(h.getDate() + 1);
      return h.toISOString();
    }
    if (como === "horas") return new Date(ahora.getTime() + 2 * 3600_000).toISOString();
    return null;
  }

  async function confirmar(ev: SubmitEvent) {
    ev.preventDefault();
    ocupado = true;
    error = "";
    try {
      const pausa = como === "parar" ? null : como === "mano" ? { a_mano: true } : { hasta: hasta() };
      const r = await pedir<{ mensaje: string }>("parar", { parar: a.parar, pausa, clave });
      hecho = r.mensaje;
      abierto = false;
    } catch (e) {
      error = (e as Error).message;
    } finally {
      clave = "";
      ocupado = false;
    }
  }
</script>

{#if hecho}
  <p class="v-ok" role="status">{hecho}</p>
{:else if !abierto}
  <div class="acciones">
    <button class="btn btn-secondary btn-sm" onclick={() => ((abierto = true), (como = "parar"))}>
      <CircleStop size={14} aria-hidden="true" /> Parar…
    </button>
    {#if a.pausable}
      <button class="btn btn-secondary btn-sm" onclick={() => ((abierto = true), (como = "noche"))}>
        <CirclePause size={14} aria-hidden="true" /> Pausar…
      </button>
    {/if}
  </div>
{:else}
  <form class="v-pila parar" onsubmit={confirmar}>
    {#if a.pausable}
      <fieldset class="opciones">
        <legend class="v-mini">Qué hacer</legend>
        <label><input type="radio" bind:group={como} value="parar" /> Pararla ahora</label>
        <label><input type="radio" bind:group={como} value="noche" /> Pausarla y seguir esta noche a las 22:00</label>
        <label><input type="radio" bind:group={como} value="horas" /> Pausarla y seguir en 2 horas</label>
        <label><input type="radio" bind:group={como} value="mano" /> Pausarla hasta que pulses «Copiar ahora»</label>
      </fieldset>
    {/if}
    <p class="v-sub">
      {como === "parar"
        ? "Se para en unos segundos. Lo copiado hasta ahora se aprovecha la próxima vez."
        : "Se para ahora y sigue sola a su hora, aprovechando lo ya copiado. Mientras, no cuenta como fallida."}
      Queda en el historial de todas las consolas.
    </p>
    <CampoClave etiqueta="Clave de administración" bind:valor={clave} />
    {#if error}<p class="v-error" role="alert">{error}</p>{/if}
    <div class="acciones">
      <button class="btn btn-primary btn-sm" type="submit" disabled={ocupado || !clave}>{como === "parar" ? "Parar" : "Pausar"}</button>
      <button class="btn btn-secondary btn-sm" type="button" onclick={() => ((abierto = false), (clave = error = ""))}>Cancelar</button>
    </div>
  </form>
{/if}

<style>
  .acciones {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }
  .acciones :global(svg) {
    vertical-align: -2px;
  }
  .opciones {
    display: grid;
    gap: var(--sp-1);
    margin: 0;
    padding: 0;
    border: 0;
  }
  .opciones label {
    display: flex;
    gap: var(--sp-2);
    align-items: center;
  }
  .parar {
    padding-top: var(--sp-2);
    border-top: 1px solid var(--border);
  }
</style>
