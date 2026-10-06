// Lo que necesita el escenario de extremo a extremo (escenario.ts) para poner
// en marcha programas de verdad y hablar con ellos: procesos (que se paran
// siempre, con sus hijos), puertos libres, HTTPS con la autoridad propia del
// servidor, TOTP, esperas con plazo (sin pausas fijas) y un servidor SMTP de
// mentira que guarda lo que recibe.
import fs from "node:fs";
import net from "node:net";
import path from "node:path";
import https from "node:https";
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { createHmac } from "node:crypto";

export const WIN = process.platform === "win32";
export const EXE = WIN ? ".exe" : "";
const T0 = Date.now();

// ---------------------------------------------------------------------------
// Registro
// ---------------------------------------------------------------------------

const seg = () => ((Date.now() - T0) / 1000).toFixed(1).padStart(6);
let pasoActual = "";
export function paso(titulo: string) {
  pasoActual = titulo;
  console.log(`\n[${seg()} s] ══ ${titulo}`);
}
export const log = (texto: string) => console.log(`[${seg()} s]    ${texto}`);
export const pasoEnCurso = () => pasoActual;

export class Fallo extends Error {}
export function comprobar(cond: unknown, mensaje: string, datos?: unknown): asserts cond {
  if (!cond) throw new Fallo(`${mensaje}${datos === undefined ? "" : `\n      ${typeof datos === "string" ? datos : JSON.stringify(datos, null, 1).slice(0, 4000)}`}`);
}
export function igual<T>(obtenido: T, esperado: T, mensaje: string) {
  const a = JSON.stringify(obtenido);
  const b = JSON.stringify(esperado);
  comprobar(a === b, `${mensaje}\n      obtenido: ${a}\n      esperado: ${b}`);
}

/** Espera a que `f` devuelva algo distinto de null/undefined/false (sin pausas fijas: pregunta cada `cada` ms). */
export async function esperar<T>(que: string, f: () => Promise<T | null | undefined | false> | T | null | undefined | false, opciones: { plazo?: number; cada?: number } = {}): Promise<T> {
  const plazo = opciones.plazo ?? 60_000;
  const cada = opciones.cada ?? 500;
  const fin = Date.now() + plazo;
  let ultimoError: unknown = null;
  for (;;) {
    try {
      const v = await f();
      if (v !== null && v !== undefined && v !== false) return v as T;
    } catch (e) {
      ultimoError = e;
    }
    if (Date.now() > fin) throw new Fallo(`Plazo agotado (${plazo / 1000} s) esperando: ${que}${ultimoError ? ` (último error: ${(ultimoError as Error).message})` : ""}`);
    await dormir(cada);
  }
}
export const dormir = (ms: number) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------------------
// Puertos y procesos
// ---------------------------------------------------------------------------

export function puertoLibre(): Promise<number> {
  return new Promise((ok, mal) => {
    const s = net.createServer();
    s.unref();
    s.on("error", mal);
    s.listen(0, "127.0.0.1", () => {
      const p = (s.address() as net.AddressInfo).port;
      s.close(() => ok(p));
    });
  });
}

export const escucha = (puerto: number) =>
  new Promise<boolean>((ok) => {
    const c = net.connect({ host: "127.0.0.1", port: puerto });
    c.once("connect", () => (c.destroy(), ok(true)));
    c.once("error", () => ok(false));
  });

const vivos = new Set<Proceso>();

export class Proceso {
  hijo: ChildProcess;
  salida: string[] = [];
  terminado = false;
  codigo: number | null = null;
  constructor(
    readonly nombre: string,
    programa: string,
    args: string[],
    opciones: { env?: Record<string, string>; registro: string; cwd?: string },
  ) {
    const archivo = fs.openSync(opciones.registro, "a");
    this.hijo = spawn(programa, args, {
      env: { ...process.env, ...opciones.env },
      cwd: opciones.cwd,
      stdio: ["ignore", "pipe", "pipe"],
      // En Linux, su propio grupo: así se para con todos sus hijos (restic, rest-server…).
      detached: !WIN,
      windowsHide: true,
    });
    const guardar = (b: Buffer) => {
      fs.writeSync(archivo, b);
      for (const l of b.toString("utf8").split(/\r?\n/)) if (l) this.salida.push(l);
      if (this.salida.length > 2000) this.salida.splice(0, this.salida.length - 2000);
    };
    this.hijo.stdout!.on("data", guardar);
    this.hijo.stderr!.on("data", guardar);
    this.hijo.on("exit", (c) => {
      this.terminado = true;
      this.codigo = c;
      vivos.delete(this);
      try {
        fs.closeSync(archivo);
      } catch {
        /* ya cerrado */
      }
    });
    vivos.add(this);
  }
  /** Para el proceso y todo lo que lanzó (en Windows, el árbol con taskkill). */
  async parar() {
    if (this.terminado || this.hijo.pid === undefined) return;
    if (WIN) spawnSync("taskkill", ["/T", "/F", "/PID", String(this.hijo.pid)], { stdio: "ignore" });
    else {
      try {
        process.kill(-this.hijo.pid, "SIGKILL");
      } catch {
        /* ya no estaba */
      }
    }
    await esperar(`que termine ${this.nombre}`, () => this.terminado, { plazo: 15_000, cada: 100 });
  }
}

