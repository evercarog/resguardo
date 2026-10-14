<script lang="ts">
  // 14a: «Órdenes esperando su turno» en el propio equipo (docs/consolas-multiples.md §5.11).
  // Las ve solo un administrador del equipo (lo comprueba el servicio con la cuenta que da
  // Windows, no la página). «Aprobar ahora» pide siempre la clave de administración (aunque
  // los ajustes estén desbloqueados): la aplica ya y queda en el historial de todas las
  // consolas, con un aviso en todas.
  import { onMount } from "svelte";
  import { Clock, MonitorCheck, X } from "@lucide/svelte";
  import { cuandoFrase } from "$lib/formato";
  import { pedir, vivo } from "./puente.svelte";
  import CampoClave from "./ajustes/CampoClave.svelte";
  import type { OrdenLocal } from "./tipos";

  let ordenes = $state<OrdenLocal[]>([]);
  let abierta = $state<string | null>(null);
  let clave = $state("");
  let ocupado = $state(false);
  let error = $state("");
  let hecho = $state("");

  async function cargar() {
    try {
      ordenes = (await pedir<{ ordenes: OrdenLocal[] }>("en_espera")).ordenes ?? [];
    } catch {
      // Una cuenta que no es administradora (o un servicio anterior): no se enseña nada.
      ordenes = [];
    }
  }
  onMount(() => {
    void cargar();
    const t = setInterval(() => void cargar(), 20_000);
    return () => clearInterval(t);
  });

  function abrir(o: OrdenLocal) {
    abierta = o.id;
    clave = error = hecho = "";
  }
  function cerrar() {
    abierta = null;
    clave = error = "";
  }

  async function aprobar(ev: SubmitEvent, o: OrdenLocal) {
    ev.preventDefault();
    ocupado = true;
    error = "";
    try {
      const r = await pedir<{ estado: string; mensaje: string }>("aprobar", { orden: o.id, clave });
      clave = "";
      abierta = null;
      hecho = r.mensaje;
      await cargar();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      ocupado = false;
    }
  }

  const ahora = $derived(vivo.datos?.ahora ? Date.parse(vivo.datos.ahora) : Date.now());
  const desde = (o: OrdenLocal) => (o.consola ? `Desde la consola «${o.consola}»` : "Desde otra consola");
</script>

{#if ordenes.length || hecho}
  <section class="v-tarjeta espera v-pila" aria-labelledby="t-espera">
    <div class="v-fila">
      <Clock size={18} aria-hidden="true" class="ico" />
      <h2 class="v-titulo" id="t-espera">Órdenes esperando su turno</h2>
    </div>
    {#if hecho}<p class="v-ok" role="status">{hecho}</p>{/if}
    {#each ordenes as o (o.id)}
      <article class="orden">
        <p class="que">{o.descripcion}</p>
        <p class="v-sub">
          {desde(o)}{o.por ? `, pedida por ${o.por}` : ""}.
          {#if o.repositorio}Repositorio «{o.repositorio}».{/if}
          Se aplica {cuandoFrase(o.aplica, ahora)}.
        </p>
        {#if !o.aprobable}
          <p class="v-mini">Su cliente no deja aprobarla en el equipo: se aplica a su hora si nadie la cancela desde una consola.</p>
        {:else if abierta === o.id}
          <form class="v-pila" onsubmit={(ev) => aprobar(ev, o)}>
            <p class="v-sub">
              Se aplicará ya, sin esperar a su hora. Quedará en el historial de todas las consolas como aprobada en este equipo por tu cuenta de Windows, con un aviso en todas.
            </p>
            <CampoClave etiqueta="Clave de administración" bind:valor={clave} />
            {#if error}<p class="v-error" role="alert">{error}</p>{/if}
            <div class="botones">
              <button type="button" class="btn btn-ghost" onclick={cerrar}><X size={15} aria-hidden="true" />No aprobarla</button>
              <button class="btn btn-primary" disabled={ocupado || !clave}>{ocupado ? "Aplicando…" : "Aprobar y aplicar ya"}</button>
            </div>
          </form>
        {:else}
          <div class="botones">
            <button class="btn btn-sm" onclick={() => abrir(o)}><MonitorCheck size={14} aria-hidden="true" />Aprobar ahora</button>
          </div>
        {/if}
      </article>
    {/each}
    {#if ordenes.length}
      <p class="v-mini">Si no esperabas alguna, cancélala desde cualquier consola y cambia la clave de administración.</p>
    {/if}
  </section>
{/if}

<style>
  .espera {
    border-color: color-mix(in srgb, var(--warn) 30%, var(--border));
    background: color-mix(in srgb, var(--warn) 4%, var(--surface));
  }
  .espera :global(.ico) {
    color: var(--warn);
  }
  .orden {
    display: grid;
    gap: 4px;
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .que {
    margin: 0;
    font-weight: 500;
  }
  .v-mini {
    margin: 0;
  }
  .botones {
    display: flex;
    justify-content: flex-end;
    gap: var(--sp-2);
    margin-top: var(--sp-1);
  }
</style>
