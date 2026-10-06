// Los actores del escenario: Resguardo Server (el programa de verdad, en un
// puerto libre y con su carpeta de datos temporal), Resguardo Agente (el
// programa de verdad, en primer plano y en modo de pruebas) y «la consola»: lo
// que hace el navegador, con la criptografía de la consola (src/lib/cripto,
// src/lib/retencion…), contra la API real.
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import path from "node:path";
import { randomUUID } from "node:crypto";
import { argon2id } from "hash-wasm";
import { aB64, aleatorio, deB64, deUtf8 } from "../../src/lib/cripto/bytes";
import { abrir, parEfimero } from "../../src/lib/cripto/sobre";
import { etiquetaEquipo, etiquetaValida, hashCodigo, kCfg, materialCliente, pruebaAdmin, pruebaCodigo, resultadoFirmado, sasV3, verificador, ARGON2, type Argon2 } from "../../src/lib/cripto/claves";
import { generarCodigo, LARGO_PREPARADO } from "../../src/lib/codigo";
import { cola } from "../../src/lib/cola";
import { esDestructiva, NIVEL, PIDE_TAMBIEN_ADMIN, sellarOrden, type Autorizacion } from "../../src/lib/cripto/ordenes";
import { claveDireccion, cifrarMensaje, descifrarMensaje } from "../../src/lib/cripto/simetrico";
import type { Cliente, Equipo, Orden } from "../../src/lib/tipos";
import { Autenticador, comprobar, deBase32, EXE, ejecutar, escucha, esperar, Fallo, igual, log, pedirHttps, Proceso, type Respuesta } from "./entorno";

/** Argon2id como la consola (hash-wasm), con memoria: en el escenario la misma clave y sal se usan muchas veces. */
const memo = new Map<string, Uint8Array>();
export const argon2: Argon2 = async (clave, sal) => {
  const k = `${aB64(clave)}|${aB64(sal)}`;
  let v = memo.get(k);
  if (!v) {
    v = (await argon2id({ password: clave, salt: sal, parallelism: ARGON2.hilos, iterations: ARGON2.pasadas, memorySize: ARGON2.memoriaKiB, hashLength: ARGON2.salida, outputType: "binary" })) as Uint8Array;
    memo.set(k, v);
  }
  return v.slice();
};

export const FINALES = ["hecha", "fallida", "rechazada", "cancelada", "caducada"];

// ---------------------------------------------------------------------------
// Resguardo Server
// ---------------------------------------------------------------------------

/** La llave pública de publicación de PRUEBAS (crates/protocolo/tests/fixtures), docs/actualizaciones.md. */
export const LLAVE_PRUEBAS_PUB = fileURLToPath(new URL("../../../crates/protocolo/tests/fixtures/llave-pruebas-a.pub", import.meta.url));

export class Servidor {
  proceso: Proceso | null = null;
  ca = "";
  constructor(
    readonly nombre: string,
    readonly binario: string,
    readonly datos: string,
    readonly puerto: number,
    readonly registro: string,
  ) {}
  get url() {
    return `https://127.0.0.1:${this.puerto}`;
  }
  async arrancar() {
    this.proceso = new Proceso(this.nombre, this.binario, ["--datos", this.datos, "--escuchar", `127.0.0.1:${this.puerto}`], {
      env: { RESGUARDO_PRUEBA_SIN_ESPERA: "1", RESGUARDO_LLAVES_PRUEBAS: LLAVE_PRUEBAS_PUB },
      registro: this.registro,
    });
    await esperar(`que ${this.nombre} escuche en ${this.puerto}`, async () => {
      comprobar(!this.proceso!.terminado, `${this.nombre} terminó al arrancar (código ${this.proceso!.codigo})`, this.proceso!.salida.join("\n"));
      return (await escucha(this.puerto)) && fs.existsSync(path.join(this.datos, "tls", "ca.crt"));
    }, { plazo: 60_000, cada: 200 });
    this.ca = fs.readFileSync(path.join(this.datos, "tls", "ca.crt"), "utf8");
    log(`${this.nombre} en marcha en ${this.url} (datos en ${this.datos})`);
  }
  codigoArranque(): string {
    return fs.readFileSync(path.join(this.datos, "codigo-arranque.txt"), "utf8").trim();
  }
  async parar() {
    await this.proceso?.parar();
    await esperar(`que ${this.nombre} deje de escuchar`, async () => !(await escucha(this.puerto)), { plazo: 15_000, cada: 200 });
  }
}

