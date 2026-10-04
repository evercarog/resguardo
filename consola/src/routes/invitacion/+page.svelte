<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import * as api from "$lib/api";
  import { app, cargarClientes } from "$lib/estado.svelte";
  import type { Totp } from "$lib/tipos";
  import Portada from "$lib/componentes/Portada.svelte";
  import CampoClave from "$lib/componentes/CampoClave.svelte";
  import AltaTotp from "$lib/componentes/AltaTotp.svelte";
  import CodigosRecuperacion from "$lib/componentes/CodigosRecuperacion.svelte";

  // El token va en el fragmento (#…): no llega a los registros de ningún servidor.
  const token = typeof location !== "undefined" ? location.hash.slice(1) : "";
  const volverAqui = encodeURIComponent("/invitacion#" + token);
  let paso = $state<"cargando" | "con_cuenta" | "nueva" | "alta" | "codigos">("cargando");
  let nombre = $state("");
  let correo = $state("");
  let contrasena = $state("");
  let totp = $state<Totp | null>(null);
  let codigos = $state<string[]>([]);
  let ocupado = $state(false);
  let error = $state("");

  onMount(async () => {
    try {
      app.cuenta = await api.cuenta({ sinRedirigir: true });
      paso = "con_cuenta";
    } catch {
      paso = "nueva";
    }
  });

  async function aceptar(e?: SubmitEvent) {
    e?.preventDefault();
    error = "";
    ocupado = true;
    try {
      const r = await api.aceptarInvitacion(paso === "nueva" ? { token, correo: correo.trim(), nombre: nombre.trim(), contrasena } : { token });
      contrasena = "";
      if ("necesita" in r && r.necesita === "alta_totp") {
        totp = r.totp;
        paso = "alta";
      } else {
        await cargarClientes();
        await goto("/", { replaceState: true });
      }
    } catch (e) {
      error = (e as Error).message;
    } finally {
      ocupado = false;
    }
  }

  async function activar(codigo: string) {
    error = "";
    ocupado = true;
    try {
      const r = await api.segundoPaso({ codigo: codigo.replace(/\D/g, "") });
      app.cuenta = r.cuenta;
      codigos = r.codigos_recuperacion ?? [];
      if (codigos.length) paso = "codigos";
      else await goto("/", { replaceState: true });
    } catch (e) {
      error = (e as Error).message;
    } finally {
      ocupado = false;
    }
  }
</script>

<svelte:head><title>Invitación · Resguardo Server</title></svelte:head>

{#if !token}
  <Portada titulo="Enlace incompleto" sub="A este enlace de invitación le falta el final. Pide que te lo vuelvan a enviar.">
    <a class="btn btn-lg ancho" href="/entrar">Ir a entrar</a>
  </Portada>
{:else if paso === "con_cuenta"}
  <Portada titulo="Te han invitado a un cliente" sub="Te unirás con tu cuenta {app.cuenta?.correo}.">
    {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
    <button class="btn btn-primary btn-lg ancho" disabled={ocupado} onclick={() => aceptar()}>{ocupado ? "Uniéndote…" : "Aceptar la invitación"}</button>
  </Portada>
{:else if paso === "nueva"}
  <Portada titulo="Te han invitado" sub="Crea tu cuenta para ver y gestionar las copias de este cliente.">
    <form class="form" onsubmit={aceptar}>
      <div class="field">
        <label class="field-label" for="nombre">Tu nombre</label>
        <input id="nombre" class="input" bind:value={nombre} autocomplete="name" />
      </div>
      <div class="field">
        <label class="field-label" for="correo">Correo</label>
        <input id="correo" class="input" type="email" bind:value={correo} autocomplete="username" />
      </div>
      <CampoClave requerido id="contrasena" etiqueta="Contraseña" bind:value={contrasena} autocomplete="new-password" ayuda="Al menos 12 caracteres." />
      {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
      <button class="btn btn-primary btn-lg" disabled={ocupado || !nombre.trim() || !correo.includes("@") || contrasena.length < 12}>{ocupado ? "Creando…" : "Crear la cuenta"}</button>
    </form>
    {#snippet pie()}¿Ya tienes cuenta? <a href="/entrar?volver={volverAqui}">Entra con ella</a> y se abrirá de nuevo esta invitación.{/snippet}
  </Portada>
{:else if paso === "alta" && totp}
  <Portada titulo="Activa la verificación en dos pasos" sub="Es obligatoria para entrar en la consola.">
    <AltaTotp {totp} {ocupado} {error} enviar={activar} />
  </Portada>
{:else if paso === "codigos"}
  <Portada titulo="Códigos de recuperación" sub="Tu cuenta ya está lista.">
    <CodigosRecuperacion {codigos} seguir={() => goto("/", { replaceState: true })} />
  </Portada>
{/if}

<style>
  .ancho {
    width: 100%;
  }
</style>
