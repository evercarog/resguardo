#!/usr/bin/env node
// Guardia: el árbol no puede nombrar clientes, equipos, personas ni redes
// reales. Falla si algún archivo del repositorio contiene una palabra, nombre
// de equipo, dominio o IP de la lista prohibida.
//
// La lista no se publica: solo guarda el SHA-256 (hex) de cada término en
// minúsculas. El texto se parte en «palabras» (letras y números unidos por
// «.», «_» o «-», como `pc-01`, `ejemplo.com` o `192.0.2.7`) y se comprueba
// cada palabra y cada tramo seguido de sus partes (`servidor-01-copias` →
// `servidor`, `servidor-01`, `01-copias`…), así que un término también salta
// dentro de un nombre de equipo, de un dominio o como prefijo de una IP.
//
//   node scripts/sin-referencias.mjs            comprueba el árbol
//   node scripts/sin-referencias.mjs --hash x   imprime el hash de «x» (para añadirlo)
//
// Archivos: los de git (seguidos y nuevos sin ignorar), menos los generados
// (*.lock, package-lock.json), los binarios y los de más de 5 MB.

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const sha = (s) => createHash("sha256").update(s, "utf8").digest("hex");

if (process.argv[2] === "--hash") {
  for (const t of process.argv.slice(3)) console.log(`${sha(t.toLowerCase())}  ${t.toLowerCase()}`);
  process.exit(0);
}

const PROHIBIDOS = new Set([
  "529c6bf36b815f0e5613443fa531a0aad0da21ee728cef51de976629e9cb541c",
  "21ba508f1145bb0b4c4f8a6e88ea881ee73b5ccabc2edf788e674c6c851eca68",
  "5ce294791c870aa389a047fefa161bffaba4af0818e67264e9313f709b4d388d",
  "2c9f7d6ca26eb0ec9085e9b63288e34bd95cdea90910e4209763b135c482176f",
  "d53075411774b7b757a9214e36cddb999dcfb5e945dc9917de45415c1c25a696",
  "50a698fc48f653f1b050ea0cf63c36a7e16ceaaa0632a524c1fdd6445699b94d",
  "59a3644d09e432acdf0e681a3deb85243c8800a6da688bf06bfaaa76fa5f2234",
  "2e4528a1447b66810050a07b9a7863e49f3625c48eb9d78b86c731242fd522fa",
  "5e85509d0e1f8a19eb86d6f0d386cb1ea11b17bc8cf628ab31a266725badf592",
  "087e0aecf01151cf96eee240c2846736fb8e744aeaf059d48e3a5b88852c8dd9",
  "73009ef8ff173dfc7f04d3f07d7cfe3bd2ae3e135dcb8c272620bad3ac4ed71d",
  "9237419a86c5fc97ec96d1cdd86732e05de91c9fa3551a54460a93c1e31ab770",
  "0e47156238569ea4cf0b6eedb60f946b6a7567c6f65ec829d5f5f2a942089e89",
  "184ba0207a2272905a836ca5d2126b550d8d2a0187dee0132aba2276bc80bb14",
  "b0ab673e9a0cb3357175fe953f1a6f34403afe966365ad94622ee9f08c560e52",
  "9f879c29bf56c3ac2f2dfdd169cd5caf7b0e2993302425c402764ba989976146",
]);

// El usuario de GitHub del proyecto: vale en la dirección del propio
// repositorio («<usuario>/resguardo») y en el contacto de SECURITY.md; en
// cualquier otro sitio, no.
const USUARIO = "ad7307f32d47689ecf4639b2ab25b00fbc4e02fc27e9df2be22244d38d689b29";
const usuarioPermitido = (archivo, texto, fin) =>
  archivo === "SECURITY.md" || /^\/resguardo(?![\w-])/.test(texto.slice(fin, fin + 12));

const PALABRA = /[\p{L}\p{N}]+(?:[._-][\p{L}\p{N}]+)*/gu;
const MAX_PARTES = 6;
const cache = new Map();

/** El hash prohibido que contiene la palabra (ella o un tramo de sus partes), o null. */
function prohibida(palabra) {
  const p = palabra.toLowerCase();
  let r = cache.get(p);
  if (r !== undefined) return r;
  r = null;
  const partes = [...p.matchAll(/[\p{L}\p{N}]+/gu)];
  buscar: for (let i = 0; i < partes.length; i++) {
    for (let j = i; j < Math.min(partes.length, i + MAX_PARTES); j++) {
      const h = sha(p.slice(partes[i].index, partes[j].index + partes[j][0].length));
      if (PROHIBIDOS.has(h) || h === USUARIO) {
        r = h;
        break buscar;
      }
    }
  }
  cache.set(p, r);
  return r;
}

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const archivos = execFileSync("git", ["ls-files", "-z", "--cached", "--others", "--exclude-standard"], { cwd: raiz, maxBuffer: 64 << 20 })
  .toString("utf8")
  .split("\0")
  .filter((a) => a && !/(^|\/)\.claude\//.test(a) && !/(^|\/)(package-lock\.json|[^/]*\.lock)$/.test(a));

const hallazgos = [];
let leidos = 0;
for (const archivo of new Set(archivos)) {
  const ruta = path.join(raiz, archivo);
  let datos;
  try {
    if (statSync(ruta).size > 5 << 20) continue;
    datos = readFileSync(ruta);
  } catch {
    continue; // borrado y aún sin confirmar
  }
  if (datos.subarray(0, 8000).includes(0)) continue; // binario
  leidos++;
  const texto = datos.toString("utf8");
  const vistas = new Set(texto.match(PALABRA));
  if (![...vistas].some((w) => prohibida(w) !== null)) continue;
  for (const m of texto.matchAll(PALABRA)) {
    const h = prohibida(m[0]);
    if (h === null) continue;
    if (h === USUARIO && usuarioPermitido(archivo, texto, m.index + m[0].length)) continue;
    const antes = texto.slice(0, m.index);
    const linea = antes.split("\n").length;
    const col = m.index - antes.lastIndexOf("\n");
    hallazgos.push(`${archivo}:${linea}:${col}  «${m[0]}» (término ${h.slice(0, 12)}…)`);
  }
}

if (hallazgos.length) {
  console.error(`MAL  ${hallazgos.length} referencia(s) prohibida(s) en el árbol:`);
  for (const h of hallazgos) console.error(`  ${h}`);
  console.error("Cámbialas por ejemplos inventados (Ferretería Altamar, PC-01, 192.0.2.x, ejemplo.com…).");
  process.exit(1);
}
console.log(`bien  sin referencias prohibidas (${leidos} archivos, ${PROHIBIDOS.size + 1} términos)`);
