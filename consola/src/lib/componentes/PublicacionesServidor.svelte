<script lang="ts">
  // «Actualizaciones de los agentes» (solo el propietario del servidor, docs/actualizaciones.md §8):
  // la versión del agente que este servidor da a sus equipos y subir otra. El servidor solo la
  // acepta con la firma de la llave de publicación de Resguardo y cada archivo con su SHA-256:
  // esta página no firma nada (ni puede). Y cómo actualizar el propio servidor (a mano, por ahora).
  import { onDestroy, onMount, untrack } from "svelte";
  import { CircleAlert, CloudDownload, PackageCheck, RefreshCw, Upload } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { bytes, fechaLarga } from "$lib/formato";
  import type { PublicacionAgente, PublicacionServidor } from "$lib/tipos";
  import BotonCargando from "$lib/componentes/BotonCargando.svelte";
  import Chip from "$lib/componentes/Chip.svelte";

  const MANIFIESTO = "manifiesto-agente.json";
  const FIRMA = "manifiesto-agente.json.minisig";

  /** null: servidor anterior; undefined: cargando. */
  let r = $state<PublicacionServidor | null | undefined>(undefined);
  let subiendo = $state(false);
  let paso = $state("");
  let entrada: HTMLInputElement | undefined = $state();

  onMount(async () => {
    try {
      r = await api.publicacionServidor();
    } catch {
      r = null;
    }
  });

  // 14f: bajarlas solas de GitHub (con la misma comprobación que al subirlas).
  let buscando = $state(false);
  let guardandoGh = $state(false);
  let sondeo: ReturnType<typeof setInterval> | null = null;
  const gh = $derived(r?.github ?? null);
  function seguirBusqueda() {
    if (sondeo) return;
    const inicio = Date.now();
    sondeo = setInterval(async () => {
      try {
        const x = await api.publicacionServidor();
        if (x) r = x;
        if (!x?.github?.buscando || Date.now() - inicio > 30 * 60_000) {
          if (sondeo) clearInterval(sondeo);
          sondeo = null;
          const u = x?.github?.ultima;
          if (u?.mensaje) (u.resultado === "nueva" || u.resultado === "al_dia" ? avisar : (m: string) => fallo(new Error(m)))(u.mensaje);
        }
      } catch {
        /* se reintenta */
      }
    }, 2000);
  }
  onDestroy(() => {
    if (sondeo) clearInterval(sondeo);
  });
  $effect(() => {
    if (r?.github?.buscando) untrack(seguirBusqueda);
  });
  async function buscarAhora() {
    buscando = true;
    try {
      r = await api.buscarGithubAhora();
      seguirBusqueda();
    } catch (e) {
      fallo(e);
    } finally {
      buscando = false;
    }
  }
  async function cambiarGithub(activo: boolean, horas: number) {
    guardandoGh = true;
    try {
      r = await api.ajustesGithub({ activo, horas });
    } catch (e) {
      fallo(e);
    } finally {
      guardandoGh = false;
    }
  }
  const TEXTO_ULTIMA: Record<string, { tono: "ok" | "warn" | "bad" | "neutral"; texto: string }> = {
    nueva: { tono: "ok", texto: "Versión nueva bajada" },
    al_dia: { tono: "ok", texto: "Al día" },
    sin_red: { tono: "warn", texto: "No se pudo mirar" },
    no_cuadra: { tono: "bad", texto: "Rechazada: no cuadra" },
  };
  const HORAS = [1, 3, 6, 12, 24, 48, 168];

  const PLATAFORMA: Record<string, string> = { "windows-x86_64": "Windows", "linux-x86_64": "Linux x86_64", "linux-aarch64": "Linux arm64" };

  async function subir(ev: Event) {
    const archivos = [...((ev.currentTarget as HTMLInputElement).files ?? [])];
    if (!archivos.length) return;
    const m = archivos.find((f) => f.name === MANIFIESTO);
    const s = archivos.find((f) => f.name === FIRMA);
    if (!m || !s) {
      fallo(new Error(`Elige a la vez ${MANIFIESTO}, ${FIRMA} y los archivos de la versión (los de su carpeta).`));
      if (entrada) entrada.value = "";
      return;
    }
    subiendo = true;
    try {
      paso = "Comprobando la firma…";
      let estado = await api.ponerPublicacion(await m.text(), await s.text());
      const version = (JSON.parse(await m.text()) as { version: string }).version;
      const guardada = estado.guardadas.find((g) => g.version === version);
      for (const a of guardada?.archivos ?? []) {
        if (a.presente) continue;
        const f = archivos.find((x) => x.name === a.nombre);
        if (!f) continue;
        paso = `Subiendo ${a.nombre} (${bytes(f.size)})…`;
        estado = await api.subirArchivoPublicacion(version, a.nombre, f);
      }
      r = estado;
      const g = estado.guardadas.find((x) => x.version === version);
      avisar(g?.completa ? `Versión ${version} lista: los equipos la verán en su próxima búsqueda.` : `Versión ${version} guardada; aún faltan archivos: ${g?.archivos.filter((a) => !a.presente).map((a) => a.nombre).join(", ")}.`);
    } catch (e) {
      fallo(e);
      r = await api.publicacionServidor().catch(() => r);
    } finally {
      subiendo = false;
      paso = "";
      if (entrada) entrada.value = "";
    }
  }

  const estadoDe = (g: PublicacionAgente) => (g.completa ? { tono: "ok" as const, texto: "Completa" } : { tono: "warn" as const, texto: "Faltan archivos" });
