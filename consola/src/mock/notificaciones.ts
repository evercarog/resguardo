// Notificaciones simuladas (api-servidor.md §13): canales del servidor y de
// cada cliente, «Enviar prueba», registro de envíos y preferencias. Como el
// servidor: los secretos se guardan aparte y nunca salen (solo «configurado»),
// y crear un canal o cambiar a dónde va pide un código (aquí, cualquiera de 6
// cifras, como el TOTP del simulador). No manda nada de verdad: una dirección
// con «falla» hace que la prueba falle, para ver cómo se enseña.
import { randomUUID } from "node:crypto";
import type * as T from "../lib/tipos";
import { cambioSensible, reglasPorDefecto } from "../lib/notificaciones";
import { estado, ID } from "./estado";

type Err = (estado: number, codigo: string, mensaje: string) => Error;
type Ctx = { cuerpo: Record<string, unknown> };
type Ayudas<C> = {
  err: Err;
  /** La cuenta con sesión completa. */
  cuenta: (ctx: C) => { id: string; correo: string; nombre: string; superusuario: boolean };
  /** Miembro del cliente con al menos ese papel. */
  miembro: (ctx: C, cliente: string, minimo: T.Rol) => unknown;
};

const POR_PAPEL: Record<T.Rol, { inmediatos: T.Severidad[]; resumen: boolean }> = {
  propietario: { inmediatos: ["critico", "importante"], resumen: true },
  administrador: { inmediatos: ["critico", "importante"], resumen: true },
  tecnico: { inmediatos: ["critico"], resumen: false },
  lectura: { inmediatos: [], resumen: false },
};

const SOLO_SERVIDOR = "Los canales de un cliente no pueden mandar a este equipo ni a la red local: usa una dirección pública (o pide al propietario del servidor que lo ponga en los canales del servidor).";
const SECRETOS: Record<T.TipoCanal, string[]> = { correo: ["contrasena"], webhook: ["url", "secreto"], ntfy: ["url", "token"], telegram: ["token"] };

type Datos = {
  ajustes: Omit<T.AjustesNotif, "canales">;
  canales: Map<string, T.CanalNotif[]>;
  secretos: Map<string, Record<string, string>>;
  prefs: Map<string, { inmediatos: T.Severidad[]; resumen: boolean; etiquetas?: T.PrefEtiqueta[] }>;
  personas: Map<string, { silencio: T.Silencio | null; resumen_diario: boolean; resumen_semanal: boolean }>;
  registro: T.EnvioNotif[];
};
let st: Datos | null = null;
/** El estado de las notificaciones se siembra con el resto (y vuelve a empezar al reiniciar). */
let sembradoCon: unknown = null;

const hace = (min: number) => new Date(Date.now() - min * 60_000).toISOString();

