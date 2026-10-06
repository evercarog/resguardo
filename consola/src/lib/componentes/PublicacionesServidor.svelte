<script lang="ts">
  // «Actualizaciones de los agentes» (solo el propietario del servidor, docs/actualizaciones.md §8):
  // la versión del agente que este servidor da a sus equipos y subir otra. El servidor solo la
  // acepta con la firma de la llave de publicación de Resguardo y cada archivo con su SHA-256:
  // esta página no firma nada (ni puede). Y cómo actualizar el propio servidor (a mano, por ahora).
  import { onMount } from "svelte";
  import { CircleAlert, PackageCheck, Upload } from "@lucide/svelte";
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
