<script lang="ts">
  // Las zonas de un almacén (tarea 7b, docs/copias-en-cadena.md): la carpeta
  // principal y las demás (otros discos), cada una con su propio rest-server
  // en su puerto, de solo añadir y con sus propios usuarios. Cada zona es un
  // destino («Almacén ALMACEN-01 · Disco E») que se elige al crear un
  // repositorio. Añadir una pide la clave de administración; quitarla, además,
  // espera (sus equipos dejan de poder copiar allí; lo guardado se queda).
  import { untrack } from "svelte";
  import { HardDrive, Pencil, Plus, Trash2 } from "@lucide/svelte";
  import OrdenDialog from "./OrdenDialog.svelte";
  import RenombrarDestino from "./RenombrarDestino.svelte";
  import NuevaZona from "./NuevaZona.svelte";
  import Chip from "./Chip.svelte";
  import Ayuda from "./Ayuda.svelte";
  import { tip } from "$lib/tooltip";
  import { bytes, plural } from "$lib/formato";
  import { catalogoDe, cargarCatalogo } from "$lib/catalogoDestinos.svelte";
  import { admiteZonas, claveZona, destinosDelCliente, ZONAS_MAX, zonasDe, type DestinoVista, type ZonaVista } from "$lib/destinos";
  import type { Cliente, Equipo } from "$lib/tipos";

  let { cliente, equipo, equipos, administra, alCambiar }: { cliente: Cliente; equipo: Equipo; equipos: Equipo[]; administra: boolean; alCambiar?: () => void } = $props();

  $effect(() => {
    const c = cliente.id;
    untrack(() => void cargarCatalogo(c));
  });
  const zonas = $derived(zonasDe(equipo));
  const vistas = $derived(destinosDelCliente(equipos, catalogoDe(cliente.id)));
  const vistaDe = (z: ZonaVista) => vistas.find((v) => v.clave === claveZona(equipo.id, z.id));
  const admite = $derived(admiteZonas(equipo));

  let renombrar = $state<DestinoVista | null>(null);
  let quitar = $state<ZonaVista | null>(null);
  let anadir = $state(false);
  const libre = (z: ZonaVista) => (z.espacio ? `${bytes(z.espacio.libre)} libres de ${bytes(z.espacio.total)}` : null);
</script>

{#if zonas.length > 1 || (administra && admite)}
  <div class="zonas">
    <p class="cab"><HardDrive size={14} /><span>Zonas: los discos que sirve, cada uno con su puerto</span><Ayuda id="zona" /></p>
    <ul>
      {#each zonas as z (z.id)}
        {@const v = vistaDe(z)}
        <li>
          <span class="ic"><HardDrive size={14} /></span>
          <span class="txt">
            <strong>{v?.nombre ?? z.nombre ?? "Principal"}</strong>
            <span class="faint">{z.principal ? "La principal" : "Otra zona"} · puerto <span class="pastilla mono">{z.puerto ?? "—"}</span>{#if z.carpeta}{" · "}<span class="mono">{z.carpeta}</span>{/if}</span>
            <span class="faint">{plural(z.usuarios, "equipo copia aquí", "equipos copian aquí")}{#if libre(z)}{" · "}{libre(z)}{/if}</span>
          </span>
          {#if !z.principal}<Chip pequeno tono={z.escucha === false ? "warn" : "ok"} texto={z.escucha === false ? "Sin responder" : "En marcha"} />{/if}
          {#if administra && v}
            <button class="icon-btn" use:tip={"Cambiar el nombre"} aria-label="Cambiar el nombre de {v.nombre}" onclick={() => (renombrar = v)}><Pencil size={14} /></button>
          {/if}
          {#if administra && !z.principal}
            <button class="icon-btn" use:tip={"Quitar esta zona"} aria-label="Quitar la zona {v?.nombre ?? z.nombre}" onclick={() => (quitar = z)}><Trash2 size={14} /></button>
          {/if}
        </li>
      {/each}
    </ul>
    {#if administra}
      {#if admite}
        {#if (equipo.resumen?.guarda_copias?.zonas ?? []).length < ZONAS_MAX}
          <button class="btn btn-sm" onclick={() => (anadir = true)}><Plus size={14} />Añadir una zona (otro disco)…</button>
        {/if}
      {:else}
        <p class="faint pequeno">Para que sirva otro disco, actualiza el agente de {equipo.nombre}.</p>
      {/if}
    {/if}
  </div>
{/if}

{#if renombrar}
  <RenombrarDestino cliente={cliente.id} destino={renombrar} onclose={() => (renombrar = null)} />
{/if}

{#if quitar}
  {@const nombre = vistaDe(quitar)?.nombre ?? quitar.nombre ?? "esta zona"}
  <OrdenDialog
    {cliente}
    {equipo}
    tipo="guarda_copias"
    cuerpo={{ quitar_zona: quitar.id }}
    titulo="Quitar «{nombre}»"
    descripcion="{plural(quitar.usuarios, 'El equipo que copia', 'Los equipos que copian')} en «{nombre}» dejarán de poder hacerlo y su servidor se parará. Lo ya guardado se queda en {quitar.carpeta ?? 'su carpeta'}: para volver a usarlo, crea otra zona en esa carpeta."
    onclose={() => ((quitar = null), alCambiar?.())}
    alTerminar={() => alCambiar?.()}
  />
{/if}

{#if anadir}
  <NuevaZona {cliente} {equipo} onclose={() => ((anadir = false), alCambiar?.())} alTerminar={() => alCambiar?.()} />
{/if}

<style>
  .zonas {
    margin-top: var(--sp-3);
  }
  .cab {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 0 var(--sp-2);
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  ul {
    list-style: none;
    margin: 0 0 var(--sp-2);
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  li {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm, 6px);
    min-width: 0;
  }
  .ic {
    flex: none;
    color: var(--text-3);
    display: inline-flex;
  }
  .txt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
    font-size: var(--fs-sm);
    overflow-wrap: anywhere;
  }
  .pequeno {
    font-size: var(--fs-xs);
    margin: 0;
  }
</style>
