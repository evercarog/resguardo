<script lang="ts">
  // 0.8.3 (16b): «Ver los archivos» que no se pudieron leer en la última copia. Se piden al
  // equipo por una sesión cifrada (la de «Elegir en el equipo», con la clave de
  // administración): las rutas viajan cifradas del equipo a este navegador. Desde aquí,
  // «No copiar esto»: se añade la ruta exacta a «No copiar» de esa copia (orden `config`
  // con la clave, como cualquier cambio de las copias).
  import { onDestroy } from "svelte";
  import { CircleAlert, FileX, Folder, LoaderCircle } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { borrar, deB64 } from "$lib/cripto/bytes";
  import { etiquetaValida } from "$lib/cripto/claves";
  import { descifrarConfig } from "$lib/cripto/simetrico";
  import { comprobarLlaves } from "$lib/fijadas";
  import { kcfgDelCliente, mandarOrden, pruebaParaEquipo } from "$lib/ordenar";
  import { admiteDe, configParaEnviar, paraEditar } from "$lib/configEnvio";
  import { Sesion } from "$lib/sesion";
  import { plural } from "$lib/formato";
  import { agruparArchivos, conExclusiones, leerArchivos, nombreDe, resumenRepeticion, type ArchivoConError } from "$lib/archivosConError";
  import type { Cliente, Configuracion, Equipo } from "$lib/tipos";
  import CampoClave from "./CampoClave.svelte";

  let { cliente, equipo, repo, copia, nombreCopia, onclose }: { cliente: Cliente; equipo: Equipo; repo: string; copia: string; nombreCopia: string; onclose: () => void } = $props();

  let clave = $state("");
  let fase = $state<"clave" | "cargando" | "lista" | "enviando" | "hecho">("clave");
  let paso = $state("");
  let error = $state("");
  let archivos = $state<ArchivoConError[]>([]);
  let copias = $state(1);
  let marcados = $state<string[]>([]);
  let mensaje = $state("");
  let sesion: Sesion | null = null;
  onDestroy(() => {
    sesion?.cerrar();
    clave = "";
  });

  const grupos = $derived(agruparArchivos(archivos));
  const repeticion = $derived(resumenRepeticion(archivos, copias));
  const win = $derived(archivos.some((a) => /^[A-Za-z]:[\\/]/.test(a.ruta)));
  const alternar = (r: string) => (marcados = marcados.includes(r) ? marcados.filter((x) => x !== r) : [...marcados, r]);

  async function ver(e: SubmitEvent) {
    e.preventDefault();
    if (!clave) return void (error = "Escribe la clave de administración.");
    error = "";
    fase = "cargando";
    try {
      sesion = await Sesion.abrir({ cliente, equipo, tipo: "elegir_carpetas", secretos: { claveAdmin: clave }, alPaso: (t) => (paso = t) });
      await sesion.lista;
      if (sesion.ops && !sesion.ops.includes("archivos_con_error")) throw new Error("Este equipo aún no sabe decirlo: actualiza su agente.");
      paso = "Pidiendo la lista al equipo…";
      const r = leerArchivos(await sesion.pedir("archivos_con_error", { repo, copia, copias: 5 }, 60_000));
      archivos = r.archivos;
      copias = r.copias;
      fase = "lista";
    } catch (err) {
      error = (err as Error).message;
      fase = "clave";
    } finally {
      sesion?.cerrar();
      sesion = null;
    }
  }

  /** Añade las rutas marcadas a «No copiar» de esta copia (una orden `config` con la clave). */
  async function noCopiar() {
    if (!marcados.length) return;
    error = "";
    fase = "enviando";
    let kcfg: Uint8Array | null = null;
    let prueba: Uint8Array | null = null;
    try {
      kcfg = await kcfgDelCliente(cliente, clave);
      if (!etiquetaValida(kcfg, equipo)) throw new Error("La clave de administración no es correcta. No se ha enviado nada.");
      if ((await comprobarLlaves(cliente.id, equipo)) === "cambiada") throw new Error("Sus llaves no son las que se confirmaron al emparejarlo: revísalo en su ficha.");
      const c = await api.configEquipo(cliente.id, equipo.id);
      const cfg = paraEditar(descifrarConfig<Configuracion>(kcfg, equipo.id, c.seq, deB64(c.cifrado)));
      const k = cfg.copias.find((x) => x.id === copia);
      if (!k) throw new Error("Esa copia ya no está en el equipo.");
      const antes = k.exclusiones.length;
      k.exclusiones = conExclusiones(k.exclusiones, marcados, win);
      if (k.exclusiones.length === antes) {
        mensaje = "Ya estaban en «No copiar».";
        fase = "hecho";
        return;
      }
      prueba = await pruebaParaEquipo(cliente, equipo, clave, kcfg);
      const o = await mandarOrden({ cliente, equipo, tipo: "config", cuerpo: { config: configParaEnviar(cfg, admiteDe(equipo)) }, secretos: { prueba } });
      mensaje = `Enviado a ${equipo.nombre} (orden n.º ${o.seq}): ${plural(k.exclusiones.length - antes, "ruta nueva", "rutas nuevas")} en «No copiar» de «${nombreCopia}». Valdrá desde la próxima copia.`;
      fase = "hecho";
    } catch (err) {
      error = (err as Error).message;
      fase = "lista";
    } finally {
      borrar(kcfg);
      borrar(prueba);
    }
  }
