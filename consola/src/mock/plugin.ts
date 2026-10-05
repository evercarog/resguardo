// Servidor simulado dentro de Vite (`npm run dev:mock`). Sigue el contrato de
// docs/api-servidor.md: cookies HttpOnly, X-Resguardo en lo que cambia algo,
// errores { error, mensaje }, espera larga en las sesiones y relé binario.
//
// Atajos del simulador (no existen en el servidor real):
//   POST /api/__mock/reiniciar           vuelve a sembrar los datos
//   POST /api/__mock/reiniciar?vacio=1   servidor sin cuentas (primer arranque)
//   El TOTP acepta cualquier código de 6 cifras.
import type { Plugin } from "vite";
import type { IncomingMessage, ServerResponse } from "node:http";
import { randomUUID, randomBytes } from "node:crypto";
import { ed25519, x25519 } from "@noble/curves/ed25519.js";
import { createHash } from "node:crypto";
import { aB64 } from "../lib/cripto/bytes";
import { cabeceraPaquete } from "../lib/cripto/paquete";
import { sasV2, sasV3 } from "../lib/cripto/claves";
import { ABRE_SESION, esDestructiva, NIVEL, SOLO_ADMIN_ROL } from "../lib/cripto/ordenes";
import type * as T from "../lib/tipos";
import { auditar, DEMO, estado, sembrar, verificarCadena, type EmparejamientoMock, type EquipoMock, type OrdenMock } from "./estado";
import { historialMock } from "./historial";
import { progresoDe } from "./progreso";
import { rutasNotificaciones } from "./notificaciones";
import { importarNotas, rutasNotas, sembrarNotas } from "./notas";
import { cerrarSesion, configInicial, esperando, guardarConfig, mensajeDeConsola, procesarOrden, revisarEsperas } from "./agente";

class HttpError extends Error {
  constructor(
    readonly estado: number,
    readonly codigo: string,
    mensaje: string,
    readonly extra: Record<string, unknown> = {},
  ) {
    super(mensaje);
  }
}
const err = (estadoH: number, codigo: string, mensaje: string, extra = {}) => new HttpError(estadoH, codigo, mensaje, extra);

type Ctx = {
  req: IncomingMessage;
  res: ServerResponse;
  metodo: string;
  url: URL;
  cuerpo: Record<string, unknown>;
  crudo: Buffer;
  token: string | null;
  setCookie: (v: string) => void;
};

const RANGO: Record<T.Rol, number> = { lectura: 0, tecnico: 1, administrador: 2, propietario: 3 };

function sesionDe(ctx: Ctx, completa = true) {
  const s = ctx.token ? estado.sesiones.get(ctx.token) : undefined;
  if (!s) throw err(401, "sin_sesion", "Tu sesión terminó. Vuelve a entrar.");
  if (completa && !s.completa) throw err(401, "necesita_totp", "Falta el código de verificación.");
  return estado.cuentas.find((c) => c.id === s.cuenta)!;
}

function rolEn(cuentaId: string, cliente: string): T.Rol | null {
  return estado.miembros.get(cliente)?.find((m) => m.cuenta === cuentaId)?.rol ?? null;
}

function miembro(ctx: Ctx, cliente: string, minimo: T.Rol = "lectura") {
  const cuenta = sesionDe(ctx);
  const c = estado.clientes.find((x) => x.id === cliente);
  if (!c) throw err(404, "no_existe", "Ese cliente no existe.");
  const rol = rolEn(cuenta.id, cliente);
  if (!rol) throw err(404, "no_existe", "Ese cliente no existe.");
  if (RANGO[rol] < RANGO[minimo]) throw err(403, "prohibido", "Tu papel en este cliente no permite hacer esto.");
  return { cuenta, cliente: c, rol };
}

function equipoDe(cliente: string, id: string): EquipoMock {
  const e = estado.equipos.find((x) => x.id === id && x.cliente === cliente);
  if (!e) throw err(404, "no_existe", "Ese equipo no existe.");
  return e;
}

/** Lo que ve la consola de un equipo (sin las llaves privadas del simulador). */
function publico(e: EquipoMock): T.Equipo {
  return {
    id: e.id,
    nombre: e.nombre,
    so: e.so,
    version_agente: e.version_agente,
    box_pub: e.box_pub,
    sign_pub: e.sign_pub,
    sal_equipo: e.sal_equipo,
    etiqueta: e.etiqueta,
    rol: e.rol,
    modo: e.modo,
    confirmado: e.confirmado,
    conectado: e.conectado,
    ultimo_contacto: e.ultimo_contacto,
    estado_servicio: e.estado_servicio,
    siguiente_seq: e.siguiente_seq,
    resumen: e.resumen,
    etiquetas: e.etiquetas ?? [],
  };
}

const ordenPublica = (o: OrdenMock): T.Orden => ({
  id: o.id,
  tipo: o.tipo,
  seq: o.seq,
  emitida: o.emitida,
  emitida_por: o.emitida_por,
  not_before: o.not_before,
  caduca: o.caduca,
  estado: o.estado,
  mensaje: o.mensaje,
  detalle: o.detalle,
  firma_agente: o.firma_agente,
  actualizada: o.actualizada,
  equipo: o.equipo,
});

function cuentaPublica(c: (typeof estado.cuentas)[number]): T.Cuenta {
  return {
    id: c.id,
    correo: c.correo,
    nombre: c.nombre,
    superusuario: c.superusuario,
    clientes: estado.clientes.filter((x) => rolEn(c.id, x.id)).map((x) => ({ id: x.id, nombre: x.nombre, rol: rolEn(c.id, x.id)! })),
  };
}

function nuevaSesion(ctx: Ctx, cuenta: string, completa: boolean) {
  const token = randomBytes(24).toString("base64url");
  estado.sesiones.set(token, { cuenta, completa });
  ctx.setCookie(`resguardo_sesion=${token}; HttpOnly; SameSite=Strict; Path=/`);
  return token;
}

const totpDe = (correo: string) => {
  const secreto = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";
  return { secreto, uri: `otpauth://totp/Resguardo:${encodeURIComponent(correo)}?secret=${secreto}&issuer=Resguardo` };
};

const codigosRecuperacion = () => Array.from({ length: 10 }, () => `${randomBytes(2).toString("hex")}-${randomBytes(2).toString("hex")}`);

// ---------------------------------------------------------------------------
// Rutas
// ---------------------------------------------------------------------------

type Ruta = [string, RegExp, (ctx: Ctx, m: string[]) => unknown | Promise<unknown>];
const C = "/api/clientes/([^/]+)";
/** v1.23: «Copia de la consola» simulada (solo el propietario del servidor). */
const respaldoMock: T.RespaldoConsola = {
  activo: false,
  clave_puesta: null,
  sal: null,
  conservar: 7,
  hora: "03:30",
  carpeta: "/var/lib/resguardo-server/respaldos",
  ultima: null,
  proxima: null,
  copias: [],
  identidad: "",
};
function respaldoDe(ctx: Ctx): T.RespaldoConsola {
  if (!sesionDe(ctx).superusuario) throw err(403, "prohibido", "Solo quien administra el servidor.");
  const manana = new Date(Date.now() + 86_400_000);
  const [h, m] = respaldoMock.hora.split(":").map(Number);
  manana.setHours(h, m, 0, 0);
  return { ...respaldoMock, identidad: estado.servidor.identidad, proxima: respaldoMock.activo ? manana.toISOString() : null };
}

