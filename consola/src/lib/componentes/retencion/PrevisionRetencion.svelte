<script lang="ts">
  // «Lo que se eliminará próximamente»: la simulación con la regla de ahora
  // (retencionDetalle.ts, `prever`): lo que quitará la próxima vuelta, lo que
  // irá dejando de entrar en la regla los próximos días y por qué se queda
  // cada una de las demás. Siempre dice que es una simulación.
  import { ChevronDown, ChevronRight, Info } from "@lucide/svelte";
  import { numero, plural } from "$lib/formato";
  import { claveDia, textoSeQueda, textoSeVa, type FilaVersion, type Prevision } from "$lib/retencionDetalle";
  import type { Regla } from "$lib/tipos";
  import TablaVersiones from "./TablaVersiones.svelte";

  interface Ver {
    id: string;
    hora: string;
    copia: string | null;
    bytes: number | null;
  }
  let {
    prevision,
    versiones,
    regla,
    nombreCopia,
    dias,
    total,
    almacen = null,
    copia = null,
    dia = null,
    ahora,
  }: {
    prevision: Prevision;
    versiones: Ver[];
    regla: Regla;
    nombreCopia: (id: string | null) => string | null;
    dias: number;
    /** Cuántas versiones tiene el repositorio (todas, no solo las que conoce la consola). */
    total: number | null;
    /** Si la aplica un almacén, su nombre (no toca las sospechosas). */
    almacen?: string | null;
    copia?: string | null;
    dia?: string | null;
    ahora: number;
  } = $props();

  const fmtCuando = new Intl.DateTimeFormat("es", { weekday: "long", day: "numeric", month: "long", hour: "2-digit", minute: "2-digit" });
  const fmtDia = new Intl.DateTimeFormat("es", { weekday: "long", day: "numeric", month: "long" });
  const porId = $derived(new Map(versiones.map((v) => [v.id, v])));
  const deCopia = (ids: string[]) => ids.filter((x) => !copia || porId.get(x)?.copia === copia);
  const fila = (vid: string, quedan = false): FilaVersion => {
    const v = porId.get(vid)!;
    const p = prevision.porVersion.get(vid);
    const hora = new Date(v.hora);
    let porque: string;
    if (quedan && p?.ahora.queda) {
      porque = textoSeQueda(p.ahora.queda, hora);
      if (p.seVa) porque += ` Dejará de entrar el ${fmtDia.format(new Date(p.seVa))}: ${textoSeVa(p.motivoSeVa, hora, regla).replace(/^./, (x) => x.toLowerCase())}`;
    } else porque = textoSeVa(p?.motivoSeVa ?? null, hora, regla);
    return { id: v.id, hora: v.hora, copia: nombreCopia(v.copia), bytes: v.bytes, porque, tono: quedan ? "queda" : "va" };
  };

  const proximas = $derived(deCopia(prevision.proxima.ids));
  const proximaDia = $derived(claveDia(prevision.proxima.cuando ?? ahora));
  const verProxima = $derived(!dia || dia === proximaDia);
  const despues = $derived(prevision.porDia.map((d) => ({ ...d, ids: deCopia(d.ids) })).filter((d) => d.ids.length && (!dia || d.dia === dia)));
  const quedan = $derived(
    versiones
      .filter((v) => (!copia || v.copia === copia) && prevision.porVersion.get(v.id)?.ahora.queda)
      .sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora))
      .map((v) => v.id),
  );
  let verQuedan = $state(false);
  const id = $props.id();
</script>

<div class="notice notice-info sim">
  <Info size={16} />
  <p>
    <strong>Es una simulación</strong> con la regla de ahora sobre {plural(versiones.length, "versión", "versiones")} que conoce la consola{total && total > versiones.length ? ` (las de los últimos 60 días; el repositorio tiene ${numero(total)})` : ""}.
    La decisión es de restic al aplicarla{almacen ? `, y el almacén ${almacen} no toca las versiones cuya hora no cuadra con su subida` : ""}. Se cuenta por copia (restic agrupa por equipo y carpetas).
    {prevision.supuestas ? `Para los próximos días se suponen ${plural(prevision.supuestas, "copia nueva", "copias nuevas")} según su horario.` : ""}
    {prevision.sinHorario.length ? `De ${prevision.sinHorario.map((k) => `«${nombreCopia(k) ?? "otras"}»`).join(", ")} no se conoce el horario: no se suponen versiones nuevas suyas.` : ""}
  </p>
</div>

{#if verProxima}
  <section class="bloque" aria-labelledby="{id}-prox">
    <h3 id="{id}-prox">
      {prevision.proxima.cuando ? `La próxima vez que se aplique, el ${fmtCuando.format(new Date(prevision.proxima.cuando))}` : "Al aplicar la retención"}
      <span class="cuenta">{proximas.length ? `· se eliminarán ${plural(proximas.length, "versión", "versiones")}` : "· no se eliminaría ninguna"}</span>
    </h3>
    {#if proximas.length}
      <TablaVersiones filas={proximas.map((x) => fila(x))} titulo="Versiones que quitaría la retención la próxima vez" />
    {/if}
  </section>
{/if}

<section class="bloque" aria-labelledby="{id}-desp">
  <h3 id="{id}-desp">En los próximos {dias} días <span class="cuenta">· {despues.length ? `irán dejando de entrar ${plural(despues.reduce((s, d) => s + d.ids.length, 0), "versión", "versiones")}` : "ninguna más deja de entrar en la regla"}</span></h3>
  {#if despues.length}
    <p class="faint pie">Cada una, el día en que deja de entrar en la regla: se quitará la primera vez que se aplique la retención desde ese día.</p>
    {#each despues as d (d.dia)}
      <h4>{fmtDia.format(new Date(d.t)).replace(/^./, (x) => x.toUpperCase())} <span class="cuenta">· {plural(d.ids.length, "versión", "versiones")}</span></h4>
      <TablaVersiones filas={d.ids.map((x) => fila(x))} titulo="Versiones que dejan de entrar el {fmtDia.format(new Date(d.t))}" />
    {/each}
  {/if}
</section>

{#if !dia}
  <section class="bloque">
    <button class="abrir" aria-expanded={verQuedan} aria-controls="{id}-quedan" onclick={() => (verQuedan = !verQuedan)}>
      {#if verQuedan}<ChevronDown size={16} />{:else}<ChevronRight size={16} />{/if}
      Las que se quedan ({numero(quedan.length)}) y por qué
    </button>
    {#if verQuedan}
      <div id="{id}-quedan"><TablaVersiones filas={quedan.map((x) => fila(x, true))} titulo="Versiones que se quedan" /></div>
    {/if}
  </section>
{/if}

<style>
  .sim p {
    font-size: var(--fs-sm);
  }
  .bloque {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  h3 {
    margin: 0;
    font-size: var(--fs-body);
    font-weight: 600;
  }
  h3::first-letter {
    text-transform: uppercase;
  }
  h4 {
    margin: var(--sp-2) 0 0;
    font-size: var(--fs-sm);
    font-weight: 600;
  }
  .cuenta {
    font-weight: 400;
    color: var(--text-2);
  }
  .pie {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .abrir {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    align-self: flex-start;
    padding: 2px 4px 2px 0;
    border: none;
    background: none;
    font: inherit;
    font-weight: 600;
    color: var(--text-1);
    cursor: pointer;
  }
  .abrir:focus-visible {
    outline: 2px solid var(--focus, var(--accent));
    border-radius: var(--radius-sm);
  }
</style>
