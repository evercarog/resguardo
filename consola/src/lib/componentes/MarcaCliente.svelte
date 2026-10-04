<script lang="ts">
  // La marca de un cliente en pequeño (docs/diseno.md §4): su logo sobre una
  // placa clara (se lee igual en claro y en oscuro) o, sin logo, su inicial
  // en su acento. Decorativa: el nombre va siempre al lado, en texto.
  import type { MarcaCliente } from "$lib/tipos";
  import { iniciales } from "$lib/marca";

  let { nombre, marca, tam = 26 }: { nombre: string; marca?: MarcaCliente | null; tam?: number } = $props();
  let fallo = $state(false);
  // Si cambia el logo, se vuelve a intentar.
  $effect(() => {
    void marca?.logo;
    fallo = false;
  });
  const letras = $derived(tam >= 36 ? iniciales(nombre) : iniciales(nombre).slice(0, 1));
</script>

<span class="marca-cliente mc" class:con-logo={marca?.logo && !fallo} data-acento={marca?.acento ?? undefined} style:--tam="{tam}px" aria-hidden="true">
  {#if marca?.logo && !fallo}
    <img src={marca.logo} alt="" loading="lazy" decoding="async" onerror={() => (fallo = true)} />
  {:else}
    {letras}
  {/if}
</span>

<style>
  .mc {
    display: grid;
    place-items: center;
    flex: none;
    width: var(--tam);
    height: var(--tam);
    overflow: hidden;
    font-size: calc(var(--tam) * 0.44);
    font-weight: 650;
    line-height: 1;
    letter-spacing: -0.02em;
    color: var(--marca-texto);
    background: var(--marca-suave);
    border-radius: calc(var(--tam) * 0.27);
  }
  .con-logo {
    padding: calc(var(--tam) * 0.08);
    background: #fff;
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.08);
  }
  img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
</style>
