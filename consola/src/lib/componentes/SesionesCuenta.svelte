<script lang="ts">
  // 16k: los dispositivos con sesión abierta en la consola, con «Cerrar» y «Cerrar en
  // todos». Cerrar otras pide la contraseña y el código, y una sesión de menos de 24 h
  // no puede (el servidor lo comprueba; aquí solo se explica). Para el propietario del
  // servidor, los plazos de las sesiones.
  import { untrack } from "svelte";
  import { LogOut, Monitor, Smartphone, Timer } from "@lucide/svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import CampoClave from "./CampoClave.svelte";
  import CampoCodigo from "./CampoCodigo.svelte";
  import * as api from "$lib/api";
  import { app } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import type { PlazosSesion, SesionCuenta } from "$lib/tipos";
  import { conTopes, esMovil, frasePlazos, lineaSesion, motivoNoCerrar, ordenarSesiones } from "$lib/sesionesCuenta";

  let lista = $state<SesionCuenta[] | null>(null);
  let puede = $state(true);
  let desde = $state<string | null>(null);
  let fallo = $state("");
  function cargar() {
    api
      .sesionesCuenta()
      .then((r) => {
        lista = ordenarSesiones(r.sesiones);
        puede = r.puede_cerrar;
        desde = r.puede_desde;
        fallo = "";
      })
      .catch((e: unknown) => {
        lista = [];
        fallo = e instanceof api.ApiError && e.estado === 404 ? "Este servidor aún no enseña las sesiones: actualiza Resguardo Server." : (e as Error).message;
      });
  }
  $effect(() => untrack(cargar));

  const otras = $derived((lista ?? []).filter((s) => !s.actual));
  const motivo = $derived(motivoNoCerrar(puede, desde));

  let modal = $state<{ tipo: "una"; s: SesionCuenta } | { tipo: "todas" } | null>(null);
  let contrasena = $state("");
  let codigo = $state("");
  let ocupado = $state(false);
  let error = $state("");
  function abrir(m: NonNullable<typeof modal>) {
    modal = m;
    contrasena = codigo = error = "";
  }
  async function enviar(e: SubmitEvent) {
    e.preventDefault();
    if (!modal) return;
    error = "";
    ocupado = true;
    try {
      const c = codigo.replace(/\D/g, "");
      const r = await api.cerrarSesiones(modal.tipo === "todas" ? { otras: true, contrasena, codigo: c } : { ids: [modal.s.id], contrasena, codigo: c });
      avisar(r.cerradas === 1 ? "Sesión cerrada." : `${r.cerradas} sesiones cerradas.`);
      modal = null;
      cargar();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
      contrasena = codigo = "";
    }
  }
  const listo = $derived(!!contrasena && codigo.replace(/\D/g, "").length === 6);

  // Plazos (propietario del servidor).
  let plazos = $state<PlazosSesion | null>(null);
  let guardandoP = $state(false);
  function cargarPlazos() {
    if (!app.cuenta?.superusuario) return;
    api
      .plazosSesion()
      .then((r) => (plazos = r.plazos))
      .catch(() => (plazos = null));
  }
  $effect(() => untrack(cargarPlazos));
  async function guardarPlazos(e: SubmitEvent) {
    e.preventDefault();
    if (!plazos) return;
    guardandoP = true;
    try {
      plazos = (await api.ponerPlazosSesion(conTopes(plazos))).plazos;
      avisar("Plazos guardados: valen para las sesiones nuevas y para el tiempo sin usarla de las abiertas.");
    } catch (err) {
      avisar((err as Error).message, "bad");
    } finally {
      guardandoP = false;
    }
  }
</script>

