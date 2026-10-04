// El icono de la pestaña dice cómo va el cliente abierto, aunque se esté en
// otra pestaña: con avisos sin revisar, un punto rojo; con algo en marcha
// (una copia, una verificación…), un punto azul; si no, el de siempre.
// El dibujo es el de static/favicon.svg con el punto encima.

export type EstadoIcono = "normal" | "aviso" | "marcha";

const BASE =
  '<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#3d9a92"/><stop offset="1" stop-color="#0c615a"/></linearGradient></defs><rect width="64" height="64" rx="15" fill="url(#g)"/><path d="M32 11.5 16.5 17v13.2c0 10.1 6.5 18.9 15.5 22.3 9-3.4 15.5-12.2 15.5-22.3V17L32 11.5Z" fill="rgb(255 255 255 / 0.14)" stroke="#fff" stroke-width="3.2" stroke-linejoin="round"/><path d="M38.8 27.4a7.6 7.6 0 1 1-2.3-4.6" fill="none" stroke="#fff" stroke-width="3.2" stroke-linecap="round"/><path d="M37.6 18.8v5h-5" fill="none" stroke="#fff" stroke-width="3.2" stroke-linecap="round" stroke-linejoin="round"/>';

const PUNTO: Record<Exclude<EstadoIcono, "normal">, string> = {
  aviso: "#e5484d",
  marcha: "#3b82f6",
};

let actual: EstadoIcono | null = null;

/** Pone el icono de la pestaña (solo si cambia). */
export function ponerIcono(estado: EstadoIcono) {
  if (typeof document === "undefined" || estado === actual) return;
  actual = estado;
  let link = document.querySelector<HTMLLinkElement>('link[rel="icon"]');
  if (!link) {
    link = document.createElement("link");
    link.rel = "icon";
    link.type = "image/svg+xml";
    document.head.appendChild(link);
  }
  if (estado === "normal") {
    link.href = "/favicon.svg";
    return;
  }
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">${BASE}<circle cx="52" cy="12" r="11" fill="${PUNTO[estado]}" stroke="#fff" stroke-width="3.5"/></svg>`;
  link.href = `data:image/svg+xml,${encodeURIComponent(svg)}`;
}
