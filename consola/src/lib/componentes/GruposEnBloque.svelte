<script lang="ts">
  // 0.8.2 (C2, 15y): «No copiar» en varios equipos a la vez («Varios a la vez» en Equipos):
  // poner o quitar grupos de exclusiones de «Organización» en todas las copias de los
  // equipos elegidos. Cada copia guarda sus propias reglas (el equipo no depende de la
  // consola), así que es como «Aplicarles el cambio» (AplicarGrupoEnCopias): la clave de
  // administración una vez y una orden `config` por equipo, que solo toca las reglas de
  // esos grupos. Las configuraciones van cifradas: todo pasa en este navegador.
  import { Check, CircleAlert, FileX, LoaderCircle } from "@lucide/svelte";
  import { onDestroy } from "svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { borrar, deB64 } from "$lib/cripto/bytes";
  import { etiquetaValida } from "$lib/cripto/claves";
  import { descifrarConfig } from "$lib/cripto/simetrico";
  import { comprobarLlaves } from "$lib/fijadas";
  import { kcfgDelCliente, mandarOrden, pruebaParaEquipo } from "$lib/ordenar";
  import { admiteDe, configParaEnviar, paraEditar } from "$lib/configEnvio";
  import { ADMITE_GRUPOS, gruposEnCopias } from "$lib/exclusionesCopia";
  import { gruposExclusiones } from "$lib/gruposExclusiones";
  import { lista as enLista, plural } from "$lib/formato";
  import { actual, cargarCliente } from "$lib/estado.svelte";
  import type { Cliente, Configuracion, Equipo } from "$lib/tipos";
  import BotonCargando from "./BotonCargando.svelte";
  import CampoClave from "./CampoClave.svelte";

  let { cliente, equipos, onclose }: { cliente: Cliente; equipos: Equipo[]; onclose: () => void } = $props();

  const grupos = $derived(gruposExclusiones(actual));
  let elegidos = $state<string[]>([]);
  let quitar = $state(false);
  let clave = $state("");
  let error = $state("");
  let errorGrupos = $state("");
  let ocupado = $state(false);
  let fase = $state<"elegir" | "envio">("elegir");
  type Fila = { equipo: Equipo; estado: "espera" | "enviando" | "enviada" | "fallo" | "nada"; mensaje?: string };
  // svelte-ignore state_referenced_locally
  let filas = $state<Fila[]>(equipos.map((e) => ({ equipo: e, estado: "espera" })));
  /** Los que no saben de qué grupo viene cada regla: se les ponen las reglas, pero no se pueden quitar luego por grupo. */
  // svelte-ignore state_referenced_locally
  const sinGrupos = equipos.filter((e) => !e.resumen?.admite?.includes(ADMITE_GRUPOS));
  let kcfg: Uint8Array | null = null;
  onDestroy(() => {
    borrar(kcfg);
    kcfg = null;
    clave = "";
  });
  const alternar = (id: string) => (elegidos = elegidos.includes(id) ? elegidos.filter((x) => x !== id) : [...elegidos, id]);

  async function aplicar(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    errorGrupos = elegidos.length ? "" : "Elige al menos un grupo.";
    if (errorGrupos) return;
    if (!clave) return void (error = "Escribe la clave de administración.");
    const gs = grupos.filter((g) => elegidos.includes(g.id)).map((g) => ({ id: g.id, nombre: g.nombre, reglas: [...g.reglas] }));
    ocupado = true;
    try {
      kcfg = await kcfgDelCliente(cliente, clave);
      if (!equipos.some((x) => etiquetaValida(kcfg!, x))) throw new Error("La clave de administración no es correcta. No se ha enviado nada.");
    } catch (err) {
      borrar(kcfg);
      kcfg = null;
      error = (err as Error).message;
      ocupado = false;
      return;
    }
    fase = "envio";
    for (const f of filas) {
      f.estado = "enviando";
      let prueba: Uint8Array | null = null;
      try {
        if (!f.equipo.confirmado || f.equipo.modo === "trasladado") throw new Error("No recibe órdenes desde aquí.");
        if ((await comprobarLlaves(cliente.id, f.equipo)) === "cambiada" || !etiquetaValida(kcfg, f.equipo)) throw new Error("Sus llaves no son las que se confirmaron al emparejarlo: revísalo en su ficha.");
        const c = await api.configEquipo(cliente.id, f.equipo.id);
        const cfg = paraEditar(descifrarConfig<Configuracion>(kcfg, f.equipo.id, c.seq, deB64(c.cifrado)));
        if (!cfg.copias.length) {
          f.estado = "nada";
          f.mensaje = "No tiene copias.";
          continue;
        }
        const cambiadas = gruposEnCopias(cfg.copias, gs, quitar);
        if (!cambiadas.length) {
          f.estado = "nada";
          f.mensaje = quitar ? "Ninguna copia los usa." : "Sus copias ya los tienen.";
          continue;
        }
        prueba = await pruebaParaEquipo(cliente, f.equipo, clave, kcfg);
        const config = configParaEnviar(cfg, admiteDe(f.equipo));
        const o = await mandarOrden({ cliente, equipo: f.equipo, tipo: "config", cuerpo: { config }, secretos: { prueba } });
        f.estado = "enviada";
        f.mensaje = `Enviada (orden n.º ${o.seq}): ${enLista(cambiadas.map((k) => `«${k.nombre}»`))}.`;
      } catch (err) {
        f.estado = "fallo";
        f.mensaje = (err as Error).message;
      } finally {
        borrar(prueba);
      }
    }
    clave = "";
    ocupado = false;
    void cargarCliente(cliente.id, { silencioso: true });
  }
