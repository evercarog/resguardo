<script lang="ts">
  // Registrar (o cambiar) una prueba funcional (plan 0.8.0, E3, 14k): un simulacro
  // de recuperación con su fecha, motivo, alcance, quién, qué se restauró y dónde,
  // desde dónde, resultado, incidencias, el tiempo hasta tener los datos de vuelta
  // (RTO real), la antigüedad de lo recuperado (RPO), notas y adjuntos pequeños
  // (PDF, PNG o JPG). Puede salir de una restauración ya registrada: rellena el
  // día, el repositorio, el tiempo y, si se sabe, la antigüedad.
  import { ClipboardCheck, FileText, LoaderCircle, Paperclip, Trash2, Wand } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { avisar } from "$lib/avisos.svelte";
  import { actual, reloj } from "$lib/estado.svelte";
  import { bytes, fechaLarga } from "$lib/formato";
  import { aMinutos, deMinutos, deRestauracion, DESDE_PRUEBA, diaLocal, errorPrueba, MOTIVOS_PRUEBA, paraEditar, pruebaNueva, RESULTADOS_PRUEBA, textoMinutos } from "$lib/pruebasFuncionales";
  import { textoTardo, type ConMotivo } from "$lib/restauraciones";
  import { ultimos } from "$lib/informes.svelte";
  import { informeDe } from "$lib/repo";
  import { ponerFuncionales } from "$lib/funcionales.svelte";
  import type { EntradaPruebaFuncional, PruebaFuncional, PruebasFuncionales } from "$lib/tiposPruebas";

  let {
    cliente,
    editando = null,
    inicial = null,
    restauraciones = [],
    onclose,
  }: {
    cliente: string;
    /** La prueba que se cambia (sin ella, una nueva). */
    editando?: PruebaFuncional | null;
    /** Lo que se propone para una nueva (p. ej. desde una restauración). */
    inicial?: EntradaPruebaFuncional | null;
    /** Las restauraciones registradas del cliente (para enlazar una). */
    restauraciones?: ConMotivo[];
    onclose: () => void;
  } = $props();

  const hoy = diaLocal(reloj.ahora);
  const primera = (): EntradaPruebaFuncional => (editando ? paraEditar(editando) : (inicial ?? pruebaNueva(hoy)));
  const ini = primera();
  let f = $state<EntradaPruebaFuncional>(ini);
  const rto0 = deMinutos(ini.rto_min);
  const rpo0 = deMinutos(ini.rpo_min);
  let rtoTexto = $state(rto0.texto);
  let rtoUnidad = $state<"min" | "h" | "dias">(rto0.unidad);
  let rpoTexto = $state(rpo0.texto);
  let rpoUnidad = $state<"min" | "h" | "dias">(rpo0.unidad);
  const rto = $derived(aMinutos(rtoTexto, rtoUnidad));
  const rpo = $derived(aMinutos(rpoTexto, rpoUnidad));
  const entrada = $derived<EntradaPruebaFuncional>({
    ...f,
    rto_min: rto != null && Number.isFinite(rto) ? rto : null,
    rpo_min: rpo != null && Number.isFinite(rpo) ? rpo : null,
    desde: f.desde?.tipo ? { tipo: f.desde.tipo, nombre: f.desde.nombre?.trim() || null, destino: f.desde.destino ?? null } : null,
  });
  const error = $derived((rto != null && Number.isNaN(rto)) || (rpo != null && Number.isNaN(rpo)) ? "Escribe los tiempos con números (por ejemplo 2 o 1,5)." : errorPrueba(entrada, hoy));
  let tocado = $state(false);
  let guardando = $state(false);

  // Los repositorios del cliente, por equipo (para el alcance).
  const grupos = $derived(
    actual.equipos
      .filter((e) => e.confirmado !== false && e.modo !== "trasladado" && (e.resumen?.repositorios?.length ?? 0) > 0)
      .map((e) => ({ equipo: e, repos: e.resumen?.repositorios ?? [] })),
  );
  const marcado = (equipo: string, repo: string) => f.repos.some((r) => r.equipo === equipo && r.repo === repo);
  function alternar(equipo: string, repo: string, si: boolean) {
    f.repos = si ? [...f.repos.filter((r) => !(r.equipo === equipo && r.repo === repo)), { equipo, repo }] : f.repos.filter((r) => !(r.equipo === equipo && r.repo === repo));
  }

  // Desde una restauración registrada.
  let deRest = $state(ini.restauracion ? `${ini.restauracion.equipo}|${ini.restauracion.id}` : "");
  const nombreRepo = (eq: string, repo: string) => actual.equipos.find((e) => e.id === eq)?.resumen?.repositorios?.find((r) => r.id === repo)?.nombre ?? repo;
  const nombreEquipo = (id: string) => actual.equipos.find((e) => e.id === id)?.nombre ?? "Equipo quitado";
  /** La hora de la versión restaurada, si el último informe del equipo la tiene. */
  const horaVersion = (r: ConMotivo) => (r.version ? (informeDe(ultimos.porEquipo[r.equipo] ?? null, r.repo)?.versiones.find((v) => v.id.startsWith(r.version!))?.hora ?? null) : null);
  function usarRestauracion() {
    const r = restauraciones.find((x) => `${x.equipo}|${x.id}` === deRest);
    if (!r) {
      f.restauracion = null;
      return;
    }
    const d = deRestauracion(r, horaVersion(r));
    f = { ...d, quien: f.quien, notas: f.notas, motivo: f.motivo };
    const a = deMinutos(d.rto_min);
    const b = deMinutos(d.rpo_min);
    [rtoTexto, rtoUnidad, rpoTexto, rpoUnidad] = [a.texto, a.unidad, b.texto, b.unidad];
    avisar(d.rpo_min == null ? "Datos de la restauración puestos. La antigüedad de los datos no se sabe: escríbela tú." : "Datos de la restauración puestos: revísalos.");
  }

  // Adjuntos: los de la prueba (si ya existe) y los que se añadirán al guardar.
  const adjuntosIniciales = () => editando?.adjuntos ?? [];
  let adjuntos = $state(adjuntosIniciales());
  let pendientes = $state<File[]>([]);
  let archivo: HTMLInputElement | undefined = $state();
  const MAX = 2 * 1024 * 1024;
  async function tipoDe(file: File): Promise<string | null> {
    const b = new Uint8Array(await file.slice(0, 8).arrayBuffer());
    if (b[0] === 0x25 && b[1] === 0x50 && b[2] === 0x44 && b[3] === 0x46 && b[4] === 0x2d) return "application/pdf";
    if (b[0] === 0x89 && b[1] === 0x50 && b[2] === 0x4e && b[3] === 0x47) return "image/png";
    if (b[0] === 0xff && b[1] === 0xd8 && b[2] === 0xff) return "image/jpeg";
    return null;
  }
  async function elegirArchivos(fs: FileList | null) {
    for (const file of Array.from(fs ?? [])) {
      if (file.size > MAX) {
        avisar(`«${file.name}» pesa demasiado: como mucho 2 MB.`, "bad");
        continue;
      }
      if (!(await tipoDe(file))) {
        avisar(`«${file.name}»: solo PDF, PNG o JPG.`, "bad");
        continue;
      }
      if (adjuntos.length + pendientes.length >= 5) {
        avisar("Como mucho 5 adjuntos por prueba.", "bad");
        break;
      }
      pendientes = [...pendientes, file];
    }
    if (archivo) archivo.value = "";
  }
  async function enBase64(file: File): Promise<string> {
    const b = new Uint8Array(await file.arrayBuffer());
    let s = "";
    for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode(...b.subarray(i, i + 0x8000));
    return btoa(s);
  }
  async function quitarAdjunto(n: number) {
    if (!editando) return;
    try {
      const x = await api.borrarAdjuntoPrueba(cliente, editando.id, n);
      ponerFuncionales(cliente, x);
      adjuntos = adjuntos.filter((a) => a.n !== n);
    } catch (e) {
      avisar((e as Error).message, "bad");
    }
  }

  async function guardar() {
    tocado = true;
    if (error || guardando) return;
    guardando = true;
    try {
      let x: PruebasFuncionales & { id: string };
      if (editando) x = await api.cambiarPruebaFuncional(cliente, editando.id, entrada);
      else x = await api.registrarPruebaFuncional(cliente, entrada);
      let ultimo: PruebasFuncionales = x;
      for (const file of pendientes) {
        try {
          ultimo = await api.anadirAdjuntoPrueba(cliente, x.id, file.name, await enBase64(file));
        } catch (e) {
          avisar(`«${file.name}»: ${(e as Error).message}`, "bad");
        }
      }
      ponerFuncionales(cliente, ultimo);
      avisar(editando ? "Prueba funcional cambiada." : "Prueba funcional registrada: cuenta ya en la regla 3-2-1-1-0.");
      onclose();
    } catch (e) {
      avisar((e as Error).message, "bad");
    } finally {
      guardando = false;
    }
  }
