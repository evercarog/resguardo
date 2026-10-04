// Compila resguardo-agente.exe y su instalador «Instalar Resguardo Agente».
//
//   npm run build:agente      solo el instalador del agente
//   npm run build:todo        el del agente y, después, el de la app (que lo
//                             incluye para «Guardar el instalador del agente…»)
//
// Deja el instalador en src-tauri/target/release/bundle/nsis (junto al de la
// app) y una copia en src-tauri/resources/agente para incluirla en la app. La
// huella SHA-256 de ese instalador se fija en la app al compilarla aquí
// (RESGUARDO_AGENT_INSTALLER_SHA256), que la comprueba antes de guardarlo.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const tauri = path.join(root, "src-tauri");
// La versión del agente es la de su crate (la plataforma, 0.7.x), no la de la app de escritorio.
const cargoAgente = fs.readFileSync(path.join(root, "crates", "agente", "Cargo.toml"), "utf8");
const version = /^version\s*=\s*"([^"]+)"/m.exec(cargoAgente)[1];
const sha256 = (file) => createHash("sha256").update(fs.readFileSync(file)).digest("hex");
const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { stdio: "inherit", ...opts });

// 1. El programa.
run("cargo", ["build", "--release", "-p", "resguardo-agente", "--bin", "resguardo-agente"], { cwd: tauri });
const agentExe = path.join(tauri, "target", "release", "resguardo-agente.exe");
const restic = path.join(tauri, "binaries", "restic-x86_64-pc-windows-msvc.exe");
for (const f of [agentExe, restic]) if (!fs.existsSync(f)) throw new Error(`Falta ${f}`);
// rest-server (opcional: «Este equipo guarda copias»), el mismo que lleva la app (scripts/fetch-rest-server.ps1).
const restServer = path.join(tauri, "binaries", "rest-server-x86_64-pc-windows-msvc.exe");
const restServerLicense = path.join(tauri, "licenses", "rest-server-LICENSE.txt");
// rclone (opcional: espejo del Servidor de copias en una nube; scripts/fetch-rclone.ps1).
const rclone = path.join(tauri, "binaries", "rclone-x86_64-pc-windows-msvc.exe");
const rcloneLicense = path.join(tauri, "licenses", "rclone-LICENSE.txt");

// 2. El instalador (makensis: el que descarga Tauri, o el del PATH).
const candidates = [path.join(process.env.LOCALAPPDATA ?? path.join(os.homedir(), "AppData", "Local"), "tauri", "NSIS", "makensis.exe"), "makensis"];
const makensis = candidates.find((c) => c === "makensis" || fs.existsSync(c));
const outDir = path.join(tauri, "target", "release", "bundle", "nsis");
fs.mkdirSync(outDir, { recursive: true });
const out = path.join(outDir, `Resguardo-Agente_${version}_x64-setup.exe`);
run(makensis, [
  "/V2",
  "/INPUTCHARSET",
  "UTF8",
  `/DVERSION=${version}`,
  `/DAGENT_EXE=${agentExe}`,
  `/DAGENT_SHA256=${sha256(agentExe)}`,
  `/DRESTIC_EXE=${restic}`,
  `/DRESTIC_LICENSE=${path.join(tauri, "licenses", "restic-LICENSE.txt")}`,
  ...(fs.existsSync(restServer) ? [`/DREST_SERVER_EXE=${restServer}`] : []),
  ...(fs.existsSync(restServerLicense) ? [`/DREST_SERVER_LICENSE=${restServerLicense}`] : []),
  ...(fs.existsSync(rclone) ? [`/DRCLONE_EXE=${rclone}`] : []),
  ...(fs.existsSync(rcloneLicense) ? [`/DRCLONE_LICENSE=${rcloneLicense}`] : []),
  `/DOUT_FILE=${out}`,
  path.join(root, "packaging", "windows", "agente.nsi"),
]);

// 3. Copia para la app y su huella.
const resDir = path.join(tauri, "resources", "agente");
fs.mkdirSync(resDir, { recursive: true });
const bundled = path.join(resDir, "Resguardo-Agente-setup.exe");
fs.copyFileSync(out, bundled);
const hash = sha256(bundled);
console.log(`\nInstalador del agente: ${out}\nSHA-256: ${hash}`);

// 4. Con --tauri, el instalador de la app (con la huella fijada).
if (process.argv.includes("--tauri")) {
  run("npx", ["tauri", "build"], { cwd: root, shell: true, env: { ...process.env, RESGUARDO_AGENT_INSTALLER_SHA256: hash } });
}