</script>

{#if r}
  <section class="card p" id="publicaciones" aria-labelledby="t-publicaciones">
    <div class="cab">
      <span class="card-icon"><PackageCheck size={18} /></span>
      <div class="cab-texto">
        <h2 id="t-publicaciones">Actualizaciones de los agentes</h2>
        <p class="faint">La versión del agente que este servidor da a sus equipos (también a los que no salen a Internet). Solo acepta versiones firmadas con la llave de publicación de Resguardo.</p>
      </div>
      {#if r.vigente}<Chip tono="ok" texto={`Da la ${r.vigente.version}`} />{:else if !r.sin_llave}<Chip tono="neutral" texto="Sin versiones" />{/if}
    </div>

    {#if r.sin_llave}
      <div class="notice notice-warn">
        <CircleAlert size={16} />
        <p>Este servidor se compiló sin llave de publicación: no acepta versiones del agente. Los equipos que salen a Internet las buscan en GitHub.</p>
      </div>
    {:else}
      {#if r.guardadas.length}
        <ul class="lista">
          {#each r.guardadas as g (g.version)}
            {@const est = estadoDe(g)}
            <li>
              <div class="fila">
                <strong>{g.version}</strong>
                <span class="faint">del {fechaLarga(g.fecha)}</span>
                <Chip pequeno tono={est.tono} texto={est.texto} />
              </div>
              <span class="faint pequeno">
                {g.archivos.map((a) => `${PLATAFORMA[a.plataforma] ?? a.plataforma}${a.presente ? "" : " (falta)"}`).join(" · ")}{g.notas ? ` — ${g.notas}` : ""}
              </span>
            </li>
          {/each}
        </ul>
      {/if}
      <div class="acciones">
        <input bind:this={entrada} class="oculto" type="file" multiple onchange={subir} aria-label="Archivos de la versión" />
        <BotonCargando class="btn btn-sm" cargando={subiendo} textoCargando={paso || "Subiendo…"} onclick={() => entrada?.click()}><Upload size={14} />Subir una versión…</BotonCargando>
      </div>
      <p class="faint pequeno">
        Elige a la vez los archivos de su carpeta: <code>{MANIFIESTO}</code>, <code>{FIRMA}</code> y el instalador y los paquetes. O, en la máquina del servidor,
        <code class="selectable">sudo resguardo-server poner-publicacion &lt;carpeta&gt;</code>. Llave{r.llaves.length === 1 ? "" : "s"}: {r.llaves.join(", ")}.
      </p>
    {/if}
    {#if gh && !r.sin_llave}
      <div class="github" aria-labelledby="t-github">
        <h3 id="t-github"><CloudDownload size={15} /> Bajarlas solas de GitHub</h3>
        <p class="faint">El servidor baja la última versión publicada (el manifiesto, su firma y los archivos), comprueba la firma con la llave de publicación y el SHA-256 de cada archivo, y solo entonces la da a sus equipos. Si algo no cuadra, no la usa y lo dice aquí.</p>
        <div class="fila-gh">
          <label class="switch-row">
            <input type="checkbox" checked={gh.ajustes.activo} disabled={guardandoGh} onchange={(e) => cambiarGithub((e.currentTarget as HTMLInputElement).checked, gh.ajustes.horas)} />
            <span>Buscar sola</span>
          </label>
          <label class="cada">
            cada
            <select class="input input-sm" value={gh.ajustes.horas} disabled={guardandoGh || !gh.ajustes.activo} onchange={(e) => cambiarGithub(gh.ajustes.activo, Number((e.currentTarget as HTMLSelectElement).value))} aria-label="Cada cuántas horas">
              {#each HORAS as h (h)}<option value={h}>{h === 168 ? "semana" : h === 1 ? "hora" : `${h} h`}</option>{/each}
            </select>
          </label>
          <BotonCargando class="btn btn-sm" cargando={buscando || gh.buscando} textoCargando="Buscando en GitHub…" onclick={buscarAhora}><RefreshCw size={14} />Buscar ahora</BotonCargando>
        </div>
        {#if !gh.automatica}<p class="faint pequeno">Compilación de desarrollo: no busca sola (solo con «Buscar ahora»).</p>{/if}
        {#if gh.ultima}
          {@const t = TEXTO_ULTIMA[gh.ultima.resultado] ?? { tono: "neutral" as const, texto: gh.ultima.resultado }}
          <p class="ultima-gh">
            <Chip pequeno tono={t.tono} texto={t.texto} />
            <span class="faint">{fechaLarga(new Date(gh.ultima.cuando * 1000).toISOString())}</span>
            {#if gh.ultima.mensaje}<span>{gh.ultima.mensaje}</span>{/if}
          </p>
          {#if gh.ultima.instalador}<p class="faint pequeno">{gh.ultima.instalador}</p>{/if}
        {:else}
          <p class="faint pequeno">Todavía no ha buscado.</p>
        {/if}
      </div>
    {/if}
    {#if r.instalador}
      <p class="faint pequeno">
        Instalador del agente para «Instalar muchos equipos» y «Descargar instalador listo»:
        {#if !r.instalador.hay}ninguno todavía{#if r.instalador.de_datos}{" "}(se pone solo al bajar una versión de GitHub, o desde «Instalar muchos equipos»){/if}.
        {:else if r.instalador.version}el de la {r.instalador.version}, de una versión firmada.
        {:else if r.instalador.de_datos}uno puesto a mano.
        {:else}el que trae Resguardo Server (o el de <code>--instalador-agente</code>).{/if}
      </p>
    {/if}
    <details class="ayuda">
      <summary>Actualizar este servidor ({r.version_servidor})</summary>
      <p class="faint">
        Resguardo Server aún no se actualiza solo. Baja la versión nueva de la página de publicaciones de Resguardo en GitHub, comprueba su firma (<code>minisign -Vm …</code>)
        y su SHA-256 (<code>SHA256SUMS</code>) y, en Linux, <code class="selectable">sudo sh instalar-servidor.sh --paquete resguardo-server-…tar.gz</code> (o el <code>.deb</code>
        con <code>apt install ./…deb</code>); en Windows, su instalador. Antes de abrir la base de datos, la versión nueva hace una copia de la consola.
      </p>
    </details>
  </section>
{/if}

<style>
  .cab {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
  }
  .cab-texto {
    flex: 1;
    min-width: 0;
  }
  .cab h2 {
    margin: 0;
    font-size: var(--fs-md, 15px);
  }
  .cab p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .notice {
    margin-top: var(--sp-3);
  }
  .lista {
    list-style: none;
    margin: var(--sp-3) 0 0;
    padding: 0;
    border-top: 1px solid var(--border);
  }
  .lista li {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
  }
  .fila {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex-wrap: wrap;
  }
  .acciones {
    display: flex;
    gap: 8px;
    margin-top: var(--sp-3);
  }
  .oculto {
    display: none;
  }
  .pequeno {
    font-size: var(--fs-xs);
    margin: var(--sp-3) 0 0;
  }
  .lista .pequeno {
    margin: 0;
  }
  .github {
    margin-top: var(--sp-4);
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .github h3 {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-sm);
    font-weight: 600;
  }
  .github > p {
    margin: 4px 0 0;
    font-size: var(--fs-sm);
  }
  .fila-gh {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--sp-3);
    margin-top: var(--sp-3);
  }
  .cada {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
  }
  .ultima-gh {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: var(--sp-3) 0 0;
    font-size: var(--fs-sm);
  }
  .ayuda {
    margin-top: var(--sp-3);
    font-size: var(--fs-sm);
  }
  @media (max-width: 640px) {
    .cab {
      flex-wrap: wrap;
    }
  }
</style>
