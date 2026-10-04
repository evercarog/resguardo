<script lang="ts">
  // «Mover este cliente a otro servidor» (api-servidor.md §11), en el antiguo:
  // 1) se copia el bloque del cliente (nombre, sal y espera) para «Recibir un
  //    cliente» en el nuevo, que crea el cliente con la misma sal;
  // 2) se pega el bloque que da el nuevo (dirección, identidad, autoridad TLS
  //    y ficha) y, con la clave de administración, se manda cambiar_servidor
  //    a cada equipo, sellada: la ficha nunca pasa en claro por este servidor.
  // El progreso de cada equipo se sigue en la página «Servidor».
  import { onDestroy } from "svelte";
  import { ArrowRightLeft, KeyRound, LoaderCircle, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { ErrorLlavesCambiadas, mandarOrden } from "$lib/ordenar";
  import { huellaCorta, leerBloque } from "$lib/servidores";
  import { plural } from "$lib/formato";
  import type { Cliente, Equipo } from "$lib/tipos";
  import Ayuda from "./Ayuda.svelte";
  import AlertaLlaves from "./AlertaLlaves.svelte";
  import BloqueCopiable from "./BloqueCopiable.svelte";
  import CampoClave from "./CampoClave.svelte";

  let { cliente, equipos, onclose, alEnviar }: { cliente: Cliente; equipos: Equipo[]; onclose: () => void; alEnviar: (enviados: number) => void } = $props();

  let paso = $state<"cliente" | "destino">("cliente");
  let pegado = $state("");
  let elegidos = $state<Record<string, boolean>>({});
  let claveAdmin = $state("");
  let ocupado = $state(false);
  let pasoTxt = $state("");
  let error = $state("");
  let cambiadas = $state<Equipo | null>(null);
  let entendido = $state(false);

  const activos = $derived(equipos.filter((e) => e.confirmado && e.modo !== "trasladado"));
  $effect(() => {
    for (const e of activos) if (!(e.id in elegidos)) elegidos[e.id] = true;
  });
  const destino = $derived(pegado.trim() ? leerBloque(pegado) : null);
  const aMover = $derived(activos.filter((e) => elegidos[e.id]));
  const bloqueCliente = $derived(JSON.stringify({ nombre: cliente.nombre, sal_cliente: cliente.sal_cliente, espera_min_horas: cliente.espera_min_horas }, null, 2));

  onDestroy(() => {
    claveAdmin = pegado = "";
  });

  async function mover(e: SubmitEvent) {
    e.preventDefault();
    if (!destino || typeof destino === "string") return;
    error = "";
    ocupado = true;
    let hechos = 0;
    try {
      for (const eq of aMover) {
        pasoTxt = `${eq.nombre} (${hechos + 1} de ${aMover.length})…`;
        await mandarOrden({
          cliente,
          equipo: eq,
          tipo: "cambiar_servidor",
          cuerpo: { url: destino.url, identidad: destino.identidad, ca_pem: destino.ca_pem, ficha: destino.ficha },
          secretos: { claveAdmin },
          alPaso: (t) => (pasoTxt = `${eq.nombre}: ${t.toLowerCase()}`),
        });
        hechos++;
      }
      claveAdmin = pegado = "";
      alEnviar(hechos);
      onclose();
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) cambiadas = aMover[hechos] ?? null;
      else error = `${(err as Error).message}${hechos ? ` (ya se mandó a ${plural(hechos, "equipo", "equipos")})` : ""}`;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }
</script>

<Modal labelledby="t-mover" {onclose} width={600} dismissible={false}>
  <div class="dlg-title">
    <span class="ticon"><ArrowRightLeft size={18} /></span>
    <div>
      <h2 id="t-mover">Mover «{cliente.nombre}» a otro servidor</h2>
      <p>Los equipos se van solos al nuevo y conservan sus llaves, sus copias y su configuración. La clave de administración sigue siendo la misma.</p>
    </div>
  </div>

  {#if paso === "cliente"}
    <div class="form">
      <ol class="guia">
        <li>En el <strong>servidor nuevo</strong>, entra como superusuario y ve a <strong>Clientes → Recibir un cliente</strong>.</li>
        <li>Pega allí este bloque. No lleva ningún secreto: el nombre, la sal y la espera.</li>
      </ol>
      <BloqueCopiable texto={bloqueCliente} etiqueta="Copiar el bloque del cliente" alto={6} />
      <ol class="guia" start="3">
        <li>El nuevo te dará otro bloque (con una ficha de un solo uso). Tráelo aquí.</li>
      </ol>
      <footer>
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
        <button type="button" class="btn btn-primary" onclick={() => (paso = "destino")}>Ya tengo el bloque del servidor nuevo</button>
      </footer>
    </div>
  {:else}
    <form class="form" onsubmit={mover}>
      <div class="field">
        <label class="field-label" for="bloque-servidor">Bloque del servidor nuevo</label>
        <textarea id="bloque-servidor" class="input mono" rows="6" bind:value={pegado} placeholder={'{ "url": "https://…", "identidad": "…", "ca_pem": "-----BEGIN CERTIFICATE-----…", "ficha": "…" }'}></textarea>
        {#if typeof destino === "string"}<p class="error-campo">{destino}</p>
        {:else if destino}
          <span class="field-hint">Irán a <strong>{destino.url}</strong>, identidad <code>{huellaCorta(destino.identidad)}</code>. Compruébala con quien lo administra.</span>
        {/if}
      </div>

      <fieldset class="field equipos">
        <legend class="field-label">Equipos que se mueven</legend>
        {#each activos as e (e.id)}
          <label class="check"><input type="checkbox" bind:checked={elegidos[e.id]} />{e.nombre}{#if !e.conectado}<span class="faint">{" · "}desconectado: se moverá cuando vuelva</span>{/if}</label>
        {:else}
          <p class="faint">No hay equipos confirmados que mover.</p>
        {/each}
      </fieldset>

      <div class="notice notice-info">
        <p>Cada equipo contesta «en marcha», se da de alta en el nuevo, comprueba su identidad, sube su configuración y solo entonces deja este. Si en 24 h no lo consigue, sigue aquí y se crea un aviso.</p>
      </div>
      <label class="check"><input type="checkbox" bind:checked={entendido} />Entiendo que, al llegar, los equipos dejarán de recibir órdenes desde este servidor.</label>

      <CampoClave requerido id="clave-admin" etiqueta="Clave de administración" bind:value={claveAdmin}>
        {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
      </CampoClave>
      {#if cambiadas}<AlertaLlaves equipo={cambiadas} cliente={cliente.id} />{/if}
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={() => (paso = "cliente")}>Atrás</button>
        <button class="btn btn-primary" disabled={ocupado || !destino || typeof destino === "string" || !aMover.length || !entendido || !claveAdmin}>
          <KeyRound size={15} />Mover {plural(aMover.length, "equipo", "equipos")}
        </button>
      </footer>
    </form>
  {/if}
</Modal>

<style>
  .guia {
    margin: 0;
    padding-left: 1.3em;
    color: var(--text-2);
    font-size: var(--fs-sm);
    line-height: 1.6;
  }
  .mono {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 12px;
  }
  .equipos {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    border: none;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
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
