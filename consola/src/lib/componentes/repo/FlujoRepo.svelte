<script lang="ts">
  // El camino de los datos, como en la app de escritorio: tus archivos → el
  // repositorio (en su destino) → la copia externa o el espejo. Cada paso con
  // su estado en icono y palabra, y el horario en la flecha.
  import { ArrowRight, CircleAlert, CircleCheck, CircleDashed, Cloud, CloudOff, FolderOpen, ShieldAlert, ShieldCheck, TriangleAlert } from "@lucide/svelte";
  import { lugarDe } from "$lib/dondeGuarda";
  import { cuandoCortoEspejo } from "$lib/espejo";
  import { ICONO_LUGAR } from "../SeGuardaEn.svelte";
  import type { CopiaResumen, DestinoResumen, Equipo, RepoInforme, RepositorioResumen } from "$lib/tipos";
  import { horarioEnFrase, numero, relativo } from "$lib/formato";
  import { resultadoConError } from "$lib/salud";
  import { destinoDe, nVersiones, proteccion, ultimaEjecucion, ultimaVersion } from "$lib/repo";
  import Ayuda from "../Ayuda.svelte";

  let {
    repo,
    inf,
    copias,
    destinos,
    equipos,
    ahora,
    equipo,
  }: { repo: RepositorioResumen; inf: RepoInforme | null; copias: CopiaResumen[]; destinos: DestinoResumen[]; equipos: Equipo[]; ahora: number; /** v1.41: el equipo que copia (para decir «en este mismo equipo»). */ equipo?: Equipo } = $props();

  const suyas = $derived(copias.filter((k) => k.repo === repo.id));
  const destino = $derived(destinoDe(destinos, repo));
  const almacen = $derived(destino?.equipo_almacen ? equipos.find((e) => e.id === destino.equipo_almacen) : undefined);
  const espejo = $derived(almacen?.resumen?.guarda_copias?.espejo ?? null);
  const borrado = $derived(proteccion(inf)?.items.find((i) => i.id === "borrado") ?? null);
  const ej = $derived(ultimaEjecucion(inf));
  const ult = $derived(ultimaVersion(repo, inf) ?? suyas.map((k) => k.ultima?.cuando).filter(Boolean).sort().at(-1) ?? null);

  const tonoOrigen = $derived(
    ej?.resultado === "fallo" || suyas.some((k) => k.ultima?.estado === "fallo") ? "bad" : ej?.resultado === "aviso" ? "warn" : ult ? "ok" : "neutral",
  );
  // v1.41: dónde está el repositorio, en palabras (y si es en el mismo equipo).
  const lugar = $derived(lugarDe(destino, equipo ?? { id: "", nombre: "" }, equipos));
  const Icono = $derived(ICONO_LUGAR[lugar.clase]);
  const mismo = $derived(!!equipo && (lugar.clase === "carpeta" || lugar.clase === "almacen_propio"));
  const tercero = $derived(
    repo.externa
      ? {
          tono: inf?.externa ? (inf.externa.resultado === "fallo" ? "bad" : inf.externa.resultado === "aviso" ? "warn" : "ok") : "warn",
          nombre: repo.externa.destino,
          sub: inf?.externa?.ultima ? `Última ${relativo(inf.externa.ultima, ahora)}` : `Cada día a las ${repo.externa.hora}`,
          flecha: `cada día a las ${repo.externa.hora}`,
          texto: inf?.externa ? (inf.externa.resultado === "fallo" ? (inf.externa.mensaje_corto ?? "La última falló") : inf.externa.resultado === "aviso" ? "Con avisos" : "Al día") : "Todavía sin ninguna",
        }
      : inf?.externa
        ? {
            // El agente informa de una copia externa propia (p. ej. a la nube) aunque el resumen no la nombre.
            tono: inf.externa.resultado === "fallo" ? "bad" : inf.externa.resultado === "aviso" ? "warn" : "ok",
            nombre: "Copia externa",
            sub: inf.externa.ultima ? `Última ${relativo(inf.externa.ultima, ahora)}` : "Todavía sin ninguna",
            flecha: "copia externa",
            texto: inf.externa.resultado === "fallo" ? (inf.externa.mensaje_corto ?? "La última falló") : inf.externa.resultado === "aviso" ? "Con avisos" : "Al día",
          }
        : espejo
        ? {
            tono: resultadoConError(espejo.resultado) || espejo.destinos?.some((d) => resultadoConError(d.resultado)) ? "bad" : espejo.ultima ? "ok" : "warn",
            nombre: espejo.destinos?.length ? espejo.destinos.map((d) => (d.tipo === "nube" ? d.nube : d.carpeta)).join(", ") : `Espejo de ${almacen?.nombre}`,
            sub: espejo.ultima ? `Último ${relativo(espejo.ultima, ahora)}` : "Todavía sin ninguno",
            flecha: cuandoCortoEspejo(espejo),
            texto: resultadoConError(espejo.resultado) ? espejo.resultado!.replace(/^\s*ERROR:\s*/i, "") : espejo.destinos?.some((d) => resultadoConError(d.resultado)) ? "Algún destino falló" : espejo.ultima ? "Al día" : "Programado",
          }
        : null,
  );
</script>

