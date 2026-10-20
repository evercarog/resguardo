// 0.8.1 (H5, H2): un PDF de texto, hecho en el navegador y sin bibliotecas.
//
// El informe de la configuración de copias lleva las carpetas, las exclusiones y los
// volcados de cada copia, que solo se ven con la clave de administración (van
// cifrados en el servidor: el servidor nunca ve rutas). Por eso el PDF se arma aquí,
// en el navegador, y no en el servidor como el informe ejecutivo.
//
// Lo justo para un informe de texto: A4, Helvetica (normal y negrita, de las 14 que
// trae todo lector de PDF, sin incrustar fuentes), títulos, párrafos, listas, pares
// «clave: valor», saltos de página y un pie con «Página N de M». El texto va en
// WinAnsi (el alfabeto latino con tildes, ñ, «», €…); lo que no cabe ahí sale como
// «?». Lo prueban los vectores (scripts/vectores-informe-config.ts).

export type Bloque =
  | { t: "titulo"; texto: string }
  | { t: "h1"; texto: string }
  | { t: "h2"; texto: string }
  | { t: "p"; texto: string; tenue?: boolean }
  | { t: "lista"; items: string[] }
  | { t: "kv"; filas: [string, string][] }
  | { t: "salto" };

export interface DocumentoPdf {
  titulo: string;
  /** Lo que va en el pie de cada página (además de «Página N de M»). */
  pie: string;
  bloques: Bloque[];
}

