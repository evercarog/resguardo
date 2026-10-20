<script lang="ts">
  // 0.8.1 (D2, 15y): aplicar un grupo de exclusiones que cambió en «Organización» a las
  // copias que lo usan. Cada copia guarda sus propias reglas (el equipo no depende de la
  // consola), así que hay que mandárselas: una orden `config` por equipo, con la clave de
  // administración. Las configuraciones van cifradas: todo pasa en este navegador.
  //   1. la clave (K_cfg una vez);
  //   2. cada equipo: su configuración, y en cada copia que usa el grupo, las reglas de
  //      antes del grupo fuera y las de ahora dentro (las sueltas y los demás grupos, igual);
  //   3. su orden, con su prueba. Nunca quita ni cambia otra cosa.
  import { Check, CircleAlert, LoaderCircle, RefreshCw } from "@lucide/svelte";
  import { onDestroy } from "svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { borrar, deB64 } from "$lib/cripto/bytes";
  import { etiquetaValida } from "$lib/cripto/claves";
  import { descifrarConfig } from "$lib/cripto/simetrico";
  import { comprobarLlaves } from "$lib/fijadas";
  import { kcfgDelCliente, mandarOrden, pruebaParaEquipo } from "$lib/ordenar";
  import { admiteDe, configParaEnviar, paraEditar } from "$lib/configEnvio";
  import { copiasQueUsan, ponerAlDia } from "$lib/exclusionesCopia";
  import { lista as enLista, plural } from "$lib/formato";
  import { cargarCliente } from "$lib/estado.svelte";
  import type { Cliente, Configuracion, Equipo } from "$lib/tipos";
  import type { GrupoExclusiones } from "$lib/gruposExclusiones";
  import BotonCargando from "../BotonCargando.svelte";
  import CampoClave from "../CampoClave.svelte";

  let { cliente, equipos, grupo, onclose }: { cliente: Cliente; equipos: Equipo[]; grupo: GrupoExclusiones; onclose: () => void } = $props();

  // svelte-ignore state_referenced_locally
  const usos = copiasQueUsan(equipos, grupo).filter((x) => !x.alDia);
  const porEquipo = [...new Map(usos.map((x) => [x.equipo.id, x.equipo])).values()];
  let clave = $state("");
  let error = $state("");
  let ocupado = $state(false);
  let fase = $state<"clave" | "envio">("clave");
  let filas = $state<{ equipo: Equipo; copias: string[]; estado: "espera" | "enviando" | "enviada" | "fallo" | "nada"; mensaje?: string }[]>(
    porEquipo.map((e) => ({ equipo: e, copias: usos.filter((x) => x.equipo.id === e.id).map((x) => x.copia.nombre), estado: "espera" })),
  );
  let kcfg: Uint8Array | null = null;
  onDestroy(() => {
    borrar(kcfg);
    kcfg = null;
    clave = "";
  });

  async function aplicar(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    if (!clave) return void (error = "Escribe la clave de administración.");
    ocupado = true;
    try {
      kcfg = await kcfgDelCliente(cliente, clave);
      if (!porEquipo.some((x) => etiquetaValida(kcfg!, x))) throw new Error("La clave de administración no es correcta. No se ha enviado nada.");
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
        const cambiadas = cfg.copias.filter((k) => ponerAlDia(k, grupo));
        if (!cambiadas.length) {
          f.estado = "nada";
          f.mensaje = "Ya no usa el grupo.";
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

<Modal labelledby="t-aplicar-grupo" {onclose} width={540}>
  <div class="dlg-title">
    <span class="ticon"><RefreshCw size={18} /></span>
    <div>
      <h2 id="t-aplicar-grupo">Aplicar «{grupo.nombre}» a sus copias</h2>
      <p>{plural(usos.length, "copia tiene", "copias tienen")} las reglas de antes de este grupo. Cada una guarda las suyas: se le manda la configuración con las de ahora (lo demás no cambia).</p>
    </div>
  </div>
  <ul class="lista">
    {#each filas as f (f.equipo.id)}
      <li>
        <span class="eq"><strong>{f.equipo.nombre}</strong><span class="faint">{enLista(f.copias.map((x) => `«${x}»`))}</span></span>
        <span class="est" role="status">
          {#if f.estado === "enviando"}<LoaderCircle size={14} class="spin" />Enviando…
          {:else if f.estado === "enviada"}<Check size={14} />{f.mensaje}
          {:else if f.estado === "fallo"}<CircleAlert size={14} />{f.mensaje}
          {:else if f.estado === "nada"}{f.mensaje}{/if}
        </span>
      </li>
    {/each}
  </ul>
  {#if fase === "clave"}
    <form class="form" onsubmit={aplicar}>
      <CampoClave id="ag-clave" etiqueta="Clave de administración" bind:value={clave} requerido error={error} autofocus />
      <footer>
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
        <BotonCargando class="btn btn-primary" cargando={ocupado} textoCargando="Comprobando…">Aplicar a {plural(usos.length, "copia", "copias")}</BotonCargando>
      </footer>
    </form>
  {:else}
    <footer><button class="btn btn-primary" disabled={ocupado} onclick={onclose}>{ocupado ? "Enviando…" : "Listo"}</button></footer>
  {/if}
</Modal>

<style>
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
  }
  .eq {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 8px;
    overflow-wrap: anywhere;
  }
  .est {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
</style>
