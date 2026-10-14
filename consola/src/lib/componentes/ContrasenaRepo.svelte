<script lang="ts">
  // 14ii: «Generada / La escribo yo» para la contraseña de un repositorio nuevo,
  // igual en todos los sitios donde se crea una. Las reglas, en lib/contrasenaRepo.ts.
  // El kit lo pone cada diálogo con `claveElegida(valor)`.
  import { RefreshCw } from "@lucide/svelte";
  import CampoClave from "./CampoClave.svelte";
  import { errorContrasena, generarContrasena, MIN_ESCRITA, type ContrasenaElegida } from "$lib/contrasenaRepo";

  let {
    valor = $bindable(),
    id,
    etiqueta = "Contraseña del repositorio",
    alCambiar,
  }: {
    valor: ContrasenaElegida;
    /** Prefijo de los id de los campos (el de la escrita es `${id}-propia`). */
    id: string;
    etiqueta?: string;
    /** Al cambiar de contraseña (p. ej. para desmarcar «He guardado el kit»). */
    alCambiar?: () => void;
  } = $props();

  const error = $derived(errorContrasena(valor));
  function elegir(propia: boolean) {
    if (valor.propia === propia) return;
    valor.propia = propia;
    alCambiar?.();
  }
  function otra() {
    valor.generada = generarContrasena();
    alCambiar?.();
  }
</script>

<div class="contrasena" data-contrasena-repo={id}>
  <div class="segmented" role="radiogroup" aria-label={etiqueta}>
    <button type="button" role="radio" aria-checked={!valor.propia} class:on={!valor.propia} onclick={() => elegir(false)}>Generada (recomendado)</button>
    <button type="button" role="radio" aria-checked={valor.propia} class:on={valor.propia} onclick={() => elegir(true)}>La escribo yo</button>
  </div>
  {#if valor.propia}
    <div class="fila-campos">
      <CampoClave id="{id}-propia" etiqueta="Contraseña" bind:value={valor.escrita} />
      <CampoClave id="{id}-repetida" etiqueta="Repítela" bind:value={valor.repetida} />
    </div>
    {#if error && (valor.escrita || valor.repetida)}
      <p class="error-campo" role="alert">{error}</p>
    {:else}
      <span class="field-hint">Al menos {MIN_ESCRITA} caracteres, larga y que no uses en otro sitio. La generada es más segura.</span>
    {/if}
  {:else}
    <div class="generada">
      <span class="field-hint">La crea este navegador al azar y va sellada solo para el equipo: el servidor no la ve. Sale en el kit.</span>
      <button type="button" class="btn btn-sm" onclick={otra}><RefreshCw size={14} />Generar otra</button>
    </div>
  {/if}
</div>

<style>
  .contrasena {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .generada {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    justify-content: space-between;
  }
  .error-campo {
    margin: 0;
  }
</style>