</script>

<Modal labelledby="t-grupos-bloque" {onclose} width={560}>
  <div class="dlg-title">
    <span class="ticon"><FileX size={18} /></span>
    <div>
      <h2 id="t-grupos-bloque">No copiar en {plural(equipos.length, "equipo", "equipos")}</h2>
      <p>Pon o quita grupos de «No copiar» en todas las copias de los equipos elegidos. Cada copia guarda sus reglas: se le manda su configuración con este cambio y nada más.</p>
    </div>
  </div>
  {#if fase === "elegir"}
    <form class="form" onsubmit={aplicar}>
      <div class="modo" role="radiogroup" aria-label="Qué hacer">
        <label><input type="radio" name="gb-modo" checked={!quitar} onchange={() => (quitar = false)} />Ponerlos</label>
        <label><input type="radio" name="gb-modo" checked={quitar} onchange={() => (quitar = true)} />Quitarlos</label>
      </div>
      <fieldset class="grupos">
        <legend class="field-label">Grupos</legend>
        {#each grupos as g (g.id)}
          <label class="grupo">
            <input type="checkbox" checked={elegidos.includes(g.id)} onchange={() => alternar(g.id)} />
            <span><strong>{g.nombre}</strong><span class="faint">{g.reglas.slice(0, 4).join(", ")}{g.reglas.length > 4 ? "…" : ""}</span></span>
          </label>
        {:else}
          <p class="faint">No hay grupos: créalos en Ajustes → Organización.</p>
        {/each}
      </fieldset>
      {#if errorGrupos && !elegidos.length}<p class="error-campo" role="alert">{errorGrupos}</p>{/if}
      {#if quitar && sinGrupos.length}
        <p class="faint pequeno">{enLista(sinGrupos.slice(0, 3).map((x) => x.nombre))}{sinGrupos.length > 3 ? "…" : ""}: su agente no recuerda de qué grupo viene cada regla, así que ahí no se quitan. Actualízalo y quítalas en sus copias.</p>
      {/if}
      <p class="faint pequeno">{equipos.map((x) => x.nombre).join(", ")}</p>
      <CampoClave id="gb-clave" etiqueta="Clave de administración" bind:value={clave} requerido {error} />
      <footer>
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
        <BotonCargando class="btn btn-primary" cargando={ocupado} textoCargando="Comprobando…">{quitar ? "Quitarlos" : "Ponerlos"}</BotonCargando>
      </footer>
    </form>
  {:else}
    <ul class="lista">
      {#each filas as f (f.equipo.id)}
        <li>
          <strong>{f.equipo.nombre}</strong>
          <span class="est" role="status">
            {#if f.estado === "enviando"}<LoaderCircle size={14} class="spin" />Enviando…
            {:else if f.estado === "enviada"}<Check size={14} />{f.mensaje}
            {:else if f.estado === "fallo"}<CircleAlert size={14} />{f.mensaje}
            {:else if f.estado === "nada"}{f.mensaje}
            {:else}En espera{/if}
          </span>
        </li>
      {/each}
    </ul>
    <footer><button class="btn btn-primary" disabled={ocupado} onclick={onclose}>{ocupado ? "Enviando…" : "Listo"}</button></footer>
  {/if}
</Modal>

<style>
  .modo {
    display: flex;
    gap: 16px;
    flex-wrap: wrap;
  }
  .modo label,
  .grupo {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    cursor: pointer;
  }
  .grupos {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    padding: 0;
    border: 0;
    max-height: 280px;
    overflow: auto;
  }
  .grupo span {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .grupo .faint {
    font-size: var(--fs-xs);
  }
  .pequeno {
    font-size: var(--fs-xs);
    margin: 0;
    overflow-wrap: anywhere;
  }
  .lista {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-sm);
  }
  .lista li {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow-wrap: anywhere;
  }
  .est {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
