// Sesiones interactivas con un equipo (elegir carpetas, explorar versiones,
// progreso): api-servidor.md §7. La consola elige el id y una clave de sesión
// aleatoria, los manda dentro de la orden sellada que abre la sesión, y desde
// ahí cada mensaje va cifrado con XChaCha20-Poly1305 (claves por dirección,
// derivadas con HKDF). El servidor solo reenvía bytes opacos.
//
// Mensajes (propuesta de la consola, ver README «Preguntas»):
//   consola → equipo: { i, op, ...args }
//   equipo → consola: { i, re?: i_de_la_petición, op, ...datos | error }
import * as api from "./api";
import { enFondo } from "./actividad.svelte";
import { aB64, aleatorio, borrar, deB64 } from "./cripto/bytes";
import { claveDireccion, cifrarMensaje, descifrarMensaje } from "./cripto/simetrico";
import { mandarOrden, type Secretos } from "./ordenar";
import type { Cliente, Equipo, Orden } from "./tipos";

export interface MensajeEquipo {
  i: number;
  re?: number;
  op: string;
  error?: string;
  [k: string]: unknown;
}

export class Sesion {
  readonly id = crypto.randomUUID();
  private clave = aleatorio(32);
  private kConsola: Uint8Array;
  private kEquipo: Uint8Array;
  private i = 0;
  private ultimoEquipo = -1;
  private esperas = new Map<number, { ok: (m: MensajeEquipo) => void; mal: (e: Error) => void; t: ReturnType<typeof setTimeout>; ms: number }>();
  private parar = new AbortController();
  private oyentes = new Set<(m: MensajeEquipo) => void>();
  orden: Orden | null = null;
  /**
   * Lo que el equipo dice que admite en esta sesión (v1.15, `ops` en «lista»).
   * `null`: un agente anterior, que no lo dice (solo las operaciones de v1.2).
   */
  ops: string[] | null = null;
  /** ¿Admite el equipo esta operación? (con un agente anterior, solo las de siempre). */
  admite(op: string): boolean {
    return this.ops?.includes(op) ?? false;
  }
  abierta = false;
  /** Llega el primer mensaje del equipo («lista»): ya se puede pedir. */
  readonly lista: Promise<void>;
  private listaOk!: () => void;
  private listaMal!: (e: Error) => void;

  private constructor(
    readonly cliente: Cliente,
    readonly equipo: Equipo,
  ) {
    this.kConsola = claveDireccion(this.clave, this.id, "consola");
    this.kEquipo = claveDireccion(this.clave, this.id, "equipo");
    this.lista = new Promise((ok, mal) => {
      this.listaOk = ok;
      this.listaMal = mal;
    });
    this.lista.catch(() => {});
  }

  /** Abre la sesión con su orden (que lleva la clave de sesión dentro del sobre). */
  static async abrir(opts: {
    cliente: Cliente;
    equipo: Equipo;
    tipo: "abrir_sesion" | "explorar" | "elegir_carpetas";
    secretos?: Secretos;
    cuerpo?: Record<string, unknown>;
    alPaso?: (t: string) => void;
  }): Promise<Sesion> {
    const s = new Sesion(opts.cliente, opts.equipo);
    try {
      s.orden = await mandarOrden({
        cliente: opts.cliente,
        equipo: opts.equipo,
        tipo: opts.tipo,
        // El id va también dentro del sobre: así el agente sabe, autenticado, qué sesión abre esta orden.
        cuerpo: { ...(opts.cuerpo ?? {}), sesion: s.id, clave_sesion: aB64(s.clave) },
        secretos: opts.secretos,
        sesion: s.id,
        alPaso: opts.alPaso,
      });
    } catch (e) {
      s.olvidar();
      throw e;
    }
    s.abierta = true;
    void api.escucharSesion(opts.cliente.id, s.id, (m) => s.recibir(m.n, m.cifrado), s.parar.signal, (e) => {
      if (e.estado === 404) s.cerrada("La sesión terminó (el equipo la cerró o pasaron 10 min sin uso).");
    });
    // Mientras no llegue el «lista», se mira la orden: si el equipo la rechaza
    // (contraseña o clave incorrecta, bloqueo…), se dice por qué.
    let listo = false;
    void s.lista.then(() => (listo = true));
    const ordenId = s.orden.id;
    const vigilar = setInterval(async () => {
      if (listo || !s.abierta) return clearInterval(vigilar);
      try {
        const o = (await enFondo(() => api.ordenesEquipo(opts.cliente.id, opts.equipo.id, 10))).find((x) => x.id === ordenId);
        if (o && ["rechazada", "fallida", "caducada", "cancelada"].includes(o.estado)) {
          clearInterval(vigilar);
          s.orden = o;
          s.cerrada(o.mensaje ?? "El equipo no aceptó abrir la sesión.");
        }
      } catch {
        /* se reintenta */
      }
    }, 1500);
    // Si el equipo no responde en 90 s, se avisa (puede estar sin conexión).
    setTimeout(() => {
      clearInterval(vigilar);
      s.listaMal(new Error(opts.equipo.conectado ? "El equipo no responde. Puede estar ocupado: vuelve a intentarlo en un momento." : "El equipo no está conectado. Abrirá la sesión cuando vuelva."));
    }, 90_000);
    return s;
  }

