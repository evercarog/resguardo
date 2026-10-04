// Después de `vite build`: saca el <script> de arranque en línea de SvelteKit
// a un archivo (`_app/arranque-<hash>.js`). Así la consola funciona con la CSP
// del servidor (`script-src 'self' 'wasm-unsafe-eval'`, sin hashes ni
// 'unsafe-inline'), que se envía por cabecera y no depende de este HTML.
// Es un script clásico (no módulo) para que `document.currentScript` siga
// apuntando a su sitio en la página; las rutas a /_app son absolutas
// (kit.paths.relative = false) porque import() resuelve desde el script.
import fs from "node:fs";
import crypto from "node:crypto";

const html = new URL("../build/index.html", import.meta.url);
let s = fs.readFileSync(html, "utf8");
const re = /<script>([\s\S]*?)<\/script>/;
const m = re.exec(s);
if (!m) {
  console.log("arranque-externo: no hay script en línea (nada que hacer)");
  process.exit(0);
}
if (/["']\.\/_app\//.test(m[1])) {
  console.error("arranque-externo: el arranque usa rutas relativas; pon kit.paths.relative = false");
  process.exit(1);
}
const codigo = m[1].trim() + "\n";
const hash = crypto.createHash("sha256").update(codigo).digest("hex").slice(0, 10);
const nombre = `_app/arranque-${hash}.js`;
fs.writeFileSync(new URL(`../build/${nombre}`, import.meta.url), codigo);
s = s.replace(re, `<script src="/${nombre}"></script>`);
// El hash del script en línea ya no hace falta en la CSP del <meta>.
s = s.replace(/ 'sha256-[A-Za-z0-9+/=]+'/g, "");
fs.writeFileSync(html, s);
console.log(`arranque-externo: ${nombre}`);

// La consola va dentro del servidor y funciona sin Internet: nada de
// recursos externos en el HTML (scripts, estilos, fuentes o imágenes).
const externos = s.match(/(?:src|href)="(?:https?:)?\/\/[^"]+"/g);
if (externos) {
  console.error(`arranque-externo: el HTML carga recursos externos: ${externos.join(", ")}`);
  process.exit(1);
}
