<script lang="ts">
  import { tip } from "$lib/tooltip";
  // Mi cuenta y el servidor: contraseña, verificación en dos pasos, mis
  // notificaciones, apariencia y datos del servidor (versión, identidad y certificado).
  import Copiable from "$lib/componentes/Copiable.svelte";
  import { Check, KeyRound, Monitor, Moon, Server, ShieldCheck, Sun } from "@lucide/svelte";
  import * as api from "$lib/api";
  import { app } from "$lib/estado.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import { apariencia, aplicarApariencia, type Acento, type Densidad, type Tema } from "$lib/apariencia.svelte";
  import { aHex, deB64 } from "$lib/cripto/bytes";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import CampoClave from "$lib/componentes/CampoClave.svelte";
  import CampoCodigo from "$lib/componentes/CampoCodigo.svelte";
  import AltaTotp from "$lib/componentes/AltaTotp.svelte";
  import CodigosRecuperacion from "$lib/componentes/CodigosRecuperacion.svelte";
  import MisNotificaciones from "$lib/componentes/MisNotificaciones.svelte";
  import Modal from "$ui/componentes/Modal.svelte";
  import type { Totp } from "$lib/tipos";

  let nombre = $state(app.cuenta?.nombre ?? "");
  let guardandoNombre = $state(false);
  async function guardarNombre(e: SubmitEvent) {
    e.preventDefault();
    guardandoNombre = true;
    try {
      app.cuenta = await api.renombrarCuenta(nombre.trim());
      avisar("Nombre cambiado.");
    } catch (err) {
      avisar((err as Error).message, "bad");
    } finally {
      guardandoNombre = false;
    }
  }

  // Cambiar de aplicación de verificación y códigos de recuperación nuevos.
  let modal = $state<"totp" | "codigos" | null>(null);
  let pasoModal = $state<"pedir" | "nuevo" | "codigos">("pedir");
  let contrasenaM = $state("");
  let codigoM = $state("");
  let errorM = $state("");
  let ocupadoM = $state(false);
  let totpNuevo = $state<Totp | null>(null);
  let codigosNuevos = $state<string[]>([]);
  function abrirModal(m: "totp" | "codigos") {
    modal = m;
    pasoModal = "pedir";
    contrasenaM = codigoM = errorM = "";
    totpNuevo = null;
    codigosNuevos = [];
  }
  function cerrarModal() {
    modal = null;
    contrasenaM = codigoM = "";
  }
  async function pedirModal(e: SubmitEvent) {
    e.preventDefault();
    errorM = "";
    ocupadoM = true;
    try {
      const codigo = codigoM.replace(/\D/g, "");
      if (modal === "totp") {
        totpNuevo = (await api.totpNuevo(contrasenaM, codigo)).totp;
        pasoModal = "nuevo";
      } else {
        codigosNuevos = (await api.nuevosCodigos(contrasenaM, codigo)).codigos_recuperacion;
        pasoModal = "codigos";
      }
      contrasenaM = codigoM = "";
    } catch (err) {
      errorM = (err as Error).message;
    } finally {
      ocupadoM = false;
    }
  }
  async function confirmarTotp(codigo: string) {
    errorM = "";
    ocupadoM = true;
    try {
      app.cuenta = (await api.totpConfirmar(codigo.replace(/\D/g, ""))).cuenta;
      cerrarModal();
      avisar("Aplicación de verificación cambiada. Se han cerrado tus otras sesiones.");
    } catch (err) {
      errorM = (err as Error).message;
    } finally {
      ocupadoM = false;
    }
  }

  let actual = $state("");
  let nueva = $state("");
  let repetir = $state("");
  let ocupado = $state(false);
  let error = $state("");

  async function cambiar(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    ocupado = true;
    try {
      await api.cambiarContrasena(actual, nueva);
      actual = nueva = repetir = "";
      avisar("Contraseña cambiada. Se han cerrado tus otras sesiones.");
    } catch (err) {
      error = (err as Error).message;
    } finally {
      ocupado = false;
    }
  }

  /** La identidad del servidor en grupos de 4 (hex de la clave Ed25519), para compararla a ojo. */
  const identidad = $derived.by(() => {
    try {
      return aHex(deB64(app.servidor?.identidad ?? "")).toUpperCase().match(/.{4}/g)?.join(" ") ?? "";
    } catch {
      return app.servidor?.identidad ?? "";
    }
  });
  const TEMAS: [Tema, string, typeof Sun][] = [
    ["sistema", "Automático", Monitor],
    ["light", "Claro", Sun],
    ["dark", "Oscuro", Moon],
    ["black", "Negro", Moon],
  ];
  const DENSIDADES: [Densidad, string, string][] = [
    ["comoda", "Cómoda", "Con aire: la de siempre"],
    ["compacta", "Compacta", "Filas y tarjetas más juntas, para ver muchos equipos de un vistazo"],
  ];
  const ACENTOS: [Acento, string][] = [
    ["teal", "Verde azulado"],
    ["blue", "Azul"],
    ["indigo", "Índigo"],
    ["violet", "Violeta"],
    ["rose", "Rosa"],
    ["amber", "Ámbar"],
    ["graphite", "Grafito"],
  ];
