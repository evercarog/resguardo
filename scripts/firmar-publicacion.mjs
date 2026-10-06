#!/usr/bin/env node
// Hace y firma el manifiesto de una versión del agente (docs/actualizaciones.md,
// pasos en docs/publicar.md). Se ejecuta en el equipo de quien publica, con la
// llave privada FUERA DE LÍNEA (un USB, un equipo sin red…).
//
//   node scripts/firmar-publicacion.mjs <carpeta> [opciones]
//
// <carpeta>: la de la versión, con lo que se publica:
//   Resguardo-Agente_<versión>_x64-setup.exe          (Windows)
//   resguardo-agente-x86_64-linux-musl.tar.gz          (Linux x86_64)
//   resguardo-agente-aarch64-linux-musl.tar.gz         (Linux arm64)
// (al menos uno). Deja en ella manifiesto-agente.json y su firma
// manifiesto-agente.json.minisig.
//
// Opciones:
//   --version X.Y.Z        la versión (por defecto, la de crates/agente/Cargo.toml)
//   --llave RUTA           la llave PRIVADA de minisign (por defecto, la de minisign:
//                          ~/.minisign/minisign.key). Este script no la lee: se la pasa a minisign.
//   --minisign RUTA        el programa minisign (por defecto, el del PATH)
//   --notas "TEXTO"        notas cortas de la versión (salen en la consola)
//   --minimo-desde X.Y.Z   los equipos con una versión anterior no se actualizan solos a esta
//   --revocar ID           revoca una llave (su id de 16 hex); se puede repetir
//   --sin-url              sin la dirección de GitHub de cada archivo
//   --tambien-paquetes     firma también cada .tar.gz (lo usa instalar-agente.sh a mano)
//
// La contraseña de la llave la pide minisign en la terminal: este script nunca la ve,
// ni la guarda, ni la pasa en la línea de órdenes.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { comprobarFirma, leerLlaves } from "./lib/minisign.mjs";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const REPO = "evercarog/resguardo";
const MANIFIESTO = "manifiesto-agente.json";
const FIRMA = `${MANIFIESTO}.minisig`;

function fallo(m) {
  console.error(`\nError: ${m}\n`);
  process.exit(1);
}

// ---------- Argumentos ----------
const args = process.argv.slice(2);
if (!args.length || args.includes("--ayuda") || args.includes("-h")) {
  const lineas = fs.readFileSync(fileURLToPath(import.meta.url), "utf8").split(/\r?\n/).slice(1);
  console.log(lineas.slice(0, lineas.findIndex((l) => !l.startsWith("//"))).map((l) => l.replace(/^\/\/ ?/, "")).join("\n"));
  process.exit(args.length ? 0 : 2);
}
const carpeta = path.resolve(args[0]);
const opcion = (n) => {
  const i = args.indexOf(n);
  return i >= 0 ? args[i + 1] : undefined;
};
const todas = (n) => args.flatMap((a, i) => (a === n ? [args[i + 1]] : []));
const VERSION_RE = /^(0|[1-9]\d{0,8})\.(0|[1-9]\d{0,8})\.(0|[1-9]\d{0,8})(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$/;
const version = opcion("--version") ?? /^version\s*=\s*"([^"]+)"/m.exec(fs.readFileSync(path.join(raiz, "crates", "agente", "Cargo.toml"), "utf8"))[1];
if (!VERSION_RE.test(version)) fallo(`versión no válida: ${version}`);
const minimo = opcion("--minimo-desde");
if (minimo && !VERSION_RE.test(minimo)) fallo(`--minimo-desde no es una versión: ${minimo}`);
const revocadas = todas("--revocar").map((r) => String(r).toUpperCase());
if (revocadas.some((r) => !/^[0-9A-F]{16}$/.test(r))) fallo("--revocar espera el id de una llave (16 cifras hexadecimales).");
const notas = opcion("--notas");
if (notas && notas.length > 4000) fallo("las notas son demasiado largas (4000 caracteres como mucho).");
if (!fs.existsSync(carpeta) || !fs.statSync(carpeta).isDirectory()) fallo(`no existe la carpeta ${carpeta}`);

// ---------- La llave pública fijada ----------
const pub = fs.readFileSync(path.join(raiz, "packaging", "llave-publicacion.pub"), "utf8");
let llaves;
try {
  llaves = leerLlaves(pub);
} catch (e) {
  fallo(`packaging/llave-publicacion.pub: ${e.message}`);
}
if (!llaves.length) fallo("packaging/llave-publicacion.pub no tiene ninguna llave pública (es el marcador de posición). Pon antes la tuya (docs/publicar.md).");