  private recibir(n: number, cifrado: string) {
    let m: MensajeEquipo;
    try {
      m = descifrarMensaje<MensajeEquipo>(this.kEquipo, this.id, deB64(cifrado));
    } catch {
      return; // no es de este equipo o está alterado: se descarta
    }
    // Repeticiones: el contador del equipo solo puede crecer.
    if (typeof m.i === "number") {
      if (m.i <= this.ultimoEquipo) return;
      this.ultimoEquipo = m.i;
    }
    void n;
    if (m.op === "lista") {
      if (Array.isArray(m.ops)) this.ops = m.ops.filter((x): x is string => typeof x === "string");
      this.listaOk();
    }
    // v1.33: el equipo sigue con una operación larga (`sobre`: la petición): se le espera más.
    if (m.op === "trabajando" && typeof m.sobre === "number" && this.esperas.has(m.sobre)) {
      const w = this.esperas.get(m.sobre)!;
      clearTimeout(w.t);
      w.t = this.vencer(m.sobre, w.ms);
    }
    if (m.re !== undefined && this.esperas.has(m.re)) {
      const w = this.esperas.get(m.re)!;
      clearTimeout(w.t);
      this.esperas.delete(m.re);
      if (m.error) w.mal(new Error(m.error));
      else w.ok(m);
    }
    for (const f of this.oyentes) f(m);
  }

  private cerrada(motivo: string) {
    this.abierta = false;
    for (const w of this.esperas.values()) {
      clearTimeout(w.t);
      w.mal(new Error(motivo));
    }
    this.esperas.clear();
    this.listaMal(new Error(motivo));
  }

  /** Avisa de cada mensaje del equipo (progreso, por ejemplo). */
  escuchar(f: (m: MensajeEquipo) => void) {
    this.oyentes.add(f);
    return () => this.oyentes.delete(f);
  }

  async enviar(datos: Record<string, unknown>, i = ++this.i): Promise<number> {
    await api.enviarASesion(this.cliente.id, this.id, aB64(cifrarMensaje(this.kConsola, this.id, { ...datos, i })));
    return i;
  }

  private vencer(i: number, ms: number) {
    return setTimeout(() => {
      const w = this.esperas.get(i);
      this.esperas.delete(i);
      w?.mal(new Error("El equipo tarda en responder. Vuelve a intentarlo."));
    }, ms);
  }

  /**
   * Envía una petición y espera su respuesta (como mucho `ms` sin noticias:
   * cada «trabajando» del equipo vuelve a contar el plazo).
   */
  async pedir<T extends MensajeEquipo = MensajeEquipo>(op: string, args: Record<string, unknown> = {}, ms = 30_000): Promise<T> {
    await this.lista;
    const i = ++this.i;
    const respuesta = new Promise<MensajeEquipo>((ok, mal) => {
      this.esperas.set(i, { ok, mal, t: this.vencer(i, ms), ms });
    });
    try {
      await this.enviar({ op, ...args }, i);
    } catch (e) {
      clearTimeout(this.esperas.get(i)?.t);
      this.esperas.delete(i);
      throw e;
    }
    return (await respuesta) as T;
  }

  /** Cierra la sesión en el servidor y borra las claves de la memoria. */
  async cerrar() {
    if (this.abierta) {
      try {
        await this.enviar({ op: "cerrar" });
      } catch {
        /* da igual */
      }
      try {
        await api.cerrarSesion(this.cliente.id, this.id);
      } catch {
        /* ya estaba cerrada */
      }
    }
    this.olvidar();
  }

  private olvidar() {
    this.abierta = false;
    this.parar.abort();
    borrar(this.clave, this.kConsola, this.kEquipo);
    for (const w of this.esperas.values()) clearTimeout(w.t);
    this.esperas.clear();
  }
}
