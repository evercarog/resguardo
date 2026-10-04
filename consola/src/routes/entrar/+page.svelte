<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import * as api from "$lib/api";
  import { app } from "$lib/estado.svelte";
  import type { Restablecida, RespuestaTotp, Totp } from "$lib/tipos";
  import { fechaLarga } from "$lib/formato";
  import Portada from "$lib/componentes/Portada.svelte";
  import CampoClave from "$lib/componentes/CampoClave.svelte";
  import CampoCodigo from "$lib/componentes/CampoCodigo.svelte";
  import AltaTotp from "$lib/componentes/AltaTotp.svelte";
  import CodigosRecuperacion from "$lib/componentes/CodigosRecuperacion.svelte";
  import Ayuda from "$lib/componentes/Ayuda.svelte";
  import { ArrowLeft, KeyRound, LifeBuoy, ShieldAlert, Smartphone } from "@lucide/svelte";

  type Paso = "credenciales" | "totp" | "recuperacion" | "restablecimiento" | "alta" | "codigos";
  let paso = $state<Paso>("credenciales");
  let correo = $state("");
  let contrasena = $state("");
  let codigo = $state("");
  let recuperacion = $state("");
  let totp = $state<Totp | null>(null);
  let codigos = $state<string[]>([]);
  let ocupado = $state(false);
  let error = $state("");
  // v1.27: un propietario restableció la verificación en dos pasos de esta cuenta.
  let restablecida = $state<Restablecida | null>(null);
  let caducado = $state(false);
  let codigoRest = $state("");

  const volver = $derived.by(() => {
    const v = page.url.searchParams.get("volver");
    return v && v.startsWith("/") && !v.startsWith("//") ? v : "/";
  });
  const simulado = import.meta.env.MODE === "mock";

  async function credenciales(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    ocupado = true;
    try {
      const r = await api.entrar(correo.trim(), contrasena);
      contrasena = "";
      if (r.necesita === "alta_totp") {
        totp = r.totp;
        paso = "alta";
      } else if (r.necesita === "restablecimiento") {
        restablecida = r.restablecida;
        caducado = r.caducado;
        paso = "restablecimiento";
      } else paso = "totp";
    } catch (e) {
      error = (e as Error).message;
    } finally {
      ocupado = false;
    }
  }

  async function usarCodigo(e: SubmitEvent) {
    e.preventDefault();
    if (ocupado) return;
    error = "";
    ocupado = true;
    try {
      const r = await api.usarRestablecimiento(codigoRest.trim());
      if (r.necesita === "alta_totp") {
        totp = r.totp;
        restablecida = r.restablecida ?? restablecida;
        codigoRest = "";
        paso = "alta";
      }
    } catch (e) {
      error = (e as Error).message;
    } finally {
      ocupado = false;
    }
  }

  async function segundo(b: { codigo: string } | { recuperacion: string }) {
    if (ocupado) return;
    error = "";
    ocupado = true;
    try {
      const r: RespuestaTotp = await api.segundoPaso(b);
      app.cuenta = r.cuenta;
      if (r.restablecida) restablecida = r.restablecida;
      if (r.codigos_recuperacion?.length) {
        codigos = r.codigos_recuperacion;
        paso = "codigos";
      } else await goto(volver, { replaceState: true });
    } catch (e) {
      error = (e as Error).message;
      codigo = "";
    } finally {
      ocupado = false;
    }
  }
</script>

<svelte:head><title>Entrar · Resguardo Server</title></svelte:head>

