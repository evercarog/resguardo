<script lang="ts">
  // 0.8.1 (D2, 15y): «No copiar» de una copia: los grupos del cliente («Organización»)
  // de un toque, con sus reglas a la vista; las reglas sueltas al lado; «No copiar
  // archivos de más de N MB» con excepciones por tipo, y la vista previa (el equipo
  // cuenta cuánto deja fuera cada cosa en sus carpetas, sin copiar nada).
  //
  // Cambia la misma `CopiaConfig` del editor (guiado y «Avanzado»). Con un agente sin
  // `admite: "grupos_en_copias"` los grupos se aplican igual (sus reglas), pero el equipo
  // no recuerda de qué grupo vienen ni admite el límite de tamaño ni la vista previa.
  import { Check, Eye, FileX2, LoaderCircle, Plus, RefreshCw, TriangleAlert, X } from "@lucide/svelte";
  import { tip } from "$lib/tooltip";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import AvisoExclusiones from "./AvisoExclusiones.svelte";
  import { actual } from "$lib/estado.svelte";
  import { gruposExclusiones } from "$lib/gruposExclusiones";
  import { alDia, alternarGrupo, cuerpoVista, EXCEPCIONES_HABITUALES, MAX_EXCEPCIONES, MAX_MB, ponerAlDia, ponerSueltas, sueltas, usaGrupo, type VistaPrevia } from "$lib/exclusionesCopia";
  import { pedirVista } from "$lib/exclusionesPedir";
  import { bytes, numero, plural } from "$lib/formato";
  import type { Cliente, CopiaConfig, EquipoDetalle } from "$lib/tipos";

  let {
    k,
    id,
    equipo,
    cliente,
    prueba,
    admite,
    version = null,
  }: {
    k: CopiaConfig;
    id: string;
    equipo: EquipoDetalle;
    cliente: Cliente;
    prueba?: Uint8Array;
    /** ¿Entiende el agente los grupos, el tamaño máximo y la vista previa (`grupos_en_copias`)? */
    admite: boolean;
    version?: string | null;
  } = $props();

  const grupos = $derived(gruposExclusiones(actual));
  /** Los que la copia usa y ya no están en «Organización» (se quitaron): se ven igual. */
  const huerfanos = $derived((k.grupos_exclusiones ?? []).filter((g) => !grupos.some((x) => x.id === g.id)));
  const usado = (gid: string) => k.grupos_exclusiones?.find((g) => g.id === gid);
  const otras = $derived(sueltas(k));
  let verReglas = $state(false);

  // --- Tamaño máximo ---
  const conLimite = $derived(k.mayores_mb != null);
  function ponerLimite(si: boolean) {
    if (si) {
      k.mayores_mb = k.mayores_mb ?? 500;
      if (!k.excepto_tamano?.length) k.excepto_tamano = ["*.pst", "*.bak"];
    } else {
      k.mayores_mb = null;
      k.excepto_tamano = [];
    }
  }
  let excepcion = $state("");
  function anadirExcepcion(t: string) {
    const x = t.trim();
    if (!x || (k.excepto_tamano ?? []).includes(x) || (k.excepto_tamano?.length ?? 0) >= MAX_EXCEPCIONES) return void (excepcion = "");
    k.excepto_tamano = [...(k.excepto_tamano ?? []), x];
    excepcion = "";
  }
  const propuestas = $derived(EXCEPCIONES_HABITUALES.filter((x) => !(k.excepto_tamano ?? []).includes(x)));

  // --- Vista previa ---
  let vista = $state<VistaPrevia | null>(null);
  let pidiendo = $state(false);
  let paso = $state("");
  let error = $state("");
  const puedeVer = $derived(admite && !!prueba && k.carpetas.length > 0);
  async function verCuanto() {
    if (!prueba || pidiendo) return;
    pidiendo = true;
    error = "";
    try {
      vista = await pedirVista(cliente, equipo, cuerpoVista($state.snapshot(k) as CopiaConfig, grupos), prueba, (t) => (paso = t));
    } catch (e) {
      error = (e as Error).message;
    } finally {
      pidiendo = false;
      paso = "";
    }
  }
  const cuanto = (x: { archivos: number; bytes: number }) => (x.archivos ? `${bytes(x.bytes)} en ${plural(x.archivos, "archivo", "archivos")}` : "nada");
  const nombreGrupo = (gid: string) => grupos.find((g) => g.id === gid)?.nombre ?? huerfanos.find((g) => g.id === gid)?.nombre ?? gid;
</script>