/** Para todo lo que sigue en marcha (al terminar, bien o mal). */
export async function pararTodo() {
  for (const p of [...vivos]) await p.parar().catch(() => {});
}

/** Ejecuta un programa hasta que termine y devuelve su salida (para la línea de órdenes del agente y del servidor). */
export function ejecutar(programa: string, args: string[], opciones: { env?: Record<string, string>; entrada?: string; plazo?: number } = {}) {
  const r = spawnSync(programa, args, {
    env: { ...process.env, ...opciones.env },
    input: opciones.entrada,
    encoding: "utf8",
    timeout: opciones.plazo ?? 120_000,
    windowsHide: true,
  });
  return { codigo: r.status, salida: `${r.stdout ?? ""}${r.stderr ?? ""}`, error: r.error };
}

/** Quita una carpeta aunque tenga archivos de solo lectura (los de restic). */
export function borrarCarpeta(dir: string) {
  for (let i = 0; i < 5; i++) {
    try {
      fs.rmSync(dir, { recursive: true, force: true, maxRetries: 3 });
      return;
    } catch {
      if (WIN) spawnSync("attrib", ["-R", path.join(dir, "*"), "/S", "/D"], { stdio: "ignore" });
    }
  }
}

// ---------------------------------------------------------------------------
// HTTPS con la autoridad propia del servidor
// ---------------------------------------------------------------------------

export interface Respuesta<T = any> {
  estado: number;
  cuerpo: T;
  cabeceras: Record<string, string | string[] | undefined>;
  texto: string;
}

export function pedirHttps(url: string, opciones: { metodo?: string; cuerpo?: unknown; cabeceras?: Record<string, string>; ca?: string; plazo?: number } = {}): Promise<Respuesta> {
  return new Promise((ok, mal) => {
    const u = new URL(url);
    // Un Buffer va tal cual (p. ej. un archivo de una publicación); lo demás, como JSON.
    const crudo = Buffer.isBuffer(opciones.cuerpo);
    const datos = opciones.cuerpo === undefined ? undefined : crudo ? (opciones.cuerpo as Buffer) : Buffer.from(JSON.stringify(opciones.cuerpo));
    const r = https.request(
      {
        host: u.hostname,
        port: u.port,
        path: u.pathname + u.search,
        method: opciones.metodo ?? "GET",
        ca: opciones.ca,
        // Sin autoridad (la primera vez, para descargarla): como el agente al vincularse.
        rejectUnauthorized: !!opciones.ca,
        headers: { Accept: "application/json", ...(datos ? { "Content-Type": crudo ? "application/octet-stream" : "application/json", "Content-Length": String(datos.length) } : {}), ...opciones.cabeceras },
        timeout: opciones.plazo ?? 60_000,
      },
      (res) => {
        const trozos: Buffer[] = [];
        res.on("data", (b) => trozos.push(b));
        res.on("end", () => {
          const texto = Buffer.concat(trozos).toString("utf8");
          let cuerpo: unknown = texto;
          try {
            cuerpo = texto ? JSON.parse(texto) : null;
          } catch {
            /* no es JSON */
          }
          ok({ estado: res.statusCode ?? 0, cuerpo, cabeceras: res.headers, texto });
        });
      },
    );
    r.on("timeout", () => r.destroy(new Error(`Sin respuesta de ${url}`)));
    r.on("error", mal);
    if (datos) r.write(datos);
    r.end();
  });
}

// ---------------------------------------------------------------------------
// TOTP (RFC 6238, SHA-1, 6 cifras, 30 s), como el autenticador del móvil
// ---------------------------------------------------------------------------

export function deBase32(s: string): Buffer {
  const alfabeto = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
  let bits = "";
  for (const c of s.replace(/=+$/, "").toUpperCase()) {
    const v = alfabeto.indexOf(c);
    if (v < 0) throw new Error("base32 no válido");
    bits += v.toString(2).padStart(5, "0");
  }
  const out: number[] = [];
  for (let i = 0; i + 8 <= bits.length; i += 8) out.push(parseInt(bits.slice(i, i + 8), 2));
  return Buffer.from(out);
}

export function hotp(secreto: Buffer, paso: number): string {
  const msg = Buffer.alloc(8);
  msg.writeBigUInt64BE(BigInt(paso));
  const h = createHmac("sha1", secreto).update(msg).digest();
  const o = h[h.length - 1] & 0x0f;
  const n = ((h[o] & 0x7f) << 24) | (h[o + 1] << 16) | (h[o + 2] << 8) | h[o + 3];
  return String(n % 1_000_000).padStart(6, "0");
}

