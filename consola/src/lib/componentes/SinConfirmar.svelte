<script lang="ts">
  // Un equipo que se unió y se quedó en «Falta confirmar el número de comprobación»:
  // «Confirmar» sigue el emparejamiento desde «Añadir equipo» y «Quitar» lo anula (el
  // equipo nunca recibió la clave de administración ni guardó nada; queda en la
  // auditoría). Si es la misma máquina que otro ya dado de alta, lo dice y solo ofrece
  // quitarlo. Solo para administradores.
  import { goto } from "$app/navigation";
  import { Check, Trash2 } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { actual, cargarCliente, puede } from "$lib/estado.svelte";
  import { duplicadoDe } from "$lib/salud";
  import type { Equipo } from "$lib/tipos";

  let { cliente, equipo, compacto = false }: { cliente: string; equipo: Equipo; compacto?: boolean } = $props();

  const duplicado = $derived(duplicadoDe(equipo, actual.equipos));
  const admin = $derived(puede.administrar(actual.cliente?.rol));
  let preguntando = $state(false);
  let ocupado = $state(false);

  const CADUCADO = "Su código ya caducó: el servidor lo quita solo en unas horas. Si es un equipo nuevo, vincúlalo otra vez desde «Añadir equipo».";

  /** El emparejamiento que sigue a medias para este equipo (o null si ya caducó). */
  async function suEmparejamiento() {
    const l = await api.aMedias(cliente);
    return l.find((m) => m.equipo.id === equipo.id) ?? null;
  }

  async function confirmar() {
    ocupado = true;
    try {
      const m = await suEmparejamiento();
      if (m) await goto(`/c/${cliente}/emparejar?seguir=${encodeURIComponent(m.id)}`);
      else avisar(CADUCADO, "info");
    } catch (e) {
      fallo(e);
    } finally {
      ocupado = false;
    }
  }

  async function quitar() {
    ocupado = true;
    try {
      const m = await suEmparejamiento();
      if (!m) {
        avisar(CADUCADO, "info");
        return;
      }
      await api.cancelarEmparejamiento(cliente, m.id);
      avisar(`Quitado: «${equipo.nombre}» ya no sale en la lista.`);
      if (location.pathname.includes(`/equipos/${equipo.id}`)) await goto(`/c/${cliente}/equipos`);
      void cargarCliente(cliente, { silencioso: true });
    } catch (e) {
      fallo(e);
    } finally {
      ocupado = false;
      preguntando = false;
    }
  }
</script>

{#if !compacto}
  <p class="motivo">
    {#if duplicado}Duplicado: este equipo ya está dado de alta como «{duplicado.nombre}». Este intento de vincularlo sobra: quítalo.{:else}Falta confirmar el número de comprobación. Hasta entonces no recibe órdenes ni la clave de administración.{/if}
  </p>
{/if}
{#if admin}
  <span class="acciones" class:compacto>
    {#if preguntando}
      <span class="pregunta">¿Quitarlo? Nunca guardó nada.</span>
      <button class="btn btn-sm btn-danger" disabled={ocupado} onclick={quitar}>{ocupado ? "Quitando…" : "Sí, quitar"}</button>
      <button class="btn btn-sm btn-ghost" disabled={ocupado} onclick={() => (preguntando = false)}>No</button>
    {:else}
      {#if !duplicado}<button class="btn btn-sm btn-primary" disabled={ocupado} onclick={confirmar}><Check size={14} />Confirmar</button>{/if}
      <button class="btn btn-sm {duplicado ? 'btn-primary' : 'btn-ghost'}" disabled={ocupado} onclick={() => (preguntando = true)} aria-label="Quitar {equipo.nombre}"><Trash2 size={14} />Quitar</button>
    {/if}
  </span>
{/if}

<style>
  .motivo {
    margin: 0;
  }
  .acciones {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  .acciones:not(.compacto) {
    margin-top: var(--sp-2);
  }
  .pregunta {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
</style>