{#if paso === "credenciales"}
  <Portada titulo="Entrar" sub="Gestiona las copias de los equipos de tus clientes.">
    <form class="form" onsubmit={credenciales}>
      <div class="field">
        <label class="field-label" for="correo">Correo</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input id="correo" class="input" type="email" bind:value={correo} autocomplete="username" required autofocus placeholder="tu@empresa.com" />
      </div>
      <CampoClave requerido id="contrasena" etiqueta="Contraseña" bind:value={contrasena} autocomplete="current-password" />
      {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
      <button class="btn btn-primary btn-lg" disabled={ocupado || !correo || !contrasena}>{ocupado ? "Entrando…" : "Seguir"}</button>
    </form>
    {#if simulado}
      <p class="simulado faint">Servidor simulado: ana@ejemplo.com · resguardo · cualquier código de 6 cifras.</p>
    {/if}
  </Portada>
{:else if paso === "totp"}
  <Portada titulo="Verificación en dos pasos" sub="Un segundo paso para que nadie entre solo con tu contraseña.">
    <div class="paso-dos">
      <span class="ic-movil" aria-hidden="true"><Smartphone size={22} /></span>
      <ol class="instrucciones">
        <li>Abre la aplicación de verificación del móvil (Google Authenticator, Microsoft Authenticator, 1Password…).</li>
        <li>Busca <strong>Resguardo</strong>{correo ? ` (${correo.trim()})` : ""}.</li>
        <li>Escribe aquí las 6 cifras que enseña ahora. Cambian cada 30 segundos: si está a punto de cambiar, espera a la siguiente.</li>
      </ol>
    </div>
    <form
      class="form"
      onsubmit={(e) => {
        e.preventDefault();
        segundo({ codigo: codigo.replace(/\D/g, "") });
      }}
    >
      <CampoCodigo bind:value={codigo} {error} autofocus alCompletar={(c) => segundo({ codigo: c })} />
      <button class="btn btn-primary btn-lg" disabled={ocupado || codigo.replace(/\D/g, "").length !== 6}>{ocupado ? "Comprobando…" : "Entrar"}</button>
    </form>
    <details class="perdido">
      <summary><LifeBuoy size={15} />¿Perdiste el teléfono o cambiaste de móvil?</summary>
      <div class="perdido-cuerpo">
        <p>Entra con uno de los <strong>códigos de recuperación</strong> que guardaste al activar la verificación (parecen <code>k7m2-q9x4</code>). Cada uno sirve una vez.</p>
        <button type="button" class="btn btn-sm" onclick={() => ((paso = "recuperacion"), (error = ""))}><KeyRound size={14} />Usar un código de recuperación</button>
        <p class="faint">Ya dentro, en «Mi cuenta y servidor», podrás vincular el móvil nuevo y sacar códigos nuevos.</p>
        <p class="faint">¿Tampoco tienes los códigos? <strong>Pide al propietario que la restablezca</strong> (en Personas): te dará un código de un solo uso para vincular el móvil nuevo al entrar.</p>
      </div>
    </details>
    {#snippet pie()}<button type="button" class="link volver" onclick={() => ((paso = "credenciales"), (codigo = ""), (error = ""))}><ArrowLeft size={14} />Entrar con otra cuenta</button>{/snippet}
  </Portada>
{:else if paso === "recuperacion"}
  <Portada titulo="Código de recuperación" sub="Escribe uno de los códigos que guardaste al activar la verificación. Cada uno sirve una vez.">
    <form
      class="form"
      onsubmit={(e) => {
        e.preventDefault();
        segundo({ recuperacion: recuperacion.trim() });
      }}
    >
      <div class="field">
        <label class="field-label" for="rec">Código de recuperación</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input id="rec" class="input mono" bind:value={recuperacion} autocomplete="off" placeholder="xxxx-xxxx" autofocus aria-required="true" aria-invalid={!!error} aria-describedby={error ? "rec-error" : undefined} />
        {#if error}<span class="error-campo" id="rec-error" role="alert">{error}</span>{/if}
      </div>
      <button class="btn btn-primary btn-lg" disabled={ocupado || !recuperacion.trim()}>{ocupado ? "Comprobando…" : "Entrar"}</button>
      <button type="button" class="link centro" onclick={() => ((paso = "totp"), (error = ""))}><ArrowLeft size={14} />Volver al código de la aplicación</button>
    </form>
  </Portada>
{:else if paso === "restablecimiento" && restablecida}
  <Portada titulo="Vincula tu móvil otra vez" sub="Escribe el código que te dio quien restableció tu verificación en dos pasos.">
    {@render aviso(restablecida)}
    {#if caducado}
      <p class="error-campo" role="alert">Ese código ya caducó (dura 24 horas). Pide a {restablecida.por} que la restablezca otra vez.</p>
    {:else}
      <form class="form" onsubmit={usarCodigo}>
        <div class="field">
          <label class="field-label" for="codigo-rest">Código de un solo uso</label>
          <!-- svelte-ignore a11y_autofocus -->
          <input id="codigo-rest" class="input mono" bind:value={codigoRest} autocomplete="off" autocapitalize="characters" spellcheck="false" placeholder="XXXX-XXXX-XXXX-XXXX" autofocus aria-required="true" aria-invalid={!!error} aria-describedby={error ? "codigo-rest-error" : undefined} />
          {#if error}<span class="error-campo" id="codigo-rest-error" role="alert">{error}</span>{/if}
        </div>
        <button class="btn btn-primary btn-lg" disabled={ocupado || codigoRest.replace(/[^A-Za-z0-9]/g, "").length < 16}>{ocupado ? "Comprobando…" : "Seguir"}</button>
      </form>
    {/if}
    <p class="faint pequeno">¿No lo tienes? Pídeselo a {restablecida.por}: sin él, nadie puede vincular otro móvil a tu cuenta aunque sepa tu contraseña.</p>
    {#snippet pie()}<button type="button" class="link volver" onclick={() => ((paso = "credenciales"), (codigoRest = ""), (error = ""))}><ArrowLeft size={14} />Entrar con otra cuenta</button>{/snippet}
  </Portada>
{:else if paso === "alta" && totp}
  <Portada titulo="Activa la verificación en dos pasos" sub="Es obligatoria: protege la consola aunque alguien sepa tu contraseña.">
    {#if restablecida}{@render aviso(restablecida)}{/if}
    <AltaTotp {totp} {ocupado} {error} enviar={(c) => segundo({ codigo: c.replace(/\D/g, "") })} />
    {#snippet pie()}<span class="tip">¿Qué es esto? <Ayuda id="totp" /></span>{/snippet}
  </Portada>
{:else if paso === "codigos"}
  <Portada titulo="Códigos de recuperación" sub="La verificación en dos pasos ya está activa.">
    {#if restablecida}{@render aviso(restablecida)}{/if}
    <CodigosRecuperacion {codigos} seguir={() => goto(volver, { replaceState: true })} />
  </Portada>
{/if}

{#snippet aviso(r: Restablecida)}
  <p class="aviso-rest" role="status">
    <ShieldAlert size={16} aria-hidden="true" />
    <span>Tu verificación en dos pasos se restableció el {fechaLarga(r.cuando)} por {r.por}. Si no lo pediste tú, avísale cuanto antes.</span>
  </p>
{/snippet}

<style>
  .aviso-rest {
    display: flex;
    gap: var(--sp-2);
    align-items: flex-start;
    margin: 0 0 var(--sp-4);
    padding: var(--sp-3);
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
    color: var(--text-1);
    background: var(--warn-soft);
    border-radius: var(--radius);
  }
  .aviso-rest :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--warn);
  }
  .pequeno {
    margin: var(--sp-4) 0 0;
    font-size: var(--fs-xs);
  }
  .centro {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    align-self: center;
  }
  .paso-dos {
    display: flex;
    gap: var(--sp-3);
    margin-bottom: var(--sp-5);
    padding: var(--sp-4);
    background: var(--surface-2);
    border-radius: var(--radius-lg);
  }
  .ic-movil {
    display: grid;
    flex: none;
    place-items: center;
    width: 40px;
    height: 40px;
    color: var(--accent-text);
    background: var(--accent-soft);
    border-radius: var(--radius);
  }
  .instrucciones {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    padding-left: 18px;
    font-size: var(--fs-sm);
    line-height: var(--lh-sm);
    color: var(--text-2);
  }
  .instrucciones strong {
    color: var(--text-1);
  }
  .perdido {
    margin-top: var(--sp-5);
    border-top: 1px solid var(--border);
    padding-top: var(--sp-4);
  }
  .perdido summary {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
    font-weight: 500;
    color: var(--accent-text);
    cursor: pointer;
    list-style: none;
  }
  .perdido summary::-webkit-details-marker {
    display: none;
  }
  .perdido-cuerpo {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-3);
    margin-top: var(--sp-3);
    font-size: var(--fs-sm);
    color: var(--text-2);
    animation: rise var(--dur) var(--ease-out) both;
  }
  .perdido-cuerpo p {
    margin: 0;
  }
  .perdido-cuerpo .faint {
    font-size: var(--fs-xs);
  }
  .volver {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .simulado {
    margin: var(--sp-5) 0 0;
    font-size: var(--fs-xs);
    text-align: center;
  }
  .tip {
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }
</style>
