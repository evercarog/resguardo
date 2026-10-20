<script lang="ts">
  // 0.8.1 (B2, 15a): renombrar, unir o borrar una etiqueta en todos sus equipos.
  // Una orden `etiquetas_equipo` por equipo (lib/gestionEtiquetas.svelte.ts): se ve
  // el progreso («3 de 5 equipos…») y, al final, qué equipos no se pudieron cambiar.
  import { Check, CircleAlert, Clock, Combine, PenLine, Trash2, TriangleAlert } from "@lucide/svelte";
  import { untrack } from "svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import BotonCargando from "../BotonCargando.svelte";
  import EtiquetaChip from "../EtiquetaChip.svelte";
  import { actual } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { plural } from "$lib/formato";
  import { etiquetasDe, limpiarEtiqueta, mismaEtiqueta } from "$lib/etiquetas.svelte";
  import { cambiosEnEquipos, errorNombreEtiqueta } from "$lib/gestionEtiquetas";
  import { cambiarEtiqueta, conPedidas, type ResultadoCambio } from "$lib/gestionEtiquetas.svelte";
  import { admiteDatosEquipo } from "$lib/datosEquipo";
  import type { Equipo } from "$lib/tipos";

  let { modo, nombre, onclose }: { modo: "renombrar" | "unir" | "borrar"; nombre: string; onclose: () => void } = $props();

  /** Cuántos la llevan al abrir (mientras cambia, la lista de equipos se recarga). */
  const llevan = untrack(() => cambiosEnEquipos(conPedidas(actual.equipos), nombre, null).length);
  const otras = $derived(etiquetasDe(conPedidas(actual.equipos)).filter((t) => !mismaEtiqueta(t.nombre, nombre)));
  let nuevo = $state("");
  let destino = $state("");
  const errorNuevo = $derived(modo === "renombrar" ? errorNombreEtiqueta(nuevo) : null);
  const a = $derived(modo === "borrar" ? null : modo === "unir" ? destino || null : limpiarEtiqueta(nuevo));
  /** Al renombrar a una que ya existe, es unirlas: se usa su nombre tal cual. */
  const existente = $derived(modo === "renombrar" && a ? otras.find((t) => mismaEtiqueta(t.nombre, a)) : undefined);
  const final = $derived(existente?.nombre ?? a);
  /** Lo que se enseña antes de cambiar; al empezar se congela (los equipos cambian mientras tanto). */
  let congelado = $state<{ afectados: { equipo: Equipo; etiquetas: string[] }[]; existente: boolean } | null>(null);
  const afectados = $derived(congelado?.afectados ?? cambiosEnEquipos(conPedidas(actual.equipos), nombre, modo === "borrar" ? null : (final ?? "?")));
  const anteriores = $derived(afectados.filter((c) => !admiteDatosEquipo(c.equipo)).length);
  const listo = $derived(modo === "borrar" || (!!final && (modo === "unir" || !errorNuevo) && !(modo === "renombrar" && final === nombre)));

  let ocupado = $state(false);
  let paso = $state("");
  let error = $state("");
  let resultado = $state<ResultadoCambio | null>(null);

  async function hacer(e: SubmitEvent) {
    e.preventDefault();
    if (!listo || ocupado) return;
    ocupado = true;
    error = "";
    congelado = { afectados, existente: !!existente };
    try {
      resultado = await cambiarEtiqueta(nombre, modo === "borrar" ? null : final, (t) => (paso = t));
      const bien = resultado.equipos.filter((r) => r.estado === "hecho").length;
      if (bien === resultado.equipos.length && !resultado.avisos.length) {
        avisar(modo === "borrar" ? `«${nombre}» quitada de ${plural(bien, "equipo", "equipos")}.` : modo === "unir" ? `«${nombre}» unida a «${final}».` : `«${nombre}» ahora se llama «${final}».`);
        onclose();
      }
    } catch (err) {
      error = (err as Error).message;
      congelado = null;
    } finally {
      ocupado = false;
      paso = "";
    }
  }

  const titulo = $derived(modo === "borrar" ? `¿Quitar «${nombre}» de todos sus equipos?` : modo === "unir" ? `Unir «${nombre}» con otra etiqueta` : `Renombrar «${nombre}»`);
  const fallidos = $derived(resultado?.equipos.filter((r) => r.estado !== "hecho") ?? []);
</script>

