<script lang="ts">
  // Plan 0.8.1, B1 (15bb): «Ajustes» lleva a la primera pestaña que puede ver
  // este papel («Personas y ajustes» para los propietarios). Es también la
  // dirección de antes de «Personas y ajustes»: lo que lleve detrás se conserva.
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { actual } from "$lib/estado.svelte";
  import { SECCIONES, pestanasDe } from "$lib/secciones";
  import Cargando from "$lib/componentes/Cargando.svelte";

  $effect(() => {
    const rol = actual.cliente?.rol;
    if (!rol || actual.id !== page.params.c) return;
    const p = pestanasDe(SECCIONES.find((s) => s.id === "ajustes")!, rol)[0];
    if (p) void goto(`/c/${actual.id}${p.ruta}${page.url.search}${page.url.hash}`, { replaceState: true });
  });
</script>

<Cargando />
