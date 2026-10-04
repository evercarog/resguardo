<script lang="ts">
  // Campo para una clave o contraseña: se puede mostrar, no se autocompleta
  // ni se guarda en el navegador, y su valor solo vive mientras el diálogo está abierto.
  import { Eye, EyeOff } from "@lucide/svelte";
  import type { Snippet } from "svelte";

  let {
    value = $bindable(""),
    etiqueta,
    ayuda,
    id,
    autocomplete = "off",
    autofocus = false,
    error = "",
    requerido = false,
    extra,
  }: {
    value?: string;
    etiqueta: string;
    ayuda?: string;
    id: string;
    autocomplete?: "off" | "current-password" | "new-password";
    autofocus?: boolean;
    error?: string;
    /** Obligatorio: se anuncia como tal (aria-required); el formulario no se envía sin él. */
    requerido?: boolean;
    extra?: Snippet;
  } = $props();
  let ver = $state(false);
</script>

<div class="field">
  <div class="label-row">
    <label class="field-label" for={id}>{etiqueta}</label>
    {@render extra?.()}
  </div>
  <div class="caja">
    <!-- svelte-ignore a11y_autofocus -->
    <input
      {id}
      class="input"
      type={ver ? "text" : "password"}
      bind:value
      {autocomplete}
      {autofocus}
      spellcheck="false"
      autocapitalize="off"
      aria-invalid={!!error}
      aria-required={requerido || undefined}
      aria-describedby={ayuda || error ? `${id}-ayuda` : undefined}
      data-1p-ignore={autocomplete === "off" ? "" : undefined}
      data-lpignore={autocomplete === "off" ? "true" : undefined}
    />
    <button type="button" class="icon-btn" aria-label={ver ? "Ocultar" : "Mostrar"} aria-pressed={ver} onclick={() => (ver = !ver)}>
      {#if ver}<EyeOff size={15} />{:else}<Eye size={15} />{/if}
    </button>
  </div>
  {#if error}<span class="error-campo" id="{id}-ayuda">{error}</span>{:else if ayuda}<span class="field-hint" id="{id}-ayuda">{ayuda}</span>{/if}
</div>

<style>
  .caja {
    position: relative;
  }
  .caja .input {
    padding-right: 38px;
  }
  .caja .icon-btn {
    position: absolute;
    top: 3px;
    right: 3px;
  }
  .input[aria-invalid="true"] {
    border-color: var(--bad);
  }
</style>