<Modal labelledby="t-cambiar-etiqueta" {onclose} width={540} {ocupado} dismissible={!ocupado}>
  <div class="dlg-title">
    <span class="ticon" class:peligro={modo === "borrar"}>
      {#if modo === "borrar"}<Trash2 size={18} />{:else if modo === "unir"}<Combine size={18} />{:else}<PenLine size={18} />{/if}
    </span>
    <div>
      <h2 id="t-cambiar-etiqueta">{titulo}</h2>
      <p><EtiquetaChip {nombre} /> · {plural(llevan, "equipo la lleva", "equipos la llevan")}</p>
    </div>
  </div>

  {#if resultado}
    <!-- Lo que pasó en cada equipo: lo que no se pudo, a la vista. -->
    <p>
      {plural(resultado.equipos.filter((r) => r.estado === "hecho").length, "equipo cambiado", "equipos cambiados")} de {resultado.equipos.length}.
      {#if fallidos.length}Estos aún no:{/if}
    </p>
    {#if fallidos.length}
      <ul class="res">
        {#each fallidos as r (r.equipo.id)}
          <li class:fallo={r.estado === "fallo"}>
            {#if r.estado === "pedido"}<Clock size={14} />{:else}<CircleAlert size={14} />{/if}
            <span><strong>{r.equipo.nombre}</strong> <span class="faint">{r.texto}</span></span>
          </li>
        {/each}
      </ul>
    {/if}
    {#each resultado.avisos as x (x)}<div class="notice notice-warn"><TriangleAlert size={16} /><p>{x}</p></div>{/each}
    <footer>
      <button class="btn btn-primary" onclick={onclose}>Cerrar</button>
    </footer>
  {:else}
    <form class="form" onsubmit={hacer}>
      {#if modo === "renombrar"}
        <div class="field">
          <label class="field-label" for="et-nuevo">Nombre nuevo</label>
          <!-- svelte-ignore a11y_autofocus -->
          <input id="et-nuevo" class="input" bind:value={nuevo} maxlength="32" placeholder={nombre} autofocus aria-invalid={!!(nuevo && errorNuevo)} />
          {#if nuevo && errorNuevo}<p class="error-campo">{errorNuevo}</p>{:else if congelado ? congelado.existente : existente}<p class="field-hint">Ya hay una etiqueta «{final}»: se unen (los equipos con las dos la llevarán una vez).</p>{/if}
        </div>
      {:else if modo === "unir"}
        <div class="field">
          <label class="field-label" for="et-destino">Unir con</label>
          <select id="et-destino" class="input" bind:value={destino}>
            <option value="" disabled>Elige la etiqueta que se queda</option>
            {#each otras as t (t.nombre)}<option value={t.nombre}>{t.nombre} ({plural(t.n, "equipo", "equipos")})</option>{/each}
          </select>
          <p class="field-hint">Los equipos con «{nombre}» pasan a llevar la que elijas (sin repetirla), y «{nombre}» desaparece. La que se queda conserva su color y su plantilla; si no tiene, toma los de «{nombre}».</p>
        </div>
      {:else}
        <p>Se quita de {plural(afectados.length, "equipo", "equipos")}: {afectados.map((c) => c.equipo.nombre).join(", ")}. Los equipos y sus copias no cambian; solo pierden la etiqueta. Queda en el historial de cada equipo y en el registro de esta consola.</p>
      {/if}
      {#if modo !== "borrar" && final && afectados.length}
        <p class="faint resumen">Cambia en {plural(afectados.length, "equipo", "equipos")}: {afectados.map((c) => c.equipo.nombre).join(", ")}. Una orden a cada uno: lo verán igual todas sus consolas y queda en su historial. El color, la plantilla y la categoría pasan a «{final}».</p>
      {/if}
      {#if anteriores}<p class="faint resumen">{plural(anteriores, "equipo tiene", "equipos tienen")} un agente anterior: ahí cambia solo en esta consola.</p>{/if}
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      {#if paso}<p class="paso" role="status">{paso}</p>{/if}
      <footer>
        <button type="button" class="btn btn-ghost" onclick={onclose} disabled={ocupado}>Cancelar</button>
        <BotonCargando class="btn {modo === 'borrar' ? 'btn-danger' : 'btn-primary'}" disabled={!listo} cargando={ocupado} textoCargando="Cambiando…">
          {#if modo === "borrar"}<Trash2 size={14} />{:else}<Check size={14} />{/if}{modo === "borrar" ? `Quitar de ${plural(afectados.length, "equipo", "equipos")}` : modo === "unir" ? "Unir" : "Renombrar"}
        </BotonCargando>
      </footer>
    </form>
  {/if}
</Modal>

<style>
  .ticon.peligro {
    color: var(--bad);
  }
  .resumen {
    font-size: var(--fs-sm);
    margin: 0;
  }
  .paso {
    font-size: var(--fs-sm);
    color: var(--text-2);
    margin: 0;
  }
  .res {
    list-style: none;
    padding: 0;
    margin: 0 0 var(--sp-3);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .res li {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    font-size: var(--fs-sm);
    color: var(--warn);
  }
  .res li.fallo {
    color: var(--bad);
  }
  .res li span {
    color: var(--text-1);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: var(--sp-4);
  }
</style>
