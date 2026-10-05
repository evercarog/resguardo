// Prueba de carga: muchas consolas abiertas a la vez contra un servidor de verdad
// (docs/capacidad.md). Cada «pestaña» pide lo que pide la consola en reposo
// (medido con un navegador: «Estado», «Equipos», «Añadir equipo»…, peor caso
// 16 peticiones por minuto), con varias cuentas y dos agentes reales enviando
// informes. Mide latencias, errores, CPU y memoria del servidor.
//
//   cargo build --release -p resguardo-servidor --bin resguardo-server   (o debug)
//   cargo build -p resguardo-agente --bins
//   cd consola && npx tsx scripts/e2e/carga.ts
//
// Variables: PESTANAS (150, «50 clientes × 3 consolas»), POR_MINUTO (16 por pestaña),
// FASES («1,4»: multiplicadores de la carga), SEGUNDOS (60 por fase), CLIENTES (50),
// RESGUARDO_SERVIDOR (el binario; por defecto release si está, si no debug).
// Procesos normales en 127.0.0.1 con carpetas temporales: nunca toca servicios instalados.
import fs from "node:fs";
import https from "node:https";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import type { Cliente } from "../../src/lib/tipos";
import { Agente, binario, Consola, Servidor } from "./actores";
import { Autenticador, deBase32, dormir, log, pararTodo, puertoLibre, WIN } from "./entorno";

const AQUI = path.dirname(fileURLToPath(import.meta.url));
const RAIZ = path.resolve(AQUI, "../../..");
const TARGET = path.join(RAIZ, "src-tauri", "target");
const env = (k: string, d: number) => Number(process.env[k] ?? d) || d;
const PESTANAS = env("PESTANAS", 150);
const POR_MINUTO = env("POR_MINUTO", 16);
const SEGUNDOS = env("SEGUNDOS", 60);
const CLIENTES = env("CLIENTES", 50);
const FASES = (process.env.FASES ?? "1,4").split(",").map(Number).filter((x) => x > 0);
/** Cuentas: la dueña y 9 invitadas (aceptar invitaciones tiene su límite por IP: 10 cada 15 min). */
const CUENTAS = 10;
/** Pestañas por cuenta (cada cuenta tiene su límite: 1200 peticiones por minuto). */
const POR_CUENTA = Math.ceil(PESTANAS / CUENTAS);
if (POR_CUENTA * POR_MINUTO * Math.max(...FASES) > 1150) console.log(`AVISO: ${POR_CUENTA * POR_MINUTO * Math.max(...FASES)} peticiones/min por cuenta: saltará el límite por cuenta (1200).`);

const servidorBin =
  process.env.RESGUARDO_SERVIDOR ?? [binario(path.join(TARGET, "release"), "resguardo-server"), binario(path.join(TARGET, "debug"), "resguardo-server")].find((p) => fs.existsSync(p))!;
const base = fs.mkdtempSync(path.join(process.env.RESGUARDO_E2E_TMP ?? os.tmpdir(), "carga-"));
const dir = (...p: string[]) => path.join(base, ...p);
fs.mkdirSync(dir("registros"), { recursive: true });

/** CPU (s) y memoria (MB) de un proceso. */
function medirProceso(pid: number): { cpu: number; mb: number } {
  if (WIN) {
    const r = spawnSync("powershell", ["-NoProfile", "-Command", `$p = Get-Process -Id ${pid}; "$($p.CPU) $($p.WorkingSet64)"`], { encoding: "utf8" });
    const [cpu, ws] = r.stdout.trim().replace(",", ".").split(/\s+/).map(Number);
    return { cpu, mb: ws / 1e6 };
  }
  const [ut, st] = fs.readFileSync(`/proc/${pid}/stat`, "utf8").split(" ").slice(13, 15).map(Number);
  const rss = Number(fs.readFileSync(`/proc/${pid}/statm`, "utf8").split(" ")[1]) * 4096;
  return { cpu: (ut + st) / 100, mb: rss / 1e6 };
}

const s = new Servidor("Servidor", servidorBin, dir("servidor"), await puertoLibre(), dir("registros", "servidor.log"));
await s.arrancar();
const pid = s.proceso!.hijo.pid!;
const duena = new Consola(s);
await duena.primerArranque("ana@ejemplo.com", "Ana", "una contraseña bien larga para la consola");
const clientes: Cliente[] = [];
for (let i = 0; i < CLIENTES; i++) clientes.push(await duena.ok("POST", "/api/clientes", { nombre: `Cliente de prueba ${i + 1}`, espera_min_horas: 1 }));
const c0 = clientes[0];
Object.assign(c0, await duena.ok("GET", `/api/clientes/${c0.id}`));
const agenteBin = binario(path.join(TARGET, "debug"), "resguardo-agente");
if (fs.existsSync(agenteBin)) {
  for (const n of ["CAJA", "OFICINA"]) await duena.emparejar(c0, new Agente(n, agenteBin, dir(`agente-${n}`), dir("registros", `agente-${n}.log`)), "caballo batería grapa correcta");
}

