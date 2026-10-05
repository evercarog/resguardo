<script lang="ts">
  // Un repositorio en Estado (como la tarjeta de la web anterior): tranquila,
  // cuatro datos como mucho. Nombre y estado; dónde y de qué equipo; última
  // versión y próxima; la copia externa; la protección y los 14 días. El
  // detalle está en la página del repositorio.
  import { ChevronRight, Cloud, CloudOff } from "@lucide/svelte";
  import type { CopiaResumen, Equipo, RepoInforme, RepositorioResumen } from "$lib/tipos";
  import { bytes, numero, relativo } from "$lib/formato";
  import { bytesRepo, destinoDe, dias, estadoRepo, nVersiones, proteccion, ultimaEjecucion, ultimaVersion } from "$lib/repo";
  import { primeraCopiaFrase } from "$lib/salud";
  import Chip from "../Chip.svelte";
  import AnilloProteccion from "./AnilloProteccion.svelte";
  import DiasCuadros from "./DiasCuadros.svelte";
  // v1.41: dónde se guarda (y si se queda en el mismo equipo).
  import { actual } from "$lib/estado.svelte";
  import { comprobacionLugar, lugarDe, riesgoMismoEquipo } from "$lib/dondeGuarda";
  import SeGuardaEn from "../SeGuardaEn.svelte";

  let { repo, inf, equipo, cliente, ahora }: { repo: RepositorioResumen; inf: RepoInforme | null; equipo: Equipo; cliente: string; ahora: number } = $props();

  const copias = $derived((equipo.resumen?.copias ?? []).filter((k: CopiaResumen) => k.repo === repo.id));
  const destino = $derived(destinoDe(equipo.resumen?.destinos, repo));
  const todos = $derived(actual.equipos.length ? actual.equipos : [equipo]);
  const lugar = $derived(lugarDe(destino, equipo, todos));
  const riesgo = $derived(riesgoMismoEquipo(repo, equipo, todos));
  const prot = $derived(proteccion(inf, comprobacionLugar(repo, equipo, todos)));
  const ej = $derived(ultimaEjecucion(inf));
  const ult = $derived(ultimaVersion(repo, inf));
  const n = $derived(nVersiones(repo, inf));
  const proxima = $derived(
    copias
      .map((k) => k.proxima)
      .filter((x): x is string => !!x && Date.parse(x) > ahora)
      .sort()[0],
  );
  const estado = $derived(estadoRepo(repo, inf, copias, ahora));
  const href = $derived(`/c/${cliente}/equipos/${equipo.id}/repositorios/${encodeURIComponent(repo.id)}`);
</script>

<a class="card tarjeta tone-{estado.tono}" {href}>
  <div class="cab">
    <div class="nombre">
      <strong>{repo.nombre}</strong>
      <span class="faint">{equipo.nombre}</span>
      <SeGuardaEn pequeno {lugar} riesgo={!!riesgo} />
    </div>
    <Chip pequeno tono={estado.tono} texto={estado.texto} />
  </div>

  {#if n}
    <p class="linea num">
      Última versión <strong>{relativo(ult, ahora)}</strong>{#if proxima}{" · "}próxima {relativo(proxima, ahora)}{/if}
    </p>
    <p class="linea faint num">{numero(n)} {n === 1 ? "versión" : "versiones"}{#if bytesRepo(repo, inf) != null}{" · "}{bytes(bytesRepo(repo, inf))}{/if}</p>
  {:else if ej && ej.resultado !== "fallo"}
    <p class="linea faint">Última copia {relativo(ej.hora, ahora)}; el detalle de sus versiones llega con el próximo informe del equipo.</p>
  {:else}
    <p class="linea faint">{repo.solo_lectura ? "Importado de otro equipo." : primeraCopiaFrase(copias, ahora).replace(" También puedes pulsar «Copiar ahora».", "")}</p>
  {/if}

  <p class="linea externa" class:sin={!repo.externa && !inf?.externa}>
    {#if !repo.externa && inf?.externa}<Cloud size={13} />Copia externa{#if inf.externa.ultima}{" · "}última {relativo(inf.externa.ultima, ahora)}{/if}{#if inf.externa.resultado === "fallo"}{" · "}<span class="mal">falló</span>{/if}
    {:else if repo.externa}<Cloud size={13} />A «{repo.externa.destino}» cada día a las {repo.externa.hora}{#if inf?.externa?.ultima}{" · "}última {relativo(inf.externa.ultima, ahora)}{/if}
    {:else}<CloudOff size={13} />Sin copia externa{/if}
  </p>

  <div class="pie">
    {#if prot}
      <span class="prot"><AnilloProteccion proteccion={prot} tamano={16} /><span class="num">Protección {prot.puntuacion} de {prot.total}</span>{#if prot.pendientes}<span class="faint">{" · "}{prot.pendientes} por revisar</span>{/if}</span>
    {/if}
    {#if inf}<DiasCuadros dias={dias(inf, 14, ahora)} tamano="mini" etiqueta="Últimos 14 días de «{repo.nombre}»" />{/if}
    <ChevronRight size={16} />
  </div>
</a>

<style>
  .tarjeta {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: var(--sp-4) var(--sp-5);
    color: inherit;
    text-decoration: none;
    transition: border-color var(--dur-fast) var(--ease);
  }
  .tarjeta:hover {
    border-color: var(--border-strong);
  }
  .cab {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--sp-3);
    margin-bottom: 4px;
  }
  .nombre {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .nombre strong {
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .nombre .faint {
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
  }
  .linea {
    margin: 0;
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
  }
  .linea strong {
    font-weight: 500;
  }
  .externa {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
  }
  .externa.sin {
    color: var(--text-3);
  }
  .pie {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 14px;
    margin-top: 6px;
    padding-top: 10px;
    border-top: 1px solid var(--border);
    font-size: var(--fs-xs);
  }
  .prot {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .pie > :global(svg:last-child) {
    margin-left: auto;
    color: var(--text-3);
  }
  .mal {
    color: var(--bad);
  }
  @media (max-width: 480px) {
    .pie > :global(svg:last-child) {
      display: none;
    }
  }
</style>