</script>

<Modal labelledby="t-pf" {onclose} width={680}>
  <div class="dlg-title">
    <span class="ticon"><ClipboardCheck size={18} /></span>
    <div>
      <h2 id="t-pf">{editando ? "Cambiar la prueba funcional" : "Registrar una prueba funcional"}</h2>
      <p>Un simulacro de recuperación: recuperar datos como el día de un problema y apuntar cuánto se tardó y de cuándo eran. Sin contraseñas en los textos.</p>
    </div>
  </div>
  <form
    class="form"
    onsubmit={(e) => {
      e.preventDefault();
      void guardar();
    }}
  >
    {#if restauraciones.length}
      <div class="field">
        <label class="field-label" for="pf-rest">A partir de una restauración registrada <span class="faint">(opcional)</span></label>
        <div class="fila-campo">
          <select id="pf-rest" class="input" bind:value={deRest}>
            <option value="">Ninguna</option>
            {#each restauraciones.slice(0, 60) as r (`${r.equipo}|${r.id}`)}
              <option value={`${r.equipo}|${r.id}`}>{fechaLarga(r.hora)} · «{nombreRepo(r.equipo, r.repo)}» de {nombreEquipo(r.equipo)} · tardó {textoTardo(r.duracion_s)}{r.resultado === "ok" ? "" : " · falló"}</option>
            {/each}
          </select>
          <button type="button" class="btn" disabled={!deRest} onclick={usarRestauracion}><Wand size={15} />Usar sus datos</button>
        </div>
        <span class="field-hint">Pone el día, el repositorio, lo restaurado, el tiempo y, si se sabe, la antigüedad de los datos.</span>
      </div>
    {/if}

    <div class="dos">
      <div class="field">
        <label class="field-label" for="pf-fecha">Fecha</label>
        <input id="pf-fecha" class="input num" type="date" max={hoy} bind:value={f.fecha} required />
      </div>
      <div class="field">
        <label class="field-label" for="pf-motivo">Motivo</label>
        <select id="pf-motivo" class="input" bind:value={f.motivo}>
          {#each MOTIVOS_PRUEBA as m (m.id)}<option value={m.id}>{m.texto}</option>{/each}
        </select>
      </div>
    </div>

    <div class="field">
      <span class="field-label" id="pf-l-alcance">Alcance</span>
      <div class="segmented" role="radiogroup" aria-labelledby="pf-l-alcance">
        <button type="button" role="radio" aria-checked={f.alcance === "cliente"} class:on={f.alcance === "cliente"} onclick={() => (f.alcance = "cliente")}>Todo el cliente</button>
        <button type="button" role="radio" aria-checked={f.alcance === "repos"} class:on={f.alcance === "repos"} onclick={() => (f.alcance = "repos")}>Unos repositorios</button>
      </div>
      {#if f.alcance === "repos"}
        <div class="repos" role="group" aria-label="Repositorios probados">
          {#each grupos as g (g.equipo.id)}
            <div class="grupo">
              <span class="faint pequeno">{g.equipo.nombre}</span>
              {#each g.repos as r (r.id)}
                <label class="check"><input type="checkbox" checked={marcado(g.equipo.id, r.id)} onchange={(e) => alternar(g.equipo.id, r.id, e.currentTarget.checked)} />{r.nombre}</label>
              {/each}
            </div>
          {:else}
            <p class="faint">Este cliente aún no tiene repositorios.</p>
          {/each}
        </div>
      {/if}
    </div>

    <div class="field">
      <label class="field-label" for="pf-que">Qué se restauró</label>
      <textarea id="pf-que" class="input" rows="2" maxlength={2000} bind:value={f.que} placeholder="Por ejemplo: la carpeta de facturas del último mes y la base de datos de ventas"></textarea>
    </div>
    <div class="dos">
      <div class="field">
        <label class="field-label" for="pf-donde">Dónde se restauró</label>
        <input id="pf-donde" class="input" maxlength={200} bind:value={f.donde} placeholder="Por ejemplo: una máquina virtual de pruebas" />
      </div>
      <div class="field">
        <label class="field-label" for="pf-quien">Quién la hizo</label>
        <input id="pf-quien" class="input" maxlength={80} bind:value={f.quien} placeholder="Tú, si lo dejas en blanco" />
      </div>
    </div>
    <div class="dos">
      <div class="field">
        <label class="field-label" for="pf-desde">Desde dónde</label>
        <select
          id="pf-desde"
          class="input"
          value={f.desde?.tipo ?? ""}
          onchange={(e) => (f.desde = e.currentTarget.value ? { tipo: e.currentTarget.value as NonNullable<EntradaPruebaFuncional["desde"]>["tipo"], nombre: f.desde?.nombre ?? "", destino: f.desde?.destino ?? null } : null)}
        >
          <option value="">Sin decir</option>
          {#each DESDE_PRUEBA as d (d.id)}<option value={d.id}>{d.texto}</option>{/each}
        </select>
      </div>
      {#if f.desde}
        <div class="field">
          <label class="field-label" for="pf-desde-n">Cuál <span class="faint">(opcional)</span></label>
          <input id="pf-desde-n" class="input" maxlength={120} value={f.desde.nombre ?? ""} oninput={(e) => f.desde && (f.desde = { ...f.desde, nombre: e.currentTarget.value })} placeholder="Por ejemplo: la copia en la nube" />
        </div>
      {/if}
    </div>

    <div class="field">
      <span class="field-label" id="pf-l-res">Resultado</span>
      <div class="segmented" role="radiogroup" aria-labelledby="pf-l-res">
        {#each RESULTADOS_PRUEBA as r (r.id)}
          <button type="button" role="radio" aria-checked={f.resultado === r.id} class:on={f.resultado === r.id} onclick={() => (f.resultado = r.id)}>{r.texto}</button>
        {/each}
      </div>
    </div>
    {#if f.resultado !== "ok" || f.incidencias}
      <div class="field">
        <label class="field-label" for="pf-inc">Incidencias</label>
        <textarea id="pf-inc" class="input" rows="2" maxlength={2000} bind:value={f.incidencias} placeholder="Qué falló o qué hubo que arreglar"></textarea>
      </div>
    {/if}

    <div class="dos">
      <div class="field">
        <label class="field-label" for="pf-rto">Tiempo hasta tener los datos de vuelta <span class="faint">(RTO)</span></label>
        <div class="fila-campo">
          <input id="pf-rto" class="input num corto" inputmode="decimal" bind:value={rtoTexto} placeholder="—" />
          <select class="input" bind:value={rtoUnidad} aria-label="Unidad del tiempo">
            <option value="min">minutos</option>
            <option value="h">horas</option>
            <option value="dias">días</option>
          </select>
        </div>
        <span class="field-hint">Desde que empezó la recuperación hasta tener los datos listos para usar{rto != null && Number.isFinite(rto) ? `: ${textoMinutos(rto)}` : ""}.</span>
      </div>
      <div class="field">
        <label class="field-label" for="pf-rpo">Antigüedad de los datos recuperados <span class="faint">(RPO)</span></label>
        <div class="fila-campo">
          <input id="pf-rpo" class="input num corto" inputmode="decimal" bind:value={rpoTexto} placeholder="—" />
          <select class="input" bind:value={rpoUnidad} aria-label="Unidad de la antigüedad">
            <option value="min">minutos</option>
            <option value="h">horas</option>
            <option value="dias">días</option>
          </select>
        </div>
        <span class="field-hint">Lo que se habría perdido: el tiempo entre la versión recuperada y el momento del simulacro{rpo != null && Number.isFinite(rpo) ? `: ${textoMinutos(rpo)}` : ""}.</span>
      </div>
    </div>

    <div class="field">
      <label class="field-label" for="pf-notas">Notas <span class="faint">(opcional)</span></label>
      <textarea id="pf-notas" class="input" rows="2" maxlength={2000} bind:value={f.notas} placeholder="Lo que conviene recordar para la próxima"></textarea>
    </div>

    <div class="field">
      <span class="field-label">Adjuntos <span class="faint">(opcional: PDF, PNG o JPG, 2 MB como mucho, hasta 5)</span></span>
      {#if adjuntos.length || pendientes.length}
        <ul class="adjuntos">
          {#each adjuntos as a (a.n)}
            <li>
              <FileText size={14} /><a class="link-suave" href={api.urlAdjuntoPrueba(cliente, editando?.id ?? "", a.n)} download>{a.nombre}</a><span class="faint">{bytes(a.tamano)}</span>
              <button type="button" class="btn btn-ghost btn-sm" aria-label="Quitar {a.nombre}" onclick={() => quitarAdjunto(a.n)}><Trash2 size={14} /></button>
            </li>
          {/each}
          {#each pendientes as p, i (`${p.name}-${i}`)}
            <li>
              <FileText size={14} /><span>{p.name}</span><span class="faint">{bytes(p.size)} · se sube al guardar</span>
              <button type="button" class="btn btn-ghost btn-sm" aria-label="Quitar {p.name}" onclick={() => (pendientes = pendientes.filter((_, j) => j !== i))}><Trash2 size={14} /></button>
            </li>
          {/each}
        </ul>
      {/if}
      <div>
        <input bind:this={archivo} id="pf-archivo" class="sr-only" type="file" accept=".pdf,.png,.jpg,.jpeg,application/pdf,image/png,image/jpeg" multiple onchange={(e) => elegirArchivos(e.currentTarget.files)} />
        <label class="btn btn-sm" for="pf-archivo"><Paperclip size={14} />Añadir un adjunto</label>
      </div>
    </div>

    {#if tocado && error}<div class="notice notice-danger" role="alert"><p>{error}</p></div>{/if}
    <footer>
      <button type="button" class="btn btn-ghost" onclick={onclose}>Cancelar</button>
      <button type="submit" class="btn btn-primary" disabled={guardando}>{#if guardando}<LoaderCircle size={15} class="spin" />{:else}<ClipboardCheck size={15} />{/if}{editando ? "Guardar los cambios" : "Registrar la prueba"}</button>
    </footer>
  </form>
</Modal>

<style>
  .dos {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 240px), 1fr));
    gap: var(--sp-4);
  }
  .fila-campo {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .fila-campo .input:not(.corto) {
    flex: 1;
    min-width: 0;
  }
  .corto {
    width: 7rem;
  }
  .repos {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 200px), 1fr));
    gap: var(--sp-3);
    margin-top: var(--sp-2);
    max-height: 220px;
    overflow: auto;
  }
  .grupo {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .check {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
  }
  .pequeno {
    font-size: var(--fs-xs, 12px);
  }
  .adjuntos {
    margin: 0 0 var(--sp-2);
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: var(--fs-sm);
  }
  .adjuntos li {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
