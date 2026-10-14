<script lang="ts">
  // «Quitar» o «Desconectar» un destino (plan 0.8.0, B2; plan-mejoras 14b), en su
  // tarjeta de «Repositorios y destinos» y en su página:
  //
  // - una entrada suelta del catálogo (sin equipos): «Quitar de la lista» (solo el
  //   catálogo del cliente; no toca ningún equipo ni lo guardado allí);
  // - una nube conectada en un equipo: «Desconectar» (`quitar_nube`, clave de
  //   administración), solo si nada la usa en ese equipo;
  // - el destino de un equipo sin uso: «Quitar» (`quitar_destino`, clave de administración).
  //
  // Si no se puede, el botón está apagado y dice qué lo usa («Lo usan 3 espejos»),
  // con la lista entera al lado.
  import { Trash2, Unplug } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import OrdenDialog from "./OrdenDialog.svelte";
  import BotonCargando from "./BotonCargando.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { quitarDelCatalogo } from "$lib/catalogoDestinos.svelte";
  import { destinoQuitable } from "$lib/datosEquipo";
  import { textoUsos, textoUsosCorto, usosDeVista, type DestinoVista } from "$lib/destinos";
  import { queHaceAlDesconectar } from "$lib/nubesEquipo";
  import { tip } from "$lib/tooltip";
  import type { Cliente, DestinoResumen, Equipo } from "$lib/tipos";

  interface Props {
    cliente: Cliente;
    v: DestinoVista;
    equipos: Equipo[];
    /** En la tarjeta, botones pequeños; en la página, con la explicación debajo. */
    variante?: "tarjeta" | "pagina";
    alCambiar?: () => void;
  }
  let { cliente, v, equipos, variante = "tarjeta", alCambiar }: Props = $props();

  // Una entrada suelta del catálogo (y lo que se unió a ella): solo la lista.
  const suelto = $derived(v.clase === "suelto" && !v.ids.length && !v.nube);
  // La nube conectada: lo que la usa en su equipo (repositorios, copias y espejos, también en pausa).
  const nube = $derived(v.nube ?? null);
  const usosNube = $derived.by(() => {
    if (!nube) return null;
    const ids = (nube.equipo.resumen?.destinos ?? []).filter((d) => d.tipo === "nube" && d.nube === nube.nombre).map((d) => d.id);
    return usosDeVista({ ...v, ids, zona: undefined }, [nube.equipo]);
  });
  const nubeOcupada = $derived(!!usosNube && !!textoUsosCorto(usosNube));
  // Los destinos de los equipos: los que ya no usa nada se quitan; si no, se dice qué los usa.
  const propios = $derived(equipos.flatMap((e) => (e.resumen?.destinos ?? []).filter((d) => v.ids.includes(d.id)).map((d) => ({ equipo: e, destino: d }))));
  const quitables = $derived(propios.filter((q) => destinoQuitable(q.equipo, q.destino.id)));
  const usosEquipos = $derived(usosDeVista({ ...v, nube: undefined, zona: undefined }, equipos));
  const ocupado = $derived(!!propios.length && !quitables.length && !!textoUsosCorto(usosEquipos));

  let quitarLista = $state(false);
  let quitando = $state(false);
  let errorLista = $state("");
  let desconectar = $state(false);
  let quitar = $state<{ equipo: Equipo; destino: DestinoResumen } | null>(null);
  const pequeno = $derived(variante === "tarjeta");

  async function quitarDeLaLista() {
    quitando = true;
    errorLista = "";
    try {
      for (const id of [v.clave, ...(v.alias ?? []).map((a) => a.id)]) await quitarDelCatalogo(cliente.id, id);
      avisar(`«${v.nombre}» ya no está en la lista de destinos.`);
      quitarLista = false;
      alCambiar?.();
    } catch (e) {
      errorLista = (e as Error).message;
    } finally {
      quitando = false;
    }
  }
</script>

