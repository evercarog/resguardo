<script lang="ts">
  // «Retención en el almacén» en la ficha del almacén (plan 0.8.1, B3; plan-mejoras 15s): la
  // que aplica en local (los equipos no pueden borrar allí), con lo pendiente explicado, y lo
  // que tiene **sin** retención allí (repositorios y copias derivadas que llegan al almacén),
  // con «Poner retención» (abre «Retención en el almacén…» en la página del repositorio).
  import { Database, History } from "@lucide/svelte";
  import { tip } from "$lib/tooltip";
  import { horarioEnFrase } from "$lib/formato";
  import { textoHorario } from "$lib/retencion";
  import { TEXTO_ADOPTADO, type CarpetaDelAlmacen } from "$lib/almacenUnico";
  import type { RetencionAlmacen } from "$lib/tipos";
  import Chip from "../Chip.svelte";
  import Tiempo from "../Tiempo.svelte";

  let {
    cliente,
    con,
    sin,
    administra = false,
  }: {
    cliente: string;
    con: { ra: RetencionAlmacen; dueno: CarpetaDelAlmacen }[];
    sin: CarpetaDelAlmacen[];
    administra?: boolean;
  } = $props();

  const hrefRepo = (x: CarpetaDelAlmacen) => (x.equipo && x.repo ? `/c/${cliente}/equipos/${x.equipo.id}/repositorios/${encodeURIComponent(x.repo.id)}` : null);
  const cuando = (ra: RetencionAlmacen) => (ra.horario.reglas?.length ? horarioEnFrase({ dias: [], horas: [], reglas: ra.horario.reglas }).replace(/^./, (x) => x.toLowerCase()) : (ra.horario_texto ?? textoHorario(ra.horario)));
</script>

{#if con.length || sin.length}
  <ul class="lista">
    {#each con as { ra, dueno } (ra.usuario + "/" + ra.repo)}
      {@const href = hrefRepo(dueno)}
      <li>
        <span class="ic" aria-hidden="true"><History size={14} /></span>
        <span class="txt">
          <span>{#if href}<a class="link" {href}><strong>{dueno.nombre}</strong></a>{:else}<strong class:mono={!dueno.equipo}>{dueno.nombre}</strong>{/if} <span class="faint">· {dueno.de}</span></span>
          <span class="faint">{ra.texto} · {cuando(ra)}{#if ra.ultima}{" · la última "}<Tiempo iso={ra.ultima} />{/if}</span>
          {#if ra.resultado === "fallo" && ra.mensaje}<span class="mal">{ra.mensaje}</span>
          {:else if ra.clave === "pendiente"}<span class="faint">Pendiente: su clave aún no abre el repositorio. La añade {dueno.equipo?.nombre ?? "su equipo"} cuando se conecte; hasta entonces no se aplica.</span>
          {:else if !ra.resultado}<span class="faint">Todavía no se ha aplicado: {#if ra.proxima}se aplica por primera vez <Tiempo iso={ra.proxima} />{:else}se aplica a su hora ({cuando(ra)}){/if}.</span>{/if}
        </span>
        {#if ra.resultado}<Chip pequeno tono={ra.resultado === "ok" ? "ok" : "bad"} texto={ra.resultado === "ok" ? "Aplicada" : "Falló"} />{:else}<Chip pequeno tono="neutral" texto={ra.clave === "pendiente" ? "Pendiente" : "Todavía no"} />{/if}
      </li>
    {/each}
    {#each sin as x (x.zona + "|" + x.valor)}
      {@const href = hrefRepo(x)}
      <li class="sin">
        <span class="ic" aria-hidden="true"><Database size={14} /></span>
        <span class="txt">
          <span>{#if href}<a class="link" {href}><strong>{x.nombre}</strong></a>{:else}<strong>{x.nombre}</strong>{/if} <span class="faint">· {x.de}</span>{#if x.adoptado} <span class="badge badge-sm tone-neutral" use:tip={TEXTO_ADOPTADO}>Adoptado</span>{/if}</span>
          <span class="faint">
            {#if x.tipo === "derivada"}Sin retención en el almacén: sus versiones se quedan todas. La de una copia derivada aún no se pone desde aquí.
            {:else if x.adoptado}Sin retención en el almacén. Un repositorio adoptado la tendrá cuando pase al almacén gestionado (con su propio usuario).
            {:else}Sin retención en el almacén: sus versiones se quedan todas (los equipos no pueden borrar aquí).{/if}
          </span>
        </span>
        {#if administra && href && x.tipo === "repo" && !x.adoptado}<a class="btn btn-sm" href="{href}?retencion=1">Poner retención</a>{:else}<Chip pequeno tono="neutral" texto="Sin retención" />{/if}
      </li>
    {/each}
  </ul>
{:else}
  <p class="faint vacio">Todavía no guarda ningún repositorio.</p>
{/if}

<style>
  .lista {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
    border-top: 1px solid var(--border);
  }
  li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3);
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
  }
  .ic {
    display: grid;
    flex: none;
    color: var(--text-3);
  }
  .txt {
    display: flex;
    flex: 1 1 220px;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    font-size: var(--fs-sm);
  }
  .txt strong {
    font-weight: 500;
    overflow-wrap: anywhere;
  }
  .txt > .faint,
  .mal {
    font-size: var(--fs-xs);
    overflow-wrap: anywhere;
  }
  .mal {
    color: var(--bad);
  }
  .txt :global(.badge) {
    margin-left: 4px;
  }
  .vacio {
    margin: 0;
    font-size: var(--fs-sm);
  }
</style>
