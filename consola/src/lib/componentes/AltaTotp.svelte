<script lang="ts">
  // Alta de la verificación en dos pasos: QR (generado aquí), la clave por si
  // no se puede escanear y el primer código.
  import { Copy } from "@lucide/svelte";
  import Qr from "./Qr.svelte";
  import CampoCodigo from "./CampoCodigo.svelte";
  import { avisar } from "$lib/avisos.svelte";
  import type { Totp } from "$lib/tipos";

  let { totp, enviar, ocupado = false, error = "" }: { totp: Totp; enviar: (codigo: string) => void; ocupado?: boolean; error?: string } = $props();
  let codigo = $state("");
  const agrupado = $derived(totp.secreto.replace(/(.{4})/g, "$1 ").trim());
</script>

<div class="alta">
  <ol class="pasos">
    <li>Abre tu aplicación de verificación (Google Authenticator, Microsoft Authenticator, 1Password, Aegis…).</li>
    <li>Escanea el código o escribe la clave.</li>
    <li>Escribe el código de 6 cifras que te muestra.</li>
  </ol>
  <div class="qr">
    <Qr texto={totp.uri} />
    <div class="clave">
      <span class="faint">Clave</span>
      <code class="selectable">{agrupado}</code>
      <button
        type="button"
        class="btn btn-sm btn-ghost"
        onclick={async () => {
          await navigator.clipboard.writeText(totp.secreto);
          avisar("Clave copiada.");
        }}><Copy size={14} />Copiar</button
      >
    </div>
  </div>
  <form
    class="form"
    onsubmit={(e) => {
      e.preventDefault();
      enviar(codigo);
    }}
  >
    <CampoCodigo bind:value={codigo} {error} autofocus />
    <button class="btn btn-primary btn-lg" disabled={ocupado || codigo.replace(/\D/g, "").length !== 6}>{ocupado ? "Comprobando…" : "Activar y entrar"}</button>
  </form>
</div>

<style>
  .alta {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
  }
  .pasos {
    margin: 0;
    padding-left: 20px;
    font-size: var(--fs-sm);
    color: var(--text-2);
    line-height: 1.6;
  }
  .qr {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-4);
  }
  .qr :global(svg) {
    border-radius: var(--radius);
    border: 1px solid var(--border);
  }
  .clave {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    min-width: 0;
    font-size: var(--fs-sm);
  }
  .clave code {
    font-size: 13px;
    word-break: break-all;
  }
</style>