<section class="card p" aria-labelledby="t-sesiones" data-testid="sesiones-cuenta">
  <h2 class="section-title" id="t-sesiones"><Monitor size={16} /> Dónde tienes la sesión abierta</h2>
  <p class="faint">Cada móvil u ordenador donde entraste. Si ves uno que no reconoces, ciérralo y cambia la contraseña.</p>
  {#if fallo}
    <div class="notice notice-warn"><p>{fallo}</p></div>
  {:else if lista === null}
    <p class="faint">Cargando…</p>
  {:else}
    <ul class="sesiones">
      {#each lista as s (s.id)}
        <li>
          <span class="ic" aria-hidden="true">
            {#if esMovil(s.navegador)}<Smartphone size={16} />{:else}<Monitor size={16} />{/if}
          </span>
          <span class="txt">
            <strong>{s.navegador}{#if s.actual}<span class="badge tone-info">Esta</span>{/if}{#if s.nueva && !s.actual}<span class="badge tone-warn">Nueva</span>{/if}</strong>
            <span class="faint">{lineaSesion(s)}</span>
          </span>
          {#if !s.actual}
            <button class="btn btn-ghost btn-sm" disabled={!puede} aria-label="Cerrar la sesión de {s.navegador}" onclick={() => abrir({ tipo: "una", s })}><LogOut size={14} />Cerrar</button>
          {/if}
        </li>
      {/each}
    </ul>
    {#if motivo && otras.length}
      <div class="notice notice-info"><p>{motivo}</p></div>
    {/if}
    {#if otras.length}
      <div class="acciones">
        <button class="btn btn-sm" disabled={!puede} onclick={() => abrir({ tipo: "todas" })}><LogOut size={14} />Cerrar en todos los demás</button>
      </div>
    {/if}
  {/if}

  {#if plazos}
    <form class="plazos" onsubmit={guardarPlazos}>
      <h3><Timer size={14} /> Plazos de las sesiones en este servidor</h3>
      <p class="faint">{frasePlazos(conTopes(plazos))} Solo lo cambia el propietario del servidor.</p>
      <div class="filas">
        <label>Sin usarla <input class="input" type="number" min="1" max="24" bind:value={plazos.inactividad_horas} /> h</label>
        <label>Como mucho <input class="input" type="number" min="1" max="168" bind:value={plazos.maximo_horas} /> h</label>
        <label>«Recordar» <input class="input" type="number" min="0" max="30" bind:value={plazos.recordar_dias} /> días</label>
      </div>
      <button class="btn btn-sm" disabled={guardandoP}>{guardandoP ? "Guardando…" : "Guardar los plazos"}</button>
    </form>
  {/if}
</section>

{#if modal}
  <Modal labelledby="t-cerrar-sesion" onclose={() => (modal = null)} width={460} dismissible={false} {ocupado}>
    <div class="dlg-title">
      <span class="ticon"><LogOut size={18} /></span>
      <div>
        <h2 id="t-cerrar-sesion">{modal.tipo === "todas" ? "Cerrar en todos los demás" : `Cerrar la sesión de ${modal.s.navegador}`}</h2>
        <p>{modal.tipo === "todas" ? `Se cierran ${otras.length === 1 ? "la otra sesión" : `las otras ${otras.length} sesiones`}; esta sigue abierta.` : "Ese dispositivo tendrá que volver a entrar."}</p>
      </div>
    </div>
    <form class="form" onsubmit={enviar}>
      <CampoClave requerido id="ses-contrasena" etiqueta="Tu contraseña" bind:value={contrasena} autocomplete="current-password" />
      <CampoCodigo bind:value={codigo} />
      <p class="faint pequeno">El código de tu aplicación de verificación.</p>
      {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
      <footer>
        <button type="button" class="btn btn-ghost" disabled={ocupado} onclick={() => (modal = null)}>Cancelar</button>
        <button class="btn btn-danger" disabled={ocupado || !listo}>{ocupado ? "Cerrando…" : "Cerrar"}</button>
      </footer>
    </form>
  </Modal>
{/if}

<style>
  .sesiones {
    list-style: none;
    margin: var(--sp-3) 0;
    padding: 0;
  }
  .sesiones li {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: 8px 0;
    min-width: 0;
  }
  .sesiones li + li {
    border-top: 1px solid var(--border);
  }
  .txt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow-wrap: anywhere;
  }
  .txt strong {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .txt .faint {
    font-size: var(--fs-sm);
  }
  .acciones {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .plazos {
    margin-top: var(--sp-4);
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
  }
  .plazos h3 {
    margin: 0;
    font-size: var(--fs-body);
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .plazos .faint {
    margin: 0;
  }
  .filas {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }
  .filas label {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .filas .input {
    width: 5.5rem;
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
