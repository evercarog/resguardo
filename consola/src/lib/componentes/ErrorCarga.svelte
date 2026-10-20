<script lang="ts">
  // Una carga que falló (G1, plan 0.8.1): qué no se pudo cargar, el motivo y
  // «Reintentar». Para secciones y pestañas; la pantalla entera usa
  // PantallaError. Sin esto, una petición que fallaba dejaba el esqueleto de
  // carga para siempre o un «Aún no hay…» que no era verdad.
  import { RefreshCw } from "@lucide/svelte";
  import PantallaError from "./PantallaError.svelte";
  import BotonCargando from "./BotonCargando.svelte";

  let {
    que,
    error,
    reintentar,
    tarjeta = true,
  }: {
    /** Lo que no se pudo cargar, con su artículo: «los avisos», «las órdenes». */
    que: string;
    error: string;
    reintentar: () => unknown;
    /** false: sin tarjeta alrededor (si ya va dentro de una). */
    tarjeta?: boolean;
  } = $props();

  // «No se pudieron cargar las órdenes», «No se pudo cargar lo que está en marcha».
  const titulo = $derived(`${/^(los|las|tus|sus) /.test(que) ? "No se pudieron cargar" : "No se pudo cargar"} ${que}`);

  let probando = $state(false);
  async function otraVez() {
    probando = true;
    try {
      await reintentar();
    } finally {
      probando = false;
    }
  }
</script>

<div class:card={tarjeta} class="error-carga">
  <PantallaError nivel={2} ilustracion="sin-conexion" {titulo} texto={error || "Vuelve a intentarlo en un momento."}>
    <BotonCargando class="btn btn-primary" cargando={probando} textoCargando="Reintentando…" onclick={otraVez}><RefreshCw size={15} />Reintentar</BotonCargando>
  </PantallaError>
</div>

<style>
  .error-carga :global(.pantalla-error) {
    padding-block: var(--sp-8);
  }
  .error-carga :global(.pantalla-error h2) {
    font-size: var(--fs-h2);
    line-height: 22px;
  }
</style>
