<script lang="ts">
  // «Dar acceso a ‹equipo› en este almacén» (plan 0.8.0, B3; plan-mejoras 14hh):
  // el acceso a una zona de un almacén **sin crear ningún repositorio**, para
  // poder hacer después una copia derivada allí. Con la clave de administración:
  // 1) el almacén da un usuario al equipo (`guarda_copias { anadir, zona? }`) y
  //    responde sellado solo para este navegador (usuario, contraseña, dirección
  //    y autoridad);
  // 2) el equipo lo guarda como destino (`crear_repositorio { solo_destino }`,
  //    agente con `admite: "solo_destino"`). Nada de esto lo ve el servidor.
  import { onDestroy } from "svelte";
  import { Check, KeyRound, LoaderCircle, Server, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { aB64, borrar, deB64, deUtf8 } from "$lib/cripto/bytes";
  import { abrir, parEfimero } from "$lib/cripto/sobre";
  import { ErrorLlavesCambiadas, mandarOrden } from "$lib/ordenar";
  import { avisar } from "$lib/avisos.svelte";
  import { cargarCliente } from "$lib/estado.svelte";
  import { errorRespuestaZona, idDestinoZona, nombreZonaPorDefecto, type ZonaVista } from "$lib/destinos";
  import type { Cliente, Equipo, Orden } from "$lib/tipos";
  import Ayuda from "./Ayuda.svelte";
  import CampoClave from "./CampoClave.svelte";
  import AlertaLlaves from "./AlertaLlaves.svelte";

  interface Acceso {
    destino: { tipo: "rest"; donde: string; usuario: string; secreto: string; ca_pem: string };
    zona?: string | null;
  }
  interface Props {
    cliente: Cliente;
    /** El equipo que recibe el acceso. */
    equipo: Equipo;
    /** La zona del almacén (la principal o otra). */
    zona: ZonaVista;
    onclose: () => void;
    /** Con el id del destino nuevo del equipo (para elegirlo ya en la copia derivada). */
    alDar?: (destino: string) => void;
  }
  let { cliente, equipo, zona, onclose, alDar }: Props = $props();

  const almacen = $derived(zona.almacen);
  const nombreZona = $derived(nombreZonaPorDefecto(zona));
  /** El almacén da acceso a sí mismo por localhost (como «Copiar en este mismo almacén»). */
  const propio = $derived(equipo.id === zona.almacen.id);
  let claveAdmin = $state("");
  let ocupado = $state(false);
  let pasoTxt = $state("");
  let error = $state("");
  let cambiadas = $state<Equipo | null>(null);
  let hecho = $state(false);
  onDestroy(() => (claveAdmin = ""));

  async function respuesta(eq: Equipo, o: Orden): Promise<Orden> {
    for (let i = 0; i < 80; i++) {
      const x = (await api.ordenesEquipo(cliente.id, eq.id, 10)).find((y) => y.id === o.id);
      if (x && ["hecha", "fallida", "rechazada", "cancelada", "caducada"].includes(x.estado)) return x;
      await new Promise((r) => setTimeout(r, 1500));
    }
    throw new Error(`${eq.nombre} no ha respondido todavía. Mira sus órdenes en un momento.`);
  }

  async function dar(e: SubmitEvent) {
    e.preventDefault();
    if (!claveAdmin) return;
    error = "";
    ocupado = true;
    const eph = parEfimero();
    let acceso: Acceso | null = null;
    try {
      const o = await mandarOrden({
        cliente,
        equipo: almacen,
        tipo: "guarda_copias",
        cuerpo: { anadir: equipo.id, ...(propio ? { local: true } : {}), ...(zona.principal ? {} : { zona: zona.id }) },
        secretos: { claveAdmin },
        responderA: aB64(eph.publica),
        alPaso: (t) => (pasoTxt = t),
      });
      pasoTxt = `Esperando a ${almacen.nombre}…`;
      const r = await respuesta(almacen, o);
      if (r.estado !== "hecha" || !r.detalle) throw new Error(r.mensaje ?? `${almacen.nombre} no pudo dar acceso.`);
      const sellado = (JSON.parse(r.detalle) as { sellado?: string }).sellado;
      if (!sellado) throw new Error("La respuesta no trae el acceso sellado.");
      acceso = JSON.parse(deUtf8(abrir(eph.secreta, deB64(sellado)))) as Acceso;
      // Un agente anterior ignoraría `zona` y daría un usuario de la principal: entonces no se sigue.
      const mal = errorRespuestaZona(acceso, zona);
      if (mal) throw new Error(mal);
      const id = idDestinoZona(almacen.id, zona.id);
      const o2 = await mandarOrden({
        cliente,
        equipo,
        tipo: "crear_repositorio",
        cuerpo: {
          solo_destino: true,
          destino: {
            id,
            nombre: zona.principal ? almacen.nombre : `${almacen.nombre} · ${zona.nombre ?? zona.id}`,
            tipo: "rest",
            donde: acceso.destino.donde,
            usuario: acceso.destino.usuario,
            secreto: acceso.destino.secreto,
            ca_pem: acceso.destino.ca_pem,
            equipo_almacen: almacen.id,
          },
        },
        secretos: { claveAdmin },
        alPaso: (t) => (pasoTxt = t),
      });
      acceso = null;
      pasoTxt = `Esperando a ${equipo.nombre}…`;
      const r2 = await respuesta(equipo, o2);
      if (r2.estado !== "hecha") throw new Error(r2.mensaje ?? `${equipo.nombre} no pudo guardar el acceso.`);
      claveAdmin = "";
      hecho = true;
      avisar(`${equipo.nombre} ya tiene acceso a ${nombreZona}.`);
      await cargarCliente(cliente.id, { silencioso: true });
      alDar?.(id);
    } catch (err) {
      if (err instanceof ErrorLlavesCambiadas) cambiadas = err.equipo === almacen.nombre ? almacen : equipo;
      else error = (err as Error).message;
    } finally {
      acceso = null;
      borrar(eph.secreta);
      ocupado = false;
      pasoTxt = "";
    }
  }
</script>

<Modal labelledby="t-dar-acceso" {onclose} width={540} dismissible={false}>
  <div class="dlg-title">
    <span class="ticon"><Server size={18} /></span>
    <div>
      <h2 id="t-dar-acceso">Dar acceso a {equipo.nombre} en {nombreZona}</h2>
      <p>{almacen.nombre} le da su propio usuario en esa zona, en solo añadir (no puede borrar lo ya copiado). No se crea ningún repositorio: después eliges qué guardar allí. <Ayuda id="guarda-copias" /></p>
    </div>
  </div>
  {#if cambiadas}
    <AlertaLlaves equipo={cambiadas} cliente={cliente.id} />
    <footer><button class="btn btn-primary" onclick={onclose}>Cerrar</button></footer>
  {:else if hecho}
    <div class="form">
      <div class="notice notice-success" role="status"><Check size={16} /><p>{equipo.nombre} ya puede guardar en {nombreZona}. Sigue con la copia derivada.</p></div>
      <footer><button class="btn btn-primary" onclick={onclose}>Seguir</button></footer>
    </div>
  {:else}
    <form class="form" onsubmit={dar}>
      <CampoClave requerido id="da-clave" etiqueta="Clave de administración de {cliente.nombre}" bind:value={claveAdmin} autofocus ayuda="Se usa para las dos órdenes (dar el usuario en el almacén y guardarlo en el equipo). El servidor nunca la ve.">
        {#snippet extra()}<Ayuda id="clave-admin" />{/snippet}
      </CampoClave>
      {#if error}<div class="notice notice-danger" role="alert"><TriangleAlert size={16} /><p>{error}</p></div>{/if}
      <footer>
        {#if ocupado}<span class="espera" role="status"><LoaderCircle size={15} class="spin" />{pasoTxt}</span>{/if}
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={onclose}>Cancelar</button>
        <button class="btn btn-primary" disabled={ocupado || !claveAdmin}><KeyRound size={15} />Dar acceso</button>
      </footer>
    </form>
  {/if}
</Modal>

<style>
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
</style>