const rutas: Ruta[] = [
  ["GET", /^\/api\/servidor$/, () => ({ version: "0.1.0 (simulado)", nombre: "Resguardo Server", identidad: estado.servidor.identidad, huella_ca: HUELLA_CA, inicializado: estado.servidor.inicializado, instalador_agente: true, agente_local: true })],

  ["GET", /^\/api\/servidor\/respaldo$/, (ctx) => respaldoDe(ctx)],
  [
    "PUT",
    /^\/api\/servidor\/respaldo$/,
    (ctx) => {
      respaldoDe(ctx);
      const b = ctx.cuerpo as { activo?: boolean; publica?: string; sal?: string; conservar?: number; hora?: string };
      if (!!b.publica !== !!b.sal) throw err(422, "datos", "La clave pública y su sal van juntas.");
      if (b.publica) Object.assign(respaldoMock, { clave_puesta: new Date().toISOString(), sal: b.sal });
      if (b.conservar !== undefined) respaldoMock.conservar = b.conservar;
      if (b.hora) respaldoMock.hora = b.hora;
      if (b.activo !== undefined) respaldoMock.activo = b.activo;
      if (respaldoMock.activo && !respaldoMock.clave_puesta) throw err(422, "datos", "Primero pon la clave de respaldo de la consola.");
      return respaldoDe(ctx);
    },
  ],
  [
    "POST",
    /^\/api\/servidor\/respaldo\/ahora$/,
    (ctx) => {
      respaldoDe(ctx);
      if (!respaldoMock.clave_puesta) throw err(422, "datos", "Falta poner la clave de respaldo de la consola.");
      const ahora = new Date();
      const archivo = `consola-${ahora.toISOString().replace(/[-:]/g, "").replace("T", "-").slice(0, 15)}-000.resguardo-consola`;
      const bytes = 3_400_000 + Math.round(Math.random() * 200_000);
      respaldoMock.copias = [{ archivo, bytes }, ...respaldoMock.copias].slice(0, respaldoMock.conservar);
      respaldoMock.ultima = { cuando: ahora.toISOString(), ok: true, mensaje: "Copia de la consola hecha (3.3 MB).", archivo, bytes, motivo: "manual" };
      return respaldoDe(ctx);
    },
  ],
  [
    "POST",
    /^\/api\/inicio$/,
    (ctx) => {
      if (estado.servidor.inicializado) throw err(409, "conflicto", "Este servidor ya tiene cuentas.");
      const b = ctx.cuerpo as { codigo_arranque?: string; correo?: string; nombre?: string; contrasena?: string };
      if (b.codigo_arranque?.trim() !== DEMO.codigoArranque) throw err(422, "datos", "El código de primer arranque no coincide con el del registro del servidor.");
      if (!b.correo?.includes("@") || !b.nombre?.trim() || (b.contrasena?.length ?? 0) < 12) throw err(422, "datos", "Revisa el correo, el nombre y que la contraseña tenga al menos 12 caracteres.");
      const cuenta = { id: randomUUID(), correo: b.correo, nombre: b.nombre.trim(), contrasena: b.contrasena!, superusuario: true, totp: false, recuperacion: [] };
      estado.cuentas.push(cuenta);
      estado.servidor.inicializado = true;
      nuevaSesion(ctx, cuenta.id, false);
      return { totp: totpDe(cuenta.correo) };
    },
  ],

  [
    "POST",
    /^\/api\/sesion$/,
    (ctx) => {
      const b = ctx.cuerpo as { correo?: string; contrasena?: string };
      const cuenta = estado.cuentas.find((c) => c.correo.toLowerCase() === String(b.correo ?? "").toLowerCase());
      if (!cuenta || cuenta.contrasena !== b.contrasena) throw err(401, "sin_sesion", "El correo o la contraseña no son correctos.");
      nuevaSesion(ctx, cuenta.id, false);
      // v1.27: con la verificación restablecida, hace falta el código que dio el propietario.
      const r = cuenta.restablecida;
      if (r) return { necesita: "restablecimiento", restablecida: { cuando: r.cuando, por: r.por }, caducado: r.caduca <= Date.now() };
      return cuenta.totp ? { necesita: "totp" } : { necesita: "alta_totp", totp: totpDe(cuenta.correo) };
    },
  ],

  [
    "POST",
    /^\/api\/sesion\/restablecimiento$/,
    (ctx) => {
      const s = ctx.token ? estado.sesiones.get(ctx.token) : undefined;
      if (!s) throw err(401, "sin_sesion", "Vuelve a escribir tu correo y tu contraseña.");
      const cuenta = estado.cuentas.find((c) => c.id === s.cuenta)!;
      const r = cuenta.restablecida;
      if (!r) throw err(422, "datos", "Esta cuenta no tiene ningún restablecimiento pendiente: vuelve a entrar.");
      if (r.caduca <= Date.now()) throw err(410, "caducado", "El código caducó (dura 24 horas): pide al propietario que la restablezca otra vez.");
      const limpio = (x: string) => x.replace(/[^A-Za-z0-9]/g, "").toUpperCase();
      if (limpio(String((ctx.cuerpo as { codigo?: string }).codigo ?? "")) !== limpio(r.codigo)) throw err(401, "codigo", "El código no es correcto.");
      r.sesion = ctx.token ?? undefined;
      return { necesita: "alta_totp", totp: totpDe(cuenta.correo), restablecida: { cuando: r.cuando, por: r.por } };
    },
  ],

  [
    "POST",
    /^\/api\/sesion\/totp$/,
    (ctx) => {
      const s = ctx.token ? estado.sesiones.get(ctx.token) : undefined;
      if (!s) throw err(401, "sin_sesion", "Vuelve a escribir tu correo y tu contraseña.");
      const cuenta = estado.cuentas.find((c) => c.id === s.cuenta)!;
      const restablecida = cuenta.restablecida;
      if (restablecida && restablecida.sesion !== ctx.token) throw err(403, "restablecimiento", "Falta el código que te dio el propietario: vuelve a entrar.");
      const b = ctx.cuerpo as { codigo?: string; recuperacion?: string };
      if (b.recuperacion !== undefined) {
        const i = cuenta.recuperacion.indexOf(b.recuperacion.trim().toLowerCase());
        if (i < 0) throw err(401, "necesita_totp", "Ese código de recuperación no vale o ya se usó.");
        cuenta.recuperacion.splice(i, 1);
      } else if (!/^\d{6}$/.test(String(b.codigo ?? "").replace(/\s/g, ""))) {
        throw err(401, "necesita_totp", "El código no es correcto. Mira el que muestra ahora tu aplicación.");
      }
      s.completa = true;
      if (!cuenta.totp) {
        cuenta.totp = true;
        cuenta.recuperacion = codigosRecuperacion();
        cuenta.restablecida = undefined;
        return { cuenta: cuentaPublica(cuenta), codigos_recuperacion: cuenta.recuperacion, ...(restablecida ? { restablecida: { cuando: restablecida.cuando, por: restablecida.por } } : {}) };
      }
      return { cuenta: cuentaPublica(cuenta) };
    },
  ],

  [
    "DELETE",
    /^\/api\/sesion$/,
    (ctx) => {
      if (ctx.token) estado.sesiones.delete(ctx.token);
      ctx.setCookie("resguardo_sesion=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0");
      return undefined;
    },
  ],

  ["GET", /^\/api\/cuenta$/, (ctx) => cuentaPublica(sesionDe(ctx))],
  [
    "PATCH",
    /^\/api\/cuenta$/,
    (ctx) => {
      const cuenta = sesionDe(ctx);
      const nombre = String((ctx.cuerpo as { nombre?: string }).nombre ?? "").trim();
      if (!nombre || nombre.length > 80) throw err(422, "datos", "Escribe un nombre (hasta 80 caracteres).");
      cuenta.nombre = nombre;
      return cuentaPublica(cuenta);
    },
  ],
  [
    "POST",
    /^\/api\/cuenta\/totp$/,
    (ctx) => {
      const cuenta = sesionDe(ctx);
      const b = ctx.cuerpo as { contrasena?: string; codigo?: string };
      if (b.contrasena !== cuenta.contrasena) throw err(401, "sin_sesion", "La contraseña no es correcta.");
      if (!/^\d{6}$/.test(String(b.codigo ?? ""))) throw err(401, "necesita_totp", "El código no es correcto.");
      return { totp: totpDe(cuenta.correo), caduca: new Date(Date.now() + 15 * 60_000).toISOString() };
    },
  ],
  [
    "POST",
    /^\/api\/cuenta\/totp\/confirmar$/,
    (ctx) => {
      const cuenta = sesionDe(ctx);
      if (!/^\d{6}$/.test(String((ctx.cuerpo as { codigo?: string }).codigo ?? ""))) throw err(401, "necesita_totp", "El código no es correcto.");
      for (const [t, x] of estado.sesiones) if (x.cuenta === cuenta.id && t !== ctx.token) estado.sesiones.delete(t);
      return { cuenta: cuentaPublica(cuenta) };
    },
  ],
  [
    "POST",
    /^\/api\/cuenta\/recuperacion$/,
    (ctx) => {
      const cuenta = sesionDe(ctx);
      const b = ctx.cuerpo as { contrasena?: string; codigo?: string };
      if (b.contrasena !== cuenta.contrasena) throw err(401, "sin_sesion", "La contraseña no es correcta.");
      if (!/^\d{6}$/.test(String(b.codigo ?? ""))) throw err(401, "necesita_totp", "El código no es correcto.");
      cuenta.recuperacion = codigosRecuperacion();
      return { codigos_recuperacion: cuenta.recuperacion };
    },
  ],

  [
    "PUT",
    /^\/api\/cuenta\/contrasena$/,
    (ctx) => {
      const cuenta = sesionDe(ctx);
      const b = ctx.cuerpo as { actual?: string; nueva?: string };
      if (b.actual !== cuenta.contrasena) throw err(422, "datos", "La contraseña actual no es correcta.");
      if ((b.nueva?.length ?? 0) < 12) throw err(422, "datos", "La contraseña nueva necesita al menos 12 caracteres.");
      cuenta.contrasena = b.nueva!;
      for (const [t, s] of estado.sesiones) if (s.cuenta === cuenta.id && t !== ctx.token) estado.sesiones.delete(t);
      return undefined;
    },
  ],

  // --- Todos los clientes (v1.38): solo los clientes de los que la cuenta es miembro ---
  [
    "GET",
    /^\/api\/panel$/,
    (ctx) => {
      const cuenta = sesionDe(ctx);
      const suyos = estado.clientes.filter((c) => rolEn(cuenta.id, c.id)).sort((a, b) => a.nombre.localeCompare(b.nombre));
      return {
        generado: new Date().toISOString(),
        omitidos: 0,
        clientes: suyos.map((c) => ({
          id: c.id,
          nombre: c.nombre,
          rol: rolEn(cuenta.id, c.id),
          marca: marcaJson(c.id),
          equipos: estado.equipos.filter((e) => e.cliente === c.id).map(publico),
          avisos_abiertos: estado.avisos.filter((a) => a.cliente === c.id && a.abierto).length,
          pendientes: estado.ordenes.filter((o) => o.cliente === c.id && o.estado === "pendiente" && o.not_before && Date.parse(o.not_before) > Date.now()).length,
          informes: estado.equipos.filter((e) => e.cliente === c.id && e.informes[0]).map((e) => ({ equipo: e.id, ...e.informes[0] })),
          informes_completos: true,
        })),
        progreso: suyos.flatMap((c) => progresoDe(c.id).map((p) => ({ ...p, cliente: c.id }))),
      };
    },
  ],
  [
    "GET",
    /^\/api\/panel\/progreso$/,
    (ctx) => {
      const cuenta = sesionDe(ctx);
      return estado.clientes.filter((c) => rolEn(cuenta.id, c.id)).flatMap((c) => progresoDe(c.id).map((p) => ({ ...p, cliente: c.id })));
    },
  ],

  // --- Clientes -------------------------------------------------------------
  [
    "GET",
    /^\/api\/clientes$/,
    (ctx) => {
      const cuenta = sesionDe(ctx);
      return estado.clientes
        .filter((c) => rolEn(cuenta.id, c.id))
        .map((c) => ({ id: c.id, nombre: c.nombre, rol: rolEn(cuenta.id, c.id), equipos: estado.equipos.filter((e) => e.cliente === c.id).length, avisos: estado.avisos.filter((a) => a.cliente === c.id && a.abierto).length, marca: marcaJson(c.id) }));
    },
  ],
  [
    "POST",
    /^\/api\/clientes$/,
    (ctx) => {
      const cuenta = sesionDe(ctx);
      if (!cuenta.superusuario) throw err(403, "prohibido", "Solo quien administra el servidor puede crear clientes.");
      const b = ctx.cuerpo as { nombre?: string; espera_min_horas?: number };
      const horas = Number(b.espera_min_horas);
      if (!b.nombre?.trim() || !(horas >= 1 && horas <= 168)) throw err(422, "datos", "Pon un nombre y una espera entre 1 y 168 horas.");
      const c = { id: randomUUID(), nombre: b.nombre.trim(), sal_cliente: aB64(randomBytes(16)), espera_min_horas: horas, rol: "propietario" as T.Rol };
      estado.clientes.push(c);
      estado.miembros.set(c.id, [{ cuenta: cuenta.id, rol: "propietario" }]);
      auditar(c.id, cuenta.id, "cliente.crear", c.nombre, { espera_min_horas: horas });
      return { id: c.id, nombre: c.nombre, sal_cliente: c.sal_cliente };
    },
  ],
  [
    "GET",
    new RegExp(`^${C}$`),
    (ctx, [c]) => {
      const { cliente, rol } = miembro(ctx, c);
      return { id: cliente.id, nombre: cliente.nombre, sal_cliente: cliente.sal_cliente, espera_min_horas: cliente.espera_min_horas, rol, marca: marcaJson(c) };
    },
  ],
  // v1.32: marca del cliente (logo PNG y acento), como crates/servidor/src/api/marca.rs.
  ["GET", new RegExp(`^${C}/marca$`), (ctx, [c]) => (miembro(ctx, c), marcaJson(c))],
  [
    "PUT",
    new RegExp(`^${C}/marca$`),
    (ctx, [c]) => {
      const { cuenta } = miembro(ctx, c, "administrador");
      const b = ctx.cuerpo as { acento?: string | null; logo?: string; quitar_logo?: boolean };
      if (b.acento != null && !ACENTOS_MARCA.includes(b.acento)) throw err(422, "datos", "Ese color no está entre los de la consola.");
      const m = marcasMock.get(c) ?? {};
      let logo = "igual";
      if (b.quitar_logo) (m.logo = undefined), (logo = "quitado");
      else if (typeof b.logo === "string") {
        const png = Buffer.from(b.logo, "base64");
        if (png.length > 200 * 1024) throw err(422, "datos", "El logo pesa demasiado: como mucho 200 KB.");
        if (png.subarray(0, 8).toString("hex") !== "89504e470d0a1a0a" || png.subarray(12, 16).toString() !== "IHDR") throw err(422, "datos", "El logo tiene que ser una imagen PNG.");
        m.logo = png;
        m.huella = createHash("sha256").update(png).digest("hex").slice(0, 16);
        logo = "puesto";
      }
      m.acento = b.acento ?? null;
      m.actualizada = new Date().toISOString();
      m.por = cuenta.nombre;
      marcasMock.set(c, m);
      auditar(c, cuenta.id, "cambiar_marca", c, { acento: m.acento, logo });
      return marcaJson(c);
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/marca/logo$`),
    (ctx, [c]) => {
      miembro(ctx, c);
      const png = marcasMock.get(c)?.logo;
      if (!png) throw err(404, "no_existe", "Este cliente no tiene logo.");
      ctx.res.setHeader("Content-Type", "image/png");
      ctx.res.setHeader("Cache-Control", "private, max-age=86400");
      ctx.res.end(png);
      return SIN_CUERPO;
    },
  ],
  [
    "PATCH",
    new RegExp(`^${C}$`),
    (ctx, [c]) => {
      const { cliente, cuenta, rol } = miembro(ctx, c, "propietario");
      const nombre = String((ctx.cuerpo as { nombre?: string }).nombre ?? "").trim();
      if (!nombre) throw err(422, "datos", "El nombre no puede quedar vacío.");
      auditar(c, cuenta.id, "cliente.renombrar", nombre, { antes: cliente.nombre });
      cliente.nombre = nombre;
      return { ...cliente, rol };
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/miembros$`),
    (ctx, [c]) => {
      miembro(ctx, c, "propietario");
      return (estado.miembros.get(c) ?? []).map((m) => {
        const x = estado.cuentas.find((k) => k.id === m.cuenta)!;
        return { cuenta: x.id, correo: x.correo, nombre: x.nombre, rol: m.rol };
      });
    },
  ],
  [
    "PUT",
    new RegExp(`^${C}/miembros/([^/]+)$`),
    (ctx, [c, cuentaId]) => {
      const { cuenta } = miembro(ctx, c, "propietario");
      const lista = estado.miembros.get(c) ?? [];
      const m = lista.find((x) => x.cuenta === cuentaId);
      if (!m) throw err(404, "no_existe", "Esa persona ya no es miembro.");
      const rol = (ctx.cuerpo as { rol?: T.Rol }).rol!;
      if (!(rol in RANGO)) throw err(422, "datos", "Ese papel no existe.");
      if (m.rol === "propietario" && rol !== "propietario" && lista.filter((x) => x.rol === "propietario").length === 1) throw err(409, "conflicto", "El cliente tiene que tener al menos una persona propietaria.");
      m.rol = rol;
      auditar(c, cuenta.id, "miembro.rol", estado.cuentas.find((k) => k.id === cuentaId)?.correo ?? cuentaId, { rol });
      return undefined;
    },
  ],
  [
    // v1.27: restablecer la verificación en dos pasos de otro (como el servidor: permisos, código de quien lo hace, fuera sesiones).
    "POST",
    new RegExp(`^${C}/miembros/([^/]+)/restablecer-totp$`),
    (ctx, [c, cuentaId]) => {
      const { cuenta: yo, rol } = miembro(ctx, c);
      if (!yo.superusuario && rol !== "propietario") throw err(403, "prohibido", "Tu papel en este cliente no permite hacer esto.");
      if (cuentaId === yo.id) throw err(422, "datos", "La tuya se cambia en «Mi cuenta» (o, sin el móvil, con un código de recuperación).");
      const otro = estado.cuentas.find((k) => k.id === cuentaId);
      if (!otro || !rolEn(cuentaId, c)) throw err(404, "no_existe", "No existe.");
      if (otro.superusuario) throw err(403, "prohibido", "La del propietario del servidor no se puede restablecer desde la consola.");
      const suyos = estado.clientes.filter((x) => rolEn(cuentaId, x.id));
      if (!yo.superusuario && suyos.some((x) => rolEn(cuentaId, x.id) === "propietario"))
        throw err(403, "prohibido", "Es propietario: solo el propietario del servidor puede restablecer su verificación en dos pasos.");
      if (!yo.superusuario && suyos.some((x) => rolEn(yo.id, x.id) !== "propietario"))
        throw err(403, "prohibido", "También está en clientes de los que no eres propietario: pídeselo al propietario del servidor.");
      if (!/^\d{6}$/.test(String((ctx.cuerpo as { codigo?: string }).codigo ?? ""))) throw err(401, "codigo", "El código de tu autenticador no es correcto.");
      const letras = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
      const crudo = Array.from(randomBytes(16), (x) => letras[x % 32]).join("");
      const codigo = crudo.match(/.{4}/g)!.join("-");
      const caduca = Date.now() + 24 * 3600_000;
      otro.totp = false;
      otro.recuperacion = [];
      otro.restablecida = { codigo, caduca, cuando: new Date().toISOString(), por: yo.nombre };
      for (const [t, x] of estado.sesiones) if (x.cuenta === otro.id) estado.sesiones.delete(t);
      auditar(c, yo.id, "restablecer_totp", otro.correo, { correo: otro.correo, nombre: otro.nombre });
      return { codigo, caduca: new Date(caduca).toISOString() };
    },
  ],
  [
    "DELETE",
    new RegExp(`^${C}/miembros/([^/]+)$`),
    (ctx, [c, cuentaId]) => {
      const { cuenta } = miembro(ctx, c, "propietario");
      const lista = estado.miembros.get(c) ?? [];
      const m = lista.find((x) => x.cuenta === cuentaId);
      if (m?.rol === "propietario" && lista.filter((x) => x.rol === "propietario").length === 1) throw err(409, "conflicto", "No puedes quitar a la única persona propietaria.");
      estado.miembros.set(
        c,
        lista.filter((x) => x.cuenta !== cuentaId),
      );
      auditar(c, cuenta.id, "miembro.quitar", estado.cuentas.find((k) => k.id === cuentaId)?.correo ?? cuentaId);
      return undefined;
    },
  ],
  [
    "POST",
    new RegExp(`^${C}/invitaciones$`),
    (ctx, [c]) => {
      const { cuenta } = miembro(ctx, c, "propietario");
      const rol = (ctx.cuerpo as { rol?: T.Rol }).rol ?? "lectura";
      const token = randomBytes(24).toString("base64url");
      const caduca = Date.now() + 7 * 86_400_000;
      estado.invitaciones.set(token, { cliente: c, rol, caduca });
      auditar(c, cuenta.id, "miembro.invitar", rol);
      return { enlace: `/invitacion#${token}`, caduca: new Date(caduca).toISOString() };
    },
  ],
  [
    "POST",
    /^\/api\/invitaciones\/aceptar$/,
    (ctx) => {
      const b = ctx.cuerpo as { token?: string; correo?: string; nombre?: string; contrasena?: string };
      const inv = estado.invitaciones.get(String(b.token));
      if (!inv || inv.caduca < Date.now()) throw err(404, "no_existe", "La invitación no vale o ya caducó. Pide otra.");
      const actual = ctx.token ? estado.sesiones.get(ctx.token) : undefined;
      let cuentaId: string;
      let respuesta: unknown = {};
      if (actual?.completa) {
        cuentaId = actual.cuenta;
      } else {
        if (!b.correo?.includes("@") || !b.nombre?.trim() || (b.contrasena?.length ?? 0) < 12) throw err(422, "datos", "Escribe tu correo, tu nombre y una contraseña de al menos 12 caracteres.");
        if (estado.cuentas.some((x) => x.correo === b.correo)) throw err(409, "conflicto", "Ya hay una cuenta con ese correo: entra con ella y vuelve a abrir el enlace.");
        const cuenta = { id: randomUUID(), correo: b.correo, nombre: b.nombre.trim(), contrasena: b.contrasena!, superusuario: false, totp: false, recuperacion: [] };
        estado.cuentas.push(cuenta);
        cuentaId = cuenta.id;
        nuevaSesion(ctx, cuenta.id, false);
        respuesta = { necesita: "alta_totp", totp: totpDe(cuenta.correo) };
      }
      const lista = estado.miembros.get(inv.cliente) ?? [];
      if (!lista.some((m) => m.cuenta === cuentaId)) lista.push({ cuenta: cuentaId, rol: inv.rol });
      estado.miembros.set(inv.cliente, lista);
      estado.invitaciones.delete(String(b.token));
      auditar(inv.cliente, cuentaId, "miembro.aceptar", estado.cuentas.find((k) => k.id === cuentaId)?.correo ?? "");
      return respuesta;
    },
  ],

  // --- Equipos y emparejamiento ---------------------------------------------
  [
    "GET",
    new RegExp(`^${C}/equipos$`),
    (ctx, [c]) => {
      miembro(ctx, c);
      return estado.equipos.filter((e) => e.cliente === c).map(publico);
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/equipos/([^/]+)$`),
    (ctx, [c, e]) => {
      miembro(ctx, c);
      const eq = equipoDe(c, e);
      return { ...publico(eq), ultimo_informe: eq.informes[0] ?? null };
    },
  ],
  [
    "PATCH",
    new RegExp(`^${C}/equipos/([^/]+)$`),
    (ctx, [c, e]) => {
      const { cuenta } = miembro(ctx, c, "administrador");
      const eq = equipoDe(c, e);
      const nombre = String((ctx.cuerpo as { nombre?: string }).nombre ?? "").trim();
      if (!nombre) throw err(422, "datos", "El nombre no puede quedar vacío.");
      auditar(c, cuenta.id, "equipo.renombrar", nombre, { antes: eq.nombre });
      eq.nombre = nombre;
      return publico(eq);
    },
  ],
  [
    // v1.18: etiquetas libres (técnico o más), limpias como en el servidor.
    "PUT",
    new RegExp(`^${C}/equipos/([^/]+)/etiquetas$`),
    (ctx, [c, e]) => {
      const { cuenta } = miembro(ctx, c, "tecnico");
      const eq = equipoDe(c, e);
      const xs = (ctx.cuerpo as { etiquetas?: unknown }).etiquetas;
      if (!Array.isArray(xs) || xs.length > 50) throw err(422, "datos", "Etiquetas no válidas.");
      const out: string[] = [];
      for (const x of xs) {
        const t = String(x).split(/\s+/).filter(Boolean).join(" ");
        if (!t) continue;
        if ([...t].length > 32 || /[\u0000-\u001f,]/.test(t)) throw err(422, "datos", `Etiqueta no válida: «${t.slice(0, 40)}» (hasta 32 caracteres, sin comas).`);
        if (!out.some((o) => o.toLowerCase() === t.toLowerCase())) out.push(t);
      }
      if (out.length > 10) throw err(422, "datos", "Como mucho 10 etiquetas por equipo.");
      eq.etiquetas = out;
      auditar(c, cuenta.id, "etiquetas_equipo", eq.nombre, { etiquetas: out });
      return { ...publico(eq), ultimo_informe: eq.informes[0] ?? null };
    },
  ],
  [
    "POST",
    new RegExp(`^${C}/equipos/([^/]+)/atencion$`),
    (ctx, [c, e]) => {
      miembro(ctx, c);
      equipoDe(c, e);
      return undefined;
    },
  ],
  [
    "POST",
    new RegExp(`^${C}/emparejamientos$`),
    (ctx, [c]) => {
      const { cuenta } = miembro(ctx, c, "administrador");
      // v1.41: el abierto de esta cuenta (más de 2 min por delante), en vez de otro.
      const ya = codigoDe(c, cuenta.id);
      if (ya && ya.estado === "abierto" && Date.parse(ya.caduca) > Date.now() + 120_000) return { id: ya.id, codigo: ya.codigo, caduca: ya.caduca, reutilizado: true };
      const letras = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
      const crudo = Array.from(randomBytes(10), (b) => letras[b % letras.length]).join("");
      const p: EmparejamientoMock = { id: randomUUID(), cliente: c, codigo: `${crudo.slice(0, 4)}-${crudo.slice(4, 8)}-${crudo.slice(8)}`, caduca: new Date(Date.now() + 15 * 60_000).toISOString(), estado: "abierto", creado: Date.now(), por: cuenta.id };
      estado.emparejamientos.push(p);
      auditar(c, cuenta.id, "emparejamiento.abrir", null);
      // El «equipo» se une solo a los 6 s, como si alguien escribiera el código en el instalador.
      unirSolo(p, "ALMACEN-BODEGA", "Windows 11 Pro", 6000);
      return { id: p.id, codigo: p.codigo, caduca: p.caduca, reutilizado: false };
    },
  ],
  [
    // v1.41: el código de 15 min de esta cuenta que aún sirve (o null).
    "GET",
    new RegExp(`^${C}/codigo-abierto$`),
    (ctx, [c]) => {
      const { cuenta } = miembro(ctx, c, "administrador");
      const p = codigoDe(c, cuenta.id);
      return p ? { id: p.id, codigo: p.codigo, caduca: p.caduca, estado: p.estado } : null;
    },
  ],
  [
    // v1.17: equipo preparado (instalador listo en Windows, o la línea de Linux), 24 h.
    "POST",
    new RegExp(`^${C}/instaladores$`),
    (ctx, [c]) => {
      const { cuenta } = miembro(ctx, c, "administrador");
      const b = ctx.cuerpo as { nombre?: string; so?: string; servidor?: string };
      const nombre = String(b.nombre ?? "").trim();
      const servidor = String(b.servidor ?? "").trim().replace(/\/+$/, "");
      if (b.so !== "windows" && b.so !== "linux") throw err(422, "datos", "Sistema no válido («windows» o «linux»).");
      if (!nombre || nombre.length > 80 || /["\u0000-\u001f]/.test(nombre)) throw err(422, "datos", "Nombre del equipo no válido.");
      if (!/^https:\/\/[A-Za-z0-9.\-:[\]]+$/.test(servidor)) throw err(422, "datos", "Dirección del servidor no válida (https://servidor:puerto, sin ruta).");
      const letras = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
      const crudo = Array.from(randomBytes(10), (x) => letras[x % letras.length]).join("");
      const p: EmparejamientoMock = { id: randomUUID(), cliente: c, codigo: `${crudo.slice(0, 4)}-${crudo.slice(4, 8)}-${crudo.slice(8)}`, caduca: new Date(Date.now() + 24 * 3600_000).toISOString(), estado: "abierto", creado: Date.now(), nombre, so: b.so };
      estado.emparejamientos.push(p);
      auditar(c, cuenta.id, "preparar_equipo", nombre, { so: b.so });
      // Como si alguien lo instalara a los 25 s.
      unirSolo(p, nombre, b.so === "linux" ? "Debian 12" : "Windows 11 Pro", 25_000);
      if (b.so === "linux") return { id: p.id, nombre, so: b.so, codigo: p.codigo, caduca: p.caduca, servidor, huella_ca: HUELLA_CA };
      // Un «instalador» de mentira con la cola de verdad (crates/protocolo/src/instalador.rs).
      const json = Buffer.from(JSON.stringify({ v: 1, servidor, huella_ca: HUELLA_CA, cliente: c, nombre, codigo: p.codigo }));
      const n = Buffer.alloc(4);
      n.writeUInt32BE(json.length);
      const exe = Buffer.concat([Buffer.from("MZ Resguardo Agente (instalador simulado)\n"), Buffer.from("RESGUARDO-COLA-1"), n, json, n, Buffer.from("RESGUARDO-FIN-01")]);
      // Como el servidor (instaladores.rs, nombre_archivo): sin tildes y solo ASCII.
      const limpio = (t: string) => t.normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/[^A-Za-z0-9_-]+/g, "-").replace(/^-+|-+$/g, "");
      const archivo = `Resguardo-Agente_${limpio(estado.clientes.find((x) => x.id === c)?.nombre ?? "")}_${limpio(nombre)}.exe`;
      ctx.res.setHeader("Content-Type", "application/vnd.microsoft.portable-executable");
      ctx.res.setHeader("Content-Disposition", `attachment; filename="${archivo}"`);
      ctx.res.setHeader("X-Resguardo-Emparejamiento", p.id);
      ctx.res.setHeader("X-Resguardo-Caduca", p.caduca);
      ctx.res.end(exe);
      return SIN_CUERPO;
    },
  ],
  [
    // v1.20: plantillas de copia (bytes cifrados por la consola; aquí no se leen).
    "GET",
    new RegExp(`^${C}/plantillas$`),
    (ctx, [c]) => {
      miembro(ctx, c, "administrador");
      return [...(plantillasMock.get(c) ?? new Map()).entries()].map(([id, p]) => ({ id, ...p }));
    },
  ],
  [
    "PUT",
    new RegExp(`^${C}/plantillas/([^/]+)$`),
    (ctx, [c, id]) => {
      const { cuenta } = miembro(ctx, c, "administrador");
      const cifrado = String((ctx.cuerpo as { cifrado?: string }).cifrado ?? "");
      if (!/^[A-Za-z0-9_-]{1,64}$/.test(id)) throw err(422, "datos", "Id de plantilla no válido.");
      const n = Buffer.from(cifrado, "base64").length;
      if (n < 40 || n > 64 * 1024) throw err(422, "datos", "Plantilla demasiado grande o vacía (hasta 64 KiB).");
      const m = plantillasMock.get(c) ?? new Map();
      if (!m.has(id) && m.size >= 100) throw err(422, "datos", "Como mucho 100 plantillas por cliente: borra alguna.");
      m.set(id, { cifrado, actualizada: new Date().toISOString(), por: cuenta.nombre });
      plantillasMock.set(c, m);
      auditar(c, cuenta.id, "guardar_plantilla", id, {});
      return undefined;
    },
  ],
  [
    "DELETE",
    new RegExp(`^${C}/plantillas/([^/]+)$`),
    (ctx, [c, id]) => {
      const { cuenta } = miembro(ctx, c, "administrador");
      if (!plantillasMock.get(c)?.delete(id)) throw err(404, "no_existe", "Esa plantilla ya no existe.");
      auditar(c, cuenta.id, "borrar_plantilla", id, {});
      return undefined;
    },
  ],
  [
    // v1.19: «Vincular este servidor»: el agente de la máquina del servidor se une solo.
    "POST",
    new RegExp(`^${C}/equipo-local$`),
    (ctx, [c]) => {
      const { cuenta } = miembro(ctx, c, "administrador");
      const letras = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
      const crudo = Array.from(randomBytes(10), (x) => letras[x % letras.length]).join("");
      const p: EmparejamientoMock = { id: randomUUID(), cliente: c, codigo: `${crudo.slice(0, 4)}-${crudo.slice(4, 8)}-${crudo.slice(8)}`, caduca: new Date(Date.now() + 30 * 60_000).toISOString(), estado: "abierto", creado: Date.now(), nombre: "SRV-RESGUARDO", so: "windows" };
      estado.emparejamientos.push(p);
      auditar(c, cuenta.id, "vincular_servidor_local", p.nombre ?? "", {});
      unirSolo(p, "SRV-RESGUARDO", "Windows Server 2022", 8000);
      return { id: p.id, nombre: p.nombre, so: p.so, estado: p.estado, caduca: p.caduca, creado: new Date(p.creado).toISOString(), equipo: null };
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/emparejamientos$`),
    (ctx, [c]) => {
      miembro(ctx, c, "administrador");
      return estado.emparejamientos
        .filter((p) => p.cliente === c && p.nombre && (p.estado === "abierto" || p.estado === "unido") && Date.parse(p.caduca) > Date.now())
        .sort((a, b) => b.creado - a.creado)
        .map((p) => ({ id: p.id, nombre: p.nombre, so: p.so, estado: p.estado, caduca: p.caduca, creado: new Date(p.creado).toISOString(), equipo: p.equipo?.id ?? null }));
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/emparejamientos/([^/]+)$`),
    (ctx, [c, id]) => {
      miembro(ctx, c, "administrador");
      const p = estado.emparejamientos.find((x) => x.id === id && x.cliente === c);
      if (!p) throw err(404, "no_existe", "Ese emparejamiento ya no existe.");
      if (p.estado === "abierto" && Date.parse(p.caduca) < Date.now()) p.estado = "caducado";
      const e = p.equipo;
      return {
        estado: p.estado,
        caduca: p.caduca,
        equipo: e ? { id: e.id, nombre: e.nombre, so: e.so, box_pub: e.box_pub, sign_pub: e.sign_pub, sal_equipo: e.sal_equipo } : undefined,
        // v1.26: v3 si el agente lo anuncia (0.7.10 o posterior); un agente antiguo (el del código suelto, aquí 0.7.0) se queda en v2.
        sas: e ? (agenteConSasV3(e.version_agente) ? sasV3(estado.servidor.identidad, e.box_pub, e.sign_pub, HUELLA_CA) : sasV2(estado.servidor.identidad, e.box_pub, e.sign_pub)) : undefined,
        sas_version: e ? (agenteConSasV3(e.version_agente) ? 3 : undefined) : undefined,
        // v1.17: preparados, con su nombre y (mientras sirve) el código para el alta.
        ...(p.nombre ? { nombre: p.nombre, so: p.so, ...(p.estado === "abierto" || p.estado === "unido" ? { codigo: p.codigo } : {}) } : {}),
      };
    },
  ],
  [
    "POST",
    new RegExp(`^${C}/emparejamientos/([^/]+)/confirmar$`),
    (ctx, [c, id]) => {
      const { cuenta } = miembro(ctx, c, "administrador");
      const p = estado.emparejamientos.find((x) => x.id === id && x.cliente === c);
      if (!p?.equipo || p.estado !== "unido") throw err(409, "conflicto", "Ese emparejamiento ya no espera confirmación.");
      const etiqueta = String((ctx.cuerpo as { etiqueta?: string }).etiqueta ?? "");
      if (!etiqueta) throw err(422, "datos", "Falta la etiqueta del equipo.");
      p.estado = "confirmado";
      p.equipo.etiqueta = etiqueta;
      p.equipo.confirmado = true;
      estado.equipos.push(p.equipo);
      auditar(c, cuenta.id, "equipo.emparejar", p.equipo.nombre, { so: p.equipo.so });
      return undefined;
    },
  ],
  [
    "DELETE",
    new RegExp(`^${C}/emparejamientos/([^/]+)$`),
    (ctx, [c, id]) => {
      miembro(ctx, c, "administrador");
      const p = estado.emparejamientos.find((x) => x.id === id && x.cliente === c);
      if (p) p.estado = "cancelado";
      return undefined;
    },
  ],

  // --- Órdenes --------------------------------------------------------------
  [
    "POST",
    new RegExp(`^${C}/equipos/([^/]+)/ordenes$`),
    (ctx, [c, e]) => {
      const { cuenta, rol, cliente } = miembro(ctx, c, "tecnico");
      const eq = equipoDe(c, e);
      const b = ctx.cuerpo as unknown as T.NuevaOrden;
      if (!NIVEL[b.tipo]) throw err(422, "datos", "Tipo de orden desconocido.");
      if (rol === "tecnico" && SOLO_ADMIN_ROL.has(b.tipo)) throw err(403, "prohibido", "Los técnicos no pueden mandar esta orden.");
      if (b.seq !== eq.siguiente_seq) throw err(409, "conflicto", "Otra orden se adelantó. Se volverá a preparar.", { siguiente_seq: eq.siguiente_seq });
      if (Date.parse(b.caduca) > Date.now() + 7 * 86_400_000 + 120_000) throw err(422, "datos", "La orden caduca demasiado tarde (máximo 7 días).");
      if (typeof b.sellado !== "string" || b.sellado.length > 87_382) throw err(422, "datos", "El sobre ocupa más de 64 KiB.");
      // El servidor no ve el cuerpo: decide por el tipo. (desvincular y guarda_copias
      // solo son destructivas según el cuerpo; ahí manda el not_before que declara la consola.)
      const dest = esDestructiva(b.tipo) || b.not_before !== null;
      if (dest && (!b.not_before || Date.parse(b.not_before) < Date.now() + cliente.espera_min_horas * 3600_000 - 120_000))
        throw err(422, "datos", `Esta orden necesita esperar al menos ${cliente.espera_min_horas} h.`, { espera_min_horas: cliente.espera_min_horas });
      const o: OrdenMock = {
        id: randomUUID(),
        cliente: c,
        equipo: e,
        tipo: b.tipo,
        seq: b.seq,
        emitida: new Date().toISOString(),
        emitida_por: { id: cuenta.id, nombre: cuenta.nombre },
        not_before: b.not_before,
        caduca: b.caduca,
        estado: "pendiente",
        mensaje: null,
        detalle: null,
        firma_agente: null,
        actualizada: new Date().toISOString(),
        sellado: b.sellado,
        sesion: b.sesion,
        relevo: b.relevo,
      };
      // Como el servidor (protocolo/ordenes.rs): restaurar y descargar ya no abren sesión.
      if (b.sesion && !ABRE_SESION.has(b.tipo)) throw err(422, "datos", `«${b.tipo}» no abre sesión.`);
      eq.siguiente_seq++;
      estado.ordenes.push(o);
      if (b.relevo) estado.relevos.set(b.relevo.id, { id: b.relevo.id, cliente: c, estado: "subiendo", trozos: 0, bytes: 0, trozosDatos: [], maxBytes: b.relevo.max_bytes });
      if (b.not_before) {
        estado.avisos.push({ id: randomUUID(), cliente: c, equipo: e, tipo: "orden_destructiva", mensaje: `${cuenta.nombre} pidió «${b.tipo.replaceAll("_", " ")}» en ${eq.nombre}. Se hará cuando acabe la espera, salvo que alguien la cancele.`, creado: new Date().toISOString(), visto_por: null, abierto: true });
      }
      auditar(c, cuenta.id, `orden.${b.tipo}`, eq.nombre, { seq: b.seq, not_before: b.not_before });
      void procesarOrden(o);
      return ordenPublica(o);
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/equipos/([^/]+)/ordenes$`),
    (ctx, [c, e]) => {
      miembro(ctx, c);
      const limite = Number(ctx.url.searchParams.get("limite") ?? 50);
      return estado.ordenes
        .filter((o) => o.cliente === c && o.equipo === e)
        .sort((a, b) => b.seq - a.seq)
        .slice(0, limite)
        .map(ordenPublica);
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/ordenes$`),
    (ctx, [c]) => {
      miembro(ctx, c);
      const q = ctx.url.searchParams;
      if (q.get("pendientes") === "1")
        return estado.ordenes
          .filter((o) => o.cliente === c && o.estado === "pendiente" && o.not_before && Date.parse(o.not_before) > Date.now())
          .sort((a, b) => Date.parse(b.emitida) - Date.parse(a.emitida))
          .map(ordenPublica);
      // Todas, de la más reciente hacia atrás, con cursor (el índice de la siguiente).
      const limite = Math.min(200, Number(q.get("limite") ?? 50));
      const desde = Number(q.get("antes") ?? 0);
      const lista = estado.ordenes
        .filter((o) => o.cliente === c && (!q.get("equipo") || o.equipo === q.get("equipo")) && (!q.get("estado") || o.estado === q.get("estado")))
        .sort((a, b) => Date.parse(b.emitida) - Date.parse(a.emitida));
      const pagina = lista.slice(desde, desde + limite);
      return { ordenes: pagina.map(ordenPublica), siguiente: desde + limite < lista.length ? String(desde + limite) : null };
    },
  ],
  [
    "POST",
    new RegExp(`^${C}/ordenes/([^/]+)/cancelar$`),
    (ctx, [c, id]) => {
      const { cuenta } = miembro(ctx, c, "tecnico");
      const o = estado.ordenes.find((x) => x.id === id && x.cliente === c);
      if (!o) throw err(404, "no_existe", "Esa orden ya no existe.");
      const esperando = o.not_before && Date.parse(o.not_before) > Date.now();
      if (!(o.estado === "pendiente" && (esperando || !o.not_before))) throw err(409, "conflicto", "Esta orden ya no se puede cancelar: el equipo la recibió.");
      o.estado = "cancelada";
      o.actualizada = new Date().toISOString();
      const eq = estado.equipos.find((x) => x.id === o.equipo);
      for (const a of estado.avisos) if (a.tipo === "orden_destructiva" && a.equipo === o.equipo && a.abierto) a.abierto = false;
      auditar(c, cuenta.id, "orden.cancelar", eq?.nombre ?? o.equipo, { seq: o.seq, tipo: o.tipo });
      return undefined;
    },
  ],

  // --- Configuración, informes, avisos y auditoría --------------------------
  [
    "GET",
    new RegExp(`^${C}/informes$`),
    (ctx, [c]) => {
      miembro(ctx, c);
      // El último informe de cada equipo (v1.8); los que no tienen, no salen.
      return estado.equipos.filter((e) => e.cliente === c && e.informes[0]).map((e) => ({ equipo: e.id, ...e.informes[0] }));
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/progreso$`),
    (ctx, [c]) => {
      miembro(ctx, c);
      // v1.25: lo que está en marcha ahora (simulado en progreso.ts).
      return progresoDe(c);
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/equipos/([^/]+)/config$`),
    (ctx, [c, e]) => {
      miembro(ctx, c);
      const eq = equipoDe(c, e);
      if (!eq.config) throw err(404, "no_existe", "Este equipo aún no ha subido su configuración.");
      return { ...eq.config, resumen: eq.resumen };
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/resumen$`),
    (ctx, [c]) => {
      miembro(ctx, c);
      return {
        equipos: estado.equipos.filter((e) => e.cliente === c).map(publico),
        avisos_abiertos: estado.avisos.filter((a) => a.cliente === c && a.abierto).length,
        pendientes: estado.ordenes.filter((o) => o.cliente === c && o.estado === "pendiente" && o.not_before && Date.parse(o.not_before) > Date.now()).length,
      };
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/equipos/([^/]+)/informes$`),
    (ctx, [c, e]) => {
      miembro(ctx, c);
      return equipoDe(c, e).informes.slice(0, Number(ctx.url.searchParams.get("limite") ?? 20));
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/equipos/([^/]+)/historial$`),
    (ctx, [c, e]) => {
      miembro(ctx, c);
      // v1.23: lo que guarda el propio equipo (aquí, unas entradas de ejemplo de antes de llegar a este servidor).
      // v1.26: por páginas (limite, antes) y con desde/hasta/tipo, como el servidor.
      const q = ctx.url.searchParams;
      const [desde, hasta, tipos] = [q.get("desde"), q.get("hasta"), (q.get("tipo") ?? "").split(",").filter(Boolean)];
      const limite = Math.min(Math.max(Number(q.get("limite") ?? 500) || 500, 1), 2000);
      let l = historialMock(equipoDe(c, e))
        .filter((h) => (!desde || Date.parse(h.hora) > Date.parse(desde)) && (!hasta || Date.parse(h.hora) <= Date.parse(hasta)) && (!tipos.length || tipos.includes(h.tipo)))
        .sort((a, b) => Date.parse(b.hora) - Date.parse(a.hora) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
      const antes = q.get("antes");
      if (antes) {
        const i = l.findIndex((h) => h.id === antes);
        l = i < 0 ? [] : l.slice(i + 1);
      }
      return l.slice(0, limite);
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/avisos$`),
    (ctx, [c]) => {
      miembro(ctx, c);
      const abiertos = ctx.url.searchParams.get("abiertos") === "1";
      return estado.avisos
        .filter((a) => a.cliente === c && (!abiertos || a.abierto))
        .sort((a, b) => Date.parse(b.creado) - Date.parse(a.creado))
        .map(({ cliente: _c, abierto: _a, ...a }) => a);
    },
  ],
  [
    "POST",
    new RegExp(`^${C}/avisos/([^/]+)/visto$`),
    (ctx, [c, id]) => {
      // v1.10: técnico o más.
      const { cuenta } = miembro(ctx, c, "tecnico");
      const a = estado.avisos.find((x) => x.id === id && x.cliente === c);
      if (!a) throw err(404, "no_existe", "Ese aviso ya no existe.");
      a.abierto = false;
      a.visto_por = cuenta.nombre;
      return undefined;
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/auditoria$`),
    (ctx, [c]) => {
      miembro(ctx, c);
      const q = ctx.url.searchParams;
      const limite = Number(q.get("limite") ?? 100);
      const todas = estado.auditoria.get(c) ?? [];
      if (q.get("orden") === "desc") {
        const antes = q.has("antes") ? Number(q.get("antes")) : Infinity;
        return todas.filter((e) => e.n < antes).reverse().slice(0, limite);
      }
      // Como el servidor: n > desde, en orden ascendente.
      const desde = Number(q.get("desde") ?? 0);
      return todas.filter((e) => e.n > desde).slice(0, limite);
    },
  ],
  ["GET", new RegExp(`^${C}/auditoria/verificar$`), (ctx, [c]) => (miembro(ctx, c), verificarCadena(c))],

  // --- F6: recibir un cliente, fichas, paquete e importación (§11) -----------
  [
    "POST",
    /^\/api\/clientes\/recibir$/,
    (ctx) => {
      const cuenta = sesionDe(ctx);
      if (!cuenta.superusuario) throw err(403, "prohibido", "Solo quien administra el servidor puede recibir clientes.");
      const b = ctx.cuerpo as { nombre?: string; sal_cliente?: string; espera_min_horas?: number; usos?: number; dias?: number };
      if (!b.nombre?.trim() || !b.sal_cliente) throw err(422, "datos", "Faltan el nombre o la sal del cliente.");
      if (estado.clientes.some((x) => x.sal_cliente === b.sal_cliente)) throw err(409, "conflicto", "Ese cliente ya está en este servidor.");
      const horas = Number(b.espera_min_horas ?? 24);
      const c = { id: randomUUID(), nombre: b.nombre.trim(), sal_cliente: b.sal_cliente, espera_min_horas: horas, rol: "propietario" as T.Rol };
      estado.clientes.push(c);
      estado.miembros.set(c.id, [{ cuenta: cuenta.id, rol: "propietario" }]);
      auditar(c.id, cuenta.id, "recibir_cliente", c.nombre, { espera_min_horas: horas });
      return { cliente: { id: c.id, nombre: c.nombre, sal_cliente: c.sal_cliente, espera_min_horas: horas }, ...darFicha(c.id, b.usos ?? 100, b.dias ?? 7) };
    },
  ],
  [
    "POST",
    new RegExp(`^${C}/fichas$`),
    (ctx, [c]) => {
      miembro(ctx, c, "propietario");
      const b = ctx.cuerpo as { usos?: number; dias?: number };
      const dias = Number(b.dias ?? 7);
      if (!(dias >= 1 && dias <= 365)) throw err(422, "datos", "La ficha vale entre 1 y 365 días.");
      return darFicha(c, Number(b.usos ?? 1), dias);
    },
  ],
  [
    "PUT",
    new RegExp(`^${C}/paquete$`),
    (ctx, [c]) => {
      const { cliente } = miembro(ctx, c, "administrador");
      if (ctx.crudo.length > 64 * 1024 * 1024) throw err(413, "datos", "El paquete pasa de 64 MB.");
      let sal: string;
      try {
        sal = cabeceraPaquete(new Uint8Array(ctx.crudo)).sal;
      } catch {
        throw err(422, "datos", "No es un paquete de Resguardo.");
      }
      if (sal !== cliente.sal_cliente) throw err(422, "datos", "Ese paquete es de otro cliente.");
      estado.paquetes.set(c, new Uint8Array(ctx.crudo));
      return undefined;
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/paquete$`),
    (ctx, [c]) => {
      miembro(ctx, c, "administrador");
      const p = estado.paquetes.get(c);
      if (!p) throw err(404, "no_existe", "No hay ningún paquete guardado.");
      ctx.res.setHeader("Content-Type", "application/octet-stream");
      ctx.res.end(Buffer.from(p));
      return SIN_CUERPO;
    },
  ],
  ["DELETE", new RegExp(`^${C}/paquete$`), (ctx, [c]) => (miembro(ctx, c, "administrador"), estado.paquetes.delete(c), undefined)],
  [
    "POST",
    new RegExp(`^${C}/importar$`),
    (ctx, [c]) => {
      const { cuenta } = miembro(ctx, c, "propietario");
      if (estado.auditoriaImportada.has(c)) throw err(409, "conflicto", "Este cliente ya importó su historial.");
      const b = ctx.cuerpo as { origen?: string; auditoria?: T.EntradaAuditoria[]; informes?: unknown[]; avisos?: unknown[]; notas?: T.NotasExportadas };
      const aud = b.auditoria ?? [];
      // Como el servidor: la cadena, entera desde el génesis.
      let prev = "0".repeat(64);
      for (const [i, e] of aud.entries()) {
        if (e.n !== i + 1 || e.prev_hash !== prev) throw err(422, "datos", `La cadena de la actividad está rota en la entrada ${e.n}.`);
        prev = e.hash;
      }
      estado.auditoriaImportada.set(c, aud);
      const notas = importarNotas(c, b.notas);
      auditar(c, cuenta.id, "importar_cliente", b.origen ?? "", { entradas: aud.length, ultimo_hash: prev, informes: b.informes?.length ?? 0, avisos: b.avisos?.length ?? 0, ...notas });
      return { entradas: aud.length, ...notas };
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/auditoria/importada$`),
    (ctx, [c]) => {
      miembro(ctx, c, "tecnico");
      const q = ctx.url.searchParams;
      const desde = Number(q.get("desde") ?? 0);
      return (estado.auditoriaImportada.get(c) ?? []).filter((e) => e.n > desde).slice(0, Number(q.get("limite") ?? 100));
    },
  ],

  // --- Sesiones interactivas ------------------------------------------------
  [
    "POST",
    new RegExp(`^${C}/sesiones/([^/]+)/mensajes$`),
    (ctx, [c, s]) => {
      miembro(ctx, c, "tecnico");
      const ses = estado.sesionesInteractivas.get(s);
      if (!ses || ses.cliente !== c) throw err(404, "no_existe", "La sesión terminó. Ábrela de nuevo.");
      const cifrado = String((ctx.cuerpo as { cifrado?: string }).cifrado ?? "");
      const n = ses.mensajes.length + 1;
      ses.mensajes.push({ n, de: "consola", cifrado });
      ses.ultimo = Date.now();
      void mensajeDeConsola(ses, cifrado);
      return { n };
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/sesiones/([^/]+)/mensajes$`),
    async (ctx, [c, s]) => {
      miembro(ctx, c);
      const desde = Number(ctx.url.searchParams.get("desde") ?? 0);
      const nuevos = () => {
        const ses = estado.sesionesInteractivas.get(s);
        if (!ses || ses.cliente !== c) {
          // La orden que abre la sesión puede no haber llegado aún al «agente».
          const pendiente = estado.ordenes.some((o) => o.sesion === s && o.cliente === c && ["pendiente", "entregada", "en_marcha"].includes(o.estado));
          if (!pendiente) throw err(404, "no_existe", "La sesión terminó. Ábrela de nuevo.");
          return [];
        }
        return ses.mensajes.filter((m) => m.n > desde && m.de === "equipo");
      };
      let lista = nuevos();
      if (lista.length) return lista;
      // Espera larga: hasta 25 s o hasta que el «equipo» escriba.
      await new Promise<void>((ok) => {
        const t = setTimeout(ok, 25_000);
        const f = () => {
          clearTimeout(t);
          ok();
        };
        const set = esperando.get(s) ?? new Set();
        set.add(f);
        esperando.set(s, set);
        ctx.req.on("close", f);
        // Si la sesión aún no existía, se mira de vez en cuando.
        const i = setInterval(() => {
          if (estado.sesionesInteractivas.get(s)?.mensajes.some((m) => m.n > desde && m.de === "equipo")) {
            clearInterval(i);
            f();
          }
        }, 300);
        setTimeout(() => clearInterval(i), 25_000);
      });
      lista = nuevos();
      return lista;
    },
  ],
  [
    "DELETE",
    new RegExp(`^${C}/sesiones/([^/]+)$`),
    (ctx, [c, s]) => {
      miembro(ctx, c);
      cerrarSesion(s);
      return undefined;
    },
  ],

  // --- Relé -----------------------------------------------------------------
  [
    "GET",
    new RegExp(`^${C}/relevos/([^/]+)$`),
    (ctx, [c, r]) => {
      miembro(ctx, c);
      const rel = estado.relevos.get(r);
      if (!rel || rel.cliente !== c) throw err(404, "no_existe", "La descarga ya no está disponible.");
      return { estado: rel.estado, trozos: rel.trozos, bytes: rel.bytes };
    },
  ],
  [
    "GET",
    new RegExp(`^${C}/relevos/([^/]+)/trozos/(\\d+)$`),
    (ctx, [c, r, n]) => {
      miembro(ctx, c);
      const rel = estado.relevos.get(r);
      const t = rel?.cliente === c ? rel.trozosDatos[Number(n)] : undefined;
      if (!t) throw err(404, "no_existe", "Ese trozo no existe.");
      ctx.res.setHeader("Content-Type", "application/octet-stream");
      ctx.res.end(Buffer.from(t));
      return SIN_CUERPO;
    },
  ],
  [
    "DELETE",
    new RegExp(`^${C}/relevos/([^/]+)$`),
    (ctx, [c, r]) => {
      miembro(ctx, c);
      estado.relevos.delete(r);
      return undefined;
    },
  ],

  // --- Atajos del simulador -------------------------------------------------
  [
    "GET",
    /^\/api\/__mock\/entrar$/,
    (ctx) => {
      // Entra como Ana (sesión completa) y vuelve a la página pedida: para las capturas.
      const ana = estado.cuentas.find((c) => c.correo === DEMO.correo);
      if (!ana) throw err(404, "no_existe", "No hay cuenta de demostración.");
      nuevaSesion(ctx, ana.id, true);
      const volver = ctx.url.searchParams.get("volver") ?? "/";
      ctx.res.statusCode = 302;
      ctx.res.setHeader("Set-Cookie", `resguardo_sesion=${[...estado.sesiones].at(-1)![0]}; HttpOnly; SameSite=Lax; Path=/`);
      ctx.res.setHeader("Location", volver.startsWith("/") ? volver : "/");
      ctx.res.end();
      return SIN_CUERPO;
    },
  ],
  // v1.29: notificaciones (canales, prueba, registro y preferencias).
  ...rutasNotificaciones<Ctx>({ err, cuenta: (ctx) => sesionDe(ctx), miembro }),
  // v1.40: observaciones y comentarios.
  ...rutasNotas<Ctx>({ err, miembro }),
  [
    "POST",
    /^\/api\/__mock\/reiniciar$/,
    async (ctx) => {
      await iniciar(ctx.url.searchParams.get("vacio") === "1");
      return { ok: true };
    },
  ],
];

const SIN_CUERPO = Symbol("sin cuerpo");

/** Una ficha de este servidor (se muestra una vez; se guarda su hash). */
function darFicha(cliente: string, usos: number, dias: number) {
  const ficha = aB64(randomBytes(24)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  const caduca = new Date(Date.now() + dias * 86_400_000).toISOString();
  estado.fichas.push({ hash: createHash("sha256").update(ficha).digest("hex"), cliente, usos, caduca });
  return {
    ficha,
    caduca,
    usos,
    servidor: { identidad: estado.servidor.identidad, ca_pem: "-----BEGIN CERTIFICATE-----\nMIIBszCCAVmgAwIBAgIU(simulado)\n-----END CERTIFICATE-----\n" },
  };
}

async function iniciar(vacio = false) {
  await sembrar(vacio);
  sembrarNotas();
  for (const e of estado.equipos) guardarConfig(e, configInicial(e), 12);
  console.log(`\n  Resguardo Server simulado: ${vacio ? `sin cuentas (código de primer arranque: ${DEMO.codigoArranque})` : `${DEMO.correo} / ${DEMO.contrasena}, TOTP cualquier código de 6 cifras`}\n`);
}

function leer(req: IncomingMessage): Promise<Buffer> {
  return new Promise((ok, mal) => {
    const partes: Buffer[] = [];
    req.on("data", (p) => partes.push(p));
    req.on("end", () => ok(Buffer.concat(partes)));
    req.on("error", mal);
  });
}

export function mockApi(): Plugin {
  let listo: Promise<void> | null = null;
  return {
    name: "resguardo-mock-api",
    // También con `vite preview --mode mock`: la consola ya compilada (con su
    // service worker, que en `dev` no se registra) contra el mismo simulador.
    configurePreviewServer(server) {
      montar(server.middlewares);
    },
    configureServer(server) {
      montar(server.middlewares);
    },
  };
  function montar(middlewares: { use: (f: (req: IncomingMessage, res: ServerResponse, next: () => void) => void) => void }) {
    listo = iniciar(process.env.MOCK_VACIO === "1");
    setInterval(() => void revisarEsperas(), 5000);
    middlewares.use(async (req, res, next) => {
      // Marco de 375 px para las capturas de móvil (Edge sin ventana no baja de ~500 px).
      if (req.url?.startsWith("/__marco.html")) {
        res.setHeader("Content-Type", "text/html; charset=utf-8");
        return res.end(
          '<!doctype html><meta charset="utf-8"><style>html,body{margin:0;background:#888}iframe{border:0;display:block}</style><iframe id="f"></iframe><script>const p=new URLSearchParams(location.search),f=document.getElementById("f");f.width=p.get("w")||375;f.height=p.get("h")||2400;f.src=p.get("u");</script>',
        );
      }
      if (!req.url?.startsWith("/api/")) return next();
      await listo;
      const url = new URL(req.url, "http://localhost");
      const metodo = req.method ?? "GET";
      const cookies: string[] = [];
      const crudo = await leer(req);
      let cuerpo: Record<string, unknown> = {};
      try {
        cuerpo = crudo.length ? JSON.parse(crudo.toString("utf8")) : {};
      } catch {
        cuerpo = {};
      }
      const token = /(?:^|;\s*)resguardo_sesion=([^;]+)/.exec(req.headers.cookie ?? "")?.[1] ?? null;
      const ctx: Ctx = { req, res, metodo, url, cuerpo, crudo, token, setCookie: (v) => cookies.push(v) };
      const enviar = (estadoH: number, datos: unknown) => {
        if (cookies.length) res.setHeader("Set-Cookie", cookies);
        res.statusCode = estadoH;
        if (datos === undefined) return res.end();
        res.setHeader("Content-Type", "application/json; charset=utf-8");
        res.end(JSON.stringify(datos));
      };
      try {
        if (metodo !== "GET" && req.headers["x-resguardo"] !== "1") throw err(403, "prohibido", "Falta la cabecera X-Resguardo (protección CSRF).");
        // Latencia de una red de verdad (600–1500 ms), para ver los esqueletos y la barra de carga;
        // las sesiones y el relé, más rápidos (ya tienen su propia espera).
        const rapida = /\/(sesiones|relevos)\//.test(url.pathname) || url.pathname === "/api/servidor";
        await new Promise((r) => setTimeout(r, rapida ? 120 + Math.random() * 180 : 600 + Math.random() * 900));
        for (const [m, re, f] of rutas) {
          if (m !== metodo) continue;
          const x = re.exec(url.pathname);
          if (!x) continue;
          const r = await f(ctx, x.slice(1).map(decodeURIComponent));
          if (r === SIN_CUERPO) return;
          return enviar(r === undefined ? 204 : 200, r);
        }
        throw err(404, "no_existe", "Esa ruta no existe en la API.");
      } catch (e) {
        if (e instanceof HttpError) return enviar(e.estado, { error: e.codigo, mensaje: e.message, ...e.extra });
        console.error(e);
        return enviar(500, { error: "interno", mensaje: "Error interno del simulador." });
      }
    });
  }
}

/** Huella de la autoridad TLS del servidor simulado. */
const HUELLA_CA = "3F:A2:91:0C:7D:44:E8:12:5B:C0:9A:61:2E:F3:88:D7:41:0B:6C:9E:25:73:AA:5D:08:E4:C1:7F:39:B2:66:1A";
/** Marca de cada cliente (v1.32). De salida, dos clientes con su acento. */
const ACENTOS_MARCA = ["teal", "blue", "indigo", "violet", "rose", "amber", "graphite"];
const marcasMock = new Map<string, { acento?: string | null; logo?: Buffer; huella?: string; actualizada?: string; por?: string }>([
  ["0a0e1b2c-0000-4000-8000-0000000000a1", { acento: "blue" }],
  ["0a0e1b2c-0000-4000-8000-0000000000a2", { acento: "amber" }],
  ["0a0e1b2c-0000-4000-8000-0000000000a4", { acento: "teal" }],
]);
function marcaJson(c: string) {
  const m = marcasMock.get(c);
  return { acento: m?.acento ?? null, logo: m?.logo ? `/api/clientes/${c}/marca/logo?v=${m.huella}` : null, actualizada: m?.actualizada ?? null, por: m?.por ?? null };
}
/** Plantillas de copia (v1.20) por cliente: solo bytes cifrados por la consola. */
const plantillasMock = new Map<string, Map<string, { cifrado: string; actualizada: string; por: string }>>();

/** El agente anuncia SAS v3 desde 0.7.10. */
function agenteConSasV3(v: string | null | undefined): boolean {
  const [a, b, c] = String(v ?? "0").split(".").map((x) => Number(x) || 0);
  return a > 0 || b > 7 || (b === 7 && c >= 10);
}

/** El «equipo» de un emparejamiento se une solo al cabo de `ms` (como si alguien instalara el agente). */
/** El código de 15 min de esa cuenta que aún sirve (abierto o unido), el más reciente. */
function codigoDe(c: string, cuenta: string): EmparejamientoMock | undefined {
  return estado.emparejamientos
    .filter((p) => p.cliente === c && p.por === cuenta && !p.nombre && (p.estado === "abierto" || p.estado === "unido") && Date.parse(p.caduca) > Date.now())
    .sort((a, b) => b.creado - a.creado)[0];
}

function unirSolo(p: EmparejamientoMock, nombre: string, so: string, ms: number) {
      setTimeout(() => {
        if (p.estado !== "abierto" || Date.parse(p.caduca) < Date.now()) return;
        const c = p.cliente;
        const secretaBox = x25519.utils.randomSecretKey();
        const secretaFirma = ed25519.utils.randomSecretKey();
        p.equipo = {
          id: randomUUID(),
          cliente: c,
          nombre,
          so,
          version_agente: p.nombre ? "0.7.10" : "0.7.0",
          box_pub: aB64(x25519.getPublicKey(secretaBox)),
          sign_pub: aB64(ed25519.getPublicKey(secretaFirma)),
          sal_equipo: aB64(randomBytes(16)),
          etiqueta: null,
          rol: "agente",
          modo: "gestionado",
          confirmado: false,
          conectado: true,
          ultimo_contacto: new Date().toISOString(),
          estado_servicio: "en_marcha",
          siguiente_seq: 1,
          // En el servidor, el 8000 suele estar ocupado: el agente propone otro (v1.19).
          resumen: { copias: [], repositorios: [], destinos: [], puerto_libre: nombre === "SRV-RESGUARDO" ? 8002 : 8000 },
          secretaBox,
          secretaFirma,
          verificador: null,
          kcfg: null,
          contrasenas: {},
          ultimoSeqAceptado: 0,
          intentosFallidos: 0,
          codigoEmparejamiento: p.codigo,
          informes: [],
          config: null,
        };
        p.estado = "unido";
      }, ms);
}
