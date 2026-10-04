<script lang="ts">
  import { tip } from "$lib/tooltip";
  // «Servidores de respaldo» (api-servidor.md §11): hasta tres servidores a
  // los que cada equipo se va solo si este no responde en `dias` días. Cada
  // uno con su bloque (dirección, identidad, autoridad TLS y una ficha larga
  // de ese servidor). Se manda a todos los equipos y sustituye la lista que
  // tuvieran; vacía, la quita. Las fichas solo viajan dentro de la orden sellada.
  import { onDestroy } from "svelte";
  import { KeyRound, LifeBuoy, LoaderCircle, Plus, Trash2, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { ErrorLlavesCambiadas, mandarOrden } from "$lib/ordenar";
  import { huellaCorta, leerBloque } from "$lib/servidores";
  import { plural } from "$lib/formato";
  import type { Cliente, Equipo } from "$lib/tipos";
  import Ayuda from "./Ayuda.svelte";
  import AlertaLlaves from "./AlertaLlaves.svelte";
  import CampoClave from "./CampoClave.svelte";

  let { cliente, equipos, onclose, alEnviar }: { cliente: Cliente; equipos: Equipo[]; onclose: () => void; alEnviar: (texto: string) => void } = $props();

  let bloques = $state<string[]>([""]);
  let dias = $state(3);
  let claveAdmin = $state("");
  let ocupado = $state(false);
  let pasoTxt = $state("");
  let error = $state("");
  let cambiadas = $state<Equipo | null>(null);

  const activos = $derived(equipos.filter((e) => e.confirmado && e.modo !== "trasladado"));
  const leidos = $derived(bloques.map((b) => (b.trim() ? leerBloque(b) : null)));
  const validos = $derived(leidos.filter((x) => x && typeof x !== "string") as Exclude<ReturnType<typeof leerBloque>, string>[]);
  const hayErrores = $derived(leidos.some((x) => typeof x === "string"));
  const repetidos = $derived(new Set(validos.map((v) => v.url)).size !== validos.length);

  onDestroy(() => {
    claveAdmin = "";
    bloques = [];
  });

  async function enviar(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    ocupado = true;
    let hechos = 0;
    try {
      const servidores = validos.map((v) => ({ url: v.url, identidad: v.identidad, ca_pem: v.ca_pem, ficha: v.ficha }));
      for (const eq of activos) {
        pasoTxt = `${eq.nombre} (${hechos + 1} de ${activos.length})…`;
        await mandarOrden({ cliente, equipo: eq, tipo: "servidores_respaldo", cuerpo: { servidores, dias }, secretos: { claveAdmin } });
        hechos++;
      }
      claveAdmin = "";
      bloques = [""];
      alEnviar(servidores.length ? `Respaldo mandado a ${plural(hechos, "equipo", "equipos")}: ${plural(servidores.length, "servidor", "servidores")}, tras ${plural(dias, "día", "días")} sin respuesta.` : `Pedido a ${plural(hechos, "equipo", "equipos")} que quiten sus servidores de respaldo.`);
      onclose();
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) cambiadas = activos[hechos] ?? null;
      else error = `${(err as Error).message}${hechos ? ` (ya se mandó a ${plural(hechos, "equipo", "equipos")})` : ""}`;
    } finally {
      ocupado = false;
      pasoTxt = "";
    }
  }
</script>

<Modal labelledby="t-respaldo" {onclose} width={600} dismissible={false}>
  <form class="form" onsubmit={enviar}>
    <div class="dlg-title">
      <span class="ticon"><LifeBuoy size={18} /></span>
      <div>
        <h2 id="t-respaldo">Servidores de respaldo <Ayuda id="respaldo" /></h2>
        <p>Si este servidor deja de responder, cada equipo se da de alta solo en el primero de la lista que le acepte. Se manda a {plural(activos.length, "equipo", "equipos")} y sustituye la lista que tuvieran.</p>
      </div>
    </div>
    <p class="faint pequeno">En cada servidor de respaldo: recibe allí este cliente («Recibir un cliente») y pide una ficha larga en «Servidor → Dar una ficha». Pega aquí su bloque.</p>

    {#each bloques as _, i (i)}
      {@const l = leidos[i]}
      <div class="field">
        <div class="label-row">
          <label class="field-label" for="resp-{i}">Servidor {i + 1}</label>
          <button type="button" class="icon-btn" aria-label="Quitar el servidor {i + 1}" use:tip={"Quitar"} onclick={() => (bloques = bloques.filter((_, j) => j !== i))}><Trash2 size={14} /></button>
        </div>
        <textarea id="resp-{i}" class="input mono" rows="4" bind:value={bloques[i]} placeholder={'{ "url": "https://…", "identidad": "…", "ca_pem": "…", "ficha": "…" }'}></textarea>
        {#if typeof l === "string"}<p class="error-campo">{l}</p>
        {:else if l}<span class="field-hint">{l.url} · identidad <span class="pastilla mono">{huellaCorta(l.identidad)}</span></span>{/if}
      </div>
    {/each}
    {#if bloques.length < 3}
      <button type="button" class="btn btn-sm btn-ghost anadir" onclick={() => (bloques = [...bloques, ""])}><Plus size={14} />Añadir otro servidor</button>
    {/if}
    {#if repetidos}<p class="error-campo">Hay un servidor repetido.</p>{/if}

    <div class="field">
      <label class="field-label" for="dias-resp">Irse tras</label>
      <select id="dias-resp" class="input" bind:value={dias}>
        {#each [1, 2, 3, 5, 7, 14, 30] as d (d)}<option value={d}>{d === 1 ? "1 día" : `${d} días`} sin respuesta{d === 3 ? " (recomendado)" : ""}</option>{/each}
      </select>
    </div>
    {#if !validos.length && !hayErrores}
      <div class="notice notice-warn"><TriangleAlert size={16} /><p>Sin ningún servidor, los equipos quitarán los respaldos que tuvieran.</p></div>
    {/if}

    <CampoClave requerido id="clave-admin" etiqueta="Clave de administración" bind:value={claveAdmin}>
      {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
    </CampoClave>
    {#if cambiadas}<AlertaLlaves equipo={cambiadas} cliente={cliente.id} />{/if}
    {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
    <footer>
      {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
      <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
      <button class="btn btn-primary" disabled={ocupado || hayErrores || repetidos || !activos.length || !claveAdmin}>
        <KeyRound size={15} />{validos.length ? "Mandar a los equipos" : "Quitar los respaldos"}
      </button>
    </footer>
  </form>
</Modal>

<style>
  .pequeno {
    margin: 0;
    font-size: var(--fs-sm);
  }
  .mono {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 12px;
  }
  .anadir {
    align-self: flex-start;
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