<section class="card flujo" aria-labelledby="t-flujo">
  <h2 class="section-title" id="t-flujo">Cómo se protegen tus datos</h2>
  <ol class="pasos">
    <li class="paso tone-{tonoOrigen}">
      <span class="ic" aria-hidden="true"><FolderOpen size={18} /></span>
      <div class="cuerpo">
        <span class="k">Tus archivos</span>
        <strong>{suyas.length ? suyas.map((k) => k.nombre).join(", ") : repo.solo_lectura ? "Importado de otro equipo" : "Ninguna copia guarda aquí"}</strong>
        <span class="st">
          {#if tonoOrigen === "ok"}<CircleCheck size={13} />{:else if tonoOrigen === "bad"}<CircleAlert size={13} />{:else if tonoOrigen === "warn"}<TriangleAlert size={13} />{:else}<CircleDashed size={13} />{/if}
          {#if tonoOrigen === "bad"}La última copia falló{:else if ult}Última copia {relativo(ult, ahora)}{:else}Todavía sin copias{/if}
        </span>
      </div>
    </li>

    <li class="flecha" aria-hidden="true"><ArrowRight size={16} /><span>{suyas[0] ? horarioEnFrase(suyas[0].horario).replace(/^Cada día laborable/, "Días laborables") : ""}</span></li>

    <li class="paso tone-{borrado?.estado === 'ok' || destino?.inmutable ? 'ok' : borrado?.estado === 'fallo' || borrado?.estado === 'aviso' ? 'warn' : 'neutral'}">
      <span class="ic" aria-hidden="true"><Icono size={18} /></span>
      <div class="cuerpo">
        <span class="k">Repositorio <Ayuda id="repositorio" /></span>
        <strong>{repo.nombre}</strong>
        <span class="faint sub">{lugar.texto}{lugar.detalle ? ` · ${lugar.detalle}` : ""}{nVersiones(repo, inf) ? ` · ${numero(nVersiones(repo, inf))} versiones` : ""}</span>
        {#if mismo && !tercero}<span class="st aviso-mismo"><TriangleAlert size={13} /> En el mismo equipo que protege</span>{/if}
        <span class="st">
          {#if borrado?.estado === "ok" || destino?.inmutable}<ShieldCheck size={13} /> Protegido contra borrado
          {:else if borrado?.estado === "aviso" || borrado?.estado === "fallo"}<ShieldAlert size={13} /> Se puede borrar desde el equipo
          {:else}<CircleDashed size={13} /> Protección contra borrado sin comprobar{/if}
        </span>
      </div>
    </li>

    <li class="flecha" class:off={!tercero} aria-hidden="true"><ArrowRight size={16} /><span>{tercero?.flecha ?? ""}</span></li>

    <li class="paso tone-{tercero?.tono ?? 'neutral'}" class:discontinuo={!tercero}>
      <span class="ic" aria-hidden="true">{#if tercero}<Cloud size={18} />{:else}<CloudOff size={18} />{/if}</span>
      <div class="cuerpo">
        <span class="k">Copia externa <Ayuda id="copia-externa" /></span>
        <strong>{tercero?.nombre ?? "Todavía no hay"}</strong>
        <span class="faint sub">{tercero?.sub ?? "Todas las versiones están en un solo lugar."}</span>
        {#if tercero}
          <span class="st">
            {#if tercero.tono === "ok"}<CircleCheck size={13} />{:else if tercero.tono === "bad"}<CircleAlert size={13} />{:else}<TriangleAlert size={13} />{/if}
            {tercero.texto}
          </span>
        {/if}
      </div>
    </li>
  </ol>
</section>

<style>
  .flujo {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-5);
  }
  .pasos {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: stretch;
    gap: var(--sp-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .paso {
    display: flex;
    gap: var(--sp-3);
    min-width: 0;
    padding: var(--sp-4);
    background: var(--surface-2);
    border-radius: var(--radius-lg);
  }
  .paso.discontinuo {
    background: transparent;
    border: 1px dashed var(--border-strong);
  }
  .ic {
    display: grid;
    flex: none;
    place-items: center;
    width: 36px;
    height: 36px;
    color: var(--text-2);
    background: var(--surface);
    border-radius: var(--radius);
  }
  .cuerpo {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .k {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    font-weight: 500;
    color: var(--text-3);
  }
  strong {
    font-weight: 500;
    overflow-wrap: anywhere;
  }
  .sub {
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
    overflow-wrap: anywhere;
  }
  .st {
    display: block;
    margin-top: 4px;
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
    font-weight: 500;
    color: var(--tone);
  }
  .st :global(svg) {
    margin-right: 4px;
    vertical-align: -2px;
  }
  .tone-neutral .st {
    font-weight: 400;
    color: var(--text-3);
  }
  .st.aviso-mismo {
    font-weight: 500;
    color: var(--warn);
  }
  .flecha {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    max-width: 120px;
    color: var(--text-3);
    text-align: center;
  }
  .flecha span {
    font-size: var(--fs-xs);
    line-height: var(--lh-xs);
  }
  .flecha.off {
    opacity: 0.5;
  }
  @media (max-width: 900px) {
    .pasos {
      grid-template-columns: minmax(0, 1fr);
    }
    .flecha {
      flex-direction: row;
      justify-content: flex-start;
      max-width: none;
      padding-left: var(--sp-6);
    }
    .flecha :global(svg) {
      transform: rotate(90deg);
    }
  }
</style>