/** Un autenticador: cada código vale una sola vez (el servidor admite ±1 paso), así que se lleva la cuenta del último. */
export class Autenticador {
  private ultimo = -1;
  constructor(private secreto: Buffer) {}
  async codigo(): Promise<string> {
    for (;;) {
      const ahora = Math.floor(Date.now() / 1000 / 30);
      const p = Math.max(ahora - 1, this.ultimo + 1);
      if (p <= ahora + 1) {
        this.ultimo = p;
        return hotp(this.secreto, p);
      }
      await dormir(1000);
    }
  }
}

// ---------------------------------------------------------------------------
// Servidor SMTP de mentira (127.0.0.1, sin cifrar ni usuario): guarda cada correo
// ---------------------------------------------------------------------------

export interface Correo {
  de: string;
  para: string[];
  datos: string;
  asunto: string;
}

export class BuzonSmtp {
  correos: Correo[] = [];
  private servidor: net.Server;
  puerto = 0;
  /** `carpeta`: donde dejar cada correo recibido (.eml), para mirarlo si algo falla. */
  constructor(private carpeta?: string) {
    this.servidor = net.createServer((s) => this.atender(s));
  }
  async abrir() {
    await new Promise<void>((ok) => this.servidor.listen(0, "127.0.0.1", ok));
    this.puerto = (this.servidor.address() as net.AddressInfo).port;
  }
  cerrar() {
    this.servidor.close();
  }
  private atender(s: net.Socket) {
    let resto = "";
    let enDatos = false;
    let actual: Correo = { de: "", para: [], datos: "", asunto: "" };
    const decir = (l: string) => s.write(`${l}\r\n`);
    decir("220 buzon.prueba ESMTP de mentira");
    s.on("data", (b) => {
      resto += b.toString("latin1");
      for (;;) {
        const i = resto.indexOf("\r\n");
        if (i < 0) break;
        const linea = resto.slice(0, i);
        resto = resto.slice(i + 2);
        if (enDatos) {
          if (linea === ".") {
            enDatos = false;
            const cuerpo = Buffer.from(actual.datos, "latin1").toString("utf8");
            actual.datos = cuerpo;
            actual.asunto = decodificarAsunto(cuerpo);
            this.correos.push(actual);
            if (this.carpeta) fs.writeFileSync(path.join(this.carpeta, `correo-${this.correos.length}.eml`), cuerpo);
            actual = { de: "", para: [], datos: "", asunto: "" };
            decir("250 2.0.0 Guardado");
          } else actual.datos += `${linea.startsWith("..") ? linea.slice(1) : linea}\r\n`;
          continue;
        }
        const orden = linea.slice(0, 4).toUpperCase();
        if (orden === "EHLO") {
          s.write("250-buzon.prueba\r\n250-8BITMIME\r\n250 SMTPUTF8\r\n");
        } else if (orden === "HELO") decir("250 buzon.prueba");
        else if (orden === "MAIL") {
          actual.de = linea.replace(/^MAIL FROM:\s*/i, "");
          decir("250 2.1.0 Vale");
        } else if (orden === "RCPT") {
          actual.para.push(linea.replace(/^RCPT TO:\s*/i, ""));
          decir("250 2.1.5 Vale");
        } else if (orden === "DATA") {
          enDatos = true;
          decir("354 Adelante");
        } else if (orden === "RSET") {
          actual = { de: "", para: [], datos: "", asunto: "" };
          decir("250 Vale");
        } else if (orden === "NOOP") decir("250 Vale");
        else if (orden === "QUIT") {
          decir("221 Adiós");
          s.end();
        } else decir("502 No sé hacer eso");
      }
    });
    s.on("error", () => {});
  }
}

/** El asunto de un correo (con las palabras codificadas de RFC 2047 en UTF-8). */
function decodificarAsunto(correo: string): string {
  const cab = correo.split(/\r?\n\r?\n/)[0].replace(/\r?\n[ \t]+/g, " ");
  const m = /^Subject:\s*(.*)$/im.exec(cab);
  if (!m) return "";
  // Las palabras codificadas seguidas se juntan en bytes antes de leer el UTF-8 (una «ñ» puede quedar partida entre dos).
  return m[1]
    .replace(/=\?utf-8\?[bq]\?[^?]*\?=(?:\s+=\?utf-8\?[bq]\?[^?]*\?=)*/gi, (bloque: string) => {
      const bytes: Buffer[] = [];
      for (const [, modo, txt] of bloque.matchAll(/=\?utf-8\?([bq])\?([^?]*)\?=/gi))
        bytes.push(modo.toLowerCase() === "b" ? Buffer.from(txt, "base64") : Buffer.from(txt.replace(/_/g, " ").replace(/=([0-9a-f]{2})/gi, (_x, h: string) => String.fromCharCode(parseInt(h, 16))), "latin1"));
      return Buffer.concat(bytes).toString("utf8");
    })
    .trim();
}
