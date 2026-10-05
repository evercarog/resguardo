// «La consola en vivo» del escenario: el canal `GET /api/clientes/{c}/vivo`
// (WebSocket) como lo abre el navegador (cookie de sesión y `Origin` del propio
// servidor), con un cliente WebSocket mínimo sobre TLS (node:tls) que confía en
// la autoridad del servidor de pruebas. Apunta cada mensaje con la hora en que
// llegó, para medir cuánto tarda la consola en enterarse.
import tls from "node:tls";
import { createHash, randomBytes } from "node:crypto";
import { Fallo } from "./entorno";

export interface MensajeVivo {
  /** Date.now() al llegar. */
  llegada: number;
  m: { t: string; equipo?: string | null; orden?: string; estado?: string; [k: string]: unknown };
}

export class OyenteVivo {
  readonly mensajes: MensajeVivo[] = [];
  private socket: tls.TLSSocket | null = null;
  private resto = Buffer.alloc(0);
  private trozos: Buffer[] = [];
  private esperando = new Set<() => void>();
  cerrado = false;
  codigoCierre: number | null = null;

  /** Abre el canal de un cliente en `base` (https://127.0.0.1:<puerto>) con esa cookie. */
  static async abrir(base: string, cliente: string, cookie: string, ca: string, origen = base): Promise<OyenteVivo> {
    const url = new URL(base);
    const o = new OyenteVivo();
    const clave = randomBytes(16).toString("base64");
    const s = tls.connect({ host: url.hostname, port: Number(url.port), ca, servername: url.hostname === "127.0.0.1" ? undefined : url.hostname });
    o.socket = s;
    await new Promise<void>((ok, mal) => {
      s.once("secureConnect", ok);
      s.once("error", mal);
    });
    s.write(
      [
        `GET /api/clientes/${cliente}/vivo HTTP/1.1`,
        `Host: ${url.host}`,
        "Upgrade: websocket",
        "Connection: Upgrade",
        `Sec-WebSocket-Key: ${clave}`,
        "Sec-WebSocket-Version: 13",
        `Origin: ${origen}`,
        `Cookie: ${cookie}`,
        "",
        "",
      ].join("\r\n"),
    );
    const cabecera = await new Promise<string>((ok, mal) => {
      let buf = Buffer.alloc(0);
      const alDato = (d: Buffer) => {
        buf = Buffer.concat([buf, d]);
        const fin = buf.indexOf("\r\n\r\n");
        if (fin < 0) return;
        s.off("data", alDato);
        o.resto = buf.subarray(fin + 4);
        ok(buf.subarray(0, fin).toString("utf8"));
      };
      s.on("data", alDato);
      s.once("error", mal);
      s.once("close", () => mal(new Fallo("El servidor cerró antes de abrir el canal en vivo")));
    });
    const estado = Number(/^HTTP\/1\.1 (\d+)/.exec(cabecera)?.[1] ?? 0);
    if (estado !== 101) {
      s.destroy();
      throw Object.assign(new Fallo(`El canal en vivo no se abrió (${estado}): ${cabecera.split("\r\n")[0]}`), { estado });
    }
    const aceptada = createHash("sha1").update(`${clave}258EAFA5-E914-47DA-95CA-C5AB0DC85B11`).digest("base64");
    if (!cabecera.toLowerCase().includes(`sec-websocket-accept: ${aceptada.toLowerCase()}`)) throw new Fallo(`Respuesta del WebSocket no válida: ${cabecera.split("\r\n")[0]}`);
    s.on("data", (d: Buffer) => o.leer(d));
    s.on("close", () => {
      o.cerrado = true;
      o.avisar();
    });
    s.on("error", () => {});
    if (o.resto.length) o.leer(Buffer.alloc(0));
    return o;
  }

  private avisar() {
    for (const f of [...this.esperando]) f();
  }

  /** Lee las tramas que hayan llegado (del servidor: sin máscara). */
  private leer(d: Buffer) {
    this.resto = Buffer.concat([this.resto, d]);
    for (;;) {
      const b = this.resto;
      if (b.length < 2) return;
      const fin = (b[0] & 0x80) !== 0;
      const op = b[0] & 0x0f;
      let largo = b[1] & 0x7f;
      let p = 2;
      if (largo === 126) {
        if (b.length < 4) return;
        largo = b.readUInt16BE(2);
        p = 4;
      } else if (largo === 127) {
        if (b.length < 10) return;
        largo = Number(b.readBigUInt64BE(2));
        p = 10;
      }
      if (b.length < p + largo) return;
      const datos = b.subarray(p, p + largo);
      this.resto = b.subarray(p + largo);
      if (op === 0x9) this.mandar(0xa, datos);
      else if (op === 0x8) {
        this.codigoCierre = datos.length >= 2 ? datos.readUInt16BE(0) : null;
        this.socket?.end();
      } else if (op === 0x1 || op === 0x0) {
        this.trozos.push(Buffer.from(datos));
        if (fin) {
          const texto = Buffer.concat(this.trozos).toString("utf8");
          this.trozos = [];
          try {
            this.mensajes.push({ llegada: Date.now(), m: JSON.parse(texto) });
          } catch {
            /* no es JSON: se ignora */
          }
          this.avisar();
        }
      }
    }
  }

  /** Una trama del cliente (con máscara, como exige el protocolo). */
  private mandar(op: number, datos: Buffer) {
    const mascara = randomBytes(4);
    const cab = datos.length < 126 ? Buffer.from([0x80 | op, 0x80 | datos.length]) : Buffer.concat([Buffer.from([0x80 | op, 0x80 | 126]), Buffer.from([datos.length >> 8, datos.length & 0xff])]);
    const cuerpo = Buffer.from(datos.map((x, i) => x ^ mascara[i % 4]));
    this.socket?.write(Buffer.concat([cab, mascara, cuerpo]));
  }

  /** Espera un mensaje (desde `desde`, un índice de `mensajes`) que cumpla `f`. */
  async esperar(que: string, f: (m: MensajeVivo["m"]) => boolean, opciones: { plazo?: number; desde?: number } = {}): Promise<MensajeVivo> {
    const plazo = opciones.plazo ?? 30_000;
    const limite = Date.now() + plazo;
    for (;;) {
      const x = this.mensajes.slice(opciones.desde ?? 0).find((x) => f(x.m));
      if (x) return x;
      const ultimos = JSON.stringify(this.mensajes.slice(-10).map((x) => x.m));
      if (this.cerrado) throw new Fallo(`El canal en vivo se cerró esperando ${que}. Últimos: ${ultimos}`);
      const queda = limite - Date.now();
      if (queda <= 0) throw new Fallo(`No llegó por el canal en vivo: ${que} (${plazo / 1000} s). Últimos: ${ultimos}`);
      await new Promise<void>((ok) => {
        const t = setTimeout(hecho, Math.min(queda, 1000));
        const yo = this;
        function hecho() {
          clearTimeout(t);
          yo.esperando.delete(hecho);
          ok();
        }
        this.esperando.add(hecho);
      });
    }
  }

  cerrar() {
    if (this.cerrado) return;
    this.mandar(0x8, Buffer.from([0x03, 0xe8]));
    this.socket?.end();
    this.cerrado = true;
  }
}
