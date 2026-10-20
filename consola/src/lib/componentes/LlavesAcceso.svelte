<script lang="ts">
  // 0.8.1 (H3, plan-mejoras 12c): las llaves de acceso (passkeys) de la cuenta, para
  // entrar sin el código de la aplicación: la huella o el PIN del equipo, el móvil o una
  // llave USB. La contraseña sigue haciendo falta; el código TOTP y los códigos de
  // recuperación siguen valiendo. Dar de alta y quitar una piden la contraseña y un
  // código (docs/passkeys.md).
  import { untrack } from "svelte";
  import { Fingerprint, Plus, Trash2 } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import CampoClave from "./CampoClave.svelte";
  import CampoCodigo from "./CampoCodigo.svelte";
  import * as api from "$lib/api";
  import { avisar } from "$lib/avisos.svelte";
  import { fechaLarga, relativo } from "$lib/formato";
  import { crear, errorNavegador, soporteAqui, type PasskeyVista } from "$lib/passkeys";

  const soporte = soporteAqui();
  let lista = $state<PasskeyVista[] | null>(null);
  let max = $state(10);
  let fallo = $state("");
  function cargar() {
    api
      .passkeys()
      .then((r) => {
        lista = r.passkeys;
        max = r.max;
        fallo = "";
      })
      .catch((e: unknown) => {
        lista = [];
        fallo = e instanceof api.ApiError && e.estado === 404 ? "Este servidor aún no admite llaves de acceso: actualiza Resguardo Server." : (e as Error).message;
      });
  }
  $effect(() => untrack(cargar));

  let modal = $state<{ tipo: "alta" } | { tipo: "baja"; p: PasskeyVista } | null>(null);
  let nombre = $state("");
  let contrasena = $state("");
  let codigo = $state("");
  let ocupado = $state(false);
  let error = $state("");
  function abrir(m: NonNullable<typeof modal>) {
    modal = m;
    nombre = m.tipo === "alta" ? sugerencia() : "";
    contrasena = codigo = error = "";
  }
  /** «Este equipo (Windows)», «Este móvil»… (se puede cambiar). */
  function sugerencia() {
    const ua = typeof navigator === "undefined" ? "" : navigator.userAgent;
    const so = /Windows/.test(ua) ? "Windows" : /Android/.test(ua) ? "Android" : /iPhone|iPad/.test(ua) ? "iPhone" : /Mac/.test(ua) ? "Mac" : /Linux/.test(ua) ? "Linux" : "";
    return so ? `Este equipo (${so})` : "Mi llave";
  }
  async function enviar(e: SubmitEvent) {
    e.preventDefault();
    if (!modal) return;
    error = "";
    ocupado = true;
    const c = codigo.replace(/\D/g, "");
    try {
      if (modal.tipo === "alta") {
        const op = await api.opcionesPasskeyAlta(contrasena, c);
        contrasena = codigo = "";
        let r;
        try {
          r = await crear(op);
        } catch (err) {
          throw new Error(errorNavegador(err));
        }
        const x = await api.altaPasskey({ nombre: nombre.trim(), ...r });
        lista = x.passkeys;
        avisar(`Llave «${nombre.trim()}» dada de alta: ya puedes entrar con ella.`);
      } else {
        const x = await api.bajaPasskey(modal.p.ref, contrasena, c);
        lista = x.passkeys;
        avisar(`Llave «${modal.p.nombre}» quitada.`);
      }
      modal = null;
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
      contrasena = codigo = "";
    }
  }
  const listo = $derived(!!modal && !!contrasena && codigo.replace(/\D/g, "").length === 6 && (modal.tipo === "baja" || (nombre.trim().length > 0 && nombre.trim().length <= 60)));
</script>

<section class="card p" aria-labelledby="t-llaves">
  <h2 class="section-title" id="t-llaves"><Fingerprint size={16} /> Llaves de acceso</h2>
  <p class="faint">Entra con la huella o el PIN de este equipo, con el móvil o con una llave USB en vez de escribir el código de la aplicación. La contraseña sigue haciendo falta, y el código y los de recuperación siguen valiendo.</p>
  {#if !soporte.ok}
    <div class="notice notice-info"><p>{soporte.motivo}</p></div>
  {/if}
  {#if fallo}
    <div class="notice notice-warn"><p>{fallo}</p></div>
  {:else if lista === null}
    <p class="faint">Cargando…</p>
  {:else}
    {#if lista.length}
      <ul class="llaves">
        {#each lista as p (p.ref)}
          <li>
            <span class="ic" aria-hidden="true"><Fingerprint size={16} /></span>
            <span class="txt">
              <strong>{p.nombre}</strong>
              <span class="faint">Desde el {fechaLarga(p.creada)} · {p.usada ? `usada ${relativo(p.usada)}` : "aún sin usar"} · {p.dominio}</span>
            </span>
            <button class="btn btn-ghost btn-sm" aria-label="Quitar la llave {p.nombre}" onclick={() => abrir({ tipo: "baja", p })}><Trash2 size={14} />Quitar</button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="faint">Aún no tienes ninguna.</p>
    {/if}
    <div class="acciones">
      <button class="btn btn-sm" disabled={!soporte.ok || lista.length >= max} onclick={() => abrir({ tipo: "alta" })}><Plus size={14} />Añadir una llave de acceso</button>
    </div>
  {/if}
</section>

{#if modal}
  <Modal labelledby="t-llave" onclose={() => (modal = null)} width={460} dismissible={false} ocupado={ocupado}>
    <div class="dlg-title">
      <span class="ticon"><Fingerprint size={18} /></span>
      <div>
        <h2 id="t-llave">{modal.tipo === "alta" ? "Añadir una llave de acceso" : `Quitar «${modal.p.nombre}»`}</h2>
        <p>{modal.tipo === "alta" ? "Después, el navegador te pedirá la huella, el PIN o la llave." : "Ya no podrás entrar con ella. El código de la aplicación sigue valiendo."}</p>
      </div>
    </div>
    <form class="form" onsubmit={enviar}>
      {#if modal.tipo === "alta"}
        <div class="field">
          <label class="field-label" for="llave-nombre">Nombre de la llave</label>
          <input id="llave-nombre" class="input" maxlength="60" bind:value={nombre} autocomplete="off" />
          <span class="field-hint">Para reconocerla luego: «Portátil de la oficina», «Llave USB azul»…</span>
        </div>
      {/if}
      <CampoClave requerido id="llave-contrasena" etiqueta="Tu contraseña" bind:value={contrasena} autocomplete="current-password" />
      <CampoCodigo bind:value={codigo} />
      <p class="faint pequeno">El código de tu aplicación de verificación.</p>
      {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
      <footer>
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={() => (modal = null)}>Cancelar</button>
        <button class="btn {modal.tipo === 'baja' ? 'btn-danger' : 'btn-primary'}" disabled={ocupado || !listo}>
          {ocupado ? (modal.tipo === "alta" ? "Esperando a la llave…" : "Quitando…") : modal.tipo === "alta" ? "Seguir" : "Quitarla"}
        </button>
      </footer>
    </form>
  </Modal>
{/if}

<style>
  .llaves {
    list-style: none;
    margin: var(--sp-3) 0;
    padding: 0;
  }
  .llaves li {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: 8px 0;
    min-width: 0;
  }
  .llaves li + li {
    border-top: 1px solid var(--border);
  }
  .txt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow-wrap: anywhere;
  }
  .txt .faint {
    font-size: var(--fs-sm);
  }
  .acciones {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    flex-wrap: wrap;
  }
  .pequeno {
    font-size: var(--fs-sm);
    margin: 0;
  }
</style>
