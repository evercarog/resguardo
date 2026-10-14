<script lang="ts">
  // Poner o quitar etiquetas a un equipo (técnicos o más; v1.18). Propone las
  // que ya usa el cliente para que no salgan «Contabilidad» y «contabilidad».
  //
  // 14bb: el color de cada etiqueta se elige aquí mismo (administradores o
  // más, como en «Ajustar»), con la misma paleta y su nombre escrito. Se guarda
  // igual que desde «Ajustar»: en el servidor, por cliente, conservando la
  // plantilla y los avisos que tuviera.
  import { Palette, Tag } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { actual, cargarCliente, puede } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { etiquetasDe, limpiarEtiqueta, MAX_ETIQUETAS, MAX_LARGO } from "$lib/etiquetas.svelte";
  import { ajusteDe, colorPorNombre, cuerpoAjuste, N_COLORES, NOMBRES_COLOR } from "$lib/etiquetasGrupos";
  import { sincronizar } from "$lib/datosComunes.svelte";
  import type { Equipo } from "$lib/tipos";
  import EtiquetaChip from "./EtiquetaChip.svelte";
  import BotonCargando from "./BotonCargando.svelte";
  import { admiteDatosEquipo, alcanceDatos, cambiadoDesde, variasConsolas } from "$lib/datosEquipo";
  import { pedirAlEquipo } from "$lib/pedirAlEquipo";
  import { categoriaDe, conUnaPorCategoria } from "$lib/sedes";

  let { equipo, onclose, alGuardar }: { equipo: Equipo; onclose: () => void; alGuardar?: (e: Equipo) => void } = $props();

  // Copia para editar (lo del equipo no cambia hasta guardar).
  const inicio = () => [...(equipo.etiquetas ?? [])];
  let lista = $state<string[]>(inicio());
  let texto = $state("");
  let error = $state("");
  /** Plan 0.8.0, D2: lo que se quitó para dejar un solo valor por categoría. */
  let nota = $state("");
  let guardando = $state(false);
  const existentes = $derived(etiquetasDe(actual.equipos).filter((t) => !lista.some((x) => x.toLowerCase() === t.nombre.toLowerCase())));
  const sugeridas = $derived(texto.trim() ? existentes.filter((t) => t.nombre.toLowerCase().includes(texto.trim().toLowerCase())) : existentes);

  function anadir(t: string) {
    error = "";
    const x = limpiarEtiqueta(t);
    if (!x) {
      if (t.trim()) error = `Hasta ${MAX_LARGO} caracteres, sin comas.`;
      return;
    }
    // Si ya existe en el cliente con otras mayúsculas, se usa esa.
    const igual = etiquetasDe(actual.equipos).find((e) => e.nombre.toLowerCase() === x.toLowerCase());
    const nombre = igual?.nombre ?? x;
    if (lista.some((e) => e.toLowerCase() === nombre.toLowerCase())) return void (texto = "");
    // Plan 0.8.0, D2: un solo valor por categoría («Tipo»: Servidor o Puesto).
    const c = conUnaPorCategoria(actual.organizacion, lista, nombre);
    if (c.etiquetas.length > MAX_ETIQUETAS) return void (error = `Como mucho ${MAX_ETIQUETAS} etiquetas por equipo.`);
    lista = c.etiquetas;
    nota = c.quitadas.length ? `Quitada ${c.quitadas.map((x) => `«${x}»`).join(", ")}: «${categoriaDe(actual.organizacion, nombre)?.nombre}» tiene un solo valor por equipo.` : "";
    texto = "";
    // La recién añadida queda elegida para ponerle color.
    if (puedeColor) deColor = nombre;
  }

  // --- Color (14bb) ------------------------------------------------------------
  const puedeColor = $derived(puede.administrar(actual.cliente?.rol));
  /** El color guardado de una etiqueta (-1: el automático, por el nombre). */
  const guardado = (t: string) => ajusteDe(t, actual.etiquetas)?.color ?? -1;
  /** Los colores cambiados aquí (sin guardar), por etiqueta. */
  let colores = $state<Record<string, number>>({});
  let deColor = $state<string | null>(null);
  const elegida = $derived(deColor && lista.includes(deColor) ? deColor : (lista[lista.length - 1] ?? null));
  const colorDe = (t: string) => colores[t] ?? guardado(t);
  /** El índice que se ve (el automático si no hay uno elegido). */
  const indiceDe = (t: string) => (colorDe(t) >= 0 ? colorDe(t) : colorPorNombre(t));
  const cambiosColor = $derived(Object.entries(colores).filter(([t, c]) => lista.includes(t) && c !== guardado(t)));
  async function guardarColores() {
    for (const [t, c] of cambiosColor) {
      const previo = ajusteDe(t, actual.etiquetas);
      actual.etiquetas = await api.ponerAjusteEtiqueta(actual.id, cuerpoAjuste(t, c >= 0 ? c : null, previo?.plantilla ?? null, previo?.avisos));
    }
    // Como desde «Ajustar»: el color, a las demás consolas (por los equipos).
    if (cambiosColor.length) void sincronizar();
  }
  function teclas(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      anadir(texto);
    } else if (e.key === "Backspace" && !texto && lista.length) lista = lista.slice(0, -1);
  }
  async function guardar(e: SubmitEvent) {
    e.preventDefault();
    if (texto.trim()) anadir(texto);
    if (error) return;
    guardando = true;
    try {
      if (!cambianEtiquetas) {
        // Solo el color (14bb).
        await guardarColores();
        avisar(cambiosColor.length === 1 ? "Color guardado." : "Colores guardados.");
        onclose();
        return;
      }
      let nuevo: Equipo;
      if (admiteDatosEquipo(equipo) && actual.cliente) {
        // v1.56: las guarda el equipo y las ven igual todas sus consolas.
        const r = await pedirAlEquipo(actual.cliente, equipo, "etiquetas_equipo", { etiquetas: lista });
        avisar(r.hecha ? (lista.length ? "Etiquetas guardadas en el equipo." : "Etiquetas quitadas en el equipo.") + (variasConsolas(equipo) ? " Las verán igual todas sus consolas." : "") : r.texto);
        nuevo = r.hecha ? { ...equipo, etiquetas: lista } : equipo;
      } else {
        nuevo = await api.ponerEtiquetas(actual.id, equipo.id, lista);
        avisar(lista.length ? "Etiquetas guardadas." : "Etiquetas quitadas.");
      }
      if (cambiosColor.length) {
        try {
          await guardarColores();
        } catch (err) {
          avisar(`Las etiquetas se guardaron, pero el color no: ${(err as Error).message}`, "warn");
        }
      }
      alGuardar?.(nuevo);
      void cargarCliente(actual.id, { silencioso: true });
      onclose();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      guardando = false;
    }
  }
  const cambianEtiquetas = $derived(JSON.stringify(lista) !== JSON.stringify(equipo.etiquetas ?? []) || !!texto.trim());
  const cambiado = $derived(cambianEtiquetas || cambiosColor.length > 0);
