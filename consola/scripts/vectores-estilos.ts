// Comprobaciones estáticas de los estilos de la consola y de `ui/` (G1, plan 0.8.1).
// `npm run test:vectores` (con las demás).
//
// Variables de CSS: cada `var(--…)` tiene que estar definida en algún sitio
// (`--x:` en un CSS o en un `style="…"`, o `style:--x=` de Svelte). Una variable
// que no existe no da error: el navegador usa el valor por defecto (o el de
// reserva), y así los fallos de los espejos salieron en el color del texto en vez
// de en rojo. Se miran también las que llevan valor de reserva: la reserva
// escondía el fallo.
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const consola = join(dirname(fileURLToPath(import.meta.url)), "..");
const raices = [join(consola, "src"), join(consola, "..", "ui", "src")];

let fallos = 0;
let total = 0;
function igual(nombre: string, obtenido: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(obtenido) === JSON.stringify(esperado);
  if (!ok) fallos++;
  console.log(`${ok ? "ok  " : "MAL "} ${nombre}${ok ? "" : `\n       obtenido: ${JSON.stringify(obtenido)}\n       esperado: ${JSON.stringify(esperado)}`}`);
}

const archivos: string[] = [];
const recorrer = (d: string) => {
  for (const e of readdirSync(d, { withFileTypes: true })) {
    const p = join(d, e.name);
    if (e.isDirectory()) recorrer(p);
    else if (/\.(svelte|css|ts|js|html)$/.test(e.name)) archivos.push(p);
  }
};
for (const r of raices) recorrer(r);

// Variables que se ponen desde fuera del CSS y no se pueden ver aquí. Vacía hoy:
// añade una solo con su motivo.
const EXCEPCIONES = new Set<string>([]);

const definidas = new Set<string>();
const usos: { nombre: string; donde: string }[] = [];
for (const a of archivos) {
  const t = readFileSync(a, "utf8");
  for (const m of t.matchAll(/(?<![\w-])(--[A-Za-z0-9_-]+)\s*:/g)) definidas.add(m[1]);
  for (const m of t.matchAll(/style:(--[A-Za-z0-9_-]+)/g)) definidas.add(m[1]);
  for (const m of t.matchAll(/setProperty\(\s*["'`](--[A-Za-z0-9_-]+)/g)) definidas.add(m[1]);
  for (const m of t.matchAll(/var\(\s*(--[A-Za-z0-9_-]+)/g)) {
    // `var(--et-{i})`: el nombre se completa al pintar; no se puede comprobar.
    if (m[1].endsWith("-")) continue;
    usos.push({ nombre: m[1], donde: `${relative(join(consola, ".."), a).replaceAll("\\", "/")}:${t.slice(0, m.index).split("\n").length}` });
  }
}

console.log("\n— Variables de CSS definidas —");
const sinDefinir = usos.filter((u) => !definidas.has(u.nombre) && !EXCEPCIONES.has(u.nombre)).map((u) => `${u.nombre} (${u.donde})`);
igual(`${usos.length} usos de var(--…) en ${archivos.length} archivos: todos definidos`, sinDefinir, []);
igual("los tokens de la escala de espaciado de diseno.md existen", ["--sp-1", "--sp-2", "--sp-3", "--sp-4", "--sp-5", "--sp-6", "--sp-8", "--sp-10", "--sp-12", "--sp-16"].filter((v) => !definidas.has(v)), []);
igual("los tokens de texto de diseno.md existen", ["--fs-display", "--fs-title", "--fs-stat", "--fs-h2", "--fs-body", "--fs-sm", "--fs-xs", "--fs-overline", "--mono", "--font"].filter((v) => !definidas.has(v)), []);

console.log(`\n${total - fallos}/${total} correctas`);
if (fallos) process.exit(1);