// ---------------------------------------------------------------------------
// Resguardo Agente (en primer plano, en modo de pruebas: RESGUARDO_AGENT_DIR)
// ---------------------------------------------------------------------------

export class Agente {
  proceso: Proceso | null = null;
  id = "";
  constructor(
    readonly nombre: string,
    readonly binario: string,
    readonly dir: string,
    readonly registro: string,
  ) {
    fs.mkdirSync(dir, { recursive: true });
  }
  get env() {
    // La llave de publicación de PRUEBAS (solo la aceptan las compilaciones de desarrollo): paso 4a.
    return { RESGUARDO_AGENT_DIR: this.dir, RESGUARDO_PRUEBA_SIN_ESPERA: "1", RESGUARDO_LLAVES_PRUEBAS: LLAVE_PRUEBAS_PUB };
  }
  /** `resguardo-agente <args>` (la línea de órdenes, como un administrador en el equipo). */
  cli(args: string[]) {
    const r = ejecutar(this.binario, args, { env: this.env });
    fs.appendFileSync(this.registro, `\n$ resguardo-agente ${args.join(" ")}\n${r.salida}\n`);
    return r;
  }
  /** `vincular <código> --servidor <url>`: devuelve el número de comprobación que enseña el equipo. */
  vincular(codigo: string, srv: Servidor): string {
    const r = this.cli(["vincular", codigo, "--servidor", srv.url, "--nombre", this.nombre]);
    comprobar(r.codigo === 0, `${this.nombre}: «vincular» falló`, r.salida);
    const m = /Código de comprobación:\s*(\d{3} \d{3})/.exec(r.salida);
    comprobar(m, `${this.nombre}: «vincular» no enseñó el número de comprobación`, r.salida);
    return m[1];
  }
  /** `vincular --instalador <ruta>` (instalador listo): todo sale de la cola; devuelve el número de comprobación. */
  vincularInstalador(ruta: string): string {
    const r = this.cli(["vincular", "--instalador", ruta]);
    comprobar(r.codigo === 0, `${this.nombre}: «vincular --instalador» falló`, r.salida);
    const m = /Código de comprobación:\s*(\d{3} \d{3})/.exec(r.salida);
    comprobar(m, `${this.nombre}: «vincular --instalador» no enseñó el número de comprobación`, r.salida);
    return m[1];
  }
  arrancar() {
    if (this.proceso && !this.proceso.terminado) return;
    // Una vuelta del agente cada 10 s (lo mínimo); el canal con el servidor va aparte (WebSocket).
    this.proceso = new Proceso(this.nombre, this.binario, ["--primer-plano", "--cada", "10"], { env: this.env, registro: this.registro });
  }
  async parar() {
    await this.proceso?.parar();
  }
  /** Las últimas líneas de su registro (agent.log), para entender un fallo. */
  ultimasLineas(n = 40): string {
    try {
      return fs.readFileSync(path.join(this.dir, "agent.log"), "utf8").split(/\r?\n/).slice(-n).join("\n");
    } catch {
      return "(sin agent.log)";
    }
  }
}

// ---------------------------------------------------------------------------
// La consola (lo que hace el navegador)
// ---------------------------------------------------------------------------

export interface Secretos {
  claveAdmin?: string;
  repo?: { repo: string; contrasena: string };
}

export class Consola {
  private cookie = "";
  totp: Autenticador | null = null;
  /** Llaves fijadas (TOFU, como fijadas.ts en el navegador): equipo → box_pub|sign_pub. */
  private fijadas = new Map<string, string>();
  constructor(public srv: Servidor) {}

  /** La cookie de la sesión (para abrir el canal en vivo como el navegador, e2e/vivo.ts). */
  get cookieSesion(): string {
    return this.cookie;
  }

  async pedir(metodo: string, ruta: string, cuerpo?: unknown): Promise<Respuesta> {
    const r = await pedirHttps(this.srv.url + ruta, {
      metodo,
      cuerpo,
      ca: this.srv.ca,
      cabeceras: { ...(this.cookie ? { Cookie: this.cookie } : {}), ...(metodo !== "GET" ? { "X-Resguardo": "1" } : {}) },
    });
    const sc = r.cabeceras["set-cookie"];
    const nueva = (Array.isArray(sc) ? sc : sc ? [sc] : []).map((c) => c.split(";")[0]).find((c) => c.includes("resguardo_sesion"));
    if (nueva) this.cookie = nueva;
    return r;
  }
  /** Como `pedir`, pero exige 2xx y devuelve el cuerpo. */
  async ok<T = any>(metodo: string, ruta: string, cuerpo?: unknown): Promise<T> {
    const r = await this.pedir(metodo, ruta, cuerpo);
    comprobar(r.estado >= 200 && r.estado < 300, `${metodo} ${ruta} → ${r.estado}`, r.texto);
    return r.cuerpo as T;
  }

