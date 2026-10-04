<script lang="ts">
  // Código de 6 cifras de la aplicación de verificación.
  // Con `alCompletar`, al escribir (o pegar) la sexta cifra se envía solo.
  let { value = $bindable(""), error = "", autofocus = false, alCompletar }: { value?: string; error?: string; autofocus?: boolean; alCompletar?: (codigo: string) => void } = $props();
  function escribir() {
    const cifras = value.replace(/[^\d]/g, "").slice(0, 6);
    value = cifras.replace(/^(\d{3})(\d)/, "$1 $2");
    if (cifras.length === 6) alCompletar?.(cifras);
  }
</script>

<div class="field">
  <label class="field-label" for="codigo-totp">Código de verificación</label>
  <!-- svelte-ignore a11y_autofocus -->
  <input
    id="codigo-totp"
    class="input codigo"
    bind:value
    inputmode="numeric"
    autocomplete="one-time-code"
    pattern="[0-9 ]*"
    maxlength="7"
    placeholder="123 456"
    {autofocus}
    aria-invalid={!!error}
    aria-required="true"
    aria-describedby={error ? "codigo-totp-error" : undefined}
    oninput={escribir}
  />
  {#if error}<span class="error-campo" id="codigo-totp-error" role="alert">{error}</span>{/if}
</div>

<style>
  .codigo {
    height: 44px;
    font-family: var(--mono);
    font-size: 20px;
    letter-spacing: 0.15em;
    text-align: center;
  }
  .codigo[aria-invalid="true"] {
    border-color: var(--bad);
  }
</style>
