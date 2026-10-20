<script lang="ts">
  // Plan 0.8.1, B1 (15bb): las pestañas de una sección del cliente (Recuperar,
  // Actividad, Ajustes) encima de su contenido, que va en el panel. Las saca de
  // `lib/secciones.ts`, con lo que puede ver este papel.
  import type { Snippet } from "svelte";
  import { page } from "$app/state";
  import Pestanas, { type Pestana } from "$ui/componentes/Pestanas.svelte";
  import { actual } from "$lib/estado.svelte";
  import { SECCIONES, pestanaActiva, pestanasDe, rutaEnCliente, type SeccionDef } from "$lib/secciones";
  import { cuentaPestana } from "$lib/seccionesCuentas.svelte";

  let { seccion, children }: { seccion: SeccionDef["id"]; children: Snippet } = $props();

  const def = $derived(SECCIONES.find((s) => s.id === seccion)!);
  const c = $derived(actual.id || page.params.c || "");
  const ruta = $derived(rutaEnCliente(page.url.pathname, c) ?? "");
  const ps = $derived(pestanasDe(def, actual.cliente?.rol));
  const activa = $derived(pestanaActiva(ps, ruta));
  const pestanas = $derived<Pestana[]>(
    ps.map((p, i) => {
      const n = cuentaPestana(c, p.ruta);
      return { href: `/c/${c}${p.ruta}`, texto: p.texto, icono: p.icono, cuenta: n?.cuenta, tono: n?.tono, activa: i === activa };
    }),
  );
  const prefijo = $derived(`pestana-${seccion}`);
</script>

{#if pestanas.length > 1}
  <div class="tira">
    <Pestanas {pestanas} etiqueta={def.texto} idPanel="panel-{seccion}" {prefijo} />
  </div>
  <div class="panel" id="panel-{seccion}" role="tabpanel" aria-labelledby="{prefijo}-{Math.max(0, activa)}">
    {@render children()}
  </div>
{:else}
  {@render children()}
{/if}

<style>
  .tira {
    max-width: var(--content-max);
    margin: calc(-1 * var(--sp-2)) auto var(--sp-6);
  }
  .panel > :global(.page) {
    animation: entrar-vista var(--dur-slow) var(--ease-out) backwards;
  }
  /* 0.8.1 (H5): «Informes» tiene pestañas y se imprime: la tira no sale en el papel. */
  @media print {
    .tira {
      display: none;
    }
  }
  @media (max-width: 640px) {
    .tira {
      margin-top: 0;
      margin-bottom: var(--sp-4);
    }
  }
</style>
