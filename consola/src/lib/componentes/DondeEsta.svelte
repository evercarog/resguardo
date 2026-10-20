<script lang="ts">
  // Plan 0.8.1, C1 (15n): «Dónde está» en la página de un repositorio y de una copia:
  // cada sitio donde existen sus datos con su ruta real, lo que ocupa allí, la
  // última vez y si está al día (lib/dondeEsta.ts). Con infraestructura anotada,
  // también dónde está físicamente cada sitio.
  import { MapPin, Server } from "@lucide/svelte";
  import type { Equipo, Informe, RepositorioResumen } from "$lib/tipos";
  import type { ReglaCopia } from "$lib/regla321";
  import { reglaDeCopia } from "$lib/regla321";
  import { caminoDeCopia } from "$lib/camino";
  import { enVivoCamino } from "$lib/caminoVivo.svelte";
  import { sitiosDe } from "$lib/dondeEsta";
  import { actual, reloj } from "$lib/estado.svelte";
  import { catalogoDe } from "$lib/catalogoDestinos.svelte";
  import { listaFuncionales } from "$lib/funcionales.svelte";
  import { bytes, relativo } from "$lib/formato";
  import Copiable from "./Copiable.svelte";
  import Chip from "./Chip.svelte";
  import TipoDestino from "./TipoDestino.svelte";

  let { equipo, repo, rc, informe, cliente }: { equipo: Equipo; repo: RepositorioResumen; rc: ReglaCopia | null; informe: Informe | null | undefined; cliente: string } = $props();

  // Sin ninguna copia activa (un repositorio importado o sin copias), con una copia de mentira: lo que guarda sigue estando.
  const regla = $derived(
    rc ??
      reglaDeCopia(
        equipo,
        { id: "", nombre: "", repo: repo.id, activa: true } as Parameters<typeof reglaDeCopia>[1],
        actual.equipos.map((x) => (x.id === equipo.id ? equipo : x)),
        informe,
        catalogoDe(cliente),
        reloj.ahora,
        actual.organizacion,
        listaFuncionales(cliente),
        actual.infra,
      ),
  );
  const camino = $derived(caminoDeCopia(regla, { cliente, ahora: reloj.ahora, enVivo: enVivoCamino, equipos: actual.equipos, informe }));
  const sitios = $derived(regla ? sitiosDe(regla, camino, actual.equipos, informe) : []);
  const CLASE = { repositorio: "El repositorio", espejo: "Espejo", derivada: "Copia derivada" } as const;
</script>

{#if sitios.length}
  <section class="card donde" id="sec-donde" aria-labelledby="t-donde">
    <h2 class="section-title" id="t-donde"><MapPin size={16} />Dónde está</h2>
    <p class="faint intro">Todos los sitios donde están sus datos, con su carpeta real. Si una ruta no es la que esperas, un espejo podría estar subiéndolo todo de nuevo.</p>
    <ul class="sitios">
      {#each sitios as s (s.id)}
        <li class="sitio">
          <div class="s-cab">
            <span class="s-clase faint">{CLASE[s.clase]}</span>
            {#if s.href}<a class="link s-nombre" href={s.href}>{s.nombre}</a>{:else}<span class="s-nombre">{s.nombre}</span>{/if}
            {#if s.tipo}<TipoDestino {...s.tipo} />{/if}
            {#if s.quien}<span class="faint">· lo hace {s.quien}</span>{/if}
            <span class="s-estado"><Chip pequeno tono={s.tono} texto={s.estado} /></span>
          </div>
          <div class="s-ruta">
            {#if s.ruta}
              <Copiable texto={s.ruta} que="la ruta" />
            {:else}
              <span class="faint">{s.nota}</span>
            {/if}
          </div>
          <div class="s-datos faint">
            {#if s.bytes}<span class="num">{bytes(s.bytes)}</span>{/if}
            {#if s.cuando}<span>{s.bytes ? " · " : ""}{s.clase === "repositorio" ? "última versión" : "última vez"} {relativo(s.cuando, reloj.ahora)}</span>{/if}
            {#if s.fisico}<span class="fisico"><Server size={12} aria-hidden="true" />{s.fisico}</span>{/if}
          </div>
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .donde {
    padding: var(--sp-5);
  }
  .section-title {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .intro {
    margin: 4px 0 var(--sp-3);
    font-size: var(--fs-sm);
  }
  .sitios {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .sitio {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: var(--sp-3) 0;
    border-top: 1px solid var(--border);
  }
  .s-cab {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .s-clase {
    min-width: 7.5em;
    font-size: var(--fs-xs);
  }
  .s-nombre {
    font-weight: 500;
  }
  .s-estado {
    margin-left: auto;
  }
  .s-ruta {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
  }
  .s-ruta :global(code) {
    overflow-wrap: anywhere;
    font-size: 12px;
    color: var(--text-2);
  }
  .s-datos {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px 8px;
    font-size: var(--fs-xs);
  }
  .fisico {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
</style>
