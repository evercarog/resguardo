<script lang="ts">
  // «Aplicar una plantilla» a varios equipos a la vez (v1.52, tarea 6; p. ej. a
  // todos los de «Servidores»). Las plantillas y las copias van cifradas con la
  // clave de administración, así que todo pasa en este navegador:
  //   1. la clave (K_cfg se calcula una vez) y qué plantilla;
  //   2. el plan: a cada equipo se le añade una copia nueva con lo de la
  //      plantilla en su primer repositorio (se puede cambiar), o se salta
  //      diciendo por qué (lib/configEnvio.ts, planPlantilla);
  //   3. solo al confirmar, cada equipo recibe su orden `config` de siempre,
  //      con sus llaves comprobadas y su prueba de administración.
  // Nunca cambia ni quita las copias que ya tiene. La clave y K_cfg se borran al cerrar.
  import { Check, CircleAlert, LayoutTemplate, LoaderCircle, X } from "@lucide/svelte";
  import { onDestroy, untrack } from "svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { ApiError } from "$lib/api";
  import { borrar, deB64 } from "$lib/cripto/bytes";
  import { etiquetaValida } from "$lib/cripto/claves";
  import { descifrarConfig } from "$lib/cripto/simetrico";
  import { comprobarLlaves } from "$lib/fijadas";
  import { kcfgDelCliente, mandarOrden, pruebaParaEquipo } from "$lib/ordenar";
  import { cargarPlantillas, type Plantilla } from "$lib/plantillas";
  import { admiteDe, configParaEnviar, paraEditar, planPlantilla, type PlanPlantilla } from "$lib/configEnvio";
  import { plural } from "$lib/formato";
  import { cargarCliente } from "$lib/estado.svelte";
  import type { Cliente, Configuracion, Equipo } from "$lib/tipos";
  import BotonCargando from "./BotonCargando.svelte";
  import CampoClave from "./CampoClave.svelte";

  let { cliente, equipos, plantillaInicial = null, onclose }: { cliente: Cliente; equipos: Equipo[]; plantillaInicial?: string | null; onclose: () => void } = $props();

  type Paso = "clave" | "elegir" | "plan" | "envio";
  let paso = $state<Paso>("clave");
  let clave = $state("");
  let error = $state("");
  let ocupado = $state(false);
  let kcfg: Uint8Array | null = null;
  let plantillas = $state<Plantilla[]>([]);
  let elegida = $state<string>(untrack(() => plantillaInicial ?? ""));
  const plantilla = $derived(plantillas.find((p) => p.id === elegida));

  interface Fila {
    equipo: Equipo;
    cfg: Configuracion | null;
    plan: PlanPlantilla;
    estado?: "enviando" | "enviada" | "fallo";
    mensaje?: string;
  }
  let filas = $state<Fila[]>([]);
  const aEnviar = $derived(filas.filter((f) => f.plan.ok));

  onDestroy(() => {
    borrar(kcfg);
    kcfg = null;
    clave = "";
  });

  async function abrir(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    if (!clave) return void (error = "Escribe la clave de administración.");
    ocupado = true;
    try {
      borrar(kcfg);
      kcfg = await kcfgDelCliente(cliente, clave);
      if (!equipos.some((x) => etiquetaValida(kcfg!, x))) throw new Error("La clave de administración no es correcta. No se ha enviado nada.");
      const r = await cargarPlantillas(cliente.id, kcfg);
      plantillas = r.lista;
      if (!r.lista.length) throw new Error("No hay plantillas que se abran con esta clave: se guardan desde «Cambiar las copias» de un equipo («Guardar como plantilla…»).");
      if (!plantillas.some((p) => p.id === elegida)) elegida = plantillas[0].id;
      paso = "elegir";
    } catch (err) {
      borrar(kcfg);
      kcfg = null;
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }

  /** Lee y descifra la configuración de cada equipo y calcula qué le pasaría. No envía nada. */
  async function preparar(e: SubmitEvent) {
    e.preventDefault();
    if (!kcfg || !plantilla) return;
    error = "";
    ocupado = true;
    try {
      const out: Fila[] = [];
      for (const eq of equipos) {
        const no = (motivo: string): Fila => ({ equipo: eq, cfg: null, plan: { ok: false, motivo } });
        if (!eq.confirmado || eq.modo === "trasladado") {
          out.push(no("No recibe órdenes desde aquí."));
          continue;
        }
        if ((await comprobarLlaves(cliente.id, eq)) === "cambiada" || !etiquetaValida(kcfg, eq)) {
          out.push(no("Sus llaves no son las que se confirmaron al emparejarlo: revísalo en su ficha."));
          continue;
        }
        let cfg: Configuracion;
        try {
          const c = await api.configEquipo(cliente.id, eq.id);
          cfg = descifrarConfig<Configuracion>(kcfg, eq.id, c.seq, deB64(c.cifrado));
        } catch (err) {
          if (err instanceof ApiError && err.codigo === "no_existe")
            cfg = { v: 1, copias: [], repositorios: eq.resumen?.repositorios?.map((r) => ({ id: r.id, nombre: r.nombre, destino: r.destino })) ?? [], destinos: eq.resumen?.destinos ?? [] };
          else {
            out.push(no(err instanceof ApiError ? err.message : "No se pudo descifrar su configuración con esta clave."));
            continue;
          }
        }
        cfg = paraEditar(cfg);
        out.push({ equipo: eq, cfg, plan: planPlantilla(cfg, $state.snapshot(plantilla) as Plantilla, admiteDe(eq), `copia-${crypto.randomUUID().slice(0, 8)}`) });
      }
      filas = out;
      paso = "plan";
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }

  async function enviar() {
    if (!kcfg) return;
    paso = "envio";
    for (const f of filas) if (f.plan.ok) f.estado = "enviando";
    // Uno tras otro: cada uno calcula su prueba (Argon2) y no se satura el navegador.
    for (const f of filas) {
      if (!f.plan.ok || !f.cfg) continue;
      let prueba: Uint8Array | null = null;
      try {
        prueba = await pruebaParaEquipo(cliente, f.equipo, clave, kcfg);
        const nueva = { ...f.cfg, copias: [...f.cfg.copias, f.plan.copia] };
        const config = configParaEnviar(JSON.parse(JSON.stringify(nueva)) as Configuracion, admiteDe(f.equipo));
        const o = await mandarOrden({ cliente, equipo: f.equipo, tipo: "config", cuerpo: { config }, secretos: { prueba } });
        f.estado = "enviada";
        f.mensaje = `Enviada (orden n.º ${o.seq}). Verás su respuesta en su ficha.`;
      } catch (err) {
        f.estado = "fallo";
        f.mensaje = (err as Error).message;
      } finally {
        borrar(prueba);
      }
    }
    borrar(kcfg);
    kcfg = null;
    clave = "";
    void cargarCliente(cliente.id, { silencioso: true });
  }
  const terminado = $derived(paso === "envio" && filas.every((f) => f.estado !== "enviando"));
</script>

<Modal labelledby="t-aplicar-pl" {onclose} width={620} dismissible={paso !== "envio" || terminado}>
  <div class="dlg-title">
    <span class="ticon"><LayoutTemplate size={18} /></span>
    <div>
      <h2 id="t-aplicar-pl">Aplicar una plantilla a {plural(equipos.length, "equipo", "equipos")}</h2>
      <p>Cada equipo recibe una copia nueva con lo de la plantilla. Las copias que ya tiene no cambian. Antes de enviar verás qué le pasa a cada uno.</p>
    </div>
  </div>

  {#if paso === "clave"}
    <form class="form" onsubmit={abrir}>
      <CampoClave requerido id="pl-clave" etiqueta="Clave de administración" bind:value={clave} {error} ayuda="Las plantillas y las copias van cifradas: solo se ven con la clave del cliente." autofocus />
      <footer>
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
        <BotonCargando class="btn btn-primary" type="submit" cargando={ocupado} textoCargando="Comprobando…">Seguir</BotonCargando>
      </footer>
    </form>
  {:else if paso === "elegir"}
    <form class="form" onsubmit={preparar}>
      <div class="field">
        <label class="field-label" for="pl-elegida">Plantilla</label>
        <select id="pl-elegida" class="input" bind:value={elegida}>
          {#each plantillas as p (p.id)}<option value={p.id}>{p.nombre}{p.id === plantillaInicial ? " (la de la etiqueta)" : ""}</option>{/each}
        </select>
        {#if plantilla}<span class="field-hint">{plural(plantilla.copia.carpetas.length, "carpeta", "carpetas")}: {plantilla.copia.carpetas.join(", ")}. Tienen que existir en cada equipo.</span>{/if}
      </div>
      {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
      <footer>
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
        <BotonCargando class="btn btn-primary" type="submit" cargando={ocupado} textoCargando="Leyendo sus copias…">Ver qué pasará</BotonCargando>
      </footer>
    </form>
  {:else}
    {#if paso === "plan"}<p>{aEnviar.length ? `Se añadirá «${plantilla?.nombre}» a ${plural(aEnviar.length, "equipo", "equipos")}.` : "A ninguno de los equipos elegidos se le puede añadir."}</p>{/if}
    <ul class="lista-b" aria-live="polite">
      {#each filas as f (f.equipo.id)}
        <li class:apagado={!f.plan.ok}>
          <span class="ic">
            {#if f.estado === "enviando"}<LoaderCircle size={15} class="spin" />{:else if f.estado === "enviada"}<Check size={15} />{:else if f.estado === "fallo"}<CircleAlert size={15} />{/if}
          </span>
          <span class="nombre">{f.equipo.nombre}</span>
          {#if f.mensaje}
            <span class="que" class:mal={f.estado === "fallo"}>{f.mensaje}</span>
          {:else if f.plan.ok}
            {@const pl = f.plan}
            <span class="que">
              en
              {#if pl.repos.length > 1 && paso === "plan"}
                <select class="input mini" aria-label="Repositorio de la copia en {f.equipo.nombre}" bind:value={pl.copia.repo}>
                  {#each pl.repos as r (r.id)}<option value={r.id}>{r.nombre}</option>{/each}
                </select>
              {:else}«{pl.repos.find((r) => r.id === pl.copia.repo)?.nombre}»{/if}
            </span>
          {:else}
            <span class="que faint">{f.plan.motivo}</span>
          {/if}
        </li>
      {/each}
    </ul>
    <footer>
      {#if paso === "plan"}
        <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
        <button type="button" class="btn btn-primary" disabled={!aEnviar.length} onclick={enviar}>Enviar a {plural(aEnviar.length, "equipo", "equipos")}</button>
      {:else}
        {#if !terminado}<span class="espera"><LoaderCircle size={15} class="spin" />Enviando…</span>{/if}
        <button type="button" class="btn btn-primary" disabled={!terminado} onclick={onclose}><X size={15} />Cerrar</button>
      {/if}
    </footer>
  {/if}
</Modal>

<style>
  .lista-b {
    display: flex;
    flex-direction: column;
    max-height: 50vh;
    overflow: auto;
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .lista-b li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
    padding: 9px var(--sp-3);
    border-top: 1px solid var(--border);
    font-size: var(--fs-sm);
  }
  .lista-b li:first-child {
    border-top: none;
  }
  .apagado {
    opacity: 0.7;
  }
  .ic {
    display: grid;
    place-items: center;
    width: 18px;
    color: var(--text-3);
  }
  .nombre {
    font-weight: 500;
  }
  .que {
    display: inline-flex;
    flex: 1;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    min-width: 0;
    color: var(--text-2);
    overflow-wrap: anywhere;
  }
  .que.mal {
    color: var(--bad);
  }
  .mini {
    width: auto;
    max-width: 100%;
    height: 30px;
    padding: 0 8px;
  }
  .espera {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
</style>