// ---------- Los archivos ----------
const PLATAFORMAS = [
  { plataforma: "windows-x86_64", tipo: "instalador-nsis", nombre: `Resguardo-Agente_${version}_x64-setup.exe` },
  { plataforma: "linux-x86_64", tipo: "tar.gz", nombre: "resguardo-agente-x86_64-linux-musl.tar.gz" },
  { plataforma: "linux-aarch64", tipo: "tar.gz", nombre: "resguardo-agente-aarch64-linux-musl.tar.gz" },
];
const sha256 = (f) => createHash("sha256").update(fs.readFileSync(f)).digest("hex");
const archivos = [];
for (const p of PLATAFORMAS) {
  const ruta = path.join(carpeta, p.nombre);
  if (!fs.existsSync(ruta)) {
    console.log(`(sin ${p.nombre}: no se publica para ${p.plataforma})`);
    continue;
  }
  const tamano = fs.statSync(ruta).size;
  if (tamano === 0 || tamano > 512 * 1024 * 1024) fallo(`${p.nombre}: tamaño no válido (${tamano} bytes).`);
  const a = { plataforma: p.plataforma, tipo: p.tipo, nombre: p.nombre, sha256: sha256(ruta), tamano };
  if (!args.includes("--sin-url")) a.url = `https://github.com/${REPO}/releases/download/v${version}/${p.nombre}`;
  archivos.push(a);
}
if (!archivos.length) fallo(`no hay ningún archivo de la versión ${version} en ${carpeta} (ver --ayuda).`);
// Otro instalador de Windows con otra versión en el nombre: casi seguro un error.
const otros = fs.readdirSync(carpeta).filter((n) => /^Resguardo-Agente_.*_x64-setup\.exe$/.test(n) && n !== PLATAFORMAS[0].nombre);
if (otros.length) fallo(`en la carpeta hay instaladores de otra versión (${otros.join(", ")}): deja solo los de la ${version}.`);

const manifiesto = {
  formato: 1,
  producto: "resguardo-agente",
  version,
  fecha: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
  canal: "estable",
  ...(minimo ? { minimo_desde: minimo } : {}),
  ...(notas ? { notas } : {}),
  ...(revocadas.length ? { revocadas } : {}),
  archivos,
};
const texto = JSON.stringify(manifiesto, null, 2) + "\n";
if (Buffer.byteLength(texto) > 64 * 1024) fallo("el manifiesto pasa de 64 KB.");
const rutaManifiesto = path.join(carpeta, MANIFIESTO);
const rutaFirma = path.join(carpeta, FIRMA);
fs.writeFileSync(rutaManifiesto, texto);
fs.rmSync(rutaFirma, { force: true });
console.log(`\nManifiesto de la versión ${version}:\n${texto}`);

// ---------- minisign ----------
function buscarMinisign() {
  const dada = opcion("--minisign");
  if (dada) return dada;
  const nombres = process.platform === "win32" ? ["minisign.exe"] : ["minisign"];
  for (const d of (process.env.PATH ?? "").split(path.delimiter)) {
    for (const n of nombres) if (d && fs.existsSync(path.join(d, n))) return path.join(d, n);
  }
  fallo("no encuentro minisign. Instálalo (docs/publicar.md: winget, scoop o el zip oficial) o di dónde está con --minisign.");
}
const minisign = buscarMinisign();
const ver = spawnSync(minisign, ["-v"], { encoding: "utf8" });
console.log(`minisign: ${(ver.stdout || ver.stderr || "").trim() || minisign}`);

function firmar(archivo, comentario) {
  // Prehash (BLAKE2b), el formato de minisign ≥ 0.10: -H lo fuerza también en versiones anteriores.
  const a = ["-S", "-H", "-m", archivo, "-t", comentario];
  const llave = opcion("--llave");
  if (llave) a.push("-s", llave);
  console.log(`\nFirmando ${path.basename(archivo)} (minisign te pide la contraseña de la llave)...`);
  const r = spawnSync(minisign, a, { stdio: "inherit" });
  if (r.status !== 0) fallo(`minisign no firmó ${path.basename(archivo)} (código ${r.status}).`);
}
firmar(rutaManifiesto, `resguardo-agente ${version}`);

// ---------- Comprobar la firma (lo mismo que hará el agente) ----------
const firma = fs.readFileSync(rutaFirma, "utf8");
const { error } = comprobarFirma(fs.readFileSync(rutaManifiesto), firma, llaves, revocadas);
if (error) fallo(`la firma no vale: ${error}. No publiques esta carpeta.`);
// Y con el propio minisign y la llave pública del repositorio.
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "llave-"));
let comprobada = false;
for (const k of llaves) {
  const p = path.join(tmp, "llave.pub");
  fs.writeFileSync(p, `untrusted comment: minisign public key ${k.id}\n${k.b64}\n`);
  if (spawnSync(minisign, ["-V", "-q", "-p", p, "-m", rutaManifiesto], { stdio: "ignore" }).status === 0) comprobada = true;
}
fs.rmSync(tmp, { recursive: true, force: true });
if (!comprobada) fallo("minisign -V no acepta la firma con la llave de packaging/llave-publicacion.pub.");
console.log("\nFirma comprobada (con minisign y como la comprueba el agente).");

if (args.includes("--tambien-paquetes")) {
  for (const a of archivos.filter((x) => x.tipo === "tar.gz")) firmar(path.join(carpeta, a.nombre), `${a.nombre} ${version}`);
}

console.log(`
Siguiente paso (docs/publicar.md):
  1. Sube a la publicación v${version} de GitHub: ${archivos.map((a) => a.nombre).join(", ")},
     ${MANIFIESTO} y ${FIRMA}.
  2. En cada Resguardo Server (o en la consola: Servidor → Actualizaciones de los agentes):
       sudo resguardo-server poner-publicacion "<esta carpeta>"
`);
