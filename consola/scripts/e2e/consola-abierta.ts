// Un servidor de verdad con dos agentes emparejados, en marcha hasta Ctrl+C,
// para mirar la consola en un navegador y CONTAR lo que pide (docs/capacidad.md).
//
//   cargo build -p resguardo-servidor --features consola-integrada -p resguardo-agente --bins
//   cd consola && npx tsx scripts/e2e/consola-abierta.ts
//
// Escribe en <carpeta>/sesion.json la dirección, la cookie de la sesión (para
// ponerla en el navegador) y el cliente. Procesos normales en 127.0.0.1 con
// carpetas temporales: nunca toca servicios instalados.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { Cliente } from "../../src/lib/tipos";
import { Agente, binario, Consola, Servidor } from "./actores";
import { log, pararTodo, puertoLibre } from "./entorno";

const AQUI = path.dirname(fileURLToPath(import.meta.url));
const RAIZ = path.resolve(AQUI, "../../..");
const TARGET = process.env.RESGUARDO_E2E_TARGET ?? path.join(RAIZ, "src-tauri", "target", "debug");
const CLAVE_ADMIN = "caballo batería grapa correcta";

const base = fs.mkdtempSync(path.join(process.env.RESGUARDO_E2E_TMP ?? os.tmpdir(), "consola-abierta-"));
const dir = (...p: string[]) => path.join(base, ...p);
fs.mkdirSync(dir("registros"), { recursive: true });
const s = new Servidor("Servidor", binario(TARGET, "resguardo-server"), dir("servidor"), Number(process.env.PUERTO ?? 0) || (await puertoLibre()), dir("registros", "servidor.log"));
await s.arrancar();
const consola = new Consola(s);
await consola.primerArranque("ana@ejemplo.com", "Ana", "una contraseña bien larga para la consola");
const c: Cliente = await consola.ok("POST", "/api/clientes", { nombre: "Ferretería Altamar", espera_min_horas: 1 });
Object.assign(c, await consola.ok("GET", `/api/clientes/${c.id}`));
const agentes = ["CAJA", "OFICINA"].map((n) => new Agente(n, binario(TARGET, "resguardo-agente"), dir(`agente-${n}`), dir("registros", `agente-${n}.log`)));
for (const a of agentes) await consola.emparejar(c, a, CLAVE_ADMIN);
const sesion = { url: s.url, cookie: consola.cookieSesion, cliente: c.id, carpeta: base };
fs.writeFileSync(dir("sesion.json"), JSON.stringify(sesion, null, 2));
log(`Listo: ${JSON.stringify(sesion)}`);
log("Ctrl+C para parar.");
const parar = async () => {
  await pararTodo();
  process.exit(0);
};
process.on("SIGINT", parar);
process.on("SIGTERM", parar);
await new Promise(() => {});
