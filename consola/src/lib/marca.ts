// Marca del cliente (v1.32): el logo se convierte SIEMPRE a PNG en este
// navegador antes de mandarlo. Así ningún SVG (con sus posibles scripts,
// manejadores o referencias externas) llega al servidor ni a otros
// navegadores: se dibuja como imagen (un <img> no ejecuta nada ni carga
// recursos de fuera) en un lienzo, y se guarda lo pintado.

/** Lo que se acepta al elegir el archivo. */
export const TIPOS_LOGO = "image/png,image/jpeg,image/webp,image/svg+xml";
/** Tamaño máximo del archivo elegido y del PNG que se manda. */
export const MAX_ARCHIVO = 1024 * 1024;
export const MAX_PNG = 200 * 1024;

function leerComoDataUrl(f: Blob): Promise<string> {
  return new Promise((ok, mal) => {
    const r = new FileReader();
    r.onload = () => ok(String(r.result));
    r.onerror = () => mal(new Error("No se pudo leer el archivo."));
    r.readAsDataURL(f);
  });
}

function aBase64(b: Blob): Promise<string> {
  return leerComoDataUrl(b).then((u) => u.slice(u.indexOf(",") + 1));
}

/**
 * Convierte la imagen elegida en un PNG de como mucho 512 px de lado (y
 * 200 KB; si no cabe, prueba más pequeño). Devuelve el PNG en base64 y una
 * URL para la vista previa. Errores con un texto para el usuario.
 */
export async function logoAPng(f: File): Promise<{ base64: string; vista: string; ancho: number; alto: number }> {
  if (!TIPOS_LOGO.split(",").includes(f.type)) throw new Error("El logo tiene que ser una imagen PNG, JPG, WebP o SVG.");
  if (f.size > MAX_ARCHIVO) throw new Error("La imagen pesa demasiado: como mucho 1 MB.");
  // data: (la CSP admite `img-src 'self' data:`, no `blob:`).
  const url = await leerComoDataUrl(f);
  const img = new Image();
  img.decoding = "async";
  img.src = url;
  try {
    await img.decode();
  } catch {
    throw new Error("No se pudo abrir la imagen. Prueba con un PNG.");
  }
  // Un SVG sin tamaño propio se dibuja a 512 px.
  const w0 = img.naturalWidth || 512;
  const h0 = img.naturalHeight || 512;
  for (const lado of [512, 384, 256, 160]) {
    const k = Math.min(1, lado / Math.max(w0, h0));
    const ancho = Math.max(1, Math.round(w0 * k));
    const alto = Math.max(1, Math.round(h0 * k));
    const lienzo = document.createElement("canvas");
    lienzo.width = ancho;
    lienzo.height = alto;
    const ctx = lienzo.getContext("2d");
    if (!ctx) throw new Error("Este navegador no puede preparar el logo.");
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(img, 0, 0, ancho, alto);
    let png: Blob | null;
    try {
      png = await new Promise<Blob | null>((ok) => lienzo.toBlob(ok, "image/png"));
    } catch {
      throw new Error("No se pudo convertir esa imagen. Prueba con un PNG.");
    }
    if (!png) throw new Error("No se pudo convertir esa imagen. Prueba con un PNG.");
    if (png.size <= MAX_PNG) {
      const base64 = await aBase64(png);
      return { base64, vista: `data:image/png;base64,${base64}`, ancho, alto };
    }
  }
  throw new Error("El logo sigue pesando más de 200 KB al reducirlo. Prueba con una imagen más sencilla.");
}

/** Un `data:` URL como archivo (sin `fetch`: la CSP no deja pedir `data:`). */
export function dataUrlAArchivo(url: string, nombre = "logo"): File {
  const m = /^data:([^;,]+)(;base64)?,(.*)$/s.exec(url);
  if (!m) throw new Error("La imagen guardada no es válida.");
  const datos = m[2] ? Uint8Array.from(atob(m[3]), (c) => c.charCodeAt(0)) : new TextEncoder().encode(decodeURIComponent(m[3]));
  return new File([datos], nombre, { type: m[1] });
}

/** Iniciales para el monograma (sin logo): «Ferretería Altamar» → «FA». */
export function iniciales(nombre: string): string {
  const ps = nombre
    .trim()
    .split(/\s+/)
    .filter((p) => /[\p{L}\p{N}]/u.test(p));
  return ((ps[0]?.[0] ?? "·") + (ps.length > 1 ? (ps[1]?.[0] ?? "") : "")).toUpperCase();
}
