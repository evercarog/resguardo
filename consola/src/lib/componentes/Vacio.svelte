<script lang="ts">
  // Estado vacío: un icono de línea (o, en los momentos clave, una de las
  // ilustraciones del producto), una frase y como mucho una acción.
  import type { Component, Snippet } from "svelte";
  import Ilustracion, { type NombreIlustracion } from "$ui/componentes/Ilustracion.svelte";
  let {
    icono: Icono,
    ilustracion,
    titulo,
    texto,
    children,
  }: { icono?: Component<{ size?: number; strokeWidth?: number }>; ilustracion?: NombreIlustracion; titulo: string; texto?: string; children?: Snippet } = $props();
</script>

<div class="empty-state" class:con-ilustracion={!!ilustracion}>
  {#if ilustracion}<Ilustracion nombre={ilustracion} />{:else if Icono}<Icono size={32} strokeWidth={1.5} />{/if}
  <p><strong>{titulo}</strong>{#if texto}<span>{texto}</span>{/if}</p>
  {@render children?.()}
</div>
