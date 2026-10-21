<script lang="ts">
  // C1 (plan 0.8.2, 15uu): «Consolas» en Ajustes. La lista de las consolas que gestionan
  // este equipo, «Quitar» por consola y «Usar sin consola» (dejarlas todas). Solo la ve un
  // administrador del equipo (lo comprueba el servicio con la cuenta que da Windows, no la
  // página). Dejar una pide siempre la clave de administración escrita en ese momento (si
  // el equipo la tiene) y la consola lo sabe: «dejó esta consola desde el propio equipo».
  import { onMount } from "svelte";
  import { LogOut, Server, X } from "@lucide/svelte";
  import { plural, relativo } from "$lib/formato";
  import { pedir } from "../puente.svelte";
  import CampoClave from "./CampoClave.svelte";
  import type { ConsolaLocal } from "../tipos";

  let { alCambiar }: { alCambiar?: () => void } = $props();

  let consolas = $state<ConsolaLocal[] | null>(null);
  let conClave = $state(true);
  let noPuede = $state("");
  /** La que se va a dejar: su identidad o «todas». */
  let abierta = $state<string | null>(null);
  let clave = $state("");
  let ocupado = $state(false);
  let error = $state("");
  let hecho = $state("");

  async function cargar() {
    try {
      const r = await pedir<{ consolas: ConsolaLocal[]; con_clave: boolean }>("consolas");
      consolas = r.consolas ?? [];
      conClave = r.con_clave;
      noPuede = "";
    } catch (e) {
      // Una cuenta que no es administradora (o un servicio anterior, que no lo sabe).
      consolas = [];
      noPuede = (e as Error).message;
    }
  }
  onMount(() => void cargar());

  function abrir(que: string) {
    abierta = que;
    clave = error = hecho = "";
  }
  function cerrar() {
    abierta = null;
    clave = error = "";
  }

  const todas = $derived(abierta === "todas");
  const elegida = $derived(consolas?.find((c) => c.identidad === abierta) ?? null);
  const enEspera = $derived(todas ? (consolas ?? []).reduce((n, c) => n + c.en_espera, 0) : (elegida?.en_espera ?? 0));
  const siguen = $derived((consolas ?? []).filter((c) => c.identidad !== abierta).map((c) => `«${c.nombre}»`));

  async function dejar(ev: SubmitEvent) {
    ev.preventDefault();
    ocupado = true;
    error = "";
    try {
      const r = await pedir<{ mensaje: string }>("dejar_consola", todas ? { todas: true, clave } : { consola: abierta, clave });
      clave = "";
      abierta = null;
      hecho = r.mensaje;
      await cargar();
      alCambiar?.();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      ocupado = false;
    }
  }
</script>

{#if consolas === null}
  <p class="v-sub">Preguntando al servicio…</p>
{:else if noPuede}
  <section class="v-tarjeta v-pila" aria-labelledby="t-consolas">
    <div class="v-fila"><Server size={18} aria-hidden="true" /><h2 class="v-titulo" id="t-consolas">Consolas</h2></div>
    <p class="v-sub">{noPuede}</p>
  </section>
{:else if consolas.length || hecho}
  <section class="v-tarjeta v-pila" aria-labelledby="t-consolas">
    <div class="v-fila"><Server size={18} aria-hidden="true" /><h2 class="v-titulo" id="t-consolas">Consolas</h2></div>
    {#if hecho}<p class="v-ok" role="status">{hecho}</p>{/if}
    {#if consolas.length}
      <p class="v-sub">{consolas.length === 1 ? "Este equipo lo gestiona una consola." : `Este equipo lo gestionan ${consolas.length} consolas a la vez.`}</p>
      <ul class="lista">
        {#each consolas as c (c.identidad)}
          <li>
            <div class="datos">
              <p class="nombre">{c.nombre}{#if c.principal && consolas.length > 1}<span class="v-mini"> · principal</span>{/if}</p>
              <p class="v-mini">{c.url} · huella {c.huella} · {c.contacto ? `contacto ${relativo(c.contacto)}` : "nunca respondió"}{c.en_espera ? ` · ${plural(c.en_espera, "orden en espera", "órdenes en espera")}` : ""}</p>
            </div>
            {#if consolas.length > 1 && abierta !== c.identidad}
              <button class="btn btn-sm" onclick={() => abrir(c.identidad)}><X size={14} aria-hidden="true" />Quitar</button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}

    {#if abierta}
      <form class="v-pila dejar" onsubmit={dejar}>
        <h3 class="v-titulo">{todas ? "Usar sin consola" : `Quitar «${elegida?.nombre ?? ""}»`}</h3>
        <ul class="v-sub que-pasa">
          <li><strong>Las copias siguen</strong> con su horario, como hasta ahora{todas ? ", pero sin consola: se cambian aquí, en Ajustes" : ""}.</li>
          <li><strong>Los destinos y el almacén no se tocan</strong>: lo ya copiado se queda donde está.</li>
          {#if todas}
            <li>Ninguna consola podrá mandarle órdenes ni ver sus copias. Para volver, se vincula otra vez con un código.</li>
          {:else}
            <li>Esa consola ya no podrá mandarle órdenes ni ver sus copias. Las demás ({siguen.join(", ")}) siguen igual.</li>
          {/if}
          {#if enEspera}
            <li>{enEspera === 1 ? "Su orden en espera no se aplicará" : `Sus ${enEspera} órdenes en espera no se aplicarán`}: quedan como canceladas en el historial{todas ? "" : " de las demás consolas"}.</li>
          {/if}
          <li>Lo que comparten las consolas del cliente (nombres, colores, plantillas…) se queda en el equipo{todas ? "" : " y en las demás"}.</li>
          <li>{todas ? "Cada consola" : "Esa consola"} lo verá como «dejó esta consola desde el propio equipo», con la fecha y tu cuenta de Windows, con un aviso.</li>
        </ul>
        {#if conClave}
          <CampoClave etiqueta="Clave de administración" bind:valor={clave} />
        {:else}
          <p class="v-mini">Este equipo no tiene clave de administración: basta con ser administrador de Windows.</p>
        {/if}
        {#if error}<p class="v-error" role="alert">{error}</p>{/if}
        <div class="botones">
          <button type="button" class="btn btn-ghost" onclick={cerrar}>No hacerlo</button>
          <button class="btn btn-danger" disabled={ocupado || (conClave && !clave)}>
            {ocupado ? "Avisando a la consola…" : todas ? "Usar sin consola" : "Quitar esta consola"}
          </button>
        </div>
      </form>
    {:else if consolas.length}
      <div class="botones">
        <button class="btn btn-sm" onclick={() => abrir("todas")}><LogOut size={14} aria-hidden="true" />Usar sin consola</button>
      </div>
      <p class="v-mini">«Usar sin consola» deja {consolas.length === 1 ? "la consola" : "todas las consolas"}: el equipo sigue copiando solo. Pide la clave de administración.</p>
    {/if}
  </section>
{/if}

<style>
  .lista {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--sp-2);
  }
  .lista li {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding-top: var(--sp-2);
    border-top: 1px solid var(--border);
  }
  .datos {
    flex: 1;
    min-width: 0;
  }
  .datos p {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .nombre {
    font-weight: 500;
  }
  .dejar {
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .que-pasa {
    margin: 0;
    padding-left: 1.2em;
    display: grid;
    gap: 4px;
  }
  .botones {
    display: flex;
    justify-content: flex-end;
    gap: var(--sp-2);
  }
  .v-mini {
    margin: 0;
  }
</style>
