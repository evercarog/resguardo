// Compila Resguardo Server para Windows con la consola dentro y su instalador
// «Instalar Resguardo Server».
//
//   npm run build:servidor
//
// 1. La consola (consola/: npm ci si falta node_modules, y npm run build).
// 2. resguardo-server.exe con la feature consola-integrada.
// 3. El instalador NSIS (packaging/windows/servidor.nsi), en
//    src-tauri/target/release/bundle/nsis, con la huella SHA-256 del programa.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const tauri = path.join(root, "src-tauri");
const consola = path.join(root, "consola");
const cargoToml = fs.readFileSync(path.join(root, "crates", "servidor", "Cargo.toml"), "utf8");
const version = /^version\s*=\s*"([^"]+)"/m.exec(cargoToml)[1];
const sha256 = (file) => createHash("sha256").update(fs.readFileSync(file)).digest("hex");
const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { stdio: "inherit", ...opts });

if (!fs.existsSync(path.join(consola, "node_modules"))) run("npm", ["ci"], { cwd: consola, shell: true });
run("npm", ["run", "build"], { cwd: consola, shell: true });
if (!fs.existsSync(path.join(consola, "build", "index.html"))) throw new Error("La consola no se compiló (falta consola/build/index.html).");

run("cargo", ["build", "--release", "-p", "resguardo-servidor", "--features", "consola-integrada", "--bin", "resguardo-server"], { cwd: tauri });
const exe = path.join(tauri, "target", "release", "resguardo-server.exe");

const candidates = [path.join(process.env.LOCALAPPDATA ?? path.join(os.homedir(), "AppData", "Local"), "tauri", "NSIS", "makensis.exe"), "makensis"];
const makensis = candidates.find((c) => c === "makensis" || fs.existsSync(c));
const outDir = path.join(tauri, "target", "release", "bundle", "nsis");
fs.mkdirSync(outDir, { recursive: true });
const out = path.join(outDir, `Resguardo-Server_${version}_x64-setup.exe`);
// El instalador del agente (npm run build:agente lo deja en src-tauri/resources/agente):
// va dentro para «Descargar instalador listo» en la consola. Sin él, el servidor lo dice.
const agentSetup = path.join(tauri, "resources", "agente", "Resguardo-Agente-setup.exe");
if (fs.existsSync(agentSetup)) console.log(`Incluye el instalador del agente (SHA-256 ${sha256(agentSetup)}).`);
else console.warn("Aviso: sin src-tauri/resources/agente/Resguardo-Agente-setup.exe (npm run build:agente): la consola no podrá dar el instalador listo.");
run(makensis, [
  "/V2",
  "/INPUTCHARSET",
  "UTF8",
  `/DVERSION=${version}`,
  `/DSERVER_EXE=${exe}`,
  `/DSERVER_SHA256=${sha256(exe)}`,
  ...(fs.existsSync(agentSetup) ? [`/DAGENT_SETUP=${agentSetup}`] : []),
  `/DOUT_FILE=${out}`,
  path.join(root, "packaging", "windows", "servidor.nsi"),
]);
console.log(`\nInstalador de Resguardo Server: ${out}\nSHA-256: ${sha256(out)}`);