  /** Primer arranque: la cuenta de propietario con el código del registro y su TOTP. */
  async primerArranque(correo: string, nombre: string, contrasena: string) {
    const r = await this.ok("POST", "/api/inicio", { codigo_arranque: this.srv.codigoArranque(), correo, nombre, contrasena });
    comprobar(r?.totp?.secreto, "El primer arranque no devolvió el secreto TOTP", r);
    this.totp = new Autenticador(deBase32(r.totp.secreto));
    const t = await this.ok("POST", "/api/sesion/totp", { codigo: await this.totp.codigo() });
    comprobar(Array.isArray(t.codigos_recuperacion) && t.codigos_recuperacion.length === 10, "Faltan los códigos de recuperación", t);
    comprobar(t.cuenta?.superusuario === true, "La primera cuenta tiene que ser la propietaria del servidor", t);
  }

  /** Entrar con correo, contraseña y TOTP (p. ej. en un servidor restaurado). */
  async entrar(correo: string, contrasena: string, totp: Autenticador) {
    this.cookie = "";
    this.totp = totp;
    const r = await this.ok("POST", "/api/sesion", { correo, contrasena });
    igual(r.necesita, "totp", "Entrar pide el segundo paso");
    await this.ok("POST", "/api/sesion/totp", { codigo: await totp.codigo() });
  }

  equipo(c: Cliente, e: string): Promise<Equipo & { ultimo_informe: { recibido: string; datos: any } | null }> {
    return this.ok("GET", `/api/clientes/${c.id}/equipos/${e}`);
  }

  async ordenes(c: Cliente, e: string): Promise<Orden[]> {
    return this.ok("GET", `/api/clientes/${c.id}/equipos/${e}/ordenes?limite=50`);
  }

  /** Comprueba (y la primera vez fija) las llaves del equipo con su etiqueta y K_cfg, como kcfgComprobada en ordenar.ts. */
  private async comprobarLlaves(c: Cliente, e: Equipo, claveAdmin: string) {
    const fijada = this.fijadas.get(`${c.id}|${e.id}`);
    if (fijada) igual(`${e.box_pub}|${e.sign_pub}`, fijada, `Las llaves de ${e.nombre} cambiaron`);
    const kcfg = kCfg(await materialCliente(argon2, claveAdmin, c.sal_cliente));
    comprobar(etiquetaValida(kcfg, e), `La etiqueta de ${e.nombre} no cuadra con la clave de administración`);
    this.fijadas.set(`${c.id}|${e.id}`, `${e.box_pub}|${e.sign_pub}`);
    return kcfg;
  }

