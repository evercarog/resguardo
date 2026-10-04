<script lang="ts">
  // Un campo de clave con «ver» (sin autocompletar del navegador: es la clave del equipo).
  import { Eye, EyeOff } from "@lucide/svelte";
  let { etiqueta, valor = $bindable(""), nueva = false }: { etiqueta: string; valor?: string; nueva?: boolean } = $props();
  let ver = $state(false);
  const id = $props.id();
</script>

<div class="field">
  <label class="field-label" for={id}>{etiqueta}</label>
  <div class="campo">
    <input
      {id}
      class="input"
      type={ver ? "text" : "password"}
      bind:value={valor}
      autocomplete={nueva ? "new-password" : "current-password"}
      spellcheck="false"
      autocapitalize="off"
    />
    <button type="button" class="icon-btn" aria-label={ver ? "Ocultar" : "Ver"} aria-pressed={ver} onclick={() => (ver = !ver)}>
      {#if ver}<EyeOff size={16} aria-hidden="true" />{:else}<Eye size={16} aria-hidden="true" />{/if}
    </button>
  </div>
</div>

<style>
  .campo {
    position: relative;
    display: flex;
    align-items: center;
  }
  .campo .input {
    padding-right: 40px;
  }
  .campo .icon-btn {
    position: absolute;
    right: 4px;
  }
</style>
