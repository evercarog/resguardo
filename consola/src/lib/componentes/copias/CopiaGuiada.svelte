<script lang="ts">
  // Una copia de carpetas, paso a paso (plan 0.7.26, bloque 3.2): Cuándo →
  // Qué → Dónde → Resumen. Los pasos hechos quedan arriba como una línea que se
  // toca para volver; lo poco habitual, en «Más opciones» dentro del paso.
  // Intro avanza; Esc cierra la copia (lo hecho se queda, sin enviar).
  //
  // Cambia la misma `CopiaConfig` que la tarjeta «Avanzado», con las mismas
  // funciones (lib/copiaGuiada.ts): las dos guardan lo mismo.
  import { untrack } from "svelte";
  import { ArrowRight, Check, FolderOpen, LoaderCircle, Plus, Save, Sparkles, TriangleAlert, X } from "@lucide/svelte";
  import { tip } from "$lib/tooltip";
  import PasoGuiado from "./PasoGuiado.svelte";
  import CuandoEmpieza from "./CuandoEmpieza.svelte";
  import ElegirRepositorio, { type PedidoRepoNuevo } from "./ElegirRepositorio.svelte";
  import ResumenCopia from "./ResumenCopia.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import EditorGanchos from "$lib/componentes/EditorGanchos.svelte";
  import TiraRegla from "$lib/componentes/regla/TiraRegla.svelte";
  import { alternarExclusion, cuandoEnFrase, EXCLUSIONES_HABITUALES, pasoHecho, PASOS_COPIA, queEnFrase, sugerencias, TEXTO_SUGERENCIA, TITULO_PASO, type PasoCopia, type Sugerencia } from "$lib/copiaGuiada";
  import { ganchosDe } from "$lib/ganchos";
  import { fraseConfig, type ReglaCopia } from "$lib/regla321";
  import { lista } from "$lib/formato";
  import type { Cliente, CopiaConfig, DestinoCatalogo, Equipo, EquipoDetalle } from "$lib/tipos";

  let {
    copias,
    i,
    paso = $bindable(),
    equipo,
    equipos,
    catalogo,
    repos,
    cliente,
    prueba,
    version,
    admite,
    regla,
    puede,
    pendientes = [],
    problemas = [],
    cambiado,
    guardando,
    textoGuardando = "",
    ahora,
    onElegirCarpetas,
    onNuevoRepo,
    onSugerencia,
    onGuardar,
    onCerrar,
  }: {
    copias: CopiaConfig[];
    i: number;
    paso: PasoCopia;
    equipo: EquipoDetalle;
    equipos: Equipo[];
    catalogo: DestinoCatalogo[];
    repos: { id: string; nombre: string; destino: string }[];
    cliente: Cliente;
    prueba?: Uint8Array;
    version: string | null;
    admite: { reglas: boolean; cadenas: boolean; despues: boolean; soloCambios: boolean; ganchos: boolean; filtros: boolean };
    /** Cómo queda en la regla 3-2-1-1-0 con lo elegido (sin repositorio, null). */
    regla: ReglaCopia | null;
    /** Las sugerencias de un clic que se pueden hacer con este equipo. */
    puede: { verificacion: boolean; prueba: boolean; espejo: boolean; derivada: boolean };
    /** Lo que se mandará además de las copias («Traer las versiones de …»). */
    pendientes?: string[];
    problemas?: string[];
    cambiado: boolean;
    guardando: boolean;
    textoGuardando?: string;
    ahora: number;
    onElegirCarpetas: () => void;
    onNuevoRepo: (p: PedidoRepoNuevo) => void;
    onSugerencia: (s: Sugerencia) => void;
    onGuardar: () => void;
    onCerrar: () => void;
  } = $props();

  const k = $derived(copias[i]);
  const repoNombre = $derived(repos.find((r) => r.id === k.repo)?.nombre ?? "");
  const resumenDe: Record<PasoCopia, () => string> = {
    cuando: () => cuandoEnFrase(copias, i),
    que: () => queEnFrase(k),
    donde: () => repoNombre || "Sin repositorio",
    resumen: () => "",
  };
  const indice = (p: PasoCopia) => PASOS_COPIA.indexOf(p);
  /** Se puede abrir un paso si los de antes están hechos. */
  const abrible = (p: PasoCopia) => PASOS_COPIA.slice(0, indice(p)).every((x) => x === "resumen" || pasoHecho(k, x));
  function siguiente(e?: SubmitEvent) {
    e?.preventDefault();
    const sig = PASOS_COPIA[indice(paso) + 1];
    if (sig && abrible(sig)) paso = sig;
  }
  function tecla(e: KeyboardEvent) {
    if (e.key !== "Escape" || e.defaultPrevented) return;
    // Un desplegable o un globo abierto se cierran solos primero.
    if ((e.target as HTMLElement).closest("details[open] summary, [role='dialog']")) return;
    e.preventDefault();
    onCerrar();
  }

  // Qué: carpetas escritas a mano (además de «Elegir en el equipo»).
  let ruta = $state("");
  function anadirRuta(e: Event) {
    e.preventDefault();
    const r = ruta.trim();
    if (r && !k.carpetas.includes(r)) k.carpetas = [...k.carpetas, r];
    ruta = "";
  }
  const otras = $derived(k.exclusiones.filter((x) => !EXCLUSIONES_HABITUALES.some((h) => h.regla === x)));
  let masQue = $state(untrack(() => ganchosDe(copias[i]?.gancho).length > 0));
  const sug = $derived(regla ? sugerencias(regla.regla.partes.filter((p) => !p.cumple_config).map((p) => p.accion), puede) : []);
  const win = $derived(/windows/i.test(equipo.so));
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="guiada" onkeydown={tecla}>
  <ol class="pasos">
    <PasoGuiado n={1} titulo={TITULO_PASO.cuando} abierto={paso === "cuando"} hecho={pasoHecho(k, "cuando")} resumen={resumenDe.cuando()} onabrir={() => (paso = "cuando")}>
      {#snippet ayuda()}<Ayuda id="inicio-copia" />{/snippet}
      <form class="paso-form" onsubmit={siguiente}>
        <CuandoEmpieza {copias} {i} admite={{ reglas: admite.reglas, cadenas: admite.cadenas, despues: admite.despues }} {version} equipo={equipo.nombre} guiado />
        <div class="pie"><button class="btn btn-primary btn-sm">Siguiente<ArrowRight size={14} /></button></div>
      </form>
    </PasoGuiado>

    <PasoGuiado n={2} titulo={TITULO_PASO.que} abierto={paso === "que"} hecho={pasoHecho(k, "que")} resumen={resumenDe.que()} deshabilitado={!abrible("que")} onabrir={() => (paso = "que")}>
      <form class="paso-form" onsubmit={siguiente}>
        <div class="field">
          <label class="field-label" for="g-nombre-{k.id}">Nombre</label>
          <input id="g-nombre-{k.id}" class="input nombre" bind:value={k.nombre} maxlength="60" />
        </div>
        <div class="field">
          <span class="field-label" id="g-carp-{k.id}">Carpetas</span>
          {#if k.carpetas.length}
            <ul class="carpetas" aria-labelledby="g-carp-{k.id}">
              {#each k.carpetas as c (c)}
                <li><FolderOpen size={14} /><span class="mono">{c}</span><button type="button" class="icon-btn icon-btn-sm" aria-label="Quitar {c}" use:tip={"Quitar"} onclick={() => (k.carpetas = k.carpetas.filter((x) => x !== c))}><X size={14} /></button></li>
              {/each}
            </ul>
          {/if}
          <div class="anadir-carpeta">
            <button type="button" class="btn btn-sm" onclick={onElegirCarpetas}><FolderOpen size={14} />Elegir en el equipo</button>
            <input class="input mono" bind:value={ruta} placeholder={win ? "C:\\Users\\nombre\\Documents" : "/home/nombre"} aria-label="Escribir una carpeta" spellcheck="false" onkeydown={(e) => e.key === "Enter" && ruta.trim() && anadirRuta(e)} />
            <button type="button" class="btn btn-sm btn-ghost" disabled={!ruta.trim()} onclick={anadirRuta}><Plus size={14} />Añadir</button>
          </div>
        </div>
        <details class="mas" bind:open={masQue}>
          <summary>Más opciones</summary>
          <div class="field">
            <span class="field-label">No copiar <Ayuda id="exclusiones" /></span>
            <div class="chips-exc">
              {#each EXCLUSIONES_HABITUALES as h (h.regla)}
                <button type="button" class="chip-exc" class:on={k.exclusiones.includes(h.regla)} aria-pressed={k.exclusiones.includes(h.regla)} use:tip={h.regla} onclick={() => alternarExclusion(k, h.regla)}>{#if k.exclusiones.includes(h.regla)}<Check size={12} />{/if}{h.texto}</button>
              {/each}
            </div>
            <textarea class="input mono" rows="2" spellcheck="false" aria-label="Otras reglas, una por línea" placeholder="Otras reglas, una por línea" value={otras.join("\n")} oninput={(e) => (k.exclusiones = [...k.exclusiones.filter((x) => EXCLUSIONES_HABITUALES.some((h) => h.regla === x)), ...e.currentTarget.value.split(/\r?\n/).map((x) => x.trim()).filter(Boolean)])}></textarea>
          </div>
          <label class="switch-row"
            ><input type="checkbox" class="switch" checked={k.solo_si_cambios !== false} disabled={!admite.soloCambios} onchange={(e) => (k.solo_si_cambios = e.currentTarget.checked)} /><span
              ><span class="con-ayuda">Solo guardar si hay cambios <Ayuda id="solo-si-cambios" /></span>{#if !admite.soloCambios}<span class="faint">Actualiza el agente para poder apagarlo.</span>{/if}</span
            ></label
          >
          <EditorGanchos id="g-{k.id}" bind:ganchos={() => ganchosDe(k.gancho), (v) => (k.gancho = v)} admite={admite.ganchos} {version} {cliente} {equipo} {prueba} />
        </details>
        <div class="pie"><button class="btn btn-primary btn-sm" disabled={!k.carpetas.length} use:tip={k.carpetas.length ? "" : "Elige al menos una carpeta"}>Siguiente<ArrowRight size={14} /></button></div>
      </form>
    </PasoGuiado>

    <PasoGuiado n={3} titulo={TITULO_PASO.donde} abierto={paso === "donde"} hecho={pasoHecho(k, "donde")} resumen={resumenDe.donde()} deshabilitado={!abrible("donde")} onabrir={() => (paso = "donde")}>
      <form class="paso-form" onsubmit={siguiente}>
        <ElegirRepositorio id={k.id} {repos} {equipo} {equipos} {catalogo} {copias} copia={k.id} bind:value={k.repo} admiteFiltros={admite.filtros} onNuevo={onNuevoRepo} />
        <div class="pie"><button class="btn btn-primary btn-sm" disabled={!k.repo}>Siguiente<ArrowRight size={14} /></button></div>
      </form>
    </PasoGuiado>

    <PasoGuiado n={4} titulo={TITULO_PASO.resumen} abierto={paso === "resumen"} deshabilitado={!abrible("resumen")} onabrir={() => (paso = "resumen")}>
      <ResumenCopia {copias} {i} {repos} {equipo} {equipos} {catalogo} />
      {#if regla}
        <div class="regla" class:cumple={regla.regla.cumple_config}>
          <TiraRegla rc={regla} cliente={cliente.id} {ahora} compacta />
          <p>{fraseConfig(regla.regla)}</p>
        </div>
        {#if sug.length}
          <div class="sugerencias">
            {#each sug as s (s)}<button type="button" class="btn btn-sm" onclick={() => onSugerencia(s)}><Sparkles size={14} />{TEXTO_SUGERENCIA[s]}</button>{/each}
          </div>
        {/if}
      {/if}
      {#if pendientes.length}
        <ul class="pendientes">
          {#each pendientes as p (p)}<li><Check size={13} />{p}</li>{/each}
        </ul>
      {/if}
      {#if problemas.length}
        <div class="notice notice-warn"><TriangleAlert size={16} /><p>Antes de guardar: {lista(problemas)}</p></div>
      {/if}
      <div class="pie">
        <button type="button" class="btn btn-ghost btn-sm" onclick={onCerrar}>Listo</button>
        <button type="button" class="btn btn-primary btn-sm" disabled={!cambiado || guardando || problemas.length > 0} onclick={onGuardar}>
          {#if guardando}<LoaderCircle size={14} class="spin" />{textoGuardando || "Guardando…"}{:else}<Save size={14} />Guardar{/if}
        </button>
      </div>
    </PasoGuiado>
  </ol>
</div>

<style>
  .guiada {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .pasos {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
  }
  .paso-form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
  }
  .pie {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .nombre {
    max-width: 360px;
  }
  .carpetas {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0 0 var(--sp-2);
    padding: 0;
    list-style: none;
  }
  .carpetas li {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 36px;
    padding: 2px 4px 2px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-size: var(--fs-sm);
  }
  .carpetas li :global(svg:first-child) {
    flex: none;
    color: var(--text-3);
  }
  .carpetas .mono {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
    font-family: var(--mono);
    font-size: 12.5px;
  }
  .anadir-carpeta {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .anadir-carpeta .input {
    flex: 1 1 200px;
    min-width: 0;
    height: 28px;
    font-family: var(--mono);
    font-size: 12.5px;
  }
  .mas {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .mas > summary {
    font-size: var(--fs-sm);
    color: var(--text-2);
    cursor: pointer;
  }
  .mas[open] {
    display: flex;
  }
  .con-ayuda {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .chips-exc {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip-exc {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 28px;
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
  .chip-exc:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  textarea.mono {
    font-family: var(--mono);
    font-size: 12.5px;
  }
  .regla {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .regla p {
    margin: 0;
  }
  .regla.cumple {
    color: var(--text-1);
  }
  .sugerencias {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .pendientes {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .pendientes li {
    display: flex;
    align-items: center;
    gap: 6px;
  }
</style>
