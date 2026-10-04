// Markdown ligero para las observaciones y los comentarios (lib/notas.svelte.ts).
//
// Solo esto: **negrita**, *cursiva* o _cursiva_, `código`, enlaces
// [texto](https://…) y direcciones sueltas (http, https y mailto), listas con
// «- », «* » o «1. » y párrafos (una línea en blanco). Todo lo demás se
// enseña tal cual: el texto se escapa ANTES de añadir las etiquetas, así que
// ningún HTML que alguien escriba llega a la página. Los enlaces abren en otra
// pestaña sin `Referer` ni acceso a esta.

const ESCAPES: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };
export const escapar = (s: string) => s.replace(/[&<>"']/g, (c) => ESCAPES[c]);

/** ¿Un enlace que se puede abrir? Solo http(s) y mailto (nada de javascript:, data:…). */
export function enlaceSeguro(url: string): boolean {
  if (/[\s<>"'`]/.test(url) || url.length > 2000) return false;
  try {
    const u = new URL(url);
    return u.protocol === "https:" || u.protocol === "http:" || u.protocol === "mailto:";
  } catch {
    return false;
  }
}

const enlace = (url: string, texto: string) => `<a href="${escapar(url)}" target="_blank" rel="noopener noreferrer nofollow">${texto}</a>`;

/** Una línea (sin escapar todavía) con sus marcas en línea. */
function enLinea(t: string): string {
  // Lo que no se toca (código y enlaces) va aparte mientras se ponen las demás marcas.
  const apartados: string[] = [];
  const apartar = (html: string) => `\u0000${apartados.push(html) - 1}\u0000`;
  let s = t.replace(/\u0000/g, "");
  s = s.replace(/`([^`\n]+)`/g, (_, c: string) => apartar(`<code>${escapar(c)}</code>`));
  s = s.replace(/\[([^\]\n]+)\]\(([^)\s]+)\)/g, (todo: string, texto: string, url: string) =>
    enlaceSeguro(url) ? apartar(enlace(url, enLinea(texto))) : todo,
  );
  s = s.replace(/\b(https?:\/\/[^\s<>"'`]+[^\s<>"'`.,;:!?)\]])|\bmailto:[^\s<>"'`]+[^\s<>"'`.,;:!?)\]]/g, (url: string) =>
    enlaceSeguro(url) ? apartar(enlace(url, escapar(url))) : url,
  );
  s = escapar(s);
  s = s.replace(/\*\*([^*\n]+?)\*\*/g, "<strong>$1</strong>");
  s = s.replace(/(^|[^\w*])\*([^*\s][^*\n]*?)\*(?![\w*])/g, "$1<em>$2</em>");
  s = s.replace(/(^|[^\w])_([^_\s][^_\n]*?)_(?!\w)/g, "$1<em>$2</em>");
  return s.replace(/\u0000(\d+)\u0000/g, (_, i: string) => apartados[Number(i)]);
}

/** El texto en HTML seguro (para `{@html}`). */
export function markdown(texto: string): string {
  const lineas = texto.replace(/\r\n?/g, "\n").split("\n");
  const out: string[] = [];
  let parrafo: string[] = [];
  let lista: { tipo: "ul" | "ol"; items: string[] } | null = null;
  const cerrarParrafo = () => {
    if (parrafo.length) out.push(`<p>${parrafo.map(enLinea).join("<br>")}</p>`);
    parrafo = [];
  };
  const cerrarLista = () => {
    if (lista) out.push(`<${lista.tipo}>${lista.items.map((i) => `<li>${enLinea(i)}</li>`).join("")}</${lista.tipo}>`);
    lista = null;
  };
  for (const l of lineas) {
    const vineta = /^\s*[-*+]\s+(.*)$/.exec(l);
    const numero = /^\s*\d{1,3}[.)]\s+(.*)$/.exec(l);
    if (vineta || numero) {
      const tipo = vineta ? "ul" : "ol";
      cerrarParrafo();
      if (lista && lista.tipo !== tipo) cerrarLista();
      lista ??= { tipo, items: [] };
      lista.items.push((vineta ?? numero)![1]);
    } else if (!l.trim()) {
      cerrarParrafo();
      cerrarLista();
    } else {
      cerrarLista();
      parrafo.push(l.trim());
    }
  }
  cerrarParrafo();
  cerrarLista();
  return out.join("");
}

/** La primera línea con texto, sin marcas (como el `titulo` del servidor). */
export function primeraLinea(texto: string, max = 80): string {
  const l = (texto.split(/\r?\n/).find((x) => x.trim()) ?? "").trim();
  const sinLista = l.replace(/^[#>*+\-\s]+/, "").replace(/^\d+[.)]\s+/, "");
  const limpio = sinLista.replace(/\[([^\]]+)\]\([^)]*\)/g, "$1").replace(/[*_`]/g, "").trim();
  return limpio.length > max ? `${limpio.slice(0, max - 1).trimEnd()}…` : limpio;
}