</script>

<Modal labelledby="t-archivos-error" {onclose} width={720} ocupado={fase === "cargando" || fase === "enviando"}>
  <div class="dlg-title">
    <span class="ticon"><FileX size={18} /></span>
    <div>
      <h2 id="t-archivos-error">Archivos que no se pudieron leer · «{nombreCopia}»</h2>
      <p>De la última copia de {equipo.nombre}. Las rutas viajan cifradas del equipo a este navegador: el servidor no las ve.</p>
    </div>
  </div>

  {#if fase === "clave" || fase === "cargando"}
    <form class="form" onsubmit={ver}>
      <CampoClave requerido id="ae-clave" etiqueta="Clave de administración" bind:value={clave} autocomplete="off" />
      {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
      <footer>
        <button type="button" class="btn btn-ghost" disabled={fase === "cargando"} onclick={onclose}>Cancelar</button>
        <button class="btn btn-primary" disabled={fase === "cargando" || !clave}>
          {#if fase === "cargando"}<LoaderCircle size={14} class="spin" />{paso || "Abriendo la sesión…"}{:else}Ver los archivos{/if}
        </button>
      </footer>
    </form>
  {:else if fase === "hecho"}
    <p>{mensaje}</p>
    <footer><button class="btn btn-primary" onclick={onclose}>Hecho</button></footer>
  {:else}
    {#if !archivos.length}
      <p>En la última copia se pudieron leer todos los archivos (o el equipo ya no guarda la lista).</p>
    {:else}
      <p><strong>{plural(archivos.length, "archivo", "archivos")}</strong>{#if repeticion}. {repeticion}{/if}</p>
      <div class="grupos">
        {#each grupos as g (g.clase)}
          <section class="grupo" aria-label={g.titulo}>
            <h3>{g.titulo} <span class="faint">· {g.total}</span></h3>
            <p class="faint explica">{g.explica}</p>
            {#each g.carpetas as cp (cp.carpeta)}
              <div class="carpeta">
                <label class="fila-c">
                  <input type="checkbox" checked={marcados.includes(cp.carpeta)} onchange={() => alternar(cp.carpeta)} />
                  <Folder size={14} aria-hidden="true" /><span class="mono ruta">{cp.carpeta}</span>
                  <span class="faint">(toda la carpeta)</span>
                </label>
                <ul>
                  {#each cp.archivos as a (a.ruta)}
                    <li>
                      <label>
                        <input type="checkbox" checked={marcados.includes(a.ruta)} disabled={marcados.includes(cp.carpeta)} onchange={() => alternar(a.ruta)} />
                        <span class="mono">{nombreDe(a.ruta)}</span>
                        {#if copias > 1 && a.veces >= copias}<span class="badge tone-warn">siempre</span>{/if}
                      </label>
                      {#if g.clase === "otro" && a.motivo}<span class="faint motivo">{a.motivo}</span>{/if}
                    </li>
                  {/each}
                </ul>
              </div>
            {/each}
          </section>
        {/each}
      </div>
    {/if}
    {#if error}<p class="error-campo" role="alert"><CircleAlert size={14} />{error}</p>{/if}
    <footer>
      <button type="button" class="btn btn-ghost" disabled={fase === "enviando"} onclick={onclose}>Cerrar</button>
      {#if archivos.length}
        <button class="btn btn-primary" disabled={fase === "enviando" || !marcados.length} onclick={noCopiar}>
          {#if fase === "enviando"}<LoaderCircle size={14} class="spin" />Enviando…{:else}No copiar lo marcado{marcados.length ? ` (${marcados.length})` : ""}{/if}
        </button>
      {/if}
    </footer>
  {/if}
</Modal>

<style>
  .grupos {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    max-height: 55vh;
    overflow: auto;
    margin: var(--sp-2) 0;
  }
  .grupo h3 {
    margin: 0;
    font-size: var(--fs-body);
  }
  .explica {
    margin: 2px 0 var(--sp-2);
    font-size: var(--fs-sm);
  }
  .carpeta ul {
    list-style: none;
    margin: 4px 0 var(--sp-2) 22px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .fila-c,
  .carpeta li label {
    display: flex;
    gap: 6px;
    align-items: center;
    min-width: 0;
  }
  .ruta,
  .mono {
    overflow-wrap: anywhere;
  }
  .motivo {
    display: block;
    margin-left: 22px;
    font-size: var(--fs-xs);
    overflow-wrap: anywhere;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: var(--sp-3);
  }
</style>