  /**
   * Manda una orden como `mandarOrden` (ordenar.ts): la autorización según su
   * nivel, sellada para el equipo, con su `siguiente_seq`. Las destructivas
   * llevan su `not_before`: los programas de este escenario arrancan con
   * RESGUARDO_PRUEBA_SIN_ESPERA=1 (solo en compilaciones de desarrollo), así que
   * la espera es de 0 s y la orden se fecha un minuto antes para no esperar.
   */
  async mandar(
    c: Cliente,
    equipoId: string,
    tipo: string,
    cuerpo: Record<string, unknown> = {},
    secretos: Secretos = {},
    /** `sinComprobar`: sin mirar antes la etiqueta (para ver que el propio equipo rechaza una clave que no es). */
    /** `relevo`: para «descargar» (el relé del servidor por donde sube el equipo). */
    /** `esperaS` (v1.49): una destructiva que espera de verdad esos segundos (se queda en el equipo hasta entonces). */
    extra: { responderA?: string; sesion?: string; alta?: { codigo: string }; esperar?: boolean; esperaS?: number; sinComprobar?: boolean; relevo?: { id: string; max_bytes: number } } = {},
  ): Promise<Orden> {
    const e = await this.equipo(c, equipoId);
    const autorizacion: Autorizacion = { prueba_admin: null, clave_repo: null };
    const necesitaAdmin = NIVEL[tipo] === "admin" || PIDE_TAMBIEN_ADMIN.has(tipo);
    const cuerpoFinal = { ...cuerpo };
    if (extra.alta) {
      const clave = secretos.claveAdmin!;
      const kcfg = kCfg(await materialCliente(argon2, clave, c.sal_cliente));
      const prueba = await pruebaAdmin(argon2, clave, e.sal_equipo);
      const ver = aB64(verificador(prueba));
      Object.assign(cuerpoFinal, { verificador: ver, k_cfg: aB64(kcfg), espera_min_horas: c.espera_min_horas });
      autorizacion.prueba_admin = aB64(prueba);
      autorizacion.prueba_codigo = pruebaCodigo(extra.alta.codigo, e.id, ver);
      comprobar(etiquetaValida(kcfg, e), "La etiqueta del equipo no cuadra con la clave (alta)");
      this.fijadas.set(`${c.id}|${e.id}`, `${e.box_pub}|${e.sign_pub}`);
    } else if (necesitaAdmin) {
      comprobar(secretos.claveAdmin, `«${tipo}» pide la clave de administración`);
      if (!extra.sinComprobar) await this.comprobarLlaves(c, e, secretos.claveAdmin);
      autorizacion.prueba_admin = aB64(await pruebaAdmin(argon2, secretos.claveAdmin, e.sal_equipo));
    }
    if (NIVEL[tipo] === "repo") {
      comprobar(secretos.repo, `«${tipo}» pide la contraseña del repositorio`);
      if (!this.fijadas.has(`${c.id}|${e.id}`)) {
        comprobar(secretos.claveAdmin, "Primera orden con contraseña a este equipo: hace falta también la clave de administración");
        await this.comprobarLlaves(c, e, secretos.claveAdmin);
      } else igual(`${e.box_pub}|${e.sign_pub}`, this.fijadas.get(`${c.id}|${e.id}`), `Las llaves de ${e.nombre} cambiaron`);
      autorizacion.clave_repo = secretos.repo;
    }
    const espera = e.espera_min_horas ?? c.espera_min_horas;
    const contexto = { espejo: e.resumen?.guarda_copias?.espejo ?? null, copiasActivas: (e.resumen?.copias ?? []).filter((k) => k.activa !== false).length };
    const destructiva = esDestructiva(tipo, cuerpoFinal, espera, contexto);
    // `esperar`: una destructiva con su espera de verdad (se queda en el equipo hasta su not_before).
    const sinEspera = destructiva && !extra.esperar && extra.esperaS === undefined;
    // `esperaS`: su not_before, a esos segundos de ahora (sellarOrden suma un minuto de margen).
    const corta = destructiva && extra.esperaS !== undefined;
    const p = sellarOrden(
      {
        cliente: c.id,
        equipo: e,
        seq: e.siguiente_seq,
        tipo,
        cuerpo: cuerpoFinal,
        autorizacion,
        responderA: extra.responderA ?? null,
        esperaHoras: sinEspera ? 0 : corta ? extra.esperaS! / 3600 : espera,
        contexto,
        // Como ordenar.ts (quién la manda): en este escenario todas las cuentas son de «Ana».
        por: "Ana",
      },
      sinEspera ? new Date(Date.now() - 61_000) : corta ? new Date(Date.now() - 60_000) : new Date(),
    );
    if (destructiva) comprobar(p.meta.not_before, `«${tipo}» es destructiva y tiene que llevar not_before`);
    const r = await this.pedir("POST", `/api/clientes/${c.id}/equipos/${e.id}/ordenes`, {
      tipo: p.meta.tipo,
      seq: p.meta.seq,
      sellado: p.sellado,
      caduca: p.meta.caduca,
      not_before: p.meta.not_before,
      sesion: extra.sesion ?? null,
      relevo: extra.relevo ?? null,
    });
    comprobar(r.estado === 200, `El servidor no aceptó la orden «${tipo}» (${r.estado})`, r.texto);
    return r.cuerpo as Orden;
  }

  /** Espera el resultado firmado de una orden (y comprueba la firma del equipo, como la consola). */
  async resultado(c: Cliente, equipoId: string, o: Orden, opciones: { plazo?: number; estados?: string[] } = {}): Promise<Orden> {
    const estados = opciones.estados ?? FINALES;
    const x = await esperar(
      `el resultado de «${o.tipo}» (n.º ${o.seq})`,
      async () => (await this.ordenes(c, equipoId)).find((y) => y.id === o.id && estados.includes(y.estado)),
      { plazo: opciones.plazo ?? 90_000, cada: 400 },
    );
    if (x.firma_agente) {
      const e = await this.equipo(c, equipoId);
      comprobar(resultadoFirmado(e.sign_pub, x), `El resultado de «${o.tipo}» no lo firmó el equipo`, x);
    }
    return x;
  }