{#if suelto}
  <button class="btn btn-ghost quitar" class:btn-sm={pequeno} onclick={() => (quitarLista = true)}><Trash2 size={pequeno ? 12 : 14} />Quitar</button>
{/if}
{#if nube}
  {#if nubeOcupada && usosNube}
    <span class="apagado" use:tip={textoUsos(usosNube) ?? ""}>
      <button class="btn btn-ghost" class:btn-sm={pequeno} disabled aria-describedby={variante === "pagina" ? "qd-nube" : undefined}><Unplug size={pequeno ? 12 : 14} />{textoUsosCorto(usosNube)}</button>
    </span>
    {#if variante === "pagina"}<p class="faint motivo" id="qd-nube">{textoUsos(usosNube)} Quítalo antes de ahí para poder desconectarla de {nube.equipo.nombre}.</p>{/if}
  {:else}
    <button class="btn btn-ghost quitar" class:btn-sm={pequeno} onclick={() => (desconectar = true)}><Unplug size={pequeno ? 12 : 14} />Desconectar{variante === "pagina" ? ` de ${nube.equipo.nombre}` : ""}</button>
  {/if}
{/if}
{#each quitables as q (q.equipo.id + q.destino.id)}
  <button class="btn btn-ghost quitar" class:btn-sm={pequeno} onclick={() => (quitar = q)}><Trash2 size={pequeno ? 12 : 14} />{quitables.length > 1 || propios.length > 1 ? `Quitar de ${q.equipo.nombre}` : "Quitar"}</button>
{/each}
{#if ocupado}
  <span class="apagado" use:tip={textoUsos(usosEquipos) ?? ""}>
    <button class="btn btn-ghost" class:btn-sm={pequeno} disabled aria-describedby={variante === "pagina" ? "qd-eq" : undefined}><Trash2 size={pequeno ? 12 : 14} />{textoUsosCorto(usosEquipos)}</button>
  </span>
  {#if variante === "pagina"}<p class="faint motivo" id="qd-eq">{textoUsos(usosEquipos)} Para quitarlo, antes hay que quitar eso.</p>{/if}
{/if}

{#if quitarLista}
  <Modal labelledby="t-quitar-lista" onclose={() => (quitarLista = false)} width={460}>
    <div class="dlg-title"><span class="ticon"><Trash2 size={18} /></span><div><h2 id="t-quitar-lista">Quitar «{v.nombre}» de la lista</h2><p>Nadie lo usa todavía. Solo sale de la lista de destinos de {cliente.nombre}: no toca ningún equipo ni lo que haya en el bucket.</p></div></div>
    {#if errorLista}<div class="notice notice-danger" role="alert"><p>{errorLista}</p></div>{/if}
    <footer>
      <button class="btn btn-ghost" disabled={quitando} onclick={() => (quitarLista = false)}>Cancelar</button>
      <BotonCargando class="btn btn-danger" cargando={quitando} textoCargando="Quitando…" onclick={quitarDeLaLista}>Quitar de la lista</BotonCargando>
    </footer>
  </Modal>
{/if}
{#if desconectar && nube}
  <OrdenDialog
    {cliente}
    equipo={nube.equipo}
    tipo="quitar_nube"
    cuerpo={{ nombre: nube.nombre }}
    titulo="Desconectar «{nube.nombre}»"
    descripcion={`${nube.equipo.nombre} olvidará el permiso de «${nube.nombre}». Lo ya subido se queda allí. ${queHaceAlDesconectar(nube.equipo, nube.tipo)}`}
    accion="Desconectar"
    onclose={() => ((desconectar = false), alCambiar?.())}
    alTerminar={() => alCambiar?.()}
  />
{/if}
{#if quitar}
  <OrdenDialog
    {cliente}
    equipo={quitar.equipo}
    tipo="quitar_destino"
    cuerpo={{ destino: quitar.destino.id }}
    titulo="Quitar este destino"
    descripcion={quitar.destino.tipo === "local"
      ? `${quitar.equipo.nombre} olvidará «${quitar.destino.nombre}». No se borra nada de su carpeta: si aún tiene copias guardadas, te lo dirá y seguirán ahí.`
      : `${quitar.equipo.nombre} olvidará «${quitar.destino.nombre}» y sus credenciales. Lo guardado allí se queda.`}
    accion="Quitar el destino"
    onclose={() => ((quitar = null), alCambiar?.())}
    alTerminar={() => alCambiar?.()}
  />
{/if}

<style>
  .quitar {
    color: var(--bad);
  }
  .apagado {
    display: inline-flex;
  }
  .motivo {
    flex-basis: 100%;
    margin: 0;
    font-size: var(--fs-sm);
  }
</style>