function datos(): Datos {
  if (st && sembradoCon === estado) return st;
  sembradoCon = estado;
  const correo: T.CanalNotif = {
    id: randomUUID(),
    tipo: "correo",
    nombre: "Correo de la oficina",
    activo: true,
    config: { host: "smtp.altamar.ejemplo.com", puerto: 587, seguridad: "starttls", usuario: "avisos@altamar.ejemplo.com", remitente: "Resguardo <avisos@altamar.ejemplo.com>" },
    secretos: { contrasena: "configurado" },
    reglas: reglasPorDefecto(),
    completo: true,
    actualizado: hace(60 * 24 * 3),
    por: "Ana Restrepo",
  };
  const telegram: T.CanalNotif = {
    id: randomUUID(),
    tipo: "telegram",
    nombre: "Grupo de soporte",
    activo: true,
    config: { chat_id: "-1001234567890" },
    secretos: { token: "configurado" },
    reglas: { ...reglasPorDefecto(), severidades: ["critico"], resumen_semanal: true },
    completo: true,
    actualizado: hace(60 * 24 * 2),
    por: "Ana Restrepo",
  };
  const envio = (e: Partial<T.EnvioNotif> & Pick<T.EnvioNotif, "titulo" | "estado">): T.EnvioNotif => ({
    id: randomUUID(),
    creado: hace(30),
    enviado: null,
    canal: { id: correo.id, nombre: correo.nombre, tipo: "correo" },
    ambito: "servidor",
    cliente: ID.altamar,
    destino: "ana@ejemplo.com",
    tipo: "aviso",
    severidad: "critico",
    intentos: 1,
    siguiente: null,
    error: null,
    nota: null,
    ...e,
  });
  st = {
    ajustes: { url_consola: null, max_por_hora: 10, hora_resumen: "08:00", dia_semanal: 1 },
    canales: new Map([["servidor", [correo, telegram]]]),
    secretos: new Map<string, Record<string, string>>([
      [correo.id, { contrasena: "contraseña-del-simulador" }],
      [telegram.id, { token: "123456:SIMULADOxxxxxxxxxxxxxxxxxxxxxxxxx" }],
    ]),
    prefs: new Map(),
    personas: new Map(),
    registro: [
      envio({ titulo: "Falló la copia «Contabilidad» en «CAJA-01»", estado: "pendiente", destino: "luis@altamar.ejemplo.com", intentos: 0, siguiente: new Date(Date.now() + 5 * 3_600_000).toISOString(), nota: "En horas de silencio: sale a las 07:00." }),
      envio({ titulo: "Volvió a funcionar la copia «Documentos» en «RECEPCION»", estado: "enviado", tipo: "recuperacion", severidad: "informativo", creado: hace(50), enviado: hace(50) }),
      envio({
        titulo: "Falló la copia «Documentos» en «RECEPCION»",
        estado: "enviado",
        canal: { id: telegram.id, nombre: telegram.nombre, tipo: "telegram" },
        destino: "chat -1001234567890",
        creado: hace(190),
        enviado: hace(188),
        intentos: 2,
        nota: null,
      }),
      envio({ titulo: "Falló la copia «Documentos» en «RECEPCION»", estado: "enviado", creado: hace(190), enviado: hace(190) }),
      envio({ titulo: "«PORTATIL-MARTA» no conecta con el servidor", severidad: "importante", estado: "fallido", cliente: ID.sur, destino: "marta@cafedelsur.ejemplo.com", creado: hace(60 * 26), intentos: 1, error: "El servidor de correo no aceptó el usuario o la contraseña (535).", nota: "No se reintenta: el error no se arregla solo." }),
      envio({ titulo: "Resumen semanal de copias · 21 – 28 sep", tipo: "resumen", severidad: "informativo", estado: "enviado", cliente: null, creado: hace(60 * 24 * 6), enviado: hace(60 * 24 * 6) }),
    ],
  };
  return st;
}

const vista = (c: T.CanalNotif, secretos: Record<string, string>): T.CanalNotif => ({
  ...c,
  secretos: Object.fromEntries(Object.keys(secretos).map((k) => [k, "configurado" as const])),
  completo: completo(c, secretos),
});

function completo(c: T.CanalNotif, s: Record<string, string>) {
  if (c.tipo === "correo") return !!c.config.host && !!c.config.remitente;
  if (c.tipo === "telegram") return !!s.token && !!c.config.chat_id;
  return !!s.url;
}

const hostDe = (url: string) => {
  try {
    return new URL(url).host;
  } catch {
    return undefined;
  }
};

