<script lang="ts">
  // «Copia de la consola» (v1.23, solo el propietario del servidor): cada noche
  // el servidor guarda una copia cifrada de sí mismo con la clave de respaldo de
  // la consola. Esa clave se elige aquí y no sale del navegador: al servidor
  // solo le llega la clave pública que sale de ella (lib/cripto/respaldo.ts).
  // Al ponerla se muestra su kit para imprimir, como el de los repositorios.
  import { onMount } from "svelte";
  import { CircleAlert, DatabaseBackup, KeyRound, LifeBuoy, Play, Printer, ShieldCheck } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import * as api from "$lib/api";
  import { app } from "$lib/estado.svelte";
  import { avisar, fallo } from "$lib/avisos.svelte";
  import { bytes, fechaLarga, plural } from "$lib/formato";
  import { fuerza } from "$lib/fuerza";
  import { huellaCorta } from "$lib/servidores";
  import { argon2Navegador } from "$lib/cripto/argon2";
  import { MIN_CLAVE_RESPALDO, publicaRespaldo, salRespaldo } from "$lib/cripto/respaldo";
  import type { RespaldoConsola } from "$lib/tipos";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import BloqueCopiable from "$lib/componentes/BloqueCopiable.svelte";
  import BotonCargando from "$lib/componentes/BotonCargando.svelte";
  import CampoClave from "$lib/componentes/CampoClave.svelte";
  import Chip from "$lib/componentes/Chip.svelte";
  import Tiempo from "$lib/componentes/Tiempo.svelte";

  /** null: servidor anterior (sin «Copia de la consola»); undefined: cargando. */
  let r = $state<RespaldoConsola | null | undefined>(undefined);
  let haciendo = $state(false);
  let cambiando = $state(false);

  onMount(async () => {
    try {
      r = await api.respaldoConsola();
    } catch {
      r = null;
    }
  });

  const estado = $derived.by(() => {
    if (!r) return null;
    if (!r.clave_puesta) return { tono: "warn" as const, texto: "Sin poner" };
    if (!r.activo) return { tono: "neutral" as const, texto: "Apagada" };
    if (r.ultima && !r.ultima.ok) return { tono: "bad" as const, texto: "La última falló" };
    if (!r.ultima) return { tono: "info" as const, texto: "Aún sin copias" };
    return { tono: "ok" as const, texto: "Activa" };
  });

  async function hacerAhora() {
    haciendo = true;
    try {
      r = await api.respaldoConsolaAhora();
      avisar(r.ultima?.mensaje ?? "Copia de la consola hecha.");
    } catch (e) {
      fallo(e);
      r = await api.respaldoConsola().catch(() => r);
    } finally {
      haciendo = false;
    }
  }

  async function encender(activo: boolean) {
    cambiando = true;
    try {
      r = await api.cambiarRespaldoConsola({ activo });
      avisar(activo ? "La copia de la consola vuelve a hacerse cada noche." : "Copia de la consola apagada.");
    } catch (e) {
      fallo(e);
    } finally {
      cambiando = false;
    }
  }

  // ---- Poner (o cambiar) la clave ----
  let dlg = $state(false);
  let paso = $state<"clave" | "kit">("clave");
  let clave = $state("");
  let repetida = $state("");
  let hora = $state("03:30");
  let conservar = $state(7);
  let guardando = $state(false);
  let impreso = $state(false);
  let error = $state("");
  const f = $derived(fuerza(clave));
  const valida = $derived([...clave].length >= MIN_CLAVE_RESPALDO && f.nivel >= 2 && clave === repetida);
  // Con la huella de la identidad: la restauración comprueba que la copia (firmada) es de este servidor.
  const restaurar = $derived(
    `resguardo-server restaurar-respaldo consola-AAAAMMDD-….resguardo-consola --confiar-en ${huellaCorta(r?.identidad ?? app.servidor?.identidad ?? "")}`,
  );

  function abrir() {
    clave = "";
    repetida = "";
    hora = r?.hora ?? "03:30";
    conservar = r?.conservar ?? 7;
    impreso = false;
    error = "";
    paso = "clave";
    dlg = true;
  }
  function cerrar() {
    clave = "";
    repetida = "";
    dlg = false;
  }

  async function guardar(ev: SubmitEvent) {
    ev.preventDefault();
    if (!valida) return;
    guardando = true;
    error = "";
    try {
      const sal = salRespaldo();
      const publica = await publicaRespaldo(argon2Navegador, clave, sal);
      r = await api.cambiarRespaldoConsola({ publica, sal, activo: true, hora, conservar });
      paso = "kit";
    } catch (e) {
      error = (e as Error).message;
    } finally {
      guardando = false;
    }
  }

  async function terminar() {
    cerrar();
    avisar("Clave de respaldo puesta. Esta noche se hará la primera copia (o pulsa «Hacer ahora»).");
  }
