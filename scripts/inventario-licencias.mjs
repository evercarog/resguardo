// Inventario de licencias de las dependencias (Rust y JavaScript) → THIRD-PARTY.md
//
//   node scripts/inventario-licencias.mjs           escribe THIRD-PARTY.md
//   node scripts/inventario-licencias.mjs --check   falla si hay licencias fuera de la lista permitida
//
// Usa solo lo que ya hay en el equipo: `cargo metadata` (sin red, con
// --offline) y los package.json de node_modules. La lista de licencias
// permitidas es la misma que la de deny.toml; mantenlas iguales.
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const check = process.argv.includes("--check");

// Compatibles con la AGPL-3.0 para distribuir el conjunto.
const ALLOWED = new Set([
  "MIT",
  "Apache-2.0",
  "Apache-2.0 WITH LLVM-exception",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "ISC",
  "Zlib",
  "Unicode-3.0",
  "Unicode-DFS-2016",
  "MPL-2.0",
  "CC0-1.0",
  "0BSD",
  "BSL-1.0",
  "OFL-1.1",
  "CDLA-Permissive-2.0",
  "MIT-0",
  "AGPL-3.0-or-later",
  "GPL-3.0-or-later",
  "LGPL-2.1-or-later",
  "Unlicense",
]);

/** ¿Alguna de las alternativas de una expresión SPDX («A OR B», «A AND B») está permitida? */
function allowed(expr) {
  if (!expr) return false;
  const e = expr.replace(/[()]/g, "").replace(/\//g, " OR ");
  // Con OR basta una; con AND, todas.
  return e.split(/\s+OR\s+/).some((alt) => alt.split(/\s+AND\s+/).every((l) => ALLOWED.has(l.trim())));
}

// ---------- Rust ----------
// Solo las dependencias de la plataforma para la que se compila (por defecto,
// la de este equipo; otra con --target=<triple>).
const host = /host: (\S+)/.exec(execFileSync("rustc", ["-vV"], { encoding: "utf8" }))[1];
const target = process.argv.find((a) => a.startsWith("--target="))?.slice(9) ?? host;
const meta = JSON.parse(
  execFileSync("cargo", ["metadata", "--format-version", "1", "--offline", "--all-features", "--filter-platform", target, "--manifest-path", path.join(root, "Cargo.toml")], {
    cwd: root,
    maxBuffer: 256 * 1024 * 1024,
    encoding: "utf8",
  }),
);
const own = new Set(meta.workspace_members);
const rust = meta.packages
  .filter((p) => !own.has(p.id))
  .map((p) => ({ name: p.name, version: p.version, license: p.license ?? (p.license_file ? `ver ${p.license_file}` : null), repo: p.repository ?? "" }))
  .sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));

// ---------- JavaScript (solo lo que acaba en la interfaz y en la consola web) ----------
function npmDe(dir) {
  const seen = new Map();
  const file0 = path.join(dir, "package.json");
  if (!fs.existsSync(file0)) return [];
  const pkg = JSON.parse(fs.readFileSync(file0, "utf8"));
  function walkNpm(name) {
    if (seen.has(name)) return;
    const file = path.join(dir, "node_modules", name, "package.json");
    if (!fs.existsSync(file)) return;
    const p = JSON.parse(fs.readFileSync(file, "utf8"));
    const license = typeof p.license === "string" ? p.license : (p.license?.type ?? (Array.isArray(p.licenses) ? p.licenses.map((l) => l.type).join(" OR ") : null));
    seen.set(name, { name, version: p.version, license, repo: typeof p.repository === "string" ? p.repository : (p.repository?.url ?? "") });
    for (const dep of Object.keys(p.dependencies ?? {})) walkNpm(dep);
  }
  for (const dep of Object.keys(pkg.dependencies ?? {})) walkNpm(dep);
  return [...seen.values()].sort((a, b) => a.name.localeCompare(b.name));
}
const js = npmDe(root);
const consola = npmDe(path.join(root, "consola"));

// ---------- Salida ----------
const bad = [...rust, ...js, ...consola].filter((d) => !allowed(d.license));
const row = (d) => `| ${d.name} | ${d.version} | ${d.license ?? "**desconocida**"} |`;
const out = [
  "# Licencias de terceros",
  "",
  `Generado por \`node scripts/inventario-licencias.mjs\` para ${target}. No lo edites a mano.`,
  "",
  "Además de estas bibliotecas, Resguardo distribuye **restic** y **rest-server** (BSD-2-Clause) y **rclone** (MIT); ver [NOTICE](NOTICE).",
  "",
  `## Rust (${rust.length})`,
  "",
  "| Crate | Versión | Licencia |",
  "|---|---|---|",
  ...rust.map(row),
  "",
  `## JavaScript incluido en la interfaz (${js.length})`,
  "",
  "| Paquete | Versión | Licencia |",
  "|---|---|---|",
  ...js.map(row),
  "",
  `## JavaScript incluido en la consola web de Resguardo Server (${consola.length})`,
  "",
  "| Paquete | Versión | Licencia |",
  "|---|---|---|",
  ...consola.map(row),
  "",
  "## A revisar",
  "",
  bad.length ? bad.map((d) => `- ${d.name} ${d.version}: ${d.license ?? "sin licencia declarada"}`).join("\n") : "Ninguna: todas están en la lista permitida.",
  "",
];
if (check) {
  if (bad.length) {
    console.error(`Licencias fuera de la lista permitida:\n${bad.map((d) => `  ${d.name} ${d.version}: ${d.license}`).join("\n")}`);
    process.exit(1);
  }
  console.log(`Licencias correctas (${rust.length} crates, ${js.length + consola.length} paquetes).`);
} else {
  fs.writeFileSync(path.join(root, "THIRD-PARTY.md"), out.join("\n"));
  console.log(`THIRD-PARTY.md: ${rust.length} crates, ${js.length + consola.length} paquetes, ${bad.length} a revisar.`);
}