// Anchos de Helvetica (milésimas del tamaño), de la tabla de Adobe, para 32–126.
const ANCHOS = [
  278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667, 611, 778, 722,
  278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333,
  500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

/** Los caracteres de WinAnsi por encima de 127 que no coinciden con Latin-1. */
const WIN: Record<string, number> = {
  "€": 0x80,
  "‚": 0x82,
  "„": 0x84,
  "…": 0x85,
  "‘": 0x91,
  "’": 0x92,
  "“": 0x93,
  "”": 0x94,
  "•": 0x95,
  "–": 0x96,
  "—": 0x97,
  "™": 0x99,
};

/** El byte de un carácter en WinAnsi (o «?»). Los espacios raros, espacio normal. */
export function byteWin(c: string): number {
  if (c === "\u00a0" || c === "\u202f" || c === "\t") return 32;
  if (c === "→") return 0x3e; // «>»: la flecha no está en WinAnsi
  const w = WIN[c];
  if (w) return w;
  const n = c.codePointAt(0) ?? 63;
  if ((n >= 32 && n <= 126) || (n >= 0xa0 && n <= 0xff)) return n;
  return 63;
}

/** El ancho de una letra (milésimas), por su byte de WinAnsi. Las de fuera de ASCII, como su letra base. */
function anchoByte(b: number): number {
  if (b >= 32 && b <= 126) return ANCHOS[b - 32];
  if (b === 0x97) return 1000;
  if (b === 0x80 || b === 0x85 || b === 0xab || b === 0xbb) return 556;
  if (b === 0xb7 || b === 0x91 || b === 0x92) return 278;
  if (b >= 0xc0 && b <= 0xff) return b >= 0xe0 ? 556 : 700;
  return 556;
}

/** El ancho de un texto en puntos. */
export function anchoTexto(t: string, tam: number, negrita = false): number {
  let w = 0;
  for (const c of t) w += anchoByte(byteWin(c));
  return (w * tam * (negrita ? 1.06 : 1)) / 1000;
}

/** Parte un texto en líneas que caben en `ancho` (por espacios; una palabra muy larga, por letras). */
export function partir(t: string, ancho: number, tam: number, negrita = false): string[] {
  const out: string[] = [];
  for (const parrafo of t.replace(/\r\n?/g, "\n").split("\n")) {
    let linea = "";
    for (const palabra0 of parrafo.split(/ +/)) {
      let palabra = palabra0;
      const prueba = linea ? `${linea} ${palabra}` : palabra;
      if (anchoTexto(prueba, tam, negrita) <= ancho) {
        linea = prueba;
        continue;
      }
      if (linea) out.push(linea);
      linea = "";
      while (anchoTexto(palabra, tam, negrita) > ancho && palabra.length > 1) {
        let n = palabra.length - 1;
        while (n > 1 && anchoTexto(palabra.slice(0, n), tam, negrita) > ancho) n--;
        out.push(palabra.slice(0, n));
        palabra = palabra.slice(n);
      }
      linea = palabra;
    }
    out.push(linea);
  }
  return out;
}

/** Un texto entre paréntesis para el PDF (escapado y en WinAnsi, con los bytes de más de 127 en octal). */
function cadena(t: string): string {
  let s = "(";
  for (const c of t) {
    const b = byteWin(c);
    if (b === 0x28 || b === 0x29 || b === 0x5c) s += `\\${String.fromCharCode(b)}`;
    else if (b < 32 || b > 126) s += `\\${b.toString(8).padStart(3, "0")}`;
    else s += String.fromCharCode(b);
  }
  return `${s})`;
}

const ANCHO_PAG = 595.28;
const ALTO_PAG = 841.89;
const MARGEN = 50;
const UTIL = ANCHO_PAG - 2 * MARGEN;
const ABAJO = 60;

interface Linea {
  x: number;
  y: number;
  tam: number;
  negrita: boolean;
  texto: string;
  gris?: boolean;
}

/** Coloca los bloques en páginas (sin el pie). */
export function maquetar(bloques: Bloque[]): Linea[][] {
  const paginas: Linea[][] = [[]];
  let y = ALTO_PAG - MARGEN;
  const nueva = () => {
    paginas.push([]);
    y = ALTO_PAG - MARGEN;
  };
  const poner = (texto: string, tam: number, negrita: boolean, x = MARGEN, interlinea = tam * 1.35, gris = false) => {
    if (y - interlinea < ABAJO) nueva();
    y -= interlinea;
    paginas[paginas.length - 1].push({ x, y, tam, negrita, texto, gris });
  };
  const hueco = (h: number) => {
    if (paginas[paginas.length - 1].length) y -= h;
  };
  for (const b of bloques) {
    switch (b.t) {
      case "titulo":
        for (const l of partir(b.texto, UTIL, 18, true)) poner(l, 18, true);
        hueco(6);
        break;
      case "h1":
        // Que un título no se quede solo al final de una página.
        if (y - 60 < ABAJO) nueva();
        hueco(12);
        for (const l of partir(b.texto, UTIL, 14, true)) poner(l, 14, true);
        hueco(4);
        break;
      case "h2":
        if (y - 50 < ABAJO) nueva();
        hueco(8);
        for (const l of partir(b.texto, UTIL, 11.5, true)) poner(l, 11.5, true);
        hueco(2);
        break;
      case "p":
        for (const l of partir(b.texto, UTIL, 9.5)) poner(l, 9.5, false, MARGEN, 13, b.tenue);
        hueco(4);
        break;
      case "lista":
        for (const it of b.items) {
          const ls = partir(it, UTIL - 12, 9.5);
          ls.forEach((l, i) => poner(i === 0 ? `• ${l}` : l, 9.5, false, i === 0 ? MARGEN : MARGEN + 9, 13));
        }
        hueco(4);
        break;
      case "kv": {
        const col = 150;
        for (const [k, v] of b.filas) {
          const ks = partir(k, col - 8, 9, true);
          const vs = partir(v || "—", UTIL - col, 9);
          const n = Math.max(ks.length, vs.length);
          if (y - n * 12.5 < ABAJO && n * 12.5 < ALTO_PAG - 2 * ABAJO) nueva();
          for (let i = 0; i < n; i++) {
            if (y - 12.5 < ABAJO) nueva();
            y -= 12.5;
            const pag = paginas[paginas.length - 1];
            if (ks[i]) pag.push({ x: MARGEN, y, tam: 9, negrita: true, texto: ks[i], gris: true });
            if (vs[i]) pag.push({ x: MARGEN + col, y, tam: 9, negrita: false, texto: vs[i] });
          }
          y -= 2;
        }
        hueco(4);
        break;
      }
      case "salto":
        if (paginas[paginas.length - 1].length) nueva();
        break;
    }
  }
  return paginas;
}

/** El PDF entero (bytes). */
export function pdfDe(doc: DocumentoPdf): Uint8Array {
  const paginas = maquetar(doc.bloques);
  const total = paginas.length;
  const objetos: string[] = [];
  // 1: catálogo, 2: páginas, 3: Helvetica, 4: Helvetica-Bold, 5: información; después, cada página y su contenido.
  const kids = paginas.map((_, i) => `${6 + i * 2} 0 R`).join(" ");
  objetos[1] = "<< /Type /Catalog /Pages 2 0 R >>";
  objetos[2] = `<< /Type /Pages /Kids [${kids}] /Count ${total} >>`;
  objetos[3] = "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>";
  objetos[4] = "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>";
  objetos[5] = `<< /Title ${cadena(doc.titulo)} /Producer (Resguardo) >>`;
  const contenidos: string[] = [];
  paginas.forEach((ls, i) => {
    let c = "";
    for (const l of ls) {
      c += `${l.gris ? "0.3 0.3 0.3 rg" : "0 0 0 rg"} BT /${l.negrita ? "F2" : "F1"} ${l.tam} Tf ${l.x.toFixed(2)} ${l.y.toFixed(2)} Td ${cadena(l.texto)} Tj ET\n`;
    }
    const pie = `${doc.pie} · Página ${i + 1} de ${total}`;
    c += `0.45 0.45 0.45 rg BT /F1 8 Tf ${MARGEN} 32 Td ${cadena(pie)} Tj ET\n`;
    c += `0.8 0.8 0.8 RG 0.5 w ${MARGEN} 44 m ${ANCHO_PAG - MARGEN} 44 l S\n`;
    contenidos.push(c);
    objetos[6 + i * 2] = `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${ANCHO_PAG} ${ALTO_PAG}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents ${7 + i * 2} 0 R >>`;
  });
  // Todo lo que se escribe es ASCII (los bytes altos van en octal): longitud = caracteres.
  const bytesCab = 15; // «%PDF-1.4\n» (9) + «%» + 4 bytes + «\n» (con las cuatro letras de un byte cada una)
  const pos: number[] = [];
  let largo = bytesCab;
  const trozos: string[] = [];
  const escribir = (t: string) => {
    trozos.push(t);
    largo += t.length;
  };
  const n = 5 + total * 2;
  for (let i = 1; i <= n; i++) {
    pos[i] = largo;
    if (i >= 6 && i % 2 === 1) {
      const c = contenidos[(i - 7) / 2];
      escribir(`${i} 0 obj\n<< /Length ${c.length} >>\nstream\n${c}endstream\nendobj\n`);
    } else {
      escribir(`${i} 0 obj\n${objetos[i]}\nendobj\n`);
    }
  }
  const xref = largo;
  let tabla = `xref\n0 ${n + 1}\n0000000000 65535 f \n`;
  for (let i = 1; i <= n; i++) tabla += `${String(pos[i]).padStart(10, "0")} 00000 n \n`;
  escribir(`${tabla}trailer\n<< /Size ${n + 1} /Root 1 0 R /Info 5 0 R >>\nstartxref\n${xref}\n%%EOF\n`);
  const cuerpo = trozos.join("");
  const out = new Uint8Array(bytesCab + cuerpo.length);
  // Cabecera con cuatro bytes altos (dice a quien lo lea que es binario).
  out.set([0x25, 0x50, 0x44, 0x46, 0x2d, 0x31, 0x2e, 0x34, 0x0a, 0x25, 0xe2, 0xe3, 0xcf, 0xd3, 0x0a]);
  for (let i = 0; i < cuerpo.length; i++) out[bytesCab + i] = cuerpo.charCodeAt(i);
  return out;
}