export function rutasNotificaciones<C extends Ctx>(h: Ayudas<C>): [string, RegExp, (ctx: C, m: string[]) => unknown][] {
  const soloServidor = (ctx: C) => {
    if (!h.cuenta(ctx).superusuario) throw h.err(403, "prohibido", "Solo quien administra el servidor.");
  };
  const codigo = (b: T.CambioCanal) => {
    if (!/^\d{6}$/.test(String(b.codigo ?? "").replace(/\D/g, ""))) throw h.err(401, "codigo", "Hace falta un código de tu aplicación de autenticación (recién sacado).");
  };
  const canalesDe = (ambito: string) => {
    const d = datos();
    if (!d.canales.has(ambito)) d.canales.set(ambito, []);
    return d.canales.get(ambito)!;
  };

  /** Este equipo o la red local (como el servidor: los canales de un cliente no pueden mandar ahí). */
  const redLocal = (host: string | undefined) => {
    const h = (host ?? "").replace(/^\[|\](:\d+)?$/g, "").replace(/:\d+$/, "").toLowerCase();
    if (h.includes(":")) return /^(f[cd]|fe[89ab]|::1$|::$)/.test(h);
    return !h.includes(".") || /^(127\.|10\.|192\.168\.|169\.254\.|172\.(1[6-9]|2\d|3[01])\.|0\.|100\.(6[4-9]|[7-9]\d|1[01]\d|12[0-7])\.)/.test(h) || /\.(local|lan|internal|localhost|corp|home\.arpa)$/.test(h);
  };

  function aplicar(ambito: string, previo: T.CanalNotif | null, b: T.CambioCanal, por: string): T.CanalNotif {
    const d = datos();
    if (cambioSensible(previo, b)) codigo(b);
    const tipo = previo?.tipo ?? b.tipo;
    if (!tipo || !SECRETOS[tipo]) throw h.err(422, "datos", "Falta el tipo de canal.");
    if (previo && b.tipo && b.tipo !== previo.tipo) throw h.err(422, "datos", "No se puede cambiar el tipo de un canal: crea otro.");
    const c: T.CanalNotif = previo ? structuredClone(previo) : { id: randomUUID(), tipo, nombre: "", activo: true, config: {}, secretos: {}, reglas: reglasPorDefecto(), completo: false, actualizado: "", por: "" };
    const secretos = { ...(d.secretos.get(c.id) ?? {}) };
    if (b.nombre !== undefined) c.nombre = b.nombre.trim();
    if (!c.nombre) c.nombre = { correo: "Correo", webhook: "Webhook", ntfy: "ntfy", telegram: "Telegram" }[tipo];
    if (b.activo !== undefined) c.activo = b.activo;
    if (b.reglas) c.reglas = { ...b.reglas, clientes: ambito === "servidor" ? b.reglas.clientes : null };
    if (b.config) {
      if (tipo === "correo") {
        if (!b.config.host?.trim()) throw h.err(422, "datos", "Escribe el servidor de correo (SMTP), p. ej. smtp.empresa.com.");
        if (!/^.+@.+\..+$/.test(b.config.remitente?.replace(/^.*</, "").replace(/>$/, "") ?? "")) throw h.err(422, "datos", "Escribe el remitente, p. ej. «Resguardo <copias@empresa.com>».");
        if (ambito !== "servidor" && redLocal(b.config.host.trim())) throw h.err(422, "datos", SOLO_SERVIDOR);
        c.config = { host: b.config.host.trim().toLowerCase(), puerto: b.config.puerto ?? 587, seguridad: b.config.seguridad ?? "starttls", usuario: b.config.usuario?.trim() || undefined, remitente: b.config.remitente!.trim() };
      } else if (tipo === "telegram") {
        if (!/^(-?\d{1,20}|@\w{5,63})$/.test(b.config.chat_id?.trim() ?? "")) throw h.err(422, "datos", "El chat de Telegram es un número (p. ej. -1001234567890) o @nombre_del_canal.");
        c.config = { chat_id: b.config.chat_id!.trim() };
      }
    }
    for (const [k, v] of Object.entries(b.secretos ?? {})) {
      if (!SECRETOS[tipo].includes(k)) throw h.err(422, "datos", `Este canal no tiene «${k}».`);
      if (!v) {
        delete secretos[k];
        if (k === "url") c.config.servidor = undefined;
        continue;
      }
      if (k === "url") {
        if (!v.startsWith("https://") && !/^http:\/\/(localhost|127\.)/.test(v)) throw h.err(422, "datos", "La dirección tiene que ser https:// (http:// solo en este mismo equipo).");
        if (ambito !== "servidor" && redLocal(hostDe(v))) throw h.err(422, "datos", SOLO_SERVIDOR);
        c.config.servidor = hostDe(v);
      }
      if (tipo === "telegram" && k === "token" && !/^\d+:[\w-]{30,}$/.test(v)) throw h.err(422, "datos", "El token del bot de Telegram tiene la forma 123456789:AA… (te lo da @BotFather).");
      secretos[k] = v;
    }
    c.actualizado = new Date().toISOString();
    c.por = por;
    d.secretos.set(c.id, secretos);
    return vista(c, secretos);
  }

  function probar(ambito: string, k: string, ctx: C, cliente: string | null) {
    const d = datos();
    const c = canalesDe(ambito).find((x) => x.id === k);
    if (!c) throw h.err(404, "no_existe", "No existe.");
    const yo = h.cuenta(ctx);
    const falla = Object.values(d.secretos.get(c.id) ?? {}).some((v) => v.includes("falla"));
    const ahora = new Date().toISOString();
    d.registro.unshift({
      id: randomUUID(),
      creado: ahora,
      enviado: falla ? null : ahora,
      canal: { id: c.id, nombre: c.nombre, tipo: c.tipo },
      ambito: ambito === "servidor" ? "servidor" : "cliente",
      cliente,
      destino: c.tipo === "correo" ? yo.correo : c.tipo === "telegram" ? `chat ${c.config.chat_id}` : (c.config.servidor ?? null),
      tipo: "prueba",
      severidad: "informativo",
      titulo: "Prueba",
      estado: falla ? "fallido" : "enviado",
      intentos: 1,
      siguiente: null,
      error: falla ? "No se pudo conectar." : null,
      nota: `Prueba de ${yo.nombre}.`,
    });
    return falla ? { ok: false, mensaje: "No se pudo conectar." } : { ok: true, mensaje: "Enviado. Mira si ha llegado." };
  }

  const registro = (cliente: string | null) => datos().registro.filter((e) => !cliente || e.cliente === cliente).slice(0, 100);
  const ajustes = (): T.AjustesNotif => ({ ...datos().ajustes, canales: canalesDe("servidor") });
  const prefsDe = (cliente: string, cuenta: string, rol: T.Rol): T.PrefsNotif => {
    const p = datos().prefs.get(`${cliente}:${cuenta}`);
    return p ? { ...p, propias: true } : { ...POR_PAPEL[rol], propias: false };
  };
  const personaDe = (cuenta: string) => datos().personas.get(cuenta) ?? { silencio: null, resumen_diario: false, resumen_semanal: true };
  const C = "/api/clientes/([^/]+)/notificaciones";

  const rutas: [string, RegExp, (ctx: C, m: string[]) => unknown][] = [
    ["GET", /^\/api\/servidor\/notificaciones$/, (ctx) => (soloServidor(ctx), ajustes())],
    [
      "PUT",
      /^\/api\/servidor\/notificaciones$/,
      (ctx) => {
        soloServidor(ctx);
        const b = ctx.cuerpo as { url_consola?: string; max_por_hora?: number; hora_resumen?: string; dia_semanal?: number };
        const a = datos().ajustes;
        if (b.url_consola !== undefined) {
          const u = b.url_consola.trim().replace(/\/+$/, "");
          if (u && !/^https?:\/\/[^/@?#\s]+(\/[^?#\s]*)?$/.test(u)) throw h.err(422, "datos", "La dirección de la consola tiene que empezar por https:// (o http://), sin usuario ni «?».");
          a.url_consola = u || null;
        }
        if (b.max_por_hora !== undefined) {
          if (b.max_por_hora < 1 || b.max_por_hora > 120) throw h.err(422, "datos", "El tope por hora va de 1 a 120.");
          a.max_por_hora = b.max_por_hora;
        }
        if (b.hora_resumen) a.hora_resumen = b.hora_resumen;
        if (b.dia_semanal) a.dia_semanal = b.dia_semanal;
        return ajustes();
      },
    ],
    [
      "POST",
      /^\/api\/servidor\/notificaciones\/canales$/,
      (ctx) => {
        soloServidor(ctx);
        const b = ctx.cuerpo as T.CambioCanal;
        if (b.tipo === "correo" && canalesDe("servidor").some((x) => x.tipo === "correo")) throw h.err(422, "datos", "Ya hay un canal de correo: cámbialo en vez de crear otro.");
        const c = aplicar("servidor", null, b, h.cuenta(ctx).nombre);
        canalesDe("servidor").push(c);
        return c;
      },
    ],
    [
      "PATCH",
      /^\/api\/servidor\/notificaciones\/canales\/([^/]+)$/,
      (ctx, [k]) => {
        soloServidor(ctx);
        const l = canalesDe("servidor");
        const i = l.findIndex((x) => x.id === k);
        if (i < 0) throw h.err(404, "no_existe", "No existe.");
        return (l[i] = aplicar("servidor", l[i], ctx.cuerpo as T.CambioCanal, h.cuenta(ctx).nombre));
      },
    ],
    [
      "DELETE",
      /^\/api\/servidor\/notificaciones\/canales\/([^/]+)$/,
      (ctx, [k]) => {
        soloServidor(ctx);
        const l = canalesDe("servidor");
        const i = l.findIndex((x) => x.id === k);
        if (i < 0) throw h.err(404, "no_existe", "No existe.");
        l.splice(i, 1);
        return undefined;
      },
    ],
    ["POST", /^\/api\/servidor\/notificaciones\/canales\/([^/]+)\/prueba$/, (ctx, [k]) => (soloServidor(ctx), probar("servidor", k, ctx, null))],
    ["GET", /^\/api\/servidor\/notificaciones\/registro$/, (ctx) => (soloServidor(ctx), registro(null))],

    [
      "GET",
      new RegExp(`^${C}$`),
      (ctx, [c]) => {
        h.miembro(ctx, c, "propietario");
        const propios = canalesDe(`cliente:${c}`);
        const correoPropio = propios.find((x) => x.tipo === "correo" && x.activo && x.completo);
        const correoServidor = canalesDe("servidor").find((x) => x.tipo === "correo" && x.activo && x.completo);
        const correo = correoPropio ? { de: "cliente" as const, nombre: correoPropio.nombre } : correoServidor ? { de: "servidor" as const, nombre: correoServidor.nombre } : null;
        const delServidor = canalesDe("servidor")
          .filter((x) => x.tipo !== "correo" && x.activo && x.completo && (!x.reglas.clientes || x.reglas.clientes.includes(c)))
          .map((x) => ({ id: x.id, nombre: x.nombre, tipo: x.tipo, severidades: x.reglas.severidades }));
        return { canales: propios, correo, servidor: { canales: delServidor, url_consola: datos().ajustes.url_consola } } satisfies T.NotifCliente;
      },
    ],
    [
      "POST",
      new RegExp(`^${C}/canales$`),
      (ctx, [c]) => {
        h.miembro(ctx, c, "propietario");
        const b = ctx.cuerpo as T.CambioCanal;
        if (b.tipo === "correo" && canalesDe(`cliente:${c}`).some((x) => x.tipo === "correo")) throw h.err(422, "datos", "Ya hay un canal de correo: cámbialo en vez de crear otro.");
        const k = aplicar(`cliente:${c}`, null, b, h.cuenta(ctx).nombre);
        canalesDe(`cliente:${c}`).push(k);
        return k;
      },
    ],
    [
      "PATCH",
      new RegExp(`^${C}/canales/([^/]+)$`),
      (ctx, [c, k]) => {
        h.miembro(ctx, c, "propietario");
        const l = canalesDe(`cliente:${c}`);
        const i = l.findIndex((x) => x.id === k);
        if (i < 0) throw h.err(404, "no_existe", "No existe.");
        return (l[i] = aplicar(`cliente:${c}`, l[i], ctx.cuerpo as T.CambioCanal, h.cuenta(ctx).nombre));
      },
    ],
    [
      "DELETE",
      new RegExp(`^${C}/canales/([^/]+)$`),
      (ctx, [c, k]) => {
        h.miembro(ctx, c, "propietario");
        const l = canalesDe(`cliente:${c}`);
        const i = l.findIndex((x) => x.id === k);
        if (i < 0) throw h.err(404, "no_existe", "No existe.");
        l.splice(i, 1);
        return undefined;
      },
    ],
    ["POST", new RegExp(`^${C}/canales/([^/]+)/prueba$`), (ctx, [c, k]) => (h.miembro(ctx, c, "propietario"), probar(`cliente:${c}`, k, ctx, c))],
    ["GET", new RegExp(`^${C}/registro$`), (ctx, [c]) => (h.miembro(ctx, c, "propietario"), registro(c))],
    [
      "GET",
      new RegExp(`^${C}/personas$`),
      (ctx, [c]) => {
        h.miembro(ctx, c, "propietario");
        return (estado.miembros.get(c) ?? []).map((m) => {
          const cuenta = estado.cuentas.find((x) => x.id === m.cuenta)!;
          return { cuenta: m.cuenta, nombre: cuenta.nombre, correo: cuenta.correo, rol: m.rol, preferencias: prefsDe(c, m.cuenta, m.rol), ...personaDe(m.cuenta) } satisfies T.PersonaNotif;
        });
      },
    ],
    [
      "PUT",
      new RegExp(`^${C}/personas/([^/]+)$`),
      (ctx, [c, cuenta]) => {
        h.miembro(ctx, c, cuenta === h.cuenta(ctx).id ? "lectura" : "propietario");
        const m = (estado.miembros.get(c) ?? []).find((x) => x.cuenta === cuenta);
        if (!m) throw h.err(404, "no_existe", "No existe.");
        const b = ctx.cuerpo as { inmediatos?: T.Severidad[]; resumen?: boolean; etiquetas?: T.PrefEtiqueta[] };
        // v1.4x: sin `etiquetas`, se conservan las que hubiera (como el servidor).
        const etiquetas = b.etiquetas ?? datos().prefs.get(`${c}:${cuenta}`)?.etiquetas ?? [];
        if (etiquetas.some((x) => !x.etiqueta.trim() || x.etiqueta.includes(","))) throw h.err(422, "datos", "Etiqueta no válida.");
        datos().prefs.set(`${c}:${cuenta}`, { inmediatos: [...new Set(b.inmediatos ?? [])], resumen: !!b.resumen, etiquetas: etiquetas.map((x) => ({ etiqueta: x.etiqueta.trim(), inmediatos: [...new Set(x.inmediatos)] })) });
        return prefsDe(c, cuenta, m.rol);
      },
    ],
    ["GET", /^\/api\/cuenta\/notificaciones$/, (ctx) => mias(h.cuenta(ctx).id)],
    [
      "PUT",
      /^\/api\/cuenta\/notificaciones$/,
      (ctx) => {
        const yo = h.cuenta(ctx);
        const b = ctx.cuerpo as { silencio?: T.Silencio | null; resumen_diario?: boolean; resumen_semanal?: boolean };
        const p = { ...personaDe(yo.id) };
        if (b.silencio !== undefined) {
          if (b.silencio && ![b.silencio.desde, b.silencio.hasta].every((x) => /^([01]\d|2[0-3]):[0-5]\d$/.test(String(x)))) throw h.err(422, "datos", "Las horas de silencio van como HH:MM (p. ej. 22:00 a 07:00).");
          p.silencio = b.silencio ? { desde: b.silencio.desde, hasta: b.silencio.hasta, salvo_criticos: b.silencio.salvo_criticos ?? true } : null;
        }
        if (b.resumen_diario !== undefined) p.resumen_diario = b.resumen_diario;
        if (b.resumen_semanal !== undefined) p.resumen_semanal = b.resumen_semanal;
        datos().personas.set(yo.id, p);
        return mias(yo.id);
      },
    ],
  ];

  function mias(cuenta: string): T.MisNotif {
    const a = datos().ajustes;
    const correoServidor = canalesDe("servidor").some((x) => x.tipo === "correo" && x.activo && x.completo);
    const clientes = estado.clientes
      .map((cl) => ({ cl, rol: (estado.miembros.get(cl.id) ?? []).find((m) => m.cuenta === cuenta)?.rol }))
      .filter((x): x is { cl: (typeof estado.clientes)[number]; rol: T.Rol } => !!x.rol)
      .map(({ cl, rol }) => ({
        id: cl.id,
        nombre: cl.nombre,
        rol,
        correo: correoServidor || canalesDe(`cliente:${cl.id}`).some((x) => x.tipo === "correo" && x.activo && x.completo),
        preferencias: prefsDe(cl.id, cuenta, rol),
        etiquetas: [...new Set(estado.equipos.filter((e) => e.cliente === cl.id).flatMap((e) => e.etiquetas ?? []))].sort((a, b) => a.localeCompare(b, "es")),
      }));
    return { ...personaDe(cuenta), hora_resumen: a.hora_resumen, dia_semanal: a.dia_semanal, clientes };
  }

  return rutas;
}
