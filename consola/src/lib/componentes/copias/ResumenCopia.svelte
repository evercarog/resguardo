<script lang="ts">
  // Una copia en una línea (plan 0.7.26, bloque 3.1): cuándo · qué → dónde,
  // con el icono del tipo de destino; y debajo, sus espejos y copias derivadas
  // agrupados por quién los hace (lib/espejosCopia.ts). Sin emojis: iconos.
  import { CalendarClock, CornerDownRight, Database, FolderOpen, GitBranch, HardDrive, Link2, Repeat } from "@lucide/svelte";
  import { tip } from "$lib/tooltip";
  import TipoDestino from "$lib/componentes/TipoDestino.svelte";
  import { cuandoEnFrase, inicioDe, queEnFrase } from "$lib/copiaGuiada";
  import { espejosDeCopia } from "$lib/espejosCopia";
  import { pasosDelRepo } from "$lib/cadenas";
  import { tipoDePaso } from "$lib/regla321";
  import type { CopiaConfig, DestinoCatalogo, Equipo } from "$lib/tipos";

  let {
    copias,
    i,
    repos,
    equipo,
    equipos,
    catalogo,
    espejos = true,
  }: {
    copias: CopiaConfig[];
    i: number;
    repos: { id: string; nombre: string }[];
    equipo: Equipo;
    equipos: Equipo[];
    catalogo: DestinoCatalogo[];
    /** Con los espejos y derivadas debajo. */
    espejos?: boolean;
  } = $props();

  const k = $derived(copias[i]);
  const modo = $derived(inicioDe(k));
  const ICONO = { horario: CalendarClock, cadena: Link2, despues: Repeat } as const;
  const Icono = $derived(ICONO[modo]);
  const repo = $derived(repos.find((r) => r.id === k.repo));
  const p0 = $derived(k.repo ? pasosDelRepo(equipo, k.repo, equipos)[0] : undefined);
  const t0 = $derived(p0 ? tipoDePaso(p0, equipos, catalogo) : null);
  const grupos = $derived(espejos && k.repo ? espejosDeCopia(equipo, k.repo, equipos) : []);
</script>

<div class="resumen-copia">
  <p class="linea">
    <span class="trozo"><Icono size={14} />{cuandoEnFrase(copias, i)}</span>
    <span class="sep" aria-hidden="true">·</span>
    <span class="trozo"><FolderOpen size={14} />{queEnFrase({ carpetas: k.carpetas, exclusiones: [] })}</span>
    <span class="sep" aria-hidden="true">→</span>
    <span class="trozo destino">
      {#if t0}<TipoDestino {...t0} soloIcono />{:else}<Database size={14} />{/if}
      <strong>{repo?.nombre ?? "Sin repositorio"}</strong>{#if p0}<span class="faint">({p0.texto})</span>{/if}
    </span>
  </p>
  {#if grupos.length}
    <ul class="espejos" aria-label="Espejos y copias derivadas de «{repo?.nombre ?? k.repo}»">
      {#each grupos as g (g.clave)}
        <li>
          <CornerDownRight size={13} />
          <span class="quien">{#if g.quien.almacen}<HardDrive size={13} />{:else}<GitBranch size={13} />{/if}{g.quien.nombre}<span class="faint que">{g.quien.almacen ? (g.pasos.length === 1 ? "espejo" : "espejos") : g.pasos.length === 1 ? "copia derivada" : "copias derivadas"}</span></span>
          {#each g.pasos as p, j (j)}
            {@const tp = tipoDePaso(p, equipos, catalogo)}
            <span class="paso" use:tip={p.detalle + (p.noInmutable ? ` · ${p.noInmutable}` : "")}>
              {#if tp}<TipoDestino {...tp} soloIcono />{/if}{p.texto}
            </span>
          {/each}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .resumen-copia {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .linea {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px 8px;
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .trozo {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    overflow-wrap: break-word;
  }
  .trozo > :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .destino strong {
    color: var(--text-1);
    font-weight: 550;
  }
  .sep {
    color: var(--text-3);
  }
  .espejos {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0 0 0 4px;
    list-style: none;
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .espejos li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px 10px;
  }
  .espejos li > :global(svg) {
    flex: none;
    color: var(--text-3);
  }
  .quien {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-weight: 600;
    color: var(--text-1);
  }
  .quien > :global(svg) {
    color: var(--text-3);
  }
  .que {
    font-weight: 400;
  }
  .paso {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .paso > :global(svg) {
    color: var(--text-3);
  }
</style>
