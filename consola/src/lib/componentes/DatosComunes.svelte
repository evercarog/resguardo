<script lang="ts">
  // 0.7.26 (bloque 8): «Lo mismo en todas las consolas» (docs/consolas-multiples.md §6.5).
  // Reparte solo lo que no pide clave (colores, nombres, plantillas cifradas) y deja
  // un aviso pequeño mientras haya algo que necesite a una persona: datos distintos
  // entre consolas (se elige cuál vale para todas; nada se pisa sin preguntar), el
  // tipo y las marcas de destinos por repartir (con la clave) y plantillas de otra
  // consola por traer (se abren aquí con la clave). Solo administradores.
  import { untrack } from "svelte";
  import { ChevronRight, KeyRound, LayoutTemplate, Shuffle, TriangleAlert } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import BotonCargando from "./BotonCargando.svelte";
  import CampoClave from "./CampoClave.svelte";
  import { actual, puede } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { seguirCambios } from "$lib/vivo.svelte";
  import { cargarCatalogo, catalogoDe } from "$lib/catalogoDestinos.svelte";
  import { destinosDelCliente } from "$lib/destinos";
  import { comunes, elegir, repartirConClave, sincronizar, traerPlantillas } from "$lib/datosComunes.svelte";
  import { claseDe, diferencia, equiposQueGuardan, pendiente, textoAviso, type Diferencia } from "$lib/datosComunes";

  const admin = $derived(puede.administrar(actual.cliente?.rol));

  // Al abrir el cliente y cuando algo cambia (en esta consola o en otra, por los equipos).
  $effect(() => {
    const c = actual.id;
    if (!c || !admin) return;
    return untrack(() => {
      void sincronizar();
      return seguirCambios(
        () => {
          if (actual.id !== c) return;
          void cargarCatalogo(c, true);
          void sincronizar();
        },
        { ms: 120_000, toca: (x) => x.t === "datos_comunes" },
      );
    });
  });

  const p = $derived(comunes.cliente === actual.id ? pendiente(comunes.datos) : pendiente(null));
  const hayEquipos = $derived(equiposQueGuardan(actual.equipos).length > 0);
  const texto = $derived(admin && hayEquipos ? textoAviso(p) : null);

  /** El nombre con que se ve aquí cada destino (para no enseñar su id). */
  const nombreDestino = $derived.by(() => {
    const vistos = destinosDelCliente(actual.equipos, catalogoDe(actual.id));
    return (id: string) => vistos.find((d) => d.clave === id)?.nombre ?? null;
  });
  const diferencias = $derived(p.diferencias.map((f) => ({ fila: f, d: diferencia(f, { destino: nombreDestino }) })));
  const conClave = $derived(p.porEnviarConClave.length + p.sinCompartirConClave.length);
  const pideClave = $derived(conClave > 0 || p.porTraer.length > 0 || diferencias.some((x) => x.d.conClave));

  let abierto = $state(false);
  let clave = $state("");
  let errorClave = $state("");
  let paso = $state("");
  let ocupado = $state<string | null>(null);

  function cerrar() {
    abierto = false;
    clave = "";
    errorClave = "";
    paso = "";
  }

  async function hacer(id: string, f: () => Promise<string>, necesitaClave: boolean) {
    if (!actual.cliente || ocupado) return;
    if (necesitaClave && !clave) return void (errorClave = "Escribe la clave de administración.");
    ocupado = id;
    errorClave = "";
    try {
      avisar(await f());
    } catch (err) {
      errorClave = (err as Error).message;
    } finally {
      ocupado = null;
      paso = "";
    }
  }

  const elegirLado = (d: Diferencia, lado: "aqui" | "otra") =>
    hacer(`${d.clave}|${lado}`, () => elegir(actual.cliente!, actual.equipos, d, lado, clave, (t) => (paso = t)), d.conClave);
  const repartir = () => hacer("repartir", () => repartirConClave(actual.cliente!, actual.equipos, clave, (t) => (paso = t)), true);
  const traer = () => hacer("traer", () => traerPlantillas(actual.cliente!, clave, (t) => (paso = t)), true);

  /** El color de una etiqueta (para la muestra), si el dato es un color. */
  function colorDe(clave: string, valor: unknown): number | null {
    if (claseDe(clave)?.clase !== "etiqueta.color") return null;
    const c = (valor as { color?: unknown } | null)?.color;
    return typeof c === "number" ? c : null;
  }
</script>

