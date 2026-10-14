<script lang="ts">
  // Plan 0.8.0, D1: la sede de un equipo o de un destino. Un desplegable con las sedes
  // del cliente y, primero, lo que vale sin asignar («La principal (Oficina Norte)»,
  // «La de su equipo»). Guarda al elegir (administradores); para los demás, solo el texto.
  // La sede cuenta en la regla 3-2-1: repartirla a las demás consolas pide la clave
  // (el aviso de «Datos comunes» lo ofrece).
  import { Building2 } from "@lucide/svelte";
  import Ayuda from "./Ayuda.svelte";
  import { actual, puede } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { sedePorId } from "$lib/sedes";

  let {
    asignada,
    sinAsignar,
    guardar,
    id = "sede",
    etiqueta = "Sede",
  }: {
    /** La sede asignada (id) o null. */
    asignada: string | null;
    /** Qué vale sin asignar: «La principal (Oficina Norte)», «La de su equipo (Planta Sur)», «Ninguna: manda su tipo». */
    sinAsignar: string;
    guardar: (sede: string | null) => Promise<void>;
    id?: string;
    etiqueta?: string;
  } = $props();

  const sedes = $derived([...actual.organizacion.sedes].sort((a, b) => a.nombre.localeCompare(b.nombre, "es")));
  let ocupado = $state(false);
  const editable = $derived(puede.administrar(actual.cliente?.rol));

  async function cambiar(v: string) {
    ocupado = true;
    try {
      await guardar(v || null);
      const s = sedePorId(actual.organizacion, v);
      avisar(s ? `Ahora en «${s.nombre}».` : "Sin sede asignada: vale la de por defecto.");
    } catch (e) {
      fallo(e);
    } finally {
      ocupado = false;
    }
  }
</script>

{#if sedes.length}
  <span class="sede">
    <label class="lbl" for={id}><Building2 size={14} />{etiqueta}</label>
    {#if editable}
      <select {id} class="input input-sm" value={asignada ?? ""} disabled={ocupado} onchange={(e) => cambiar(e.currentTarget.value)}>
        <option value="">{sinAsignar}</option>
        {#each sedes as s (s.id)}<option value={s.id}>{s.nombre}</option>{/each}
      </select>
    {:else}
      <span {id}>{sedePorId(actual.organizacion, asignada)?.nombre ?? sinAsignar}</span>
    {/if}
    <Ayuda id="sedes" />
  </span>
{/if}

<style>
  .sede {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    font-size: var(--fs-sm);
  }
  .lbl {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
    font-weight: 500;
  }
  .lbl :global(svg) {
    color: var(--text-3);
  }
  select {
    width: auto;
    max-width: 100%;
  }
</style>
