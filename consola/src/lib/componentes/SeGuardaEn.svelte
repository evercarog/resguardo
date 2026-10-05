<script lang="ts" module>
  import { CircleHelp, Cloud, Globe, HardDrive, Network, Server, TriangleAlert, Usb } from "@lucide/svelte";
  import type { ClaseLugar } from "$lib/dondeGuarda";
  /** Un icono por cada clase de sitio (el estado nunca va solo con color: también el texto). */
  export const ICONO_LUGAR: Record<ClaseLugar, typeof Server> = {
    almacen: Server,
    almacen_propio: Server,
    carpeta: HardDrive,
    usb: Usb,
    red: Network,
    nube: Cloud,
    servidor: Globe,
    desconocido: CircleHelp,
  };
</script>

<script lang="ts">
  // «Se guarda en: Almacén SERVIDOR-ALTAMAR (otro equipo) · D:\Copias»: una línea, con el
  // icono de cada clase de sitio. Si se queda en el mismo equipo que protege
  // (sin nada fuera), en el color de aviso, con su icono y «mismo equipo».
  import { tip } from "$lib/tooltip";
  import type { Lugar } from "$lib/dondeGuarda";

  let {
    lugar,
    riesgo = false,
    pequeno = false,
    etiqueta = "Se guarda en:",
  }: {
    lugar: Lugar;
    /** Se queda en el mismo equipo sin nada fuera: en tono de aviso. */
    riesgo?: boolean;
    /** En filas y tarjetas: más pequeño. */
    pequeno?: boolean;
    etiqueta?: string;
  } = $props();

  const Icono = $derived(riesgo ? TriangleAlert : ICONO_LUGAR[lugar.clase]);
</script>

<span class="se-guarda" class:riesgo class:pequeno use:tip={riesgo ? "Si este equipo se daña o lo cifra un ransomware, se pierden los archivos y sus copias a la vez." : null}>
  <span class="ic" aria-hidden="true"><Icono size={pequeno ? 13 : 15} /></span>
  <span class="txt">
    {#if etiqueta}<span class="et">{etiqueta}</span>{" "}{/if}<strong>{lugar.texto}</strong>{#if lugar.detalle}<span class="det">{" · "}{lugar.detalle}</span>{/if}{#if riesgo}<span class="sr-only"> (aviso: en el mismo equipo que protege)</span>{/if}
  </span>
</span>

<style>
  .se-guarda {
    display: inline-flex;
    align-items: flex-start;
    gap: 6px;
    min-width: 0;
    font-size: var(--fs-sm);
    line-height: 1.45;
    color: var(--text-2);
  }
  .se-guarda.pequeno {
    gap: 5px;
    font-size: var(--fs-xs);
  }
  .ic {
    display: inline-flex;
    flex: none;
    margin-top: 2px;
    color: var(--text-3);
  }
  .pequeno .ic {
    margin-top: 1px;
  }
  .txt {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .et {
    color: var(--text-3);
  }
  strong {
    font-weight: 600;
    color: var(--text-1);
  }
  .det {
    color: var(--text-3);
  }
  .riesgo .ic,
  .riesgo strong {
    color: var(--warn);
  }
</style>