  /** Manda y espera a que quede `hecha` (si no, falla con su mensaje). */
  async hecha(c: Cliente, equipoId: string, tipo: string, cuerpo: Record<string, unknown> = {}, secretos: Secretos = {}, extra: Parameters<Consola["mandar"]>[5] = {}, plazo?: number) {
    const o = await this.mandar(c, equipoId, tipo, cuerpo, secretos, extra);
    const r = await this.resultado(c, equipoId, o, { plazo });
    comprobar(r.estado === "hecha", `«${tipo}» quedó «${r.estado}»: ${r.mensaje ?? ""}`, r.detalle ?? undefined);
    return r;
  }

  /**
   * «Añadir equipo»: código, `vincular` en el equipo, número de comprobación (SAS v3), confirmar con la etiqueta y `alta`.
   * `forma` (v1.48): «navegador» (por defecto: el código de 15 min lo genera la consola y al servidor solo le
   * llega su hash), «instalador» (instalador listo: la cola la arma la consola con su código, lib/cola.ts, y el
   * equipo se vincula con `vincular --instalador`) o «servidor» (la forma de antes: el servidor genera el código).
   */
  async emparejar(c: Cliente, ag: Agente, claveAdmin: string, forma: "navegador" | "instalador" | "servidor" = "navegador"): Promise<Equipo> {
    let empId: string;
    let codigo: string;
    let sasEquipo: string;
    if (forma === "servidor") {
      const emp = await this.ok("POST", `/api/clientes/${c.id}/emparejamientos`);
      comprobar(typeof emp.codigo === "string" && !emp.codigo_navegador, "Forma de antes: el servidor da su código", emp);
      [empId, codigo] = [emp.id, emp.codigo];
      sasEquipo = ag.vincular(codigo, this.srv);
    } else if (forma === "navegador") {
      codigo = generarCodigo();
      const emp = await this.ok("POST", `/api/clientes/${c.id}/emparejamientos`, { codigo_hash: hashCodigo(codigo) });
      comprobar(emp.codigo === undefined && emp.codigo_navegador === true, "Con el hash, el servidor no da (ni tiene) el código", emp);
      empId = emp.id;
      sasEquipo = ag.vincular(codigo, this.srv);
    } else {
      // Instalador listo armado en el navegador: el genérico (aquí, unos bytes cualquiera: el agente solo
      // lee la cola del final) más la cola con el código de la consola.
      codigo = generarCodigo(LARGO_PREPARADO);
      const p = await this.ok("POST", `/api/clientes/${c.id}/instaladores`, { nombre: ag.nombre, so: "linux", servidor: this.srv.url, codigo_hash: hashCodigo(codigo) });
      comprobar(p.codigo === undefined && p.codigo_navegador === true && p.cliente === c.id, "Preparado con el hash: sin el código", p);
      empId = p.id;
      const ruta = path.join(ag.dir, "Resguardo-Agente-listo.exe");
      fs.writeFileSync(ruta, Buffer.concat([Buffer.from("MZ instalador genérico de prueba"), Buffer.from(cola({ v: 1, servidor: p.servidor, huella_ca: p.huella_ca, cliente: p.cliente, nombre: p.nombre, codigo }))]));
      sasEquipo = ag.vincularInstalador(ruta);
    }
    const unido = await esperar("que el equipo se una", async () => {
      const x = await this.ok("GET", `/api/clientes/${c.id}/emparejamientos/${empId}`);
      return x.estado === "unido" ? x : null;
    });
    if (forma === "servidor") igual(unido.codigo, codigo, "Forma de antes: el servidor da el código para el alta");
    else igual([unido.codigo, unido.codigo_hash, unido.codigo_navegador], [undefined, hashCodigo(codigo), true], "El servidor solo tiene el hash del código");
    const servidor = await this.ok("GET", "/api/servidor");
    igual(unido.sas_version, 3, "El equipo anuncia el SAS v3");
    const eq = unido.equipo;
    if (forma === "instalador") igual(eq.nombre, ag.nombre, "El equipo entra con el nombre preparado");
    const sasConsola = sasV3(servidor.identidad, eq.box_pub, eq.sign_pub, servidor.huella_ca);
    igual(sasConsola, sasEquipo, `${ag.nombre}: el número de comprobación de la consola y el del equipo`);
    igual(unido.sas, sasEquipo, "El número que da el servidor");
    const kcfg = kCfg(await materialCliente(argon2, claveAdmin, c.sal_cliente));
    await this.ok("POST", `/api/clientes/${c.id}/emparejamientos/${empId}/confirmar`, { etiqueta: etiquetaEquipo(kcfg, eq.id, eq.box_pub, eq.sign_pub) });
    ag.id = eq.id;
    ag.arrancar();
    await this.hecha(c, eq.id, "alta", {}, { claveAdmin }, { alta: { codigo } });
    log(`${ag.nombre} emparejado (${forma}, SAS ${sasEquipo}) y dado de alta: ${eq.id}`);
    return this.equipo(c, eq.id);
  }

