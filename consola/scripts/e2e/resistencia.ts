// Prueba de resistencia («soak»): Resguardo Server y varios agentes DE VERDAD
// funcionando horas seguidas mientras se les rompe cosas a propósito, midiendo
// lo que crece (memoria, handles, hilos, base de datos, archivos temporales) y
// lo que se queda colgado (órdenes, tareas «en marcha», canales). Ver
// docs/estabilidad.md.
//
//   # optimizados (release) pero con el modo de pruebas: la carpeta del agente en
//   # RESGUARDO_AGENT_DIR y las órdenes que reducen la protección sin su espera de 1 h
//   # (RESGUARDO_PRUEBA_SIN_ESPERA), que solo existen con debug_assertions:
//   CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true cargo build --release -p resguardo-servidor \
//       --bin resguardo-server -p resguardo-agente --bin resguardo-agente --target-dir src-tauri/target-resistencia
//   cd consola && npx tsx scripts/e2e/resistencia.ts
//
// Qué monta: un servidor (los agentes le llegan a través de un «cable» que se
// puede cortar, Cable), un segundo servidor (un equipo con dos consolas), un
// agente que guarda copias (con espejo a una carpeta y retención), tres equipos
// que copian cada 5 minutos (uno en un rest-server aparte con cuota, para
// llenarlo), verificación, órdenes, sesiones, descargas por el relé y unas
// cuantas «pestañas» de consola con su canal en vivo.
//
// Qué rompe (cada cosa una vez y luego en bucle): reiniciar el servidor (rápido
// y con una caída de minutos), cortar la red de los equipos con el servidor
// (sin respuesta, no «rechazada»), matar y volver a arrancar agentes (también
// en mitad de una copia), apagar el almacén, cortar la red con el almacén,
// llenar el destino (rest-server con --max-size: «507 Insufficient Storage»,
// lo mismo que un disco lleno) y estropear un archivo de un repositorio.
//
// Variables: MINUTOS (130), RESGUARDO_SERVIDOR, RESGUARDO_AGENTE (los binarios),
// PESTANAS (6), RESGUARDO_E2E_TMP, RESGUARDO_E2E_CONSERVAR=1.
// Procesos normales en 127.0.0.1 con carpetas temporales: nunca toca servicios
// instalados ni ProgramData (los agentes, en modo de pruebas).
import fs from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import https from "node:https";
import { randomBytes } from "node:crypto";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { aB64, aleatorio } from "../../src/lib/cripto/bytes";
import { etiquetaEquipo, kCfg, materialCliente, sasV3 } from "../../src/lib/cripto/claves";
import { crearCodigo, cuerpoAnadir, leerCodigo } from "../../src/lib/conexion";
import { almacenDe, nuevaClave, reglaParaOrden } from "../../src/lib/retencion";
import { destinoDe, informeDe } from "../../src/lib/repo";
import type { Cliente, Regla } from "../../src/lib/tipos";
import { argon2, Agente, binario, Consola, FINALES, SesionE2E, Servidor } from "./actores";
import { OyenteVivo } from "./vivo";
import { borrarCarpeta, BuzonSmtp, comprobar, dormir, EXE, esperar, Fallo, log, pararTodo, Proceso, puertoLibre, WIN } from "./entorno";

const AQUI = path.dirname(fileURLToPath(import.meta.url));
const RAIZ = path.resolve(AQUI, "../../..");
const TARGET = path.join(RAIZ, "src-tauri");
const MINUTOS = Number(process.env.MINUTOS ?? 130);
const PESTANAS = Number(process.env.PESTANAS ?? 6);
const CLAVE_ADMIN = "caballo batería grapa correcta";
const CORREO = "ana@ejemplo.com";
const CONTRASENA = "una contraseña bien larga para la consola";
const MIN = 60_000;

const primero = (...ps: string[]) => ps.find((p) => fs.existsSync(p));
const servidorBin =
  process.env.RESGUARDO_SERVIDOR ??
  primero(binario(path.join(TARGET, "target-resistencia", "release"), "resguardo-server"), binario(path.join(TARGET, "target", "debug"), "resguardo-server"));
const agenteBinOrigen =
  process.env.RESGUARDO_AGENTE ?? primero(binario(path.join(TARGET, "target-resistencia", "release"), "resguardo-agente"), binario(path.join(TARGET, "target", "debug"), "resguardo-agente"));
const binarios = path.join(TARGET, "binaries");
const resticOrigen = primero(path.join(binarios, `restic${EXE}`), path.join(binarios, "restic-x86_64-pc-windows-msvc.exe"), path.join(binarios, "restic-x86_64-unknown-linux-gnu"));
const restServerOrigen = primero(path.join(binarios, `rest-server${EXE}`), path.join(binarios, "rest-server-x86_64-pc-windows-msvc.exe"), path.join(binarios, "rest-server-x86_64-unknown-linux-gnu"));
if (!servidorBin || !agenteBinOrigen || !resticOrigen || !restServerOrigen) {
  console.error("Faltan binarios (servidor, agente, restic o rest-server): ver la cabecera de este archivo.");
  process.exit(1);
}

const base = fs.mkdtempSync(path.join(process.env.RESGUARDO_E2E_TMP ?? os.tmpdir(), "resistencia-"));
const dir = (...p: string[]) => path.join(base, ...p);
const registros = dir("registros");
fs.mkdirSync(registros, { recursive: true });
const binDir = dir("bin");
fs.mkdirSync(binDir);
const agenteBin = binario(binDir, "resguardo-agente");
fs.copyFileSync(agenteBinOrigen, agenteBin);
fs.copyFileSync(resticOrigen, binario(binDir, "restic"));
fs.copyFileSync(restServerOrigen, binario(binDir, "rest-server"));
// El rest-server «con cuota» (el destino que se llena), aparte para distinguirlo.
fs.mkdirSync(dir("bin-cuota"));
const restCuotaBin = binario(dir("bin-cuota"), "rest-server");
fs.copyFileSync(restServerOrigen, restCuotaBin);
if (!WIN) for (const b of [agenteBin, binario(binDir, "restic"), binario(binDir, "rest-server"), restCuotaBin]) fs.chmodSync(b, 0o755);

const T0 = Date.now();
const minuto = () => (Date.now() - T0) / MIN;
const diario = fs.openSync(dir("diario.log"), "a");
/** Lo que pasa (y lo que ve la consola), con la hora: lo que se revisa después. */
function apuntar(que: string, datos?: unknown) {
  const linea = `[${minuto().toFixed(1).padStart(6)} min] ${que}${datos === undefined ? "" : ` ${typeof datos === "string" ? datos : JSON.stringify(datos)}`}`;
  console.log(linea);
  fs.writeSync(diario, `${linea}\n`);
}
const incidencias: { min: number; que: string; datos?: unknown }[] = [];
function incidencia(que: string, datos?: unknown) {
  incidencias.push({ min: Math.round(minuto() * 10) / 10, que, datos });
  apuntar(`INCIDENCIA: ${que}`, datos);
}

// ---------------------------------------------------------------------------
// El «cable»: un proxy TCP entre los equipos y un servidor que se puede cortar
// ---------------------------------------------------------------------------

/**
 * Corte «negro»: las conexiones abiertas dejan de pasar datos y las nuevas se
 * aceptan pero no llegan a ningún sitio (como un cable de red quitado o un
 * cortafuegos que descarta), así se ve si algo se queda esperando para siempre.
 */
