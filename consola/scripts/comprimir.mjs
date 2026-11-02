// 16x (0): después de `vite build`, deja junto a cada archivo de texto de
// `build/` su versión comprimida en brotli (`.br`) y en gzip (`.gz`). Resguardo
// Server las lleva dentro y manda la que pida el navegador (`Accept-Encoding`),
// sin comprimir nada al servir (sin gastar procesador en una máquina pequeña).
// Solo si la comprimida es más pequeña (lo diminuto no gana nada).
import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";
import { fileURLToPath } from "node:url";

const raiz = fileURLToPath(new URL("../build/", import.meta.url));
const TEXTO = /\.(js|mjs|css|html|svg|json|webmanifest|txt|xml|map)$/i;
const t = { n: 0, original: 0, br: 0, gz: 0 };

function recorrer(dir) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) recorrer(p);
    else if (TEXTO.test(e.name)) comprimir(p);
  }
}

function comprimir(p) {
  const datos = fs.readFileSync(p);
  const br = zlib.brotliCompressSync(datos, {
    params: { [zlib.constants.BROTLI_PARAM_QUALITY]: 11, [zlib.constants.BROTLI_PARAM_SIZE_HINT]: datos.length, [zlib.constants.BROTLI_PARAM_MODE]: zlib.constants.BROTLI_MODE_TEXT },
  });
  const gz = zlib.gzipSync(datos, { level: 9 });
  t.n++;
  t.original += datos.length;
  if (br.length < datos.length) {
    fs.writeFileSync(`${p}.br`, br);
    t.br += br.length;
  } else t.br += datos.length;
  if (gz.length < datos.length) {
    fs.writeFileSync(`${p}.gz`, gz);
    t.gz += gz.length;
  } else t.gz += datos.length;
}

recorrer(raiz);
const kb = (n) => `${Math.round(n / 1024)} KB`;
console.log(`comprimir: ${t.n} archivos, ${kb(t.original)} → brotli ${kb(t.br)} · gzip ${kb(t.gz)}`);
