<script lang="ts">
  import { goto } from "$app/navigation";
  import * as api from "$lib/api";
  import { app } from "$lib/estado.svelte";
  import type { Totp } from "$lib/tipos";
  import Portada from "$lib/componentes/Portada.svelte";
  import CampoClave from "$lib/componentes/CampoClave.svelte";
  import AltaTotp from "$lib/componentes/AltaTotp.svelte";
  import CodigosRecuperacion from "$lib/componentes/CodigosRecuperacion.svelte";

  let paso = $state<"cuenta" | "alta" | "codigos">("cuenta");
  let codigoArranque = $state("");
  let nombre = $state("");
  let correo = $state("");
  let contrasena = $state("");
  let repetir = $state("");
  let totp = $state<Totp | null>(null);
  let codigos = $state<string[]>([]);
  let ocupado = $state(false);
  let error = $state("");

  const corta = $derived(contrasena.length > 0 && contrasena.length < 12);
  const distinta = $derived(repetir.length > 0 && repetir !== contrasena);
  const valido = $derived(!!codigoArranque.trim() && !!nombre.trim() && correo.includes("@") && contrasena.length >= 12 && repetir === contrasena);

  async function crear(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    ocupado = true;
    try {
      const r = await api.primerArranque({ codigo_arranque: codigoArranque.trim(), correo: correo.trim(), nombre: nombre.trim(), contrasena });
      contrasena = repetir = "";
      totp = r.totp;
      paso = "alta";
      if (app.servidor) app.servidor.inicializado = true;
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
      else await goto("/clientes?nuevo=1", { replaceState: true });
    } catch (e) {
      error = (e as Error).message;
    } finally {
      ocupado = false;
    }
  }
</script>

<svelte:head><title>Primer arranque · Resguardo Server</title></svelte:head>

{#if paso === "cuenta"}
  <Portada titulo="Bienvenido a Resguardo Server" sub="Crea la cuenta que administrará este servidor. Solo se hace una vez.">
    <form class="form" onsubmit={crear}>
      <div class="field">
        <label class="field-label" for="arranque">Código de primer arranque</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input id="arranque" class="input mono" bind:value={codigoArranque} autocomplete="off" spellcheck="false" autofocus aria-required="true" />
        <span class="field-hint">Lo muestra el instalador al terminar y el registro del servidor al arrancar sin cuentas. En Windows también está en <code>C:\ProgramData\Resguardo Server\codigo-arranque.txt</code> (solo administradores) o, en una consola de administrador, <code>resguardo-server.exe codigo-inicial</code> (en <code>C:\Program Files\Resguardo Server</code>). En Linux: <code>sudo resguardo-server codigo-inicial</code> o <code>journalctl -u resguardo-server</code>; en Docker: <code>docker logs</code>.</span>
      </div>
      <div class="field">
        <label class="field-label" for="nombre">Tu nombre</label>
        <input id="nombre" class="input" bind:value={nombre} autocomplete="name" aria-required="true" />
      </div>
      <div class="field">
        <label class="field-label" for="correo">Correo</label>
        <input id="correo" class="input" type="email" bind:value={correo} autocomplete="username" aria-required="true" />
      </div>
      <CampoClave
        requerido
        id="contrasena"
        etiqueta="Contraseña"
        bind:value={contrasena}
        autocomplete="new-password"
        ayuda="Al menos 12 caracteres. Una frase larga es más fácil de recordar."
        error={corta ? "Necesita al menos 12 caracteres." : ""}
      />
      <CampoClave requerido id="repetir" etiqueta="Repite la contraseña" bind:value={repetir} autocomplete="new-password" error={distinta ? "No coincide." : ""} />
      {#if error}<p class="error-campo" role="alert">{error}</p>{/if}
      <button class="btn btn-primary btn-lg" disabled={ocupado || !valido}>{ocupado ? "Creando…" : "Crear la cuenta"}</button>
    </form>
  </Portada>
{:else if paso === "alta" && totp}
  <Portada titulo="Activa la verificación en dos pasos" sub="Es obligatoria para todas las cuentas del servidor.">
    <AltaTotp {totp} {ocupado} {error} enviar={activar} />
  </Portada>
{:else}
  <Portada titulo="Códigos de recuperación" sub="Tu cuenta ya está lista.">
    <CodigosRecuperacion {codigos} seguir={() => goto("/clientes?nuevo=1", { replaceState: true })} />
  </Portada>
{/if}