class Cable {
  private srv: net.Server;
  private pares = new Set<{ a: net.Socket; b: net.Socket | null }>();
  cortado = false;
  puerto = 0;
  conexiones = 0;
  bytes = 0;
  constructor(
    readonly nombre: string,
    private destino: () => number,
  ) {
    this.srv = net.createServer((a) => this.unir(a));
  }
  async abrir() {
    this.puerto = await puertoLibre();
    await new Promise<void>((ok) => this.srv.listen(this.puerto, "127.0.0.1", ok));
  }
  private unir(a: net.Socket) {
    this.conexiones++;
    a.on("error", () => {});
    const par = { a, b: null as net.Socket | null };
    this.pares.add(par);
    a.on("close", () => {
      par.b?.destroy();
      this.pares.delete(par);
    });
    if (this.cortado) return; // aceptada, pero no va a ningún sitio
    const b = net.connect({ host: "127.0.0.1", port: this.destino() });
    par.b = b;
    b.on("error", () => a.destroy());
    b.on("close", () => a.destroy());
    a.on("data", (d) => {
      if (this.cortado) return;
      this.bytes += d.length;
      b.write(d);
    });
    b.on("data", (d) => {
      if (this.cortado) return;
      this.bytes += d.length;
      a.write(d);
    });
  }
  cortar() {
    this.cortado = true;
  }
  /** Vuelve la red: lo que se quedó colgado durante el corte se cierra (como al expirar en el router). */
  reanudar() {
    this.cortado = false;
    for (const p of [...this.pares]) {
      p.a.destroy();
      p.b?.destroy();
    }
  }
  cerrar() {
    this.srv.close();
    for (const p of this.pares) {
      p.a.destroy();
      p.b?.destroy();
    }
  }
}

// ---------------------------------------------------------------------------
// Medidas
// ---------------------------------------------------------------------------

interface Medida {
  pid: number;
  ws_mb: number;
  priv_mb: number;
  handles: number;
  hilos: number;
  cpu_s: number;
}

/** Un programa sin bloquear (el cable vive en este proceso: un spawnSync lo pararía). */
function salidaDe(programa: string, args: string[]): Promise<string> {
  return new Promise((ok) => {
    const h = spawn(programa, args, { windowsHide: true });
    let t = "";
    h.stdout.on("data", (d) => (t += d));
    h.on("error", () => ok(""));
    h.on("close", () => ok(t));
  });
}

async function medir(pids: number[]): Promise<Map<number, Medida>> {
  const m = new Map<number, Medida>();
  if (!pids.length) return m;
  if (WIN) {
    const salida = await salidaDe(
      "powershell",
      [
        "-NoProfile",
        "-Command",
        `Get-Process -Id ${pids.join(",")} -ErrorAction SilentlyContinue | ForEach-Object { "$($_.Id) $($_.WorkingSet64) $($_.PrivateMemorySize64) $($_.HandleCount) $($_.Threads.Count) $([math]::Round($_.TotalProcessorTime.TotalSeconds,1))" }`,
      ],
    );
    for (const l of salida.split(/\r?\n/)) {
      const [pid, ws, pr, h, t, cpu] = l.trim().replace(/,/g, ".").split(/\s+/).map(Number);
      if (pid) m.set(pid, { pid, ws_mb: Math.round(ws / 1e5) / 10, priv_mb: Math.round(pr / 1e5) / 10, handles: h, hilos: t, cpu_s: cpu });
    }
  } else {
    for (const pid of pids) {
      try {
        const st = fs.readFileSync(`/proc/${pid}/stat`, "utf8").split(" ");
        const rss = Number(fs.readFileSync(`/proc/${pid}/statm`, "utf8").split(" ")[1]) * 4096;
        const fds = fs.readdirSync(`/proc/${pid}/fd`).length;
        m.set(pid, { pid, ws_mb: Math.round(rss / 1e5) / 10, priv_mb: Math.round(rss / 1e5) / 10, handles: fds, hilos: Number(st[19]), cpu_s: (Number(st[13]) + Number(st[14])) / 100 });
      } catch {
        /* ya no está */
      }
    }
  }
  return m;
}

/** restic y rest-server lanzados desde las carpetas de esta prueba (para ver huérfanos). */
async function hijosSueltos(): Promise<{ pid: number; nombre: string; padre: number }[]> {
  if (!WIN) {
    const r = spawnSync("sh", ["-c", `ps -eo pid=,ppid=,args= | grep -F '${base}' | grep -v grep`], { encoding: "utf8" });
    return r.stdout
      .split("\n")
      .filter((l) => /restic|rest-server/.test(l))
      .map((l) => {
        const [pid, padre, ...resto] = l.trim().split(/\s+/);
        return { pid: Number(pid), padre: Number(padre), nombre: path.basename(resto[0] ?? "") };
      });
  }
  const salida = await salidaDe(
    "powershell",
    [
      "-NoProfile",
      "-Command",
      `Get-CimInstance Win32_Process -Filter "Name='restic.exe' or Name='rest-server.exe'" | Where-Object { $_.ExecutablePath -like '${base.replace(/'/g, "''")}*' } | ForEach-Object { "$($_.ProcessId) $($_.ParentProcessId) $($_.Name)" }`,
    ],
  );
  return salida
    .split(/\r?\n/)
    .filter((l) => l.trim())
    .map((l) => {
      const [pid, padre, nombre] = l.trim().split(/\s+/);
      return { pid: Number(pid), padre: Number(padre), nombre };
    });
}

function tamano(p: string): number {
  try {
    return fs.statSync(p).size;
  } catch {
    return 0;
  }
}