</script>

<Modal labelledby="t-etiquetas" {onclose} width={480}>
  <form class="form" onsubmit={guardar}>
    <div class="dlg-title">
      <span class="ticon"><Tag size={18} /></span>
      <div>
        <h2 id="t-etiquetas">Etiquetas de {equipo.nombre}</h2>
        <p>Para agrupar y filtrar equipos: «Contabilidad», «Servidores», «Sede norte»…</p>
      </div>
    </div>
    <div class="field">
      <label class="field-label" for="et-nueva">Etiquetas</label>
      <div class="caja">
        {#each lista as t (t)}<EtiquetaChip nombre={t} indice={indiceDe(t)} onquitar={() => (lista = lista.filter((x) => x !== t))} />{/each}
        <!-- svelte-ignore a11y_autofocus -->
        <input id="et-nueva" class="entrada" bind:value={texto} onkeydown={teclas} placeholder={lista.length ? "Añadir otra…" : "Escribe y pulsa Intro"} maxlength={MAX_LARGO} autocomplete="off" autofocus aria-invalid={!!error} aria-describedby="et-nueva-ayuda" />
      </div>
      {#if error}<p class="error-campo" id="et-nueva-ayuda" role="alert">{error}</p>{:else if nota}<span class="field-hint" id="et-nueva-ayuda" role="status">{nota}</span>{:else}<span class="field-hint" id="et-nueva-ayuda">Intro o coma para añadir. Hasta {MAX_ETIQUETAS} por equipo.{#if actual.organizacion.categorias.length}{" "}Las de una categoría ({actual.organizacion.categorias.map((c) => c.nombre).join(", ")}), una sola por equipo.{/if}</span>{/if}
    </div>
    {#if puedeColor && elegida}
      {@const actualColor = colorDe(elegida)}
      <fieldset class="grupo-color">
        <legend><Palette size={14} />Color</legend>
        {#if lista.length > 1}
          <div class="cual" role="group" aria-label="Etiqueta a la que se pone color">
            {#each lista as t (t)}<EtiquetaChip nombre={t} indice={indiceDe(t)} activa={t === elegida} onclick={() => (deColor = t)} />{/each}
          </div>
        {/if}
        <div class="colores">
          <label class="op" style:--et="var(--et-{colorPorNombre(elegida)})">
            <input type="radio" name="color-et-equipo" checked={actualColor < 0} onchange={() => (colores = { ...colores, [elegida]: -1 })} />
            <span class="punto" aria-hidden="true"></span>Automático<span class="faint">({NOMBRES_COLOR[colorPorNombre(elegida)].toLowerCase()})</span>
          </label>
          {#each Array.from({ length: N_COLORES }, (_, i) => i) as i (i)}
            <label class="op" style:--et="var(--et-{i})">
              <input type="radio" name="color-et-equipo" checked={actualColor === i} onchange={() => (colores = { ...colores, [elegida]: i })} />
              <span class="punto" aria-hidden="true"></span>{NOMBRES_COLOR[i]}
            </label>
          {/each}
        </div>
        <span class="field-hint">De «{elegida}», igual en todos sus equipos. El nombre siempre va escrito: el color solo ayuda a reconocerla.</span>
      </fieldset>
    {/if}
    {#if sugeridas.length}
      <div class="sugeridas">
        <span class="faint">Ya en uso:</span>
        {#each sugeridas.slice(0, 12) as t (t.nombre)}<EtiquetaChip nombre={t.nombre} n={t.n} onclick={() => anadir(t.nombre)} />{/each}
      </div>
    {/if}
    <p class="faint nota">Se guardan sin cifrar, como el nombre del equipo: úsalas para organizar, no para datos privados.{#if alcanceDatos(equipo)}{" "}{alcanceDatos(equipo)}{/if}{#if cambiadoDesde(equipo.resumen?.datos_equipo?.etiquetas)}{" "}{cambiadoDesde(equipo.resumen?.datos_equipo?.etiquetas)}.{/if} El color, la plantilla y los avisos de cada etiqueta son de esta consola.</p>
    <footer>
      <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
      <BotonCargando class="btn btn-primary" disabled={!cambiado} cargando={guardando} textoCargando="Guardando…">Guardar</BotonCargando>
    </footer>
  </form>
</Modal>

<style>
  .caja {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    min-height: 40px;
    padding: 6px 8px;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
  }
  .caja:focus-within {
    border-color: var(--accent);
    box-shadow: var(--focus);
  }
  .entrada {
    flex: 1;
    min-width: 120px;
    height: 26px;
    padding: 0 4px;
    font: inherit;
    color: var(--text-1);
    background: none;
    border: none;
    outline: none;
  }
  .sugeridas {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .nota {
    margin: 0;
    font-size: var(--fs-xs);
  }
  /* El color de la etiqueta elegida (14bb): la misma paleta que «Ajustar». */
  .grupo-color {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    padding: 0;
    border: none;
  }
  .grupo-color legend {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 6px;
    font-weight: 600;
    font-size: var(--fs-sm);
  }
  .cual,
  .colores {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .op {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 30px;
    padding: 0 10px;
    font-size: var(--fs-sm);
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
    cursor: pointer;
  }
  .op input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .op:has(input:checked) {
    color: var(--text-1);
    background: color-mix(in srgb, var(--et) 14%, var(--surface));
    border-color: color-mix(in srgb, var(--et) 70%, var(--border));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--et) 70%, var(--border));
  }
  .op:has(input:focus-visible) {
    box-shadow: var(--focus);
  }
  .punto {
    flex: none;
    width: 10px;
    height: 10px;
    background: var(--et);
    border-radius: 999px;
  }
</style>
