<script lang="ts">
  // Código QR (para la aplicación de verificación). Se genera aquí: el secreto no sale del navegador.
  import { encode } from "uqr";
  let { texto, tamano = 176 }: { texto: string; tamano?: number } = $props();
  const qr = $derived(encode(texto, { ecc: "M", border: 2 }));
  const d = $derived.by(() => {
    let p = "";
    qr.data.forEach((fila, y) => fila.forEach((on, x) => on && (p += `M${x} ${y}h1v1h-1z`)));
    return p;
  });
</script>

<svg width={tamano} height={tamano} viewBox="0 0 {qr.size} {qr.size}" shape-rendering="crispEdges" role="img" aria-label="Código QR para la aplicación de verificación">
  <rect width={qr.size} height={qr.size} fill="#fff" />
  <path {d} fill="#000" />
</svg>
