<script lang="ts">
  // «Equipos que no están en todas las consolas» (lib/consolasCliente.ts):
  // - en Estado (sin `equipo`): «N equipos no están en todas tus consolas», y
  //   por cada consola, cuáles faltan y «Conectar también…» con todos ellos;
  // - en un equipo (con `equipo`, también al terminar de darlo de alta): «Este
  //   equipo solo está en esta consola; los demás también están en «X»».
  // El botón abre el flujo de siempre («Conectar también a otra consola…»):
  // hace falta un código de conexión de esa consola y la clave de
  // administración; aquí solo se eligen ya los equipos y se dice cuál es la
  // consola esperada (su identidad, que ya tienen fijada los demás equipos).
  import { Monitor } from "@lucide/svelte";
  import { puede } from "$lib/estado.svelte";
  import { plural } from "$lib/formato";
  import { admiteVarias, equiposQueFaltan, faltaEn, fraseEquipo, nombreConsola, otrasConsolas, type OtraConsola } from "$lib/consolasCliente";
  import type { Cliente, Equipo } from "$lib/tipos";
  import ConectarConsola from "./ConectarConsola.svelte";

  let {
    cliente,
    equipos,
    equipo,
    nuevo = false,
    ahora,
  }: {
    cliente: Cliente;
    /** Todos los equipos del cliente (las consolas son del cliente, no de un filtro). */
    equipos: Equipo[];
    /** Solo lo de este equipo. */
    equipo?: Equipo;
    /** Recién dado de alta: aunque aún no haya mandado su resumen, solo está aquí. */
    nuevo?: boolean;
    ahora: number;
  } = $props();

  const consolas = $derived(otrasConsolas(equipos, ahora, nuevo && equipo ? [equipo.id] : []));
  const deEste = $derived(equipo ? faltaEn(equipo, consolas) : []);
  const faltan = $derived(equiposQueFaltan(consolas));
  const puedeConectar = $derived(puede.administrar(cliente.rol));
  let abierta = $state<{ consola: OtraConsola; ids: string[] } | null>(null);

  /** «A, B y C» (con «y N más» si son muchos). */
  function nombres(es: Equipo[], max = 4) {
    const n = es.map((e) => e.nombre);
    if (n.length > max) return `${n.slice(0, max).join(", ")} y ${n.length - max} más`;
    return n.length > 1 ? `${n.slice(0, -1).join(", ")} y ${n.at(-1)}` : (n[0] ?? "");
  }
  const anteriores = (es: Equipo[]) => es.filter((e) => e.resumen && !admiteVarias(e));
</script>

{#if equipo}
  {#each deEste as c (c.identidad)}
    <div class="notice notice-info aviso" role="note">
      <Monitor size={16} />
      <div class="cuerpo">
        <p>{fraseEquipo(equipo, c, equipos)}</p>
        {#if equipo.resumen && !admiteVarias(equipo)}
          <p class="nota">Su agente es anterior: actualízalo para poder conectarlo también allí.</p>
        {:else if puedeConectar}
          <div class="acciones">
            <button type="button" class="btn btn-sm" onclick={() => (abierta = { consola: c, ids: [equipo.id] })}>Conectar también…</button>
            <span class="nota">Necesitas un código de conexión de «{c.nombre}» y la clave de administración.</span>
          </div>
        {:else}
          <p class="nota">Pídeselo a quien administra este cliente.</p>
        {/if}
      </div>
    </div>
  {/each}
{:else if faltan.length}
  <section class="notice notice-info aviso" aria-labelledby="t-aviso-consolas">
    <Monitor size={16} />
    <div class="cuerpo">
      <p id="t-aviso-consolas"><strong>{plural(faltan.length, "equipo no está", "equipos no están")} en todas tus consolas.</strong> Un equipo nuevo solo llega a la consola donde lo añades: conéctalo también a las demás para gestionarlo desde cualquiera.</p>
      <ul>
        {#each consolas.filter((c) => c.sin.length) as c (c.identidad)}
          {@const viejos = anteriores(c.sin)}
          <li>
            <span class="texto">
              <span>{c.sin.length === 1 ? "Falta" : "Faltan"} en <strong>{nombreConsola(c)}</strong>: {nombres(c.sin)}.</span>
              <span class="nota">Ya {c.con.length === 1 ? "está" : "están"} allí {plural(c.con.length, "equipo", "equipos")}{viejos.length ? ` · ${plural(viejos.length, "necesita", "necesitan")} actualizar el agente: ${nombres(viejos)}` : ""}.</span>
            </span>
            {#if puedeConectar && viejos.length < c.sin.length}
              <button type="button" class="btn btn-sm" onclick={() => (abierta = { consola: c, ids: c.sin.map((e) => e.id) })}>Conectar también…</button>
            {/if}
          </li>
        {/each}
      </ul>
    </div>
  </section>
{/if}

{#if abierta}
  <ConectarConsola {cliente} {equipos} esperada={abierta.consola} soloEquipos={abierta.ids} onclose={() => (abierta = null)} />
{/if}

<style>
  .aviso {
    align-items: flex-start;
  }
  .cuerpo {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .cuerpo p {
    margin: 0;
  }
  .nota {
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
  }
  ul {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: 8px 0;
    border-top: 1px solid var(--border);
  }
  li .texto {
    display: flex;
    flex: 1;
    color: var(--text-1);
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  @media (max-width: 640px) {
    li {
      flex-wrap: wrap;
    }
    li .texto {
      flex-basis: 100%;
    }
  }
</style>