<div class="field no-copiar">
  <span class="field-label">No copiar <Ayuda id="exclusiones" /></span>
  {#if grupos.length || huerfanos.length}
    <div class="grupos" role="group" aria-label="Grupos de «No copiar»">
      {#each grupos as g (g.id)}
        {@const on = usaGrupo(k, g.id)}
        {@const viejo = on && !alDia(usado(g.id)!, g)}
        <button type="button" class="chip-exc" class:on aria-pressed={on} use:tip={g.reglas.join("  ")} onclick={() => alternarGrupo(k, g, !on)}>{#if on}<Check size={12} />{/if}{g.nombre}</button>
        {#if viejo}<button type="button" class="btn btn-sm btn-ghost al-dia" use:tip={"El grupo cambió en «Organización»: aplica sus reglas de ahora"} onclick={() => ponerAlDia(k, g)}><RefreshCw size={12} />Poner «{g.nombre}» al día</button>{/if}
      {/each}
      {#each huerfanos as g (g.id)}
        <button type="button" class="chip-exc on huerfano" aria-pressed="true" use:tip={"Ya no está en «Organización». Púlsalo para quitar sus reglas."} onclick={() => alternarGrupo(k, g, false)}><Check size={12} />{g.nombre}<span class="faint">· ya no está</span></button>
      {/each}
    </div>
    <details class="ver-reglas" bind:open={verReglas}>
      <summary>Ver las reglas de cada grupo</summary>
      <ul>
        {#each grupos as g (g.id)}
          <li><strong>{g.nombre}</strong><span class="mono faint">{(usado(g.id)?.reglas ?? g.reglas).join("  ")}</span></li>
        {/each}
      </ul>
      <p class="faint nota">Los grupos se cambian en <a class="link" href="/c/{cliente.id}/ajustes/organizacion">Organización</a>. Cada copia guarda sus propias reglas: al cambiar un grupo, allí se ofrece aplicarlo a las copias que lo usan.</p>
    </details>
  {/if}
  <textarea
    id="{id}-sueltas"
    class="input mono"
    rows="2"
    spellcheck="false"
    aria-label="Otras reglas, una por línea"
    placeholder="Otras reglas, una por línea"
    value={otras.join("\n")}
    oninput={(e) => ponerSueltas(k, e.currentTarget.value.split(/\r?\n/))}
  ></textarea>
  <AvisoExclusiones {k} />
  {#if (k.grupos_exclusiones?.length ?? 0) > 0 && !admite}
    <p class="faint nota">Se aplican sus reglas. El agente{version ? ` (${version})` : ""} aún no recuerda de qué grupo vienen: actualízalo para que la consola sepa qué copias usan cada grupo.</p>
  {/if}

  <div class="tamano">
    {#if admite}
      <label class="switch-row"
        ><input type="checkbox" class="switch" checked={conLimite} onchange={(e) => ponerLimite(e.currentTarget.checked)} /><span>No copiar archivos grandes<span class="faint">Para no llenar el disco con vídeos, imágenes de disco o copias sueltas.</span></span></label
      >
      {#if conLimite}
        <div class="limite">
          <label class="cuanto">De más de <input id="{id}-mb" class="input num" type="number" min="1" max={MAX_MB} step="1" value={k.mayores_mb} oninput={(e) => (k.mayores_mb = Math.trunc(Number(e.currentTarget.value)) || 0)} aria-label="Tamaño máximo en MB" /> MB</label>
          <div class="salvo">
            <span class="faint">Salvo:</span>
            {#each k.excepto_tamano ?? [] as x (x)}
              <span class="chip-x mono">{x}<button type="button" class="icon-btn mini" aria-label="Quitar la excepción {x}" onclick={() => (k.excepto_tamano = (k.excepto_tamano ?? []).filter((y) => y !== x))}><X size={11} /></button></span>
            {/each}
            <input
              class="input mono excepcion"
              bind:value={excepcion}
              placeholder="*.pst"
              aria-label="Otro tipo que se copia aunque sea grande"
              spellcheck="false"
              onkeydown={(e) => (e.key === "Enter" || e.key === ",") && excepcion.trim() && (e.preventDefault(), anadirExcepcion(excepcion))}
            />
            {#each propuestas.slice(0, 4) as p (p)}<button type="button" class="chip-sug mono" onclick={() => anadirExcepcion(p)}><Plus size={11} />{p}</button>{/each}
          </div>
          <p class="faint nota">Los tipos de «Salvo» se copian aunque pasen del límite (buzones de correo, volcados de bases). Lo que quede fuera se dice en cada versión: cuántos archivos y de qué tipos.</p>
        </div>
      {/if}
    {:else}
      <p class="faint nota">Actualiza el agente{version ? ` (tiene la ${version})` : ""} para no copiar los archivos grandes y ver antes cuánto deja fuera cada grupo.</p>
    {/if}
  </div>

  {#if admite}
    <div class="vista">
      <button type="button" class="btn btn-sm" disabled={!puedeVer || pidiendo} onclick={verCuanto} use:tip={k.carpetas.length ? "El equipo cuenta lo que deja fuera cada cosa en estas carpetas, sin copiar nada" : "Elige antes las carpetas"}>
        {#if pidiendo}<LoaderCircle size={14} class="spin" />{paso || "Revisando…"}{:else}<Eye size={14} />Ver cuánto deja fuera{/if}
      </button>
      {#if error}<p class="err" role="alert"><TriangleAlert size={14} />{error}</p>{/if}
      {#if vista}
        <table class="tabla-vista">
          <caption class="sr-only">Lo que deja fuera «No copiar» en las carpetas de «{k.nombre}»</caption>
          <tbody>
            {#each vista.grupos.filter((g) => g.archivos > 0 || usaGrupo(k, g.id)) as g (g.id)}
              <tr class:activo={usaGrupo(k, g.id)}>
                <th scope="row">{nombreGrupo(g.id)}{#if !usaGrupo(k, g.id)}<span class="faint">{" · "}sin poner</span>{/if}</th>
                <td class="num">{cuanto(g)}</td>
              </tr>
            {/each}
            {#if vista.sueltas.archivos}<tr class="activo"><th scope="row">Otras reglas</th><td class="num">{cuanto(vista.sueltas)}</td></tr>{/if}
            {#if vista.tamano}<tr class="activo"><th scope="row"><FileX2 size={13} />Más de {numero(k.mayores_mb ?? 0)} MB</th><td class="num">{cuanto(vista.tamano)}</td></tr>{/if}
            <tr class="total"><th scope="row">Se copiaría</th><td class="num">{cuanto(vista.quedan)} <span class="faint">de {bytes(vista.total.bytes)}</span></td></tr>
          </tbody>
        </table>
        <p class="faint nota">{vista.completo ? "Cuenta lo que hay ahora. Los grupos «sin poner» dicen lo que dejarían fuera si los pones." : "No dio tiempo a revisarlo todo (como mucho un minuto): es una muestra."}</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .no-copiar {
    gap: var(--sp-2);
  }
  .grupos,
  .salvo {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .chip-exc {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-height: 28px;
    padding: 0 10px;
    font: inherit;
    font-size: var(--fs-xs);
    font-weight: 550;
    color: var(--text-2);
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    cursor: pointer;
  }
  .chip-exc.on {
    color: var(--accent-text);
    background: var(--accent-soft);
    border-color: color-mix(in srgb, var(--accent) 35%, transparent);
  }
  .chip-exc:focus-visible,
  .chip-sug:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .huerfano .faint {
    font-weight: 400;
  }
  .al-dia {
    height: 28px;
    font-size: var(--fs-xs);
  }
  .ver-reglas > summary {
    font-size: var(--fs-xs);
    color: var(--text-2);
    cursor: pointer;
  }
  .ver-reglas ul {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 6px 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-xs);
  }
  .ver-reglas li {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 8px;
  }
  .mono {
    font-family: var(--mono);
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  textarea.mono {
    font-size: 12.5px;
  }
  .nota {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .tamano,
  .limite,
  .vista {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .tamano {
    margin-top: var(--sp-2);
  }
  .cuanto {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .cuanto .num {
    width: 7em;
  }
  .chip-x {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: 26px;
    padding: 0 2px 0 8px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
  }
  .mini {
    width: 22px;
    height: 22px;
  }
  .excepcion {
    width: 8em;
    height: 28px;
  }
  .chip-sug {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: 26px;
    padding: 0 8px;
    color: var(--text-2);
    background: var(--surface);
    border: 1px dashed var(--border-strong);
    border-radius: 999px;
    cursor: pointer;
  }
  .vista {
    align-items: flex-start;
  }
  .err {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--bad);
  }
  .tabla-vista {
    width: 100%;
    max-width: 520px;
    border-collapse: collapse;
    font-size: var(--fs-sm);
  }
  .tabla-vista th,
  .tabla-vista td {
    padding: 4px 0;
    text-align: left;
    font-weight: 400;
    border-bottom: 1px solid var(--border);
    overflow-wrap: anywhere;
  }
  .tabla-vista td {
    padding-left: var(--sp-3);
    text-align: right;
  }
  .tabla-vista th :global(svg) {
    vertical-align: -2px;
    margin-right: 4px;
  }
  .tabla-vista tr:not(.activo):not(.total) {
    color: var(--text-3);
  }
  .tabla-vista .total th,
  .tabla-vista .total td {
    font-weight: 600;
    border-bottom: none;
  }
</style>