{#snippet punto(clave: string, valor: unknown)}
  {#if claseDe(clave)?.clase === "etiqueta.color"}
    {@const c = colorDe(clave, valor)}
    {#if c != null}<span class="punto" style:--et="var(--et-{c})" aria-hidden="true"></span>{:else}<span class="punto vacio" aria-hidden="true"></span>{/if}
  {/if}
{/snippet}

{#if texto}
  <div class="aviso-comunes notice notice-warn" role="status">
    <Shuffle size={16} aria-hidden="true" />
    <p>
      <strong>Lo mismo en todas las consolas:</strong>
      {texto}.
      <button type="button" class="notice-action" onclick={() => (abierto = true)}>Revisar<ChevronRight size={14} /></button>
    </p>
  </div>
{/if}

{#if abierto}
  <Modal labelledby="t-comunes" onclose={cerrar} width={600}>
    <div class="dlg-title">
      <span class="ticon warn"><Shuffle size={18} /></span>
      <div>
        <h2 id="t-comunes">Lo mismo en todas las consolas</h2>
        <p>Los colores de las etiquetas, los nombres, el tipo y las marcas de los destinos y las plantillas se ven igual en todas las consolas del cliente. Lo que aquí era distinto espera a que elijas.</p>
      </div>
    </div>

    {#if diferencias.length}
      <section class="bloque" aria-labelledby="t-dif">
        <h3 id="t-dif"><TriangleAlert size={15} />Distinto en otra consola</h3>
        <ul class="difs">
          {#each diferencias as { fila, d } (d.clave)}
            <li class="dif">
              <div class="dif-texto">
                <strong>{d.titulo}</strong>
                <span class="frase" title={d.frase}>
                  <span class="lado">
                    {@render punto(d.clave, d.valorAqui)}En esta consola: <b>{d.aqui}</b>
                  </span>
                  <span class="lado">
                    {@render punto(d.clave, d.valorOtra)}{fila.consola?.trim() ? `En «${fila.consola.trim()}»` : "En la otra"}: <b>{d.otra}</b>
                  </span>
                </span>
                {#if fila.por?.trim() || d.conClave}<span class="faint pequeno">{[fila.por?.trim() ? `Lo cambió ${fila.por.trim()}` : "", d.conClave ? "elegir pide la clave" : ""].filter(Boolean).join(" · ")}</span>{/if}
              </div>
              <div class="dif-botones" role="group" aria-label="Cuál vale para todas">
                <BotonCargando type="button" class="btn btn-sm" cargando={ocupado === `${d.clave}|aqui`} textoCargando="Eligiendo…" disabled={!!ocupado} onclick={() => elegirLado(d, "aqui")}>Usar el de esta</BotonCargando>
                <BotonCargando type="button" class="btn btn-sm" cargando={ocupado === `${d.clave}|otra`} textoCargando="Eligiendo…" disabled={!!ocupado} onclick={() => elegirLado(d, "otra")}>Usar el de la otra</BotonCargando>
              </div>
            </li>
          {/each}
        </ul>
        <p class="faint pequeno">Lo que elijas se guarda aquí y se manda a los equipos del cliente: las demás consolas lo verán igual.</p>
      </section>
    {/if}

    {#if conClave}
      <section class="bloque" aria-labelledby="t-regla">
        <h3 id="t-regla"><KeyRound size={15} />Tipo y marcas de destinos por repartir</h3>
        <p class="faint pequeno">{conClave === 1 ? "1 destino tiene" : `${conClave} destinos tienen`} el tipo o las marcas puestos aquí. Cuentan en la regla 3-2-1: para que las demás consolas los vean igual, se reparten con la clave de administración.</p>
        <div><BotonCargando type="button" class="btn btn-sm" cargando={ocupado === "repartir"} textoCargando="Repartiendo…" disabled={!!ocupado} onclick={repartir}>Repartir con la clave</BotonCargando></div>
      </section>
    {/if}

    {#if p.porTraer.length}
      <section class="bloque" aria-labelledby="t-traer">
        <h3 id="t-traer"><LayoutTemplate size={15} />Plantillas de otra consola</h3>
        <p class="faint pequeno">{p.porTraer.length === 1 ? "Llegó 1 plantilla cifrada" : `Llegaron ${p.porTraer.length} plantillas cifradas`} desde otra consola. Se abren aquí, en tu navegador, con la clave de administración, y se guardan cifradas para esta consola.</p>
        <div><BotonCargando type="button" class="btn btn-sm" cargando={ocupado === "traer"} textoCargando="Trayendo…" disabled={!!ocupado} onclick={traer}>Traer las plantillas</BotonCargando></div>
      </section>
    {/if}

    {#if pideClave}
      <CampoClave id="comunes-clave" etiqueta="Clave de administración" bind:value={clave} error={errorClave} ayuda="Para el tipo y las marcas de los destinos y para abrir las plantillas. Se comprueba con cada equipo antes de mandarle nada." />
    {:else if errorClave}
      <p class="error-campo" role="alert">{errorClave}</p>
    {/if}
    {#if paso}<p class="faint pequeno" aria-live="polite">{paso}</p>{/if}
    {#if !diferencias.length && !conClave && !p.porTraer.length}<p class="faint">Todo está igual en todas las consolas.</p>{/if}

    <footer>
      <button type="button" class="btn btn-ghost" onclick={cerrar}>Cerrar</button>
    </footer>
  </Modal>
{/if}

<style>
  .aviso-comunes {
    max-width: var(--content-max);
    margin: 0 auto var(--sp-4);
    align-items: center;
  }
  .aviso-comunes p {
    min-width: 0;
  }
  .bloque {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: var(--sp-4);
  }
  .bloque h3 {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-md, 15px);
    font-weight: 600;
  }
  .difs {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .dif {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px 16px;
    padding: 10px 12px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .dif-texto {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1 1 260px;
  }
  .frase {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .frase b {
    color: var(--text-1);
    font-weight: 600;
  }
  .lado {
    min-width: 0;
  }
  .punto {
    display: inline-block;
    width: 10px;
    height: 10px;
    margin-right: 6px;
    vertical-align: -1px;
    border-radius: 50%;
    background: var(--et);
  }
  .punto.vacio {
    background: none;
    border: 1.5px solid var(--text-3);
  }
  .dif-botones {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .pequeno {
    margin: 0;
    font-size: var(--fs-sm);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: var(--sp-4);
  }
  @media (max-width: 640px) {
    .dif-botones {
      width: 100%;
    }
    .dif-botones :global(.btn) {
      flex: 1 1 auto;
    }
  }
</style>
