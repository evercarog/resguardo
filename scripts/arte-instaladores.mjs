// Imágenes de los instaladores de Windows a partir de sus SVG:
//
//   npm run arte:instaladores
//
// packaging/windows/arte/
//   cabecera.svg       → cabecera.bmp (150×57) y cabecera@2x.bmp (300×114)
//   lateral.svg        → lateral.bmp (164×314) y lateral@2x.bmp (328×628)
//   assets/logo.svg y icono-pequeno.svg → resguardo.ico (16, 20, 24, 32, 40,
//                        48, 64 y 256 px; el dibujo simplificado hasta 24 px)
//
// Las imágenes se guardan en el repositorio (makensis las necesita y así no
// hace falta resvg para compilar los instaladores); este programa las vuelve
// a hacer igual. Sin fuentes del sistema: ningún SVG lleva texto.
import { Resvg } from "@resvg/resvg-js";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const arte = path.join(root, "packaging", "windows", "arte");
const leer = (f) => fs.readFileSync(f, "utf8");

/** RGBA (sin premultiplicar) de un SVG a `ancho` píxeles. */
function pintar(svg, ancho) {
  const r = new Resvg(svg, { fitTo: { mode: "width", value: ancho }, font: { loadSystemFonts: false }, shapeRendering: 2, imageRendering: 0 });
  const img = r.render();
  return { ancho: img.width, alto: img.height, rgba: Buffer.from(img.pixels), png: img.asPng() };
}

/** BMP de 24 bits (lo que carga NSIS), sobre blanco. */
function bmp({ ancho, alto, rgba }) {
  const fila = Math.ceil((ancho * 3) / 4) * 4;
  const datos = Buffer.alloc(fila * alto);
  for (let y = 0; y < alto; y++) {
    for (let x = 0; x < ancho; x++) {
      const i = (y * ancho + x) * 4;
      const a = rgba[i + 3] / 255;
      const o = (alto - 1 - y) * fila + x * 3;
      // BGR, de abajo arriba.
      datos[o] = Math.round(rgba[i + 2] * a + 255 * (1 - a));
      datos[o + 1] = Math.round(rgba[i + 1] * a + 255 * (1 - a));
      datos[o + 2] = Math.round(rgba[i] * a + 255 * (1 - a));
    }
  }
  const cab = Buffer.alloc(54);
  cab.write("BM", 0);
  cab.writeUInt32LE(54 + datos.length, 2);
  cab.writeUInt32LE(54, 10);
  cab.writeUInt32LE(40, 14);
  cab.writeInt32LE(ancho, 18);
  cab.writeInt32LE(alto, 22);
  cab.writeUInt16LE(1, 26);
  cab.writeUInt16LE(24, 28);
  cab.writeUInt32LE(datos.length, 34);
  cab.writeInt32LE(3780, 38); // 96 ppp
  cab.writeInt32LE(3780, 42);
  return Buffer.concat([cab, datos]);
}

/** Una imagen de icono en DIB de 32 bits (BGRA de abajo arriba y la máscara AND vacía). */
function dib({ ancho, alto, rgba }) {
  const cab = Buffer.alloc(40);
  cab.writeUInt32LE(40, 0);
  cab.writeInt32LE(ancho, 4);
  cab.writeInt32LE(alto * 2, 8);
  cab.writeUInt16LE(1, 12);
  cab.writeUInt16LE(32, 14);
  const color = Buffer.alloc(ancho * alto * 4);
  for (let y = 0; y < alto; y++) {
    for (let x = 0; x < ancho; x++) {
      const i = (y * ancho + x) * 4;
      const o = ((alto - 1 - y) * ancho + x) * 4;
      color[o] = rgba[i + 2];
      color[o + 1] = rgba[i + 1];
      color[o + 2] = rgba[i];
      color[o + 3] = rgba[i + 3];
    }
  }
  const mascara = Buffer.alloc((Math.ceil(ancho / 32) * 4) * alto);
  return Buffer.concat([cab, color, mascara]);
}

/** .ico con varias medidas: DIB hasta 64 px y PNG para 256 (como hace Windows). */
function ico(imagenes) {
  const cab = Buffer.alloc(6 + 16 * imagenes.length);
  cab.writeUInt16LE(1, 2);
  cab.writeUInt16LE(imagenes.length, 4);
  let desp = cab.length;
  const datos = imagenes.map((img) => (img.ancho >= 256 ? Buffer.from(img.png) : dib(img)));
  imagenes.forEach((img, n) => {
    const e = 6 + 16 * n;
    cab.writeUInt8(img.ancho >= 256 ? 0 : img.ancho, e);
    cab.writeUInt8(img.alto >= 256 ? 0 : img.alto, e + 1);
    cab.writeUInt16LE(1, e + 4);
    cab.writeUInt16LE(32, e + 6);
    cab.writeUInt32LE(datos[n].length, e + 8);
    cab.writeUInt32LE(desp, e + 12);
    desp += datos[n].length;
  });
  return Buffer.concat([cab, ...datos]);
}

const escribir = (nombre, datos) => {
  fs.writeFileSync(path.join(arte, nombre), datos);
  console.log(`${nombre}  ${datos.length} bytes`);
};

for (const [nombre, ancho] of [
  ["cabecera", 150],
  ["lateral", 164],
]) {
  const svg = leer(path.join(arte, `${nombre}.svg`));
  escribir(`${nombre}.bmp`, bmp(pintar(svg, ancho)));
  escribir(`${nombre}@2x.bmp`, bmp(pintar(svg, ancho * 2)));
}

const logo = leer(path.join(root, "assets", "logo.svg"));
const pequeno = leer(path.join(arte, "icono-pequeno.svg"));
escribir("resguardo.ico", ico([16, 20, 24, 32, 40, 48, 64, 256].map((lado) => pintar(lado <= 24 ? pequeno : logo, lado))));