// Cuentas: la dueña y administradoras invitadas al primer cliente.
const cuentas: { cookie: string; clientes: string[] }[] = [{ cookie: duena.cookieSesion, clientes: clientes.map((c) => c.id) }];
const hacen = Math.ceil(PESTANAS / POR_CUENTA);
for (let i = 1; i < hacen; i++) {
  const inv = await duena.ok("POST", `/api/clientes/${c0.id}/invitaciones`, { rol: "administrador" });
  const token = String(inv.enlace).split("#")[1];
  const otra = new Consola(s);
  const r = await otra.ok("POST", "/api/invitaciones/aceptar", { token, correo: `persona${i}@ejemplo.com`, nombre: `Persona ${i}`, contrasena: "contraseña de la persona de prueba" });
  await otra.ok("POST", "/api/sesion/totp", { codigo: await new Autenticador(deBase32(r.totp.secreto)).codigo() });
  cuentas.push({ cookie: otra.cookieSesion, clientes: [c0.id] });
}
log(`${CLIENTES} clientes, ${cuentas.length} cuentas, ${PESTANAS} pestañas (${POR_CUENTA} por cuenta), ${POR_MINUTO} peticiones/min por pestaña; servidor: ${servidorBin}`);

// Lo que pide una pestaña en reposo (peor caso medido: «Añadir equipo»).
const RUTAS = (c: string) => [`/api/clientes/${c}/emparejamientos`, `/api/clientes/${c}/progreso`, `/api/clientes/${c}/resumen`, `/api/clientes/${c}/informes`];
const agente = new https.Agent({ keepAlive: true, maxSockets: 1000, ca: s.ca });
function get(ruta: string, cookie: string): Promise<number> {
  return new Promise((ok) => {
    const r = https.request({ host: "127.0.0.1", port: s.puerto, path: ruta, method: "GET", agent: agente, headers: { Cookie: cookie, Accept: "application/json" }, timeout: 30_000 }, (res) => {
      res.resume();
      res.on("end", () => ok(res.statusCode ?? 0));
    });
    r.on("timeout", () => r.destroy());
    r.on("error", () => ok(0));
    r.end();
  });
}

const resultados: Record<string, unknown>[] = [];
for (const factor of FASES) {
  const lat: number[] = [];
  const estados = new Map<number, number>();
  const antes = medirProceso(pid);
  const inicio = Date.now();
  const fin = inicio + SEGUNDOS * 1000;
  const cadaMs = 60_000 / (POR_MINUTO * factor);
  let maxMb = antes.mb;
  const vigia = setInterval(() => (maxMb = Math.max(maxMb, medirProceso(pid).mb)), 5000);
  await Promise.all(
    Array.from({ length: PESTANAS }, async (_, i) => {
      const cuenta = cuentas[Math.floor(i / POR_CUENTA)];
      const c = cuenta.clientes[i % cuenta.clientes.length];
      const rutas = RUTAS(c);
      await dormir(Math.random() * cadaMs);
      for (let n = 0; Date.now() < fin; n++) {
        const t = performance.now();
        const e = await get(rutas[n % rutas.length], cuenta.cookie);
        lat.push(performance.now() - t);
        estados.set(e, (estados.get(e) ?? 0) + 1);
        await dormir(Math.max(0, cadaMs - (performance.now() - t)));
      }
    }),
  );
  clearInterval(vigia);
  const despues = medirProceso(pid);
  const seg = (Date.now() - inicio) / 1000;
  lat.sort((a, b) => a - b);
  const p = (q: number) => Math.round(lat[Math.min(lat.length - 1, Math.floor(q * lat.length))] ?? 0);
  const r = {
    fase: `×${factor}`,
    peticiones_min: Math.round((lat.length / seg) * 60),
    por_segundo: Math.round(lat.length / seg),
    ms_p50: p(0.5),
    ms_p95: p(0.95),
    ms_p99: p(0.99),
    estados: Object.fromEntries(estados),
    cpu_pct_un_nucleo: Math.round(((despues.cpu - antes.cpu) / seg) * 100),
    mb: Math.round(Math.max(maxMb, despues.mb)),
  };
  resultados.push(r);
  log(JSON.stringify(r));
}
fs.writeFileSync(dir("resultados.json"), JSON.stringify({ PESTANAS, POR_MINUTO, CLIENTES, cuentas: cuentas.length, servidorBin, resultados }, null, 2));
console.log(JSON.stringify(resultados, null, 2));
await pararTodo();
process.exit(0);