  /** Una orden con `responder_a`: devuelve el detalle que el equipo selló para este «navegador». */
  async hechaSellada<T>(c: Cliente, equipoId: string, tipo: string, cuerpo: Record<string, unknown>, secretos: Secretos): Promise<T> {
    const eph = parEfimero();
    const r = await this.hecha(c, equipoId, tipo, cuerpo, secretos, { responderA: aB64(eph.publica) });
    const sellado = JSON.parse(r.detalle ?? "{}").sellado;
    comprobar(sellado, `«${tipo}» no trajo el detalle sellado`, r);
    return JSON.parse(deUtf8(abrir(eph.secreta, deB64(sellado)))) as T;
  }
}

// ---------------------------------------------------------------------------
// Sesión interactiva (como Sesion en sesion.ts), con espera larga
// ---------------------------------------------------------------------------

export class SesionE2E {
  readonly id = randomUUID();
  private clave = aleatorio(32);
  private kC = claveDireccion(this.clave, this.id, "consola");
  private kE = claveDireccion(this.clave, this.id, "equipo");
  private i = 0;
  private desde = 0;
  private recibidos: any[] = [];
  private abierta = true;
  constructor(
    private consola: Consola,
    private c: Cliente,
  ) {}

  async abrir(equipoId: string, tipo: string, cuerpo: Record<string, unknown>, secretos: Secretos) {
    const o = await this.consola.mandar(this.c, equipoId, tipo, { ...cuerpo, sesion: this.id, clave_sesion: aB64(this.clave) }, secretos, { sesion: this.id });
    void this.escuchar();
    await esperar("el primer mensaje del equipo («lista»)", async () => {
      const x = (await this.consola.ordenes(this.c, equipoId)).find((y) => y.id === o.id);
      if (x && ["rechazada", "fallida", "caducada", "cancelada"].includes(x.estado)) throw new Fallo(`El equipo no abrió la sesión: ${x.mensaje}`);
      return this.recibidos.find((m) => m.op === "lista");
    }, { plazo: 60_000 });
  }

  private async escuchar() {
    while (this.abierta) {
      try {
        const r = await this.consola.pedir("GET", `/api/clientes/${this.c.id}/sesiones/${this.id}/mensajes?desde=${this.desde}`);
        if (r.estado === 404) return;
        for (const m of (r.cuerpo ?? []) as { n: number; de: string; cifrado: string }[]) {
          this.desde = Math.max(this.desde, m.n);
          if (m.de === "equipo") this.recibidos.push(descifrarMensaje(this.kE, this.id, deB64(m.cifrado)));
        }
      } catch {
        /* se reintenta */
      }
    }
  }

  /** Lo que el equipo dice que admite en esta sesión (`lista.ops`). */
  get ops(): string[] | null {
    return this.recibidos.find((m) => m.op === "lista")?.ops ?? null;
  }

  /** Con `conError`, devuelve también una respuesta con `error` (para comprobar las validaciones). */
  async pedir(op: string, args: Record<string, unknown> = {}, conError = false): Promise<any> {
    const i = ++this.i;
    await this.consola.ok("POST", `/api/clientes/${this.c.id}/sesiones/${this.id}/mensajes`, { cifrado: aB64(cifrarMensaje(this.kC, this.id, { op, ...args, i })) });
    const m = await esperar(`la respuesta a «${op}»`, () => this.recibidos.find((x) => x.re === i), { plazo: 60_000, cada: 200 });
    if (!conError) comprobar(!m.error, `«${op}» respondió con error: ${m.error}`);
    return m;
  }

  async cerrar() {
    this.abierta = false;
    await this.consola.pedir("DELETE", `/api/clientes/${this.c.id}/sesiones/${this.id}`).catch(() => {});
  }
}

export const binario = (dir: string, nombre: string) => path.join(dir, `${nombre}${EXE}`);