/** Archivos (y bytes) bajo `p`, con un filtro por nombre. */
function contar(p: string, f: (nombre: string, ruta: string) => boolean = () => true): { n: number; bytes: number; ejemplos: string[] } {
  const r = { n: 0, bytes: 0, ejemplos: [] as string[] };
  const pila = [p];
  while (pila.length) {
    const d = pila.pop()!;
    let entradas: fs.Dirent[] = [];
    try {
      entradas = fs.readdirSync(d, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const e of entradas) {
      const q = path.join(d, e.name);
      if (e.isDirectory()) pila.push(q);
      else if (f(e.name, q)) {
        r.n++;
        r.bytes += tamano(q);
        if (r.ejemplos.length < 5) r.ejemplos.push(path.relative(base, q));
      }
    }
  }
  return r;
}

function lineasConError(archivo: string): string[] {
  try {
    return fs
      .readFileSync(archivo, "utf8")
      .split(/\r?\n/)
      .filter((l) => /ERROR|panic|pánico|thread '.*' panicked/i.test(l));
  } catch {
    return [];
  }
}

// ---------------------------------------------------------------------------
// Las «pestañas» de consola: lo que pide una consola abierta y su canal en vivo
// ---------------------------------------------------------------------------

class Pestana {
  peticiones = 0;
  estados = new Map<number, number>();
  reconexiones = 0;
  mensajesVivo = 0;
  private parar = false;
  private oyente: OyenteVivo | null = null;
  constructor(
    private srv: Servidor,
    private cookie: () => string,
    private cliente: string,
    private equipos: () => string[],
  ) {}
  private get(ruta: string): Promise<number> {
    return new Promise((ok) => {
      const r = https.request(
        { host: "127.0.0.1", port: this.srv.puerto, path: ruta, method: "GET", ca: this.srv.ca, headers: { Cookie: this.cookie(), Accept: "application/json" }, timeout: 30_000 },
        (res) => {
          res.resume();
          res.on("end", () => ok(res.statusCode ?? 0));
        },
      );
      r.on("timeout", () => r.destroy());
      r.on("error", () => ok(0));
      r.end();
    });
  }
  arrancar() {
    void this.sondear();
    void this.vivo();
  }
  private async sondear() {
    const c = this.cliente;
    // Como «Estado» y «Un equipo» en reposo: ~16 por minuto.
    let n = 0;
    await dormir(Math.random() * 3000);
    while (!this.parar) {
      const eqs = this.equipos();
      const rutas = [`/api/clientes/${c}/resumen`, `/api/clientes/${c}/progreso`, `/api/clientes/${c}/informes`, `/api/clientes/${c}/avisos?abiertos=1`, `/api/clientes/${c}/ordenes?pendientes=1`];
      if (eqs.length) rutas.push(`/api/clientes/${c}/equipos/${eqs[n % eqs.length]}`);
      const e = await this.get(rutas[n++ % rutas.length]);
      this.peticiones++;
      this.estados.set(e, (this.estados.get(e) ?? 0) + 1);
      await dormir(60_000 / 16);
    }
  }
  private async vivo() {
    let primera = true;
    while (!this.parar) {
      try {
        this.oyente = await OyenteVivo.abrir(this.srv.url, this.cliente, this.cookie(), this.srv.ca);
        if (!primera) this.reconexiones++;
        primera = false;
        const o = this.oyente;
        while (!o.cerrado && !this.parar) {
          this.mensajesVivo = Math.max(this.mensajesVivo, 0) + 0;
          await dormir(1000);
        }
        this.mensajesVivo += o.mensajes.length;
      } catch {
        /* el servidor no está: se reintenta */
      }
      // Como la consola: reabre con espera (2–5 s).
      await dormir(2000 + Math.random() * 3000);
    }
  }
  cerrar() {
    this.parar = true;
    this.oyente?.cerrar();
  }
}

// ---------------------------------------------------------------------------
// Datos de los equipos
// ---------------------------------------------------------------------------

function crearDatos(d: string, mb: number) {
  fs.mkdirSync(path.join(d, "Documentos"), { recursive: true });
  fs.mkdirSync(path.join(d, "Relleno"), { recursive: true });
  fs.writeFileSync(path.join(d, "Documentos", "factura-001.txt"), "Factura 001 · Cliente de prueba · 1.234,56 €\n".repeat(50));
  for (let i = 0; i * 4 < mb; i++) fs.writeFileSync(path.join(d, "Relleno", `parte-${i}.bin`), randomBytes(4 * 1024 * 1024));
}

/** Cada minuto, algo cambia en cada equipo (así cada copia tiene algo que guardar). */
function cambiarDatos(d: string, kib: number) {
  const n = Math.floor(minuto());
  fs.writeFileSync(path.join(d, "Documentos", `nota-${n % 40}.txt`), `minuto ${n} ${randomBytes(8).toString("hex")}\n`);
  fs.writeFileSync(path.join(d, "Relleno", `nuevo-${n % 20}.bin`), randomBytes(kib * 1024));
}

const p2 = (n: number) => String(n).padStart(2, "0");
const hm = (d: Date) => `${p2(d.getHours())}:${p2(d.getMinutes())}`;
const TODOS = [1, 2, 3, 4, 5, 6, 7];

// ---------------------------------------------------------------------------
// El escenario
// ---------------------------------------------------------------------------

class AgenteR extends Agente {
  override get env() {
    // Su propia carpeta temporal: así se ven los temporales que se dejan restic y el agente.
    const tmp = path.join(this.dir, "temp");
    fs.mkdirSync(tmp, { recursive: true });
    return { RESGUARDO_AGENT_DIR: this.dir, RESGUARDO_PRUEBA_SIN_ESPERA: "1", TEMP: tmp, TMP: tmp, RESTIC_CACHE_DIR: path.join(this.dir, "cache-restic") };
  }
}

/**
 * «Añadir equipo» (como Consola.emparejar) con el agente apuntando al cable. `vincular`
 * se lanza sin bloquear: el cable vive en este mismo proceso de Node.
 */
async function emparejarPorCable(consola: Consola, c: Cliente, ag: AgenteR, url: string) {
  const emp = await consola.ok("POST", `/api/clientes/${c.id}/emparejamientos`);
  const salida = await new Promise<string>((ok) => {
    const h = spawn(ag.binario, ["vincular", emp.codigo, "--servidor", url, "--nombre", ag.nombre], { env: { ...process.env, ...ag.env }, windowsHide: true });
    let t = "";
    h.stdout.on("data", (d) => (t += d));
    h.stderr.on("data", (d) => (t += d));
    h.on("close", () => ok(t));
  });
  fs.appendFileSync(ag.registro, `\n$ resguardo-agente vincular …\n${salida}\n`);
  const sasEquipo = /Código de comprobación:\s*(\d{3} \d{3})/.exec(salida)?.[1];
  comprobar(sasEquipo, `${ag.nombre}: «vincular» falló`, salida);
  const unido = await esperar("que el equipo se una", async () => {
    const x = await consola.ok("GET", `/api/clientes/${c.id}/emparejamientos/${emp.id}`);
    return x.estado === "unido" ? x : null;
  });
  const servidor = await consola.ok("GET", "/api/servidor");
  const eq = unido.equipo;
  comprobar(sasV3(servidor.identidad, eq.box_pub, eq.sign_pub, servidor.huella_ca) === sasEquipo, "El número de comprobación coincide");
  const kcfg = kCfg(await materialCliente(argon2, CLAVE_ADMIN, c.sal_cliente));
  await consola.ok("POST", `/api/clientes/${c.id}/emparejamientos/${emp.id}/confirmar`, { etiqueta: etiquetaEquipo(kcfg, eq.id, eq.box_pub, eq.sign_pub) });
  ag.id = eq.id;
  ag.arrancar();
  await consola.hecha(c, eq.id, "alta", {}, { claveAdmin: CLAVE_ADMIN }, { alta: { codigo: emp.codigo } });
  log(`${ag.nombre} emparejado por el cable: ${eq.id}`);
  return consola.equipo(c, eq.id);
}

async function principal() {
  console.log(`Carpeta: ${base}\nServidor: ${servidorBin}\nAgente: ${agenteBinOrigen}\nDuración: ${MINUTOS} min`);
  const buzon = new BuzonSmtp();
  await buzon.abrir();
  const s1 = new Servidor("Servidor 1", servidorBin!, dir("servidor-1"), await puertoLibre(), path.join(registros, "servidor-1.log"));
  const s2 = new Servidor("Servidor 2", servidorBin!, dir("servidor-2"), await puertoLibre(), path.join(registros, "servidor-2.log"));
  await s1.arrancar();
  await s2.arrancar();
  const cable = new Cable("equipos ↔ servidor 1", () => s1.puerto);
  await cable.abrir();
  const urlAgentes = `https://127.0.0.1:${cable.puerto}`;
  const consola = new Consola(s1);
  await consola.primerArranque(CORREO, "Ana", CONTRASENA);
  const c: Cliente = await consola.ok("POST", "/api/clientes", { nombre: "Ferretería Altamar", espera_min_horas: 1 });
  Object.assign(c, await consola.ok("GET", `/api/clientes/${c.id}`));
  // Las notificaciones por correo (lo que le llega a quien administra).
  const canal = await consola.ok("POST", "/api/servidor/notificaciones/canales", {
    tipo: "correo",
    nombre: "Correo de prueba",
    config: { host: "127.0.0.1", puerto: buzon.puerto, seguridad: "ninguna", remitente: "Resguardo <avisos@prueba.example>" },
    codigo: await consola.totp!.codigo(),
  });
  comprobar(canal.completo, "Canal de correo", canal);

  /** «Añadir equipo» con el agente apuntando al cable (no directamente al servidor). */
  const agentes: AgenteR[] = [];
  const emparejar = async (nombre: string) => {
    const ag = new AgenteR(nombre, agenteBin, dir(`agente-${nombre}`), path.join(registros, `agente-${nombre}.log`));
    agentes.push(ag);
    const eq = await emparejarPorCable(consola, c, ag, urlAgentes);
    return { ag, eq };
  };
  const { ag: A, eq: eqA } = await emparejar("ALMACEN");
  const { ag: E1, eq: eq1 } = await emparejar("EQUIPO-1");
  const { ag: E2, eq: eq2 } = await emparejar("EQUIPO-2");
  const { ag: E3, eq: eq3 } = await emparejar("EQUIPO-3");
  const equiposIds = [eqA.id, eq1.id, eq2.id, eq3.id];
  const nombreDe = new Map([
    [eqA.id, "ALMACEN"],
    [eq1.id, "EQUIPO-1"],
    [eq2.id, "EQUIPO-2"],
    [eq3.id, "EQUIPO-3"],
  ]);

  // El almacén, detrás de su propio cable (para cortar la red entre los equipos y él).
  const puertoAlmacen = await puertoLibre();
  const carpetaAlmacen = dir("almacen");
  await consola.hecha(c, eqA.id, "guarda_copias", { activo: true, carpeta: carpetaAlmacen, puerto: puertoAlmacen, solo_red_local: true }, { claveAdmin: CLAVE_ADMIN });
  const cableAlmacen = new Cable("equipos ↔ almacén", () => puertoAlmacen);
  await cableAlmacen.abrir();
  const espejo = dir("espejo");
  const ponerEspejo = (hora: string) => consola.hecha(c, eqA.id, "guarda_copias", { espejo: { destinos: [{ tipo: "carpeta", carpeta: espejo }], hora } }, { claveAdmin: CLAVE_ADMIN });
  await ponerEspejo(hm(new Date()));

  // El destino con cuota: un rest-server aparte (sin TLS, como uno de la red de la oficina).
  const datosCuota = dir("rest-cuota");
  fs.mkdirSync(datosCuota);
  const puertoCuota = await puertoLibre();
  let restCuota: Proceso | null = null;
  const arrancarCuota = async (maxBytes: number) => {
    await restCuota?.parar();
    restCuota = new Proceso("rest-server con cuota", restCuotaBin, ["--path", datosCuota, "--listen", `127.0.0.1:${puertoCuota}`, "--no-auth", "--max-size", String(maxBytes)], { registro: path.join(registros, "rest-cuota.log") });
    await esperar("el rest-server con cuota", async () => !restCuota!.terminado && (await new Promise<boolean>((ok) => net.connect(puertoCuota, "127.0.0.1").once("connect", function (this: net.Socket) { this.destroy(); ok(true); }).once("error", () => ok(false)))), { plazo: 15_000, cada: 200 });
    apuntar(`rest-server con cuota: ${Math.round(maxBytes / 1e6)} MB`);
  };
  await arrancarCuota(2_000_000_000);

  // Repositorios y copias cada 5 minutos.
  const repos = new Map<string, { repo: string; contrasena: string; datos: string }>();
  const prepararEquipo = async (eq: { id: string; nombre: string }, destino: Record<string, unknown>, mb: number) => {
    const datos = dir(`datos-${eq.nombre}`);
    crearDatos(datos, mb);
    const repo = `r-${eq.nombre.toLowerCase()}`;
    const contrasena = Buffer.from(aleatorio(24)).toString("base64url");
    await consola.hecha(c, eq.id, "crear_repositorio", { id: repo, nombre: `Copias de ${eq.nombre}`, contrasena, destino }, { claveAdmin: CLAVE_ADMIN }, {}, 180_000);
    repos.set(eq.id, { repo, contrasena, datos });
    return { repo, datos };
  };
  const accesoAlmacen = async (eq: { id: string }) => {
    const acceso = await consola.hechaSellada<any>(c, eqA.id, "guarda_copias", { anadir: eq.id }, { claveAdmin: CLAVE_ADMIN });
    // Por el cable del almacén: la misma dirección con su puerto.
    const donde = String(acceso.destino.donde).replace(`:${puertoAlmacen}/`, `:${cableAlmacen.puerto}/`);
    return { id: `almacen-${eqA.id.slice(0, 8)}`, nombre: "ALMACEN", tipo: "rest", donde, usuario: acceso.destino.usuario, secreto: acceso.destino.secreto, ca_pem: acceso.destino.ca_pem, equipo_almacen: eqA.id };
  };
  const r1 = await prepararEquipo(eq1, await accesoAlmacen(eq1), 24);
  const r2 = await prepararEquipo(eq2, await accesoAlmacen(eq2), 24);
  const r3 = await prepararEquipo(eq3, { id: "cuota", nombre: "Servidor de la oficina", tipo: "rest", donde: `http://127.0.0.1:${puertoCuota}/` }, 16);
  const horario = { dias: TODOS, horas: [], reglas: [{ tipo: "intervalo", dias: TODOS, cada_min: 5, desde: "00:00", hasta: "23:55" }] };
  const copiaDe = (eqId: string) => {
    const r = repos.get(eqId)!;
    return { id: "docs", nombre: "Documentos", repo: r.repo, carpetas: [r.datos], exclusiones: [], horario, activa: true, gancho: null, solo_si_cambios: true };
  };
  const configDe = (eqId: string, porcentaje = 10) => ({ v: 1, copias: [copiaDe(eqId)], verificaciones: { [repos.get(eqId)!.repo]: { cada_dias: 1, porcentaje } } });
  for (const eq of [eq1, eq2, eq3]) await consola.hecha(c, eq.id, "config", { config: configDe(eq.id) }, { claveAdmin: CLAVE_ADMIN });
  const secretos = (eqId: string) => ({ repo: { repo: repos.get(eqId)!.repo, contrasena: repos.get(eqId)!.contrasena } });

  // Retención en el almacén para el repositorio de EQUIPO-1 (y la del propio equipo).
  const regla: Regla = { diarias: 0, semanales: 0, mensuales: 0, anuales: 0, plazos: { horarias: "1h" } };
  await consola.hecha(c, eq1.id, "cambiar_retencion", { repo: r1.repo, ...reglaParaOrden(regla) }, { ...secretos(eq1.id), claveAdmin: CLAVE_ADMIN });
  const claveAlmacen = nuevaClave();
  await consola.hecha(c, eq1.id, "clave_almacen", { repo: r1.repo, clave: claveAlmacen }, { ...secretos(eq1.id), claveAdmin: CLAVE_ADMIN });
  const e1 = await consola.equipo(c, eq1.id);
  const eA = await consola.equipo(c, eqA.id);
  const repoRes = e1.resumen!.repositorios!.find((r) => r.id === r1.repo)!;
  const en = almacenDe(repoRes, destinoDe(e1.resumen?.destinos, repoRes), [eA, e1]);
  comprobar(en, "El almacén del repositorio de EQUIPO-1", { repoRes, destinos: e1.resumen?.destinos });
  await consola.hecha(c, eqA.id, "retencion_almacen", { usuario: en.usuario, repo: en.carpeta, clave: claveAlmacen, retencion: reglaParaOrden(regla), horario: { dias: TODOS, hora: "03:00" }, verificar: true }, { claveAdmin: CLAVE_ADMIN });
  // EQUIPO-3 quita versiones él mismo (su destino no es «solo añadir»).
  await consola.hecha(c, eq3.id, "cambiar_retencion", { repo: r3.repo, ...reglaParaOrden(regla) }, { ...secretos(eq3.id), claveAdmin: CLAVE_ADMIN });

  // EQUIPO-1, también en una segunda consola (servidor 2).
  const consola2 = new Consola(s2);
  await consola2.primerArranque(CORREO, "Ana", CONTRASENA);
  const recibido = await consola2.ok("POST", "/api/clientes/recibir", { nombre: "Ferretería Altamar", sal_cliente: aB64(aleatorio(16)), usos: 5, dias: 7 });
  const srv2 = await consola2.ok("GET", "/api/servidor");
  const codigo = crearCodigo({ url: s2.url, identidad: srv2.identidad, huella_ca: srv2.huella_ca, ficha: recibido.ficha, sal_cliente: recibido.cliente.sal_cliente, nombre: "Consola en línea", cliente: "Ferretería Altamar", caduca: recibido.caduca });
  const leido = leerCodigo(codigo, new Date(), s1.url);
  comprobar(typeof leido !== "string", "Código de conexión", leido);
  await consola.hecha(c, eq1.id, "anadir_consola", cuerpoAnadir(leido, aB64(kCfg(await materialCliente(argon2, CLAVE_ADMIN, leido.sal_cliente))), c.sal_cliente), { claveAdmin: CLAVE_ADMIN }, {}, 120_000);
  const c2: Cliente = await consola2.ok("GET", `/api/clientes/${recibido.cliente.id}`);
  await esperar("EQUIPO-1 conectado también al servidor 2", async () => (await consola2.equipo(c2, eq1.id)).conectado, { plazo: 90_000, cada: 1000 });
  apuntar("Montado: servidor 1 (+ cable), servidor 2, ALMACEN (espejo, retención), EQUIPO-1 (dos consolas), EQUIPO-2, EQUIPO-3 (destino con cuota)");

  // Pestañas de consola.
  const pestanas: Pestana[] = [];
  for (let i = 0; i < PESTANAS; i++) pestanas.push(new Pestana(s1, () => consola.cookieSesion, c.id, () => equiposIds));
  pestanas.push(new Pestana(s2, () => consola2.cookieSesion, c2.id, () => [eq1.id]));
  for (const p of pestanas) p.arrancar();

  // -------------------------------------------------------------------------
  // Lo que ve la consola (se apunta cada cambio)
  // -------------------------------------------------------------------------
  const vistos = new Map<string, string>();
  const verConsola = async () => {
    try {
      const avisos = (await consola.ok("GET", `/api/clientes/${c.id}/avisos?abiertos=1`)) as any[];
      const ahora = new Set<string>();
      for (const a of avisos) {
        const k = `aviso:${a.id}`;
        ahora.add(k);
        if (!vistos.has(k)) {
          vistos.set(k, a.mensaje);
          apuntar(`CONSOLA aviso abierto (${a.tipo}, ${nombreDe.get(a.equipo) ?? "-"}): ${a.mensaje}`);
        }
      }
      for (const k of [...vistos.keys()].filter((k) => k.startsWith("aviso:") && !ahora.has(k))) {
        apuntar(`CONSOLA aviso cerrado: ${vistos.get(k)}`);
        vistos.delete(k);
      }
      for (const id of equiposIds) {
        const e = await consola.equipo(c, id);
        const ult = e.resumen?.copias?.find((k: any) => k.id === "docs")?.ultima as any;
        const estado = `${e.conectado ? "conectado" : "SIN CONEXIÓN"}${ult ? ` · última copia ${ult.estado}${ult.mensaje ? `: ${ult.mensaje}` : ""}` : ""}`;
        const k = `equipo:${id}`;
        if (vistos.get(k) !== estado) {
          vistos.set(k, estado);
          apuntar(`CONSOLA ${nombreDe.get(id)}: ${estado}`);
        }
        for (const r of e.resumen?.repositorios ?? []) {
          const v = informeDe(e.ultimo_informe as any, r.id)?.verificacion as any;
          if (v?.ultima) {
            const kv = `verif:${id}:${r.id}`;
            const t = `${v.resultado} ${v.mensaje_corto ?? ""} ${v.mensaje ?? ""}`.trim();
            if (vistos.get(kv) !== t) {
              vistos.set(kv, t);
              apuntar(`CONSOLA verificación de ${nombreDe.get(id)}: ${t}`);
            }
          }
        }
      }
      const espejoRes = (await consola.equipo(c, eqA.id)).resumen?.guarda_copias?.espejo as any;
      if (espejoRes?.resultado && vistos.get("espejo") !== espejoRes.resultado) {
        vistos.set("espejo", espejoRes.resultado);
        apuntar(`CONSOLA espejo: ${espejoRes.resultado}`);
      }
      for (const m of buzon.correos.slice(Number(vistos.get("correos") ?? 0))) apuntar(`CORREO: ${m.asunto}`);
      vistos.set("correos", String(buzon.correos.length));
    } catch (e) {
      // Con el servidor caído no se ve nada: es lo esperado.
      if (!String((e as Error).message).match(/ECONNREFUSED|ECONNRESET|socket hang up|Sin respuesta/)) apuntar(`(consola: ${(e as Error).message.split("\n")[0]})`);
    }
  };

  /** Órdenes sin terminar desde hace más de `min` minutos (lo que se quedaría «en marcha» para siempre). */
  const colgadas = async (min: number) => {
    const out: any[] = [];
    for (const id of equiposIds) {
      for (const o of await consola.ordenes(c, id)) {
        if (FINALES.includes(o.estado)) continue;
        const desde = new Date(o.not_before ?? o.emitida).getTime();
        if (Date.now() - desde > min * MIN) out.push({ equipo: nombreDe.get(id), tipo: o.tipo, estado: o.estado, emitida: o.emitida, mensaje: o.mensaje });
      }
    }
    return out;
  };
  const progresoLargo = new Map<string, number>();
  const tareasColgadas = async (min: number) => {
    const p = (await consola.ok("GET", `/api/clientes/${c.id}/progreso`)) as { equipo: string; tareas: any[] }[];
    const ahora = new Set<string>();
    const out: any[] = [];
    for (const x of p)
      for (const t of x.tareas) {
        const k = `${x.equipo}|${t.tipo}|${t.repo ?? t.copia ?? ""}`;
        ahora.add(k);
        if (!progresoLargo.has(k)) progresoLargo.set(k, Date.now());
        if (Date.now() - progresoLargo.get(k)! > min * MIN) out.push({ equipo: nombreDe.get(x.equipo), tipo: t.tipo, fase: t.fase, desde_min: Math.round((Date.now() - progresoLargo.get(k)!) / MIN) });
      }
    for (const k of [...progresoLargo.keys()]) if (!ahora.has(k)) progresoLargo.delete(k);
    return out;
  };

  // -------------------------------------------------------------------------
  // Muestras cada minuto
  // -------------------------------------------------------------------------
  const muestras: any[] = [];
  const canalesAbiertos = () => Object.fromEntries(agentes.map((a) => [a.nombre, (fs.existsSync(path.join(a.dir, "agent.log")) ? fs.readFileSync(path.join(a.dir, "agent.log"), "utf8") : "").split("Canal con Resguardo Server abierto").length - 1]));
  const tomarMuestra = async () => {
    const procs: Record<string, number | undefined> = { servidor1: s1.proceso?.hijo.pid, servidor2: s2.proceso?.hijo.pid };
    for (const a of agentes) procs[a.nombre] = a.proceso && !a.proceso.terminado ? a.proceso.hijo.pid : undefined;
    const m = await medir(Object.values(procs).filter((x): x is number => !!x));
    const sueltos = await hijosSueltos();
    const temporales = Object.fromEntries(
      agentes.map((a) => [a.nombre, contar(path.join(a.dir, "temp")).n + contar(a.dir, (n, r) => /\.tmp$|^descarga-|\.tmp-espejo$/.test(n) && !r.includes(`${path.sep}temp${path.sep}`)).n]),
    );
    const db = (d: string) => ({
      control_kb: Math.round(tamano(path.join(d, "control.db")) / 1024),
      control_wal_kb: Math.round(tamano(path.join(d, "control.db-wal")) / 1024),
      clientes_kb: Math.round(contar(path.join(d, "clientes"), (n) => n.endsWith(".db")).bytes / 1024),
      clientes_wal_kb: Math.round(contar(path.join(d, "clientes"), (n) => n.endsWith("-wal")).bytes / 1024),
      relevos: contar(path.join(d, "relevos")).n,
      log_kb: Math.round(tamano(path.join(d, "servidor.log")) / 1024),
    });
    const muestra = {
      min: Math.round(minuto() * 10) / 10,
      procesos: Object.fromEntries(Object.entries(procs).map(([k, pid]) => [k, pid ? (m.get(pid) ?? null) : null])),
      restic_y_rest_server: sueltos.length,
      sueltos: sueltos.map((x) => x.nombre),
      temporales,
      bd1: db(s1.datos),
      bd2: db(s2.datos),
      cache_restic_mb: Object.fromEntries(agentes.map((a) => [a.nombre, Math.round(contar(path.join(a.dir, "cache-restic")).bytes / 1e6)])),
      registros_kb: Object.fromEntries(agentes.map((a) => [a.nombre, Math.round((tamano(path.join(a.dir, "agent.log")) + tamano(path.join(a.dir, "agent.old.log"))) / 1024)])),
      canales_abiertos: canalesAbiertos(),
      cable: { conexiones: cable.conexiones, mb: Math.round(cable.bytes / 1e5) / 10 },
      cable_almacen: { conexiones: cableAlmacen.conexiones, mb: Math.round(cableAlmacen.bytes / 1e5) / 10 },
      pestanas: { peticiones: pestanas.reduce((s, p) => s + p.peticiones, 0), reconexiones: pestanas.reduce((s, p) => s + p.reconexiones, 0), errores: pestanas.reduce((s, p) => s + [...p.estados].filter(([k]) => k !== 200).reduce((a, [, v]) => a + v, 0), 0) },
    };
    muestras.push(muestra);
    fs.writeFileSync(dir("muestras.json"), JSON.stringify(muestras, null, 1));
    const corto = Object.entries(muestra.procesos)
      .map(([k, v]) => `${k}=${v ? `${v.priv_mb}MB/${v.handles}h/${v.hilos}t` : "-"}`)
      .join(" ");
    apuntar(`MUESTRA ${corto} · sueltos ${sueltos.length} · temp ${JSON.stringify(temporales)} · wal1 ${muestra.bd1.control_wal_kb}+${muestra.bd1.clientes_wal_kb} KB · pestañas ${JSON.stringify(muestra.pestanas)}`);
  };

  // -------------------------------------------------------------------------
  // Averías
  // -------------------------------------------------------------------------

  /** Cuánto tarda cada equipo en volver a aparecer «conectado» (el canal) tras una avería. */
  const vueltaDe = async (que: string, ids: string[], plazoMin = 12) => {
    const t = Date.now();
    const pendientes = new Set(ids);
    const tiempos: Record<string, number> = {};
    while (pendientes.size && Date.now() - t < plazoMin * MIN) {
      for (const id of [...pendientes]) {
        try {
          if ((await consola.equipo(c, id)).conectado) {
            tiempos[nombreDe.get(id)!] = Math.round((Date.now() - t) / 1000);
            pendientes.delete(id);
          }
        } catch {
          /* aún no */
        }
      }
      await dormir(2000);
    }
    for (const id of pendientes) incidencia(`${que}: ${nombreDe.get(id)} no volvió a conectar en ${plazoMin} min`);
    apuntar(`${que}: vuelta a «conectado» en segundos`, tiempos);
    return tiempos;
  };
  const reconexiones: Record<string, Record<string, number>> = {};

  const reiniciarServidor = async (caidaS: number) => {
    apuntar(`AVERÍA: servidor 1 parado ${caidaS} s (sin aviso, como un corte de luz)`);
    await s1.parar();
    await dormir(caidaS * 1000);
    await s1.arrancar();
    reconexiones[`servidor parado ${caidaS} s @${Math.round(minuto())}`] = await vueltaDe(`Servidor parado ${caidaS} s`, equiposIds);
  };
  const cortarRed = async (s: number) => {
    apuntar(`AVERÍA: red cortada entre los equipos y el servidor ${s} s (sin respuesta)`);
    cable.cortar();
    await dormir(s * 1000);
    cable.reanudar();
    reconexiones[`red cortada ${s} s @${Math.round(minuto())}`] = await vueltaDe(`Red cortada ${s} s`, equiposIds);
  };
  const reiniciarAgente = async (ag: AgenteR, id: string, caidaS = 3) => {
    apuntar(`AVERÍA: ${ag.nombre} matado (y arrancado ${caidaS} s después)`);
    await ag.parar();
    await dormir(caidaS * 1000);
    ag.arrancar();
    reconexiones[`${ag.nombre} reiniciado @${Math.round(minuto())}`] = await vueltaDe(`${ag.nombre} reiniciado`, [id]);
  };
  /** Mata un agente justo cuando está copiando (restic a medias, bloqueo que se queda). */
  const matarCopiando = async (ag: AgenteR, id: string, datos: string) => {
    try {
      // Algo grande que copiar (una copia normal dura 1–3 s): así se la pilla a medias.
      for (let i = 0; i < 40; i++) fs.writeFileSync(path.join(datos, "Relleno", `grande-${Math.floor(minuto())}-${i}.bin`), randomBytes(4 * 1024 * 1024));
      await esperar(`que ${ag.nombre} esté copiando`, async () => {
        const p = (await consola.ok("GET", `/api/clientes/${c.id}/progreso`)) as any[];
        return p.find((x) => x.equipo === id)?.tareas.some((t: any) => t.tipo === "copia");
      }, { plazo: 8 * MIN, cada: 300 });
      await dormir(3000);
      await reiniciarAgente(ag, id);
    } catch (e) {
      incidencia(`No se pudo pillar a ${ag.nombre} copiando`, (e as Error).message.split("\n")[0]);
    }
  };
  const almacenFuera = async (min: number) => {
    apuntar(`AVERÍA: ALMACEN apagado ${min} min (con su rest-server)`);
    await A.parar();
    await dormir(min * MIN);
    A.arrancar();
    reconexiones[`almacén apagado ${min} min @${Math.round(minuto())}`] = await vueltaDe("ALMACEN encendido", [eqA.id]);
  };
  const cortarAlmacen = async (s: number) => {
    apuntar(`AVERÍA: red cortada entre los equipos y el almacén ${s} s`);
    cableAlmacen.cortar();
    await dormir(s * 1000);
    cableAlmacen.reanudar();
  };
  /** Un archivo de datos del repositorio de EQUIPO-1 en el almacén, estropeado. */
  const estropearPack = () => {
    const packs = contar(carpetaAlmacen, (n, r) => r.includes(`${path.sep}${r1.repo}${path.sep}data${path.sep}`) && /^[0-9a-f]{64}$/.test(n));
    const lista: string[] = [];
    const pila = [carpetaAlmacen];
    while (pila.length) {
      const d = pila.pop()!;
      for (const e of fs.readdirSync(d, { withFileTypes: true })) {
        const q = path.join(d, e.name);
        if (e.isDirectory()) pila.push(q);
        else if (q.includes(`${path.sep}${r1.repo}${path.sep}data${path.sep}`)) lista.push(q);
      }
    }
    lista.sort((a, b) => tamano(b) - tamano(a));
    const p = lista[0];
    comprobar(p, "Hay archivos de datos que estropear", packs);
    fs.chmodSync(p, 0o666);
    const fd = fs.openSync(p, "r+");
    const b = Buffer.alloc(64);
    fs.readSync(fd, b, 0, 64, Math.floor(tamano(p) / 2));
    for (let i = 0; i < b.length; i++) b[i] ^= 0xff;
    fs.writeSync(fd, b, 0, 64, Math.floor(tamano(p) / 2));
    fs.closeSync(fd);
    apuntar(`AVERÍA: estropeados 64 bytes de ${path.relative(base, p)} (${tamano(p)} B)`);
  };

  /** Una orden durante la prueba: se apunta lo que contesta (sin parar la prueba si falla). */
  const orden = async (srvConsola: Consola, cl: Cliente, id: string, tipo: string, cuerpo: Record<string, unknown>, sec: any = {}, plazoMin = 15) => {
    try {
      const o = await srvConsola.mandar(cl, id, tipo, cuerpo, sec);
      const r = await srvConsola.resultado(cl, id, o, { plazo: plazoMin * MIN });
      apuntar(`ORDEN «${tipo}» a ${nombreDe.get(id)}: ${r.estado}${r.mensaje ? ` — ${r.mensaje}` : ""}`);
      return r;
    } catch (e) {
      incidencia(`Orden «${tipo}» a ${nombreDe.get(id)} sin resultado`, (e as Error).message.split("\n")[0]);
      return null;
    }
  };
  const sesionYDescarga = async () => {
    const sec = secretos(eq2.id);
    const s = new SesionE2E(consola, c);
    try {
      await s.abrir(eq2.id, "explorar", { repo: sec.repo.repo }, sec);
      const vs = (await s.pedir("versiones")).versiones as { id: string }[];
      await s.pedir("listar", { version: vs[0].id, ruta: WIN ? `/${r2.datos[0].toUpperCase()}${r2.datos.slice(2).replace(/\\/g, "/")}` : r2.datos });
      apuntar(`SESIÓN explorar en EQUIPO-2: ${vs.length} versiones`);
    } catch (e) {
      incidencia("Sesión «explorar» en EQUIPO-2", (e as Error).message.split("\n")[0]);
    } finally {
      await s.cerrar();
    }
  };

  // -------------------------------------------------------------------------
  // El programa de la prueba
  // -------------------------------------------------------------------------
  const plan: { min: number; que: string; f: () => Promise<unknown> }[] = [
    { min: 8, que: "servidor: reinicio rápido", f: () => reiniciarServidor(3) },
    { min: 14, que: "red cortada 90 s", f: () => cortarRed(90) },
    { min: 21, que: "EQUIPO-2 reiniciado", f: () => reiniciarAgente(E2, eq2.id) },
    { min: 26, que: "almacén apagado 4 min", f: () => almacenFuera(4) },
    { min: 36, que: "servidor parado 3 min", f: () => reiniciarServidor(180) },
    { min: 45, que: "red con el almacén cortada 2 min", f: () => cortarAlmacen(120) },
    {
      min: 50,
      que: "destino de EQUIPO-3 lleno",
      f: async () => {
        const usado = contar(datosCuota).bytes;
        await arrancarCuota(usado + 1_000_000);
      },
    },
    { min: 55, que: "EQUIPO-1 matado copiando", f: () => matarCopiando(E1, eq1.id, r1.datos) },
    {
      min: 62,
      que: "pack estropeado y verificación completa",
      f: async () => {
        estropearPack();
        await consola.hecha(c, eq1.id, "config", { config: configDe(eq1.id, 100) }, { claveAdmin: CLAVE_ADMIN }).catch((e) => incidencia("config 100 %", (e as Error).message));
        await orden(consola, c, eq1.id, "verificar_ahora", { repo: r1.repo }, {}, 20);
      },
    },
    {
      min: 70,
      que: "orden en vuelo y servidor reiniciado",
      f: async () => {
        const o = consola.mandar(c, eq2.id, "verificar_ahora", { repo: r2.repo }).catch(() => null);
        await dormir(300);
        await reiniciarServidor(5);
        const x = await o;
        if (x) await consola.resultado(c, eq2.id, x, { plazo: 15 * MIN }).then((r) => apuntar(`ORDEN en vuelo: ${r.estado} — ${r.mensaje}`), (e) => incidencia("La orden mandada justo antes de reiniciar no terminó", (e as Error).message));
      },
    },
    {
      min: 75,
      que: "destino de EQUIPO-3 con sitio otra vez",
      f: () => arrancarCuota(2_000_000_000),
    },
    { min: 80, que: "sesión abierta y servidor reiniciado", f: async () => {
      const s = new SesionE2E(consola, c);
      const sec = secretos(eq2.id);
      await s.abrir(eq2.id, "explorar", { repo: sec.repo.repo }, sec).catch((e) => incidencia("sesión antes de reiniciar", (e as Error).message));
      await reiniciarServidor(5);
      // La sesión que quedó abierta en el equipo: tiene que cerrarse sola.
      await s.cerrar();
    } },
  ];
  plan.push({
    min: 85,
    que: "orden larga cortada por un reinicio del agente",
    f: async () => {
      // Restaurar todo junto al original: tarda unos segundos; el agente se mata en medio.
      const sec = secretos(eq2.id);
      const vs = await (async () => {
        const s = new SesionE2E(consola, c);
        await s.abrir(eq2.id, "explorar", { repo: sec.repo.repo }, sec);
        const v = (await s.pedir("versiones")).versiones as { id: string }[];
        await s.cerrar();
        return v;
      })();
      const ruta = WIN ? `/${r2.datos[0].toUpperCase()}${r2.datos.slice(2).replace(/\\/g, "/")}/Relleno` : `${r2.datos}/Relleno`;
      const o = await consola.mandar(c, eq2.id, "restaurar", { repo: sec.repo.repo, version: vs[0].id, rutas: [ruta], destino: "junto", reemplazar: false }, sec);
      await consola.resultado(c, eq2.id, o, { estados: ["en_marcha", ...FINALES], plazo: 2 * MIN });
      await reiniciarAgente(E2, eq2.id, 2);
      const r = await consola.resultado(c, eq2.id, o, { plazo: 5 * MIN }).catch(() => null);
      if (!r) incidencia("La restauración cortada por el reinicio se quedó «en marcha»");
      else apuntar(`ORDEN larga cortada: ${r.estado} — ${r.mensaje}`);
    },
  });
  // Luego, en bucle hasta el final: lo de siempre (reinicios y cortes) para ver si algo crece.
  for (let m = 90; m < MINUTOS - 20; m += 15) {
    plan.push({ min: m, que: "servidor: reinicio rápido", f: () => reiniciarServidor(3) });
    plan.push({ min: m + 5, que: "EQUIPO-2 reiniciado", f: () => reiniciarAgente(E2, eq2.id) });
    plan.push({ min: m + 10, que: "red cortada 60 s", f: () => cortarRed(60) });
  }
  // Tareas periódicas (no son averías).
  const periodicas: { cada: number; desde: number; que: string; f: () => Promise<unknown>; ultima: number }[] = [
    { cada: 1, desde: 0, que: "cambiar datos", f: async () => [r1, r2, r3].forEach((r) => cambiarDatos(r.datos, 512)), ultima: -1 },
    { cada: 1, desde: 0, que: "muestra", f: tomarMuestra, ultima: -1 },
    { cada: 1, desde: 0, que: "consola", f: verConsola, ultima: -1 },
    { cada: 10, desde: 4, que: "verificar EQUIPO-2", f: () => orden(consola, c, eq2.id, "verificar_ahora", { repo: r2.repo }), ultima: -1 },
    { cada: 15, desde: 6, que: "sesión y descarga", f: sesionYDescarga, ultima: -1 },
    { cada: 20, desde: 10, que: "retención en el almacén", f: () => orden(consola, c, eqA.id, "aplicar_retencion_almacen", { usuario: en.usuario, repo: en.carpeta }, { claveAdmin: CLAVE_ADMIN }), ultima: -1 },
    { cada: 20, desde: 12, que: "espejo otra vez", f: () => ponerEspejo(hm(new Date())).catch((e) => incidencia("poner el espejo", (e as Error).message.split("\n")[0])), ultima: -1 },
    { cada: 10, desde: 3, que: "orden desde la segunda consola", f: () => orden(consola2, c2, eq1.id, "copiar_ahora", { copia: "docs", repo: r1.repo }), ultima: -1 },
    {
      cada: 5,
      desde: 5,
      que: "colgadas",
      f: async () => {
        const o = await colgadas(20);
        if (o.length) incidencia("Órdenes sin terminar desde hace más de 20 min", o);
        const t = await tareasColgadas(30);
        if (t.length) incidencia("Tareas «en marcha» desde hace más de 30 min", t);
      },
      ultima: -1,
    },
  ];

  const enCurso = new Set<Promise<unknown>>();
  const lanzar = (que: string, f: () => Promise<unknown>) => {
    const p = f()
      .catch((e) => incidencia(`${que}: ${(e as Error).message.split("\n")[0]}`))
      .finally(() => enCurso.delete(p));
    enCurso.add(p);
    return p;
  };
  let averia: Promise<unknown> | null = null;
  apuntar(`Empieza la prueba: ${MINUTOS} min`);
  const T1 = Date.now();
  const transcurrido = () => (Date.now() - T1) / MIN;
  while (transcurrido() < MINUTOS - 15) {
    const t = transcurrido();
    for (const p of periodicas) {
      const toca = Math.floor((t - p.desde) / p.cada);
      if (t >= p.desde && toca > p.ultima) {
        p.ultima = toca;
        // Las de la consola, sin esperar (que una lenta no frene a las demás); las medidas, en orden.
        if (p.que === "muestra" || p.que === "consola") await p.f().catch((e) => incidencia(`${p.que}: ${(e as Error).message}`));
        else lanzar(p.que, p.f);
      }
    }
    if (!averia && plan.length && plan[0].min <= t) {
      const a = plan.shift()!;
      averia = lanzar(`avería «${a.que}»`, a.f).finally(() => (averia = null));
    }
    await dormir(5000);
  }
  // Los últimos 15 minutos, en calma: todo tiene que volver a estar bien.
  apuntar("CALMA: sin averías; todo tiene que recuperarse solo");
  await Promise.race([Promise.allSettled([...enCurso]), dormir(10 * MIN)]);
  const finCalma = T1 + MINUTOS * MIN;
  while (Date.now() < finCalma) {
    await tomarMuestra();
    await verConsola();
    cambiarDatos(r1.datos, 64);
    await dormir(MIN);
  }

  // -------------------------------------------------------------------------
  // Al final
  // -------------------------------------------------------------------------
  const final: Record<string, unknown> = {};
  for (const id of equiposIds) {
    const e = await consola.equipo(c, id);
    final[nombreDe.get(id)!] = { conectado: e.conectado, ultima_copia: (e.resumen?.copias?.find((k: any) => k.id === "docs") as any)?.ultima ?? null };
    if (!e.conectado) incidencia(`Al final, ${nombreDe.get(id)} sin conexión`);
    const ult = (e.resumen?.copias?.find((k: any) => k.id === "docs") as any)?.ultima;
    if (ult && ult.estado !== "ok" && id !== eq1.id) incidencia(`Al final, la última copia de ${nombreDe.get(id)} no está bien`, ult);
    if (ult && Date.now() - new Date(ult.cuando).getTime() > 12 * MIN) incidencia(`Al final, ${nombreDe.get(id)} lleva más de 12 min sin copiar (copias cada 5 min)`, ult);
  }
  const o = await colgadas(10);
  if (o.length) incidencia("Al final, órdenes sin terminar", o);
  const t = await tareasColgadas(0);
  final.tareas_en_marcha = t;
  final.avisos_abiertos = ((await consola.ok("GET", `/api/clientes/${c.id}/avisos?abiertos=1`)) as any[]).map((a) => `${a.tipo}: ${a.mensaje}`);
  final.sueltos = await hijosSueltos();
  final.correos = buzon.correos.map((m) => m.asunto);
  final.errores_registro = Object.fromEntries(agentes.map((a) => [a.nombre, [...new Set(lineasConError(path.join(a.dir, "agent.log")).map((l) => l.slice(20).replace(/\d+/g, "#")))].slice(0, 40)]));
  final.errores_servidor = [...new Set([...lineasConError(path.join(registros, "servidor-1.log")), ...lineasConError(path.join(s1.datos, "servidor.log"))].map((l) => l.replace(/\d+/g, "#")))].slice(0, 40);
  final.reconexiones = reconexiones;
  final.pestanas = pestanas.map((p) => ({ peticiones: p.peticiones, estados: Object.fromEntries(p.estados), reconexiones: p.reconexiones }));
  for (const p of pestanas) p.cerrar();
  // Lo que creció: primera muestra tras montar, la de la mitad y la última.
  const crec: Record<string, unknown> = {};
  const m0 = muestras[Math.min(3, muestras.length - 1)];
  const mN = muestras[muestras.length - 1];
  for (const k of Object.keys(mN.procesos)) {
    const a = m0.procesos[k];
    const b = mN.procesos[k];
    if (a && b) crec[k] = { priv_mb: [a.priv_mb, b.priv_mb], handles: [a.handles, b.handles], hilos: [a.hilos, b.hilos], cpu_s: b.cpu_s };
  }
  final.crecimiento = crec;
  final.bd = { inicio: m0.bd1, fin: mN.bd1 };
  final.incidencias = incidencias;
  fs.writeFileSync(dir("final.json"), JSON.stringify(final, null, 1));
  console.log(JSON.stringify(final, null, 1));
  cable.cerrar();
  cableAlmacen.cerrar();
  buzon.cerrar();
  await pararTodo();
  console.log(`\nCarpeta: ${base} (muestras.json, diario.log, final.json, registros/)`);
}

const salir = async () => {
  await pararTodo();
  process.exit(1);
};
process.on("SIGINT", salir);
void principal().catch(async (e) => {
  console.error(e instanceof Fallo ? `✗ ${e.message}` : e);
  console.error(`Carpeta: ${base}`);
  await pararTodo();
  process.exit(1);
});
void borrarCarpeta; // se conserva siempre (es para mirarla)
