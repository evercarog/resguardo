<script lang="ts">
  // Plan 0.8.0, F2 (14cc): «Conservar esta versión para siempre» (o dejar de
  // conservarla), con motivo obligatorio y la clave de administración. Va al
  // equipo dueño y, si su repositorio está en un almacén de solo añadir (que
  // aplica la retención con su propia clave), también al almacén: así ninguna
  // de las dos retenciones la quita. Quitarla reduce la protección: espera.
  import { onDestroy } from "svelte";
  import { Clock, LoaderCircle, Pin, PinOff, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import { mandarOrden } from "$lib/ordenar";
  import { cuentaAtras, fechaLarga } from "$lib/formato";
  import { destinosConservar, errorMotivo, MAX_MOTIVO, AVISO_SECRETOS } from "$lib/versionesNotas";
  import type { Cliente, Equipo, Orden, RepositorioResumen } from "$lib/tipos";
  import CampoClave from "../CampoClave.svelte";
  import BotonCargando from "../BotonCargando.svelte";
  import Ayuda from "../Ayuda.svelte";

  let {
    cliente,
    equipo,
    equipos,
    repo,
    version,
    conservar,
    onclose,
    alEnviar,
  }: { cliente: Cliente; equipo: Equipo; equipos: Equipo[]; repo: RepositorioResumen; version: string; conservar: boolean; onclose: () => void; alEnviar?: () => void } = $props();

  let motivo = $state("");
  let clave = $state("");
  let enviando = $state(false);
  let paso = $state("");
  let error = $state("");
  let hechas = $state<{ equipo: string; orden: Orden }[] | null>(null);
  const errMotivo = $derived(motivo.trim() ? errorMotivo(motivo) : null);
  const destinos = $derived(destinosConservar(equipo, repo, equipos, version, motivo, conservar));
  const espera = $derived(equipo.espera_min_horas ?? cliente.espera_min_horas);
  const listo = $derived(!errorMotivo(motivo) && clave.length > 0);

  async function enviar(ev: SubmitEvent) {
    ev.preventDefault();
    if (!listo) return;
    enviando = true;
    error = "";
    const out: { equipo: string; orden: Orden }[] = [];
    try {
      for (const d of destinos) {
        paso = d.quien === "almacen" ? `Y al almacén ${d.equipo.nombre}…` : `A ${d.equipo.nombre}…`;
        const o = await mandarOrden({ cliente, equipo: d.equipo, tipo: "conservar_version", cuerpo: d.cuerpo, secretos: { claveAdmin: clave }, alPaso: (t) => (paso = t) });
        out.push({ equipo: d.equipo.nombre, orden: o });
      }
      hechas = out;
      clave = "";
      alEnviar?.();
    } catch (err) {
      error = (err as Error).message;
      if (out.length) hechas = out;
    } finally {
      enviando = false;
      paso = "";
    }
  }
  onDestroy(() => {
    clave = "";
  });
</script>

<Modal labelledby="t-conservar" {onclose} width={500} dismissible={!enviando}>
  <form class="form" onsubmit={enviar}>
    <div class="dlg-title">
      <span class="ticon" class:danger={!conservar}>{#if conservar}<Pin size={18} />{:else}<PinOff size={18} />{/if}</span>
      <div>
        <h2 id="t-conservar">{conservar ? "Conservar esta versión para siempre" : "Dejar de conservar esta versión"}</h2>
        <p><strong>{repo.nombre}</strong> · versión <code>{version.slice(0, 8)}</code></p>
      </div>
    </div>

    {#if hechas}
      <div class="notice notice-success" role="status">
        <p>
          {#each hechas as h, i (i)}
            {#if h.orden.not_before && h.orden.estado === "pendiente"}
              Enviada a {h.equipo}: se aplicará dentro de <strong>{cuentaAtras(h.orden.not_before)}</strong>, salvo que alguien la cancele.{" "}
            {:else}
              Enviada a {h.equipo}.
            {/if}{" "}
          {/each}
          {conservar ? "En unos segundos se verá junto a la versión." : ""}
        </p>
      </div>
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer><button type="button" class="btn btn-primary" onclick={onclose}>Hecho</button></footer>
    {:else}
      <p class="desc">
        {#if conservar}
          La retención nunca la quitará: ni la del equipo ni{destinos.length > 1 ? ` la del almacén ${destinos[1].equipo.nombre}` : " la del almacén, si lo hay"}. Útil antes de liberar espacio en el origen o de un cambio grande. Queda en el historial con su motivo.
        {:else}
          La retención podrá quitarla cuando le toque. Queda en el historial con su motivo.
        {/if}
      </p>
      <label class="campo">
        <span>Motivo <span class="faint">(obligatorio)</span></span>
        <input class="input" maxlength={MAX_MOTIVO} bind:value={motivo} placeholder={conservar ? "Por ejemplo: cierre contable 2025" : "Por ejemplo: ya se archivó en otro sitio"} aria-required="true" />
        <span class="faint nota">{AVISO_SECRETOS}</span>
        {#if errMotivo}<span class="error" role="alert">{errMotivo}</span>{/if}
      </label>
      {#if !conservar}
        <div class="notice notice-warn">
          <Clock size={16} />
          <p>Reduce la protección, así que <strong>espera {espera} h</strong> antes de aplicarse (hacia el {fechaLarga(new Date(Date.now() + espera * 3600_000).toISOString())}). Hasta entonces, cualquiera del cliente puede cancelarla. <Ayuda id="espera" /></p>
        </div>
      {/if}
      {#if destinos.length > 1}
        <p class="faint nota">Se manda a {equipo.nombre} y al almacén {destinos[1].equipo.nombre}, que aplica la retención de este repositorio.</p>
      {/if}
      <CampoClave requerido id="clave-conservar" etiqueta="Clave de administración de {cliente.nombre}" bind:value={clave} ayuda="El servidor nunca la ve: se comprueba en el equipo." />
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if enviando}<span class="paso" role="status"><LoaderCircle size={15} class="spin" />{paso}</span>{/if}
        <button type="button" class="btn btn-ghost" onclick={onclose} disabled={enviando}>Cancelar</button>
        <BotonCargando class="btn {conservar ? 'btn-primary' : 'btn-danger'}" disabled={!listo} cargando={enviando} textoCargando="Enviando…">{conservar ? "Conservar para siempre" : "Dejar de conservar"}</BotonCargando>
      </footer>
    {/if}
  </form>
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .dlg-title {
    display: flex;
    gap: var(--sp-3);
    align-items: flex-start;
  }
  .dlg-title h2 {
    margin: 0;
    font-size: var(--fs-h2);
  }
  .dlg-title p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .ticon {
    display: inline-flex;
    padding: 8px;
    border-radius: var(--radius);
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .ticon.danger {
    background: var(--warn-soft);
    color: var(--warn);
  }
  .desc {
    margin: 0;
  }
  .campo {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: var(--fs-sm);
    font-weight: 550;
  }
  .nota {
    margin: 0;
    font-size: var(--fs-xs);
    font-weight: 400;
  }
  .error {
    color: var(--bad);
    font-weight: 400;
  }
  footer {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    align-items: center;
    gap: 8px;
  }
  .paso {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
</style>