</script>

<svelte:head><title>Mi cuenta y servidor · Resguardo Server</title></svelte:head>

<div class="page estrecha">
  <div class="page-top">
    <div>
      <h1>Mi cuenta y servidor</h1>
      <p>{app.cuenta?.nombre} · {app.cuenta?.correo}{app.cuenta?.superusuario ? " · administra el servidor" : ""}</p>
    </div>
  </div>

  <section class="card p">
    <h2 class="section-title">Tu nombre</h2>
    <form class="nombre" onsubmit={guardarNombre}>
      <input class="input" bind:value={nombre} maxlength="80" aria-label="Tu nombre" autocomplete="name" />
      <button class="btn" disabled={guardandoNombre || !nombre.trim() || nombre.trim() === app.cuenta?.nombre}>{guardandoNombre ? "Guardando…" : "Guardar"}</button>
    </form>
  </section>

  <section class="card p">
    <h2 class="section-title"><KeyRound size={16} /> Contraseña</h2>
    <form class="form" onsubmit={cambiar}>
      <CampoClave requerido id="actual" etiqueta="Contraseña actual" bind:value={actual} autocomplete="current-password" />
      <div class="fila-campos">
        <CampoClave requerido id="nueva" etiqueta="Contraseña nueva" bind:value={nueva} autocomplete="new-password" ayuda="Al menos 12 caracteres." error={nueva && nueva.length < 12 ? "Necesita al menos 12 caracteres." : ""} />
        <CampoClave requerido id="repetir" etiqueta="Repite la nueva" bind:value={repetir} autocomplete="new-password" error={repetir && repetir !== nueva ? "No coincide." : ""} />
      </div>
      {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
      <div class="fin"><button class="btn btn-primary" disabled={ocupado || !actual || nueva.length < 12 || nueva !== repetir}>{ocupado ? "Cambiando…" : "Cambiar la contraseña"}</button></div>
    </form>
  </section>

  <section class="card p">
    <h2 class="section-title"><ShieldCheck size={16} /> Verificación en dos pasos <Ayuda id="totp" /></h2>
    <p class="estado"><span class="badge tone-ok"><Check size={12} />Activa</span> Es obligatoria en este servidor. Si pierdes el móvil, entra con un código de recuperación.</p>
    <div class="acciones">
      <button class="btn btn-sm" onclick={() => abrirModal("totp")}>Cambiar de aplicación de verificación</button>
      <button class="btn btn-sm" onclick={() => abrirModal("codigos")}>Generar códigos de recuperación nuevos</button>
    </div>
  </section>

  <MisNotificaciones />

  <section class="card p">
    <h2 class="section-title">Apariencia</h2>
    <div class="field">
      <span class="field-label">Tema</span>
      <div class="segmented" role="group" aria-label="Tema">
        {#each TEMAS as [t, txt, Icono] (t)}
          <button class:on={apariencia.tema === t} aria-pressed={apariencia.tema === t} use:tip={t === "sistema" ? "Como el sistema (claro u oscuro)" : undefined} onclick={() => ((apariencia.tema = t), aplicarApariencia())}><Icono size={14} />{txt}</button>
        {/each}
      </div>
    </div>
    <div class="field acentos">
      <span class="field-label">Acento</span>
      <div class="colores" role="group" aria-label="Acento">
        {#each ACENTOS as [a, txt] (a)}
          <button class="color" data-c={a} class:on={apariencia.acento === a} aria-pressed={apariencia.acento === a} aria-label={txt} use:tip={txt} onclick={() => ((apariencia.acento = a), aplicarApariencia())}></button>
        {/each}
      </div>
    </div>
    <div class="field acentos">
      <span class="field-label" id="t-densidad">Densidad</span>
      <div class="segmented" role="group" aria-labelledby="t-densidad">
        {#each DENSIDADES as [d, txt, ayuda] (d)}
          <button class:on={apariencia.densidad === d} aria-pressed={apariencia.densidad === d} use:tip={ayuda} onclick={() => ((apariencia.densidad = d), aplicarApariencia())}>{txt}</button>
        {/each}
      </div>
      <span class="field-hint">Se recuerda en este navegador. La compacta junta las listas y las tablas; los botones no cambian de tamaño.</span>
    </div>
  </section>

  {#if app.servidor}
    <section class="card p">
      <h2 class="section-title"><Server size={16} /> Este servidor</h2>
      <dl>
        <div><dt>Versión</dt><dd>{app.servidor.nombre} {app.servidor.version}</dd></div>
        <div><dt>Identidad <Ayuda id="identidad" /></dt><dd><Copiable texto={identidad} que="la identidad" /></dd></div>
        <div><dt>Huella del certificado <Ayuda id="huella-ca" /></dt><dd><Copiable texto={app.servidor.huella_ca} que="la huella" /></dd></div>
      </dl>
      <p class="faint nota">Compáralas con las que muestra el servidor al instalarlo (<code>journalctl -u resguardo-server</code>). Si no coinciden, no escribas aquí ninguna clave.</p>
    </section>
  {/if}
</div>

{#if modal}
  <Modal labelledby="t-modal" onclose={cerrarModal} width={460} dismissible={false}>
    <div class="dlg-title">
      <span class="ticon"><ShieldCheck size={18} /></span>
      <div>
        <h2 id="t-modal">{modal === "totp" ? "Cambiar de aplicación de verificación" : "Códigos de recuperación nuevos"}</h2>
        <p>{modal === "totp" ? "La actual sigue valiendo hasta que confirmes la nueva." : "Los anteriores dejarán de valer."}</p>
      </div>
    </div>
    {#if pasoModal === "pedir"}
      <form class="form" onsubmit={pedirModal}>
        <CampoClave requerido id="m-contrasena" etiqueta="Tu contraseña" bind:value={contrasenaM} autocomplete="current-password" autofocus />
        <CampoCodigo bind:value={codigoM} />
        <p class="faint pequeno">El código de tu aplicación de verificación <strong>actual</strong>.</p>
        {#if errorM}<p class="error-campo" role="alert">{errorM}</p>{/if}
        <footer>
          <button type="button" class="btn btn-ghost" onclick={cerrarModal}>Cancelar</button>
          <button class="btn btn-primary" disabled={ocupadoM || !contrasenaM || codigoM.replace(/\D/g, "").length !== 6}>{ocupadoM ? "Comprobando…" : "Seguir"}</button>
        </footer>
      </form>
    {:else if pasoModal === "nuevo" && totpNuevo}
      <AltaTotp totp={totpNuevo} ocupado={ocupadoM} error={errorM} enviar={confirmarTotp} />
    {:else if pasoModal === "codigos"}
      <CodigosRecuperacion codigos={codigosNuevos} seguir={cerrarModal} />
    {/if}
  </Modal>
{/if}

<style>
  .nombre {
    display: flex;
    gap: 8px;
    max-width: 480px;
  }
  .acciones {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: var(--sp-4);
  }
  .pequeno {
    margin: -8px 0 0;
    font-size: var(--fs-xs);
  }
  .estrecha {
    max-width: 760px;
  }
  .section-title {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: var(--sp-4);
  }
  .fin {
    display: flex;
    justify-content: flex-end;
  }
  .estado {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .acentos {
    margin-top: var(--sp-4);
  }
  .colores {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .color {
    width: 28px;
    height: 28px;
    border: 2px solid transparent;
    border-radius: 999px;
    cursor: pointer;
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.08);
  }
  .color.on {
    border-color: var(--text-1);
  }
  .color[data-c="teal"] {
    background: #0f766e;
  }
  .color[data-c="blue"] {
    background: #2563eb;
  }
  .color[data-c="indigo"] {
    background: #4f46e5;
  }
  .color[data-c="violet"] {
    background: #7c3aed;
  }
  .color[data-c="rose"] {
    background: #d6336c;
  }
  .color[data-c="amber"] {
    background: #b45309;
  }
  .color[data-c="graphite"] {
    background: #3f3f46;
  }
  dl {
    display: grid;
    gap: var(--sp-3);
    margin: 0;
  }
  dl > div {
    display: grid;
    grid-template-columns: 200px minmax(0, 1fr);
    gap: var(--sp-3);
  }
  dt {
    display: flex;
    align-items: center;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  dd {
    margin: 0;
    min-width: 0;
  }
  dd :global(code) {
    font-size: 12px;
    word-break: break-all;
  }
  .nota {
    margin: var(--sp-4) 0 0;
    font-size: var(--fs-xs);
  }
  .segmented :global(svg) {
    vertical-align: -2px;
  }
  @media (max-width: 640px) {
    dl > div {
      grid-template-columns: 1fr;
      gap: 2px;
    }
    .segmented {
      grid-auto-flow: row;
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
