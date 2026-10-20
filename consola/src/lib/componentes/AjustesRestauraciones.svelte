<script lang="ts">
  // Los ajustes de las restauraciones del cliente (página «Restauraciones»), iguales en
  // todas sus consolas desde la 0.8.1 (datos comunes, docs/consolas-multiples.md §6.5):
  //  - «Pedir siempre el motivo» (15ee): obligatorio por defecto; se manda solo a los equipos.
  //  - «Recuperar en el equipo» (15mm, H1): que cada persona del equipo recupere sus propios
  //    archivos. Desactivado por defecto; activarlo pide la clave de administración (lo
  //    comprueba cada equipo, que es quien lo aplica).
  import { LoaderCircle } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { avisar } from "$lib/avisos.svelte";
  import { repartirConClave, sincronizar } from "$lib/datosComunes.svelte";
  import { ADMITE_AJUSTES, equiposQueGuardan } from "$lib/datosComunes";
  import { actual, puede } from "$lib/estado.svelte";
  import type { Restauraciones } from "$lib/tipos";
  import CampoClave from "./CampoClave.svelte";
  import Ayuda from "./Ayuda.svelte";

  let { ajustes, alCambiar }: { ajustes: Restauraciones["ajustes"]; alCambiar: (a: Restauraciones["ajustes"]) => void } = $props();

  /** Los equipos que guardan los ajustes comunes (los demás siguen con lo de cada consola). */
  const conAjustes = $derived(equiposQueGuardan(actual.equipos).filter((e) => e.resumen?.admite?.includes(ADMITE_AJUSTES)));
  const sinAjustes = $derived(equiposQueGuardan(actual.equipos).length - conAjustes.length);

  // ---------- Pedir siempre el motivo ----------
  let cambiando = $state(false);
  async function obligatorio(v: boolean) {
    if (!actual.id) return;
    cambiando = true;
    try {
      alCambiar(await api.ponerAjustesRestauraciones(actual.id, { motivo_obligatorio: v }));
      // Se manda solo a los equipos del cliente (no pide clave), como los colores de las etiquetas.
      void sincronizar();
      avisar(v ? "El motivo será obligatorio al restaurar, en todas las consolas." : "El motivo pasa a ser opcional, en todas las consolas.");
    } catch (x) {
      avisar((x as Error).message, "bad");
    } finally {
      cambiando = false;
    }
  }

  // ---------- Recuperar en el equipo (H1) ----------
  const rec = $derived(ajustes.recuperar ?? null);
  const TAMANOS = [100, 500, 1024, 5120, 10240];
  const textoMb = (mb: number) => (mb >= 1024 ? `${mb / 1024} GB` : `${mb} MB`);
  let editando = $state(false);
  let activo = $state(false);
  let maxMb = $state(1024);
  let clave = $state("");
  let pasoTxt = $state("");
  let guardando = $state(false);
  function abrir() {
    activo = rec?.activo ?? false;
    maxMb = rec?.max_mb ?? 1024;
    clave = "";
    editando = true;
  }
  async function guardarRecuperar(e: SubmitEvent) {
    e.preventDefault();
    if (!actual.id || !actual.cliente || !clave) return;
    guardando = true;
    try {
      alCambiar(await api.ponerAjustesRestauraciones(actual.id, { recuperar: { activo, max_mb: maxMb } }));
      const m = await repartirConClave(actual.cliente, actual.equipos, clave, (t) => (pasoTxt = t));
      clave = "";
      editando = false;
      avisar(activo ? `Activado: cada persona puede recuperar sus propios archivos en su equipo. ${m}` : `Desactivado en todos los equipos. ${m}`);
    } catch (x) {
      avisar((x as Error).message, "bad");
    } finally {
      guardando = false;
      pasoTxt = "";
    }
  }
</script>

<section class="card p ajustes" aria-labelledby="t-ajustes-rest">
  <h2 class="section-title" id="t-ajustes-rest">Ajustes de este cliente</h2>
  <label class="switch-row">
    <input type="checkbox" checked={ajustes.motivo_obligatorio} disabled={cambiando || !puede.administrar(actual.cliente?.rol)} onchange={(e) => obligatorio(e.currentTarget.checked)} />
    <span
      ><strong>Pedir siempre el motivo</strong> al restaurar<span class="faint"
        >Un toque basta («Otro» no pide escribir nada). Sin marcar, se pide igual pero se puede dejar en blanco. {conAjustes.length ? "Lo cumplen todas las consolas del cliente." : "Lo cumple esta consola (los agentes aún no lo reparten: actualízalos)."}</span
      ></span
    >
  </label>

  <div class="recuperar">
    <div class="rec-cab">
      <div>
        <strong>Recuperar en el equipo</strong>
        <Ayuda id="recuperar-en-el-equipo" />
        <p class="faint">
          {#if rec?.activo}
            Activado: cada persona puede recuperar, desde el Explorador o la ventana de Resguardo, las versiones anteriores de lo que ya puede leer en su equipo (hasta {textoMb(rec.max_mb)} cada vez). Nunca sobrescribe nada.
          {:else}
            Desactivado: solo se restaura desde aquí. Si lo activas, cada persona puede recuperar sus propios archivos en su equipo, sin ver contraseñas ni lo de otras cuentas.
          {/if}
        </p>
        {#if sinAjustes > 0}<p class="faint pequeno">{sinAjustes === 1 ? "1 equipo tiene" : `${sinAjustes} equipos tienen`} un agente anterior: allí sigue desactivado hasta actualizarlo.</p>{/if}
      </div>
      {#if puede.administrar(actual.cliente?.rol) && !editando}<button class="btn btn-sm" onclick={abrir}>Cambiar</button>{/if}
    </div>
    {#if editando}
      <form class="form rec-form" onsubmit={guardarRecuperar}>
        <label class="switch-row"><input type="checkbox" bind:checked={activo} /><span><strong>Que cada persona recupere sus propios archivos</strong><span class="faint">En todos los equipos del cliente. Se puede afinar por copia en la página de cada copia.</span></span></label>
        {#if activo}
          <label class="field">
            <span class="field-label">Como mucho, cada vez</span>
            <select class="input" bind:value={maxMb}>
              {#each TAMANOS as t (t)}<option value={t}>{textoMb(t)}</option>{/each}
            </select>
          </label>
        {/if}
        <CampoClave requerido id="clave-recuperar" etiqueta="Clave de administración" bind:value={clave} ayuda="Cada equipo la comprueba antes de cambiarlo." />
        <footer>
          {#if guardando}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt || "Guardando…"}</span>{/if}
          <button type="button" class="btn btn-ghost" disabled={guardando} onclick={() => ((editando = false), (clave = ""))}>Cancelar</button>
          <button class="btn btn-primary" disabled={guardando || !clave || !conAjustes.length}>Guardar en todos los equipos</button>
        </footer>
        {#if !conAjustes.length}<p class="faint pequeno">Ningún equipo de este cliente lo admite aún: actualiza sus agentes.</p>{/if}
      </form>
    {/if}
  </div>
</section>

<style>
  .ajustes {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    max-width: 660px;
  }
  .recuperar {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding-top: var(--sp-4);
    border-top: 1px solid var(--border);
  }
  .rec-cab {
    display: flex;
    gap: var(--sp-3);
    align-items: flex-start;
    justify-content: space-between;
  }
  .rec-cab p {
    margin: 4px 0 0;
    font-size: var(--fs-sm);
  }
  .rec-form footer {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    justify-content: flex-end;
    align-items: center;
  }
  .espera {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
</style>
