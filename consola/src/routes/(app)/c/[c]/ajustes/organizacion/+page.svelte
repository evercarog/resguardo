<script lang="ts">
  // Plan 0.8.1, B2 (15a, 15y): «Organización» (antes, el diálogo «Sedes y
  // categorías» de Equipos): sedes, categorías, etiquetas y grupos de exclusiones.
  import { Building2 } from "@lucide/svelte";
  import CabeceraPagina from "$lib/componentes/CabeceraPagina.svelte";
  import { actual } from "$lib/estado.svelte";
</script>

<div class="page">
  <CabeceraPagina titulo="Organización" icono={Building2} migas={[{ texto: actual.cliente?.nombre ?? "Cliente", href: `/c/${actual.id}` }, { texto: "Ajustes" }, { texto: "Organización" }]} resumen="Sedes, categorías, etiquetas y grupos de exclusiones. Lo ven igual todas las consolas del cliente." />
</div>