</script>

{#if r}
  <section class="card p" id="copia-consola" aria-labelledby="t-copia-consola">
    <div class="cab">
      <span class="card-icon"><DatabaseBackup size={18} /></span>
      <div class="cab-texto">
        <h2 id="t-copia-consola">Copia de la consola <Ayuda id="copia-consola" /></h2>
        <p class="faint">Cada noche, una copia cifrada de este servidor (cuentas, clientes, equipos, historial y su identidad) para restaurarlo en otra máquina sin volver a vincular nada.</p>
      </div>
      {#if estado}<Chip tono={estado.tono} texto={estado.texto} />{/if}
    </div>

    {#if !r.clave_puesta}
      <div class="notice notice-warn">
        <CircleAlert size={16} />
        <p>Aún no tiene clave de respaldo. Elígela y guarda su kit: sin ella, una copia de la consola no se puede abrir (tampoco el servidor la tiene).</p>
      </div>
      <div class="acciones"><button class="btn btn-primary" onclick={abrir}><KeyRound size={15} />Poner la clave de respaldo…</button></div>
    {:else}
      <dl class="datos">
        <dt>Última</dt>
        <dd>
          {#if r.ultima}
            <Tiempo iso={r.ultima.cuando} /> · {r.ultima.ok ? `correcta${r.ultima.bytes != null ? `, ${bytes(r.ultima.bytes)}` : ""}` : r.ultima.mensaje}
          {:else}Todavía ninguna{/if}
        </dd>
        <dt>Próxima</dt>
        <dd>{r.activo && r.proxima ? fechaLarga(r.proxima) : "Apagada"} · cada día a las {r.hora}</dd>
        <dt>Se guardan</dt>
        <dd>{plural(r.copias.length, "copia", "copias")} de {r.conservar} · clave puesta el {fechaLarga(r.clave_puesta)}</dd>
        <dt>Carpeta</dt>
        <dd><code class="selectable">{r.carpeta}</code></dd>
      </dl>
      <div class="acciones">
        <BotonCargando class="btn btn-primary btn-sm" cargando={haciendo} textoCargando="Haciendo la copia…" onclick={hacerAhora}><Play size={14} />Hacer ahora</BotonCargando>
        <button class="btn btn-sm" onclick={abrir}><KeyRound size={14} />Cambiar la clave…</button>
        <BotonCargando class="btn btn-sm btn-ghost" cargando={cambiando} onclick={() => encender(!r!.activo)}>{r.activo ? "Apagar la copia diaria" : "Encender la copia diaria"}</BotonCargando>
      </div>
      <div class="notice notice-info">
        <ShieldCheck size={16} />
        <p>
          Las copias se quedan en esta máquina. Para que también salgan de aquí, añade la carpeta de arriba a una copia del agente de este servidor (en su equipo:
          «Cambiar las copias» → «Añadir una copia», por ejemplo «Consola de Resguardo»). Si esta máquina es también el almacén, esa copia puede ir a un repositorio
          de su propio almacén («Copiar en este mismo almacén»): así la cubren su espejo y la nube.
        </p>
      </div>
    {/if}

    <details class="ayuda">
      <summary><LifeBuoy size={14} />Si pierdes esta máquina…</summary>
      <ol>
        <li>Instala Resguardo Server en otra máquina <strong>con la misma dirección</strong> (nombre o IP) que tenía esta, sin arrancarlo todavía (o páralo).</li>
        <li>Recupera la copia más reciente (<code>consola-….resguardo-consola</code>): de la copia del agente que la guardaba, con «Restaurar», o de tu almacén.</li>
        <li>Como administrador (en Linux, con <code>sudo</code>): <code class="selectable">resguardo-server restaurar-respaldo &lt;archivo&gt;</code> y escribe la clave de respaldo del kit. Antes te enseña la identidad del servidor que hizo la copia: tiene que ser la del kit.</li>
        <li>Arranca el servicio. Los equipos reconocen su identidad y vuelven solos, con todo su historial.</li>
      </ol>
      <p class="faint">Sin la copia, aún puedes volver a vincular cada equipo a un servidor nuevo con la misma clave de administración (ver la Ayuda).</p>
    </details>
  </section>
{/if}

{#if dlg}
  <Modal labelledby="t-clave-respaldo" onclose={cerrar} width={560} dismissible={false}>
    {#if paso === "clave"}
      <form class="form" onsubmit={guardar}>
        <div class="dlg-title">
          <span class="ticon"><KeyRound size={18} /></span>
          <div>
            <h2 id="t-clave-respaldo">{r?.clave_puesta ? "Cambiar la clave de respaldo" : "Clave de respaldo de la consola"}</h2>
            <p>Con ella se abren las copias de la consola. Se queda en este navegador: al servidor solo le llega una llave para cerrarlas, no para abrirlas.</p>
          </div>
        </div>
        {#if r?.clave_puesta}
          <div class="notice notice-warn"><CircleAlert size={16} /><p>Las copias que ya hay siguen abriéndose con la clave anterior; las nuevas, con esta.</p></div>
        {/if}
        <CampoClave requerido id="cr-clave" etiqueta="Clave de respaldo" autocomplete="new-password" autofocus bind:value={clave} ayuda="Una frase larga que no uses en otro sitio. Distinta de la clave de administración." />
        {#if clave}
          <div class="fuerza" aria-live="polite">
            <div class="barra nivel-{f.nivel}" role="meter" aria-valuemin="0" aria-valuemax="4" aria-valuenow={f.nivel} aria-label="Fuerza de la clave"><span></span></div>
            <span class="pequeno">{f.texto}{f.consejo ? ` · ${f.consejo}` : ""}</span>
          </div>
        {/if}
        <CampoClave requerido id="cr-repetida" etiqueta="Repítela" autocomplete="new-password" bind:value={repetida} error={repetida && repetida !== clave ? "No coinciden." : ""} />
        <div class="dos">
          <div class="field">
            <label class="field-label" for="cr-hora">Hora de la copia diaria</label>
            <input id="cr-hora" class="input" type="time" bind:value={hora} required />
          </div>
          <div class="field">
            <label class="field-label" for="cr-conservar">Copias que se guardan</label>
            <input id="cr-conservar" class="input" type="number" min="1" max="60" bind:value={conservar} required />
          </div>
        </div>
        {#if error}<div class="notice notice-danger" role="alert"><CircleAlert size={16} /><p>{error}</p></div>{/if}
        <footer>
          <button type="button" class="btn btn-ghost" onclick={cerrar}>Cancelar</button>
          <BotonCargando class="btn btn-primary" type="submit" cargando={guardando} textoCargando="Preparando la llave…" disabled={!valida}>Seguir: ver el kit</BotonCargando>
        </footer>
      </form>
    {:else}
      <div class="form">
        <div class="dlg-title">
          <span class="ticon"><Printer size={18} /></span>
          <div>
            <h2 id="t-clave-respaldo">Guarda el kit</h2>
            <p>Imprímelo o guárdalo en PDF fuera de esta máquina: sin la clave, las copias de la consola no se pueden abrir. No se vuelve a mostrar.</p>
          </div>
        </div>
        <article class="kit" id="kit-imprimible">
          <h3>Kit de la copia de la consola · Resguardo Server</h3>
          <dl>
            <dt>Servidor</dt><dd>{typeof location !== "undefined" ? location.host : ""}</dd>
            <dt>Identidad</dt><dd><code>{huellaCorta(r?.identidad ?? app.servidor?.identidad ?? "")}</code></dd>
            <dt>Huella TLS</dt><dd><code>{app.servidor?.huella_ca ?? ""}</code></dd>
            <dt>Clave</dt><dd><code class="selectable pw">{clave}</code></dd>
            <dt>Copias en</dt><dd><code>{r?.carpeta}</code> (cada día a las {r?.hora})</dd>
            <dt>Restaurar</dt><dd><code>{restaurar}</code><br />con el servicio parado, en una máquina con la misma dirección</dd>
            <dt>Puesta el</dt><dd>{fechaLarga(new Date().toISOString())}</dd>
          </dl>
        </article>
        <div><button type="button" class="btn" onclick={() => window.print()}><Printer size={15} />Imprimir o guardar en PDF</button></div>
        <BloqueCopiable texto={clave} etiqueta="Copiar la clave" alto={2} />
        <label class="switch-row"><input type="checkbox" bind:checked={impreso} /><span>He guardado el kit en un sitio seguro, fuera de esta máquina</span></label>
        <footer>
          <button class="btn btn-primary" disabled={!impreso} onclick={terminar}>Listo</button>
        </footer>
      </div>
    {/if}
  </Modal>
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
    display: flex;
    align-items: center;
    margin: 0;
    font-size: var(--fs-md, 15px);
  }
  .cab p {
    margin: 2px 0 0;
    font-size: var(--fs-sm);
  }
  .card > .notice {
    margin-top: var(--sp-4);
  }
  .datos {
    display: grid;
    grid-template-columns: 110px minmax(0, 1fr);
    gap: 6px 12px;
    margin: var(--sp-4) 0;
    font-size: var(--fs-sm);
  }
  .datos dt {
    color: var(--text-3);
  }
  .datos dd {
    margin: 0;
    word-break: break-word;
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
    margin: var(--sp-4) 0 var(--sp-3);
  }
  .ayuda {
    margin-top: var(--sp-3);
    font-size: var(--fs-sm);
  }
  .ayuda summary {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    font-weight: 600;
  }
  .ayuda ol {
    margin: var(--sp-2) 0;
    padding-left: 1.4em;
    display: grid;
    gap: 4px;
  }
  .fuerza {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    margin-top: -6px;
  }
  .fuerza .barra {
    flex: 0 0 140px;
    height: 6px;
    border-radius: 3px;
    background: var(--border);
    overflow: hidden;
  }
  .fuerza .barra span {
    display: block;
    height: 100%;
    width: 8%;
    background: var(--bad, #c0392b);
  }
  .nivel-1 span {
    width: 30% !important;
    background: var(--bad, #c0392b) !important;
  }
  .nivel-2 span {
    width: 55% !important;
    background: var(--warn, #d68910) !important;
  }
  .nivel-3 span {
    width: 80% !important;
    background: var(--ok, #1e8449) !important;
  }
  .nivel-4 span {
    width: 100% !important;
    background: var(--ok, #1e8449) !important;
  }
  .pequeno {
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .dos {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--sp-3);
  }
  .kit {
    padding: var(--sp-5);
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
  }
  .kit h3 {
    margin-bottom: var(--sp-3);
    font-size: var(--fs-h2);
  }
  .kit dl {
    display: grid;
    grid-template-columns: 110px minmax(0, 1fr);
    gap: 6px 12px;
    margin: 0;
    font-size: var(--fs-sm);
  }
  .kit dt {
    color: var(--text-3);
  }
  .kit dd {
    margin: 0;
    word-break: break-all;
  }
  .pw {
    font-size: 14px;
  }
  @media (max-width: 640px) {
    .cab {
      flex-wrap: wrap;
    }
  }
  @media (max-width: 560px) {
    .dos {
      grid-template-columns: 1fr;
    }
  }
  @media print {
    :global(body *) {
      visibility: hidden;
    }
    :global(#kit-imprimible),
    :global(#kit-imprimible *) {
      visibility: visible;
    }
    :global(#kit-imprimible) {
      position: fixed;
      inset: 0 auto auto 0;
      width: 100%;
      border: none;
      color: #000;
    }
  }
</style>
