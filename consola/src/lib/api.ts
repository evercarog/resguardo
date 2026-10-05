// Cliente de la API de Resguardo Server v1 (docs/api-servidor.md).
//
// - Sesión por cookie (`resguardo_sesion`, HttpOnly): el navegador la manda
//   solo; aquí nunca se ve ni se guarda.
// - Todo lo que cambia algo lleva `X-Resguardo: 1` (defensa CSRF).
// - Los errores llegan como { error, mensaje }; se convierten en ApiError con
//   un texto en español para la persona (el del servidor o uno propio).
// - Las sesiones interactivas usan espera larga (hasta 25 s por petición).
import type * as T from "./tipos";
import { conexionOk, conexionPerdida, empezar } from "./actividad.svelte";

export class ApiError extends Error {
  constructor(
    readonly codigo: string,
    mensaje: string,
    readonly estado: number,
    readonly cuerpo?: Record<string, unknown>,
  ) {
    super(mensaje);
  }
}

/** Textos por defecto (si el servidor no manda uno). */
const MENSAJES: Record<string, string> = {
  sin_sesion: "Tu sesión terminó. Vuelve a entrar.",
  necesita_totp: "Falta el código de verificación de tu aplicación.",
  prohibido: "Tu papel en este cliente no permite hacer esto. Si lo necesitas, pide a una persona propietaria que cambie tu papel.",
  no_existe: "No lo encontramos. Puede que alguien lo haya quitado.",
  conflicto: "Alguien hizo un cambio a la vez. Vuelve a intentarlo.",
  datos: "Hay datos que no son válidos. Revísalos.",
  demasiados_intentos: "Demasiados intentos. Espera unos minutos y vuelve a probar.",
  interno: "El servidor tuvo un problema. Vuelve a intentarlo en un momento; si se repite, avisa a quien lo administra (su registro dirá qué pasó).",
  codigo: "El código no vale o ya caducó.",
  red: "No hay conexión con el servidor. Comprueba la red y vuelve a intentarlo.",
};

export const mensajeDe = (codigo: string) => MENSAJES[codigo] ?? "Algo salió mal. Vuelve a intentarlo.";

/** Se llama cuando el servidor dice que no hay sesión (para volver a «Entrar»). */
let alPerderSesion: ((codigo: string) => void) | null = null;
export const onSinSesion = (fn: (codigo: string) => void) => (alPerderSesion = fn);

type Metodo = "GET" | "POST" | "PUT" | "PATCH" | "DELETE";

async function pedir<R>(metodo: Metodo, ruta: string, cuerpo?: unknown, opciones: { signal?: AbortSignal; sinRedirigir?: boolean; invisible?: boolean } = {}): Promise<R> {
  const fin = empezar(opciones.invisible);
  try {
    return await pedirSinContar<R>(metodo, ruta, cuerpo, opciones);
  } finally {
    fin();
  }
}

async function pedirSinContar<R>(metodo: Metodo, ruta: string, cuerpo: unknown, opciones: { signal?: AbortSignal; sinRedirigir?: boolean }): Promise<R> {
  const headers: Record<string, string> = { Accept: "application/json" };
  if (metodo !== "GET") headers["X-Resguardo"] = "1";
  if (cuerpo !== undefined) headers["Content-Type"] = "application/json";
  let res: Response;
  try {
    res = await fetch(ruta, {
      method: metodo,
      headers,
      body: cuerpo === undefined ? undefined : JSON.stringify(cuerpo),
      credentials: "same-origin",
      cache: "no-store",
      signal: opciones.signal,
    });
  } catch (e) {
    if ((e as Error).name === "AbortError") throw e;
    conexionPerdida();
    throw new ApiError("red", mensajeDe("red"), 0);
  }
  conexionOk();
  if (res.status === 204) return undefined as R;
  const texto = await res.text();
  let datos: unknown = undefined;
  if (texto) {
    try {
      datos = JSON.parse(texto);
    } catch {
      datos = undefined;
    }
  }
  if (!res.ok) {
    const d = (datos ?? {}) as { error?: string; mensaje?: string };
    const codigo = d.error ?? (res.status === 401 ? "sin_sesion" : res.status === 403 ? "prohibido" : res.status === 404 ? "no_existe" : res.status === 429 ? "demasiados_intentos" : "interno");
    const err = new ApiError(codigo, d.mensaje || mensajeDe(codigo), res.status, datos as Record<string, unknown>);
    if (res.status === 401 && !opciones.sinRedirigir) alPerderSesion?.(codigo);
    throw err;
  }
  return datos as R;
}

const enc = encodeURIComponent;
const cli = (c: string) => `/api/clientes/${enc(c)}`;

// ---------------------------------------------------------------------------
// Servidor y cuentas (§2)
// ---------------------------------------------------------------------------

export const servidor = () => pedir<T.Servidor>("GET", "/api/servidor");
/** v1.23: «Copia de la consola». Con un servidor anterior (404), null. */
export const respaldoConsola = () =>
  pedir<T.RespaldoConsola>("GET", "/api/servidor/respaldo", undefined, { invisible: true }).catch((x: unknown) => {
    if (x instanceof ApiError && x.estado === 404) return null;
    throw x;
  });
export const cambiarRespaldoConsola = (b: { activo?: boolean; publica?: string; sal?: string; conservar?: number; hora?: string }) =>
  pedir<T.RespaldoConsola>("PUT", "/api/servidor/respaldo", b);
export const respaldoConsolaAhora = () => pedir<T.RespaldoConsola>("POST", "/api/servidor/respaldo/ahora");

export const primerArranque = (b: { codigo_arranque: string; correo: string; nombre: string; contrasena: string }) =>
  pedir<{ totp: T.Totp }>("POST", "/api/inicio", b, { sinRedirigir: true });

export const entrar = (correo: string, contrasena: string) => pedir<T.RespuestaSesion>("POST", "/api/sesion", { correo, contrasena }, { sinRedirigir: true });

/** v1.27: el código que dio el propietario tras restablecer la verificación en dos pasos. */
export const usarRestablecimiento = (codigo: string) => pedir<T.RespuestaSesion>("POST", "/api/sesion/restablecimiento", { codigo }, { sinRedirigir: true });
export const segundoPaso = (b: { codigo: string } | { recuperacion: string }) => pedir<T.RespuestaTotp>("POST", "/api/sesion/totp", b, { sinRedirigir: true });

export const salir = () => pedir<void>("DELETE", "/api/sesion", undefined, { sinRedirigir: true });

export const cuenta = (opciones: { sinRedirigir?: boolean } = {}) => pedir<T.Cuenta>("GET", "/api/cuenta", undefined, opciones);

export const cambiarContrasena = (actual: string, nueva: string) => pedir<void>("PUT", "/api/cuenta/contrasena", { actual, nueva });
export const renombrarCuenta = (nombre: string) => pedir<T.Cuenta>("PATCH", "/api/cuenta", { nombre });
/** Nuevo autenticador: 1) con la contraseña y un código del actual; 2) con un código del nuevo (cierra las demás sesiones). */
export const totpNuevo = (contrasena: string, codigo: string) => pedir<{ totp: T.Totp; caduca: string }>("POST", "/api/cuenta/totp", { contrasena, codigo });
export const totpConfirmar = (codigo: string) => pedir<{ cuenta: T.Cuenta }>("POST", "/api/cuenta/totp/confirmar", { codigo });
/** Códigos de recuperación nuevos: los anteriores dejan de valer. */
export const nuevosCodigos = (contrasena: string, codigo: string) => pedir<{ codigos_recuperacion: string[] }>("POST", "/api/cuenta/recuperacion", { contrasena, codigo });

// ---------------------------------------------------------------------------
// Clientes y usuarios (§3)
// ---------------------------------------------------------------------------

export const clientes = () => pedir<T.ClienteResumen[]>("GET", "/api/clientes");
export const crearCliente = (nombre: string, espera_min_horas: number) =>
  pedir<{ id: string; nombre: string; sal_cliente: string }>("POST", "/api/clientes", { nombre, espera_min_horas });
export const cliente = (c: string) => pedir<T.Cliente>("GET", cli(c));
// v1.32: marca del cliente. `logo`: PNG en base64 (ya convertido en el navegador, lib/marca.ts).
export const marca = (c: string) => pedir<T.MarcaCliente>("GET", `${cli(c)}/marca`);
export const cambiarMarca = (c: string, b: { acento: T.MarcaCliente["acento"]; logo?: string; quitar_logo?: boolean }) => pedir<T.MarcaCliente>("PUT", `${cli(c)}/marca`, b);
export const renombrarCliente = (c: string, nombre: string) => pedir<T.Cliente>("PATCH", cli(c), { nombre });
export const miembros = (c: string) => pedir<T.Miembro[]>("GET", `${cli(c)}/miembros`);
export const cambiarRol = (c: string, cuentaId: string, rol: T.Rol) => pedir<void>("PUT", `${cli(c)}/miembros/${enc(cuentaId)}`, { rol });
/** v1.27: restablece la verificación en dos pasos de otro (con un código del autenticador de quien lo hace). */
export const restablecerTotp = (c: string, cuentaId: string, codigo: string) =>
  pedir<T.CodigoRestablecimiento>("POST", `${cli(c)}/miembros/${enc(cuentaId)}/restablecer-totp`, { codigo }, { sinRedirigir: true });
export const quitarMiembro = (c: string, cuentaId: string) => pedir<void>("DELETE", `${cli(c)}/miembros/${enc(cuentaId)}`);
export const invitar = (c: string, rol: T.Rol) => pedir<T.Invitacion>("POST", `${cli(c)}/invitaciones`, { rol });
export const aceptarInvitacion = (b: { token: string; correo?: string; nombre?: string; contrasena?: string }) =>
  pedir<T.RespuestaSesion | Record<string, never>>("POST", "/api/invitaciones/aceptar", b, { sinRedirigir: true });

// ---------------------------------------------------------------------------
// Clientes del servidor (v1.34, solo el propietario del servidor: cifras, sin entrar)
// ---------------------------------------------------------------------------

export const clientesDelServidor = () => pedir<T.ClientesDelServidor>("GET", "/api/servidor/clientes");
/** v1.38: «Todos los clientes»: lo de cada cliente del que la cuenta es miembro, en una petición (404 con un servidor anterior). */
export const panel = (opciones: { invisible?: boolean } = {}) => pedir<import("./global").Panel>("GET", "/api/panel", undefined, opciones);
/** v1.38: solo lo que está en marcha en todos esos clientes. */
export const panelProgreso = () => pedir<import("./global").ProgresoPanel[]>("GET", "/api/panel/progreso", undefined, { invisible: true });
/** Un cliente para otra persona: sin ser miembro, con su invitación de propietario. */
export const crearClienteParaOtro = (b: { nombre: string; espera_min_horas?: number; cuotas?: T.Cuotas }) =>
  pedir<{ id: string; nombre: string; invitacion: T.Invitacion }>("POST", "/api/servidor/clientes", b);
/** Otra invitación de propietario, solo si el cliente aún no tiene ninguno. */
export const invitarPropietario = (c: string) => pedir<{ invitacion: T.Invitacion }>("POST", `/api/servidor/clientes/${enc(c)}/invitacion`);
export const ponerCuotas = (c: string, cuotas: T.Cuotas) => pedir<{ cuotas: T.Cuotas; efectivas: T.Cuotas }>("PUT", `/api/servidor/clientes/${enc(c)}/cuotas`, cuotas);
export const ponerCuotasPredeterminadas = (cuotas: T.Cuotas) => pedir<{ predeterminadas: T.Cuotas }>("PUT", "/api/servidor/cuotas", cuotas);

// ---------------------------------------------------------------------------
// Cambiar de servidor, respaldo y exportar (§11)
// ---------------------------------------------------------------------------

/** En el servidor nuevo: crea el cliente con la sal del antiguo y da una ficha de un solo uso. */
export const recibirCliente = (b: { nombre: string; sal_cliente: string; espera_min_horas?: number; usos?: number; dias?: number }) =>
  pedir<T.ClienteRecibido>("POST", "/api/clientes/recibir", b);
export const nuevaFicha = (c: string, b: { usos?: number; dias?: number }) => pedir<T.Ficha>("POST", `${cli(c)}/fichas`, b);

/** El paquete cifrado (.resguardo-cliente): el servidor solo guarda bytes. */
export async function subirPaquete(c: string, datos: Uint8Array): Promise<void> {
  let res: Response;
  try {
    res = await fetch(`${cli(c)}/paquete`, { method: "PUT", headers: { "X-Resguardo": "1", "Content-Type": "application/octet-stream" }, body: new Blob([new Uint8Array(datos)]), credentials: "same-origin" });
  } catch {
    throw new ApiError("red", mensajeDe("red"), 0);
  }
  if (!res.ok) {
    let d: { error?: string; mensaje?: string } = {};
    try {
      d = await res.json();
    } catch {
      /* sin cuerpo */
    }
    throw new ApiError(d.error ?? "interno", d.mensaje || mensajeDe(d.error ?? "interno"), res.status);
  }
}
export async function bajarPaquete(c: string): Promise<Uint8Array | null> {
  const res = await fetch(`${cli(c)}/paquete`, { credentials: "same-origin", cache: "no-store" });
  if (res.status === 404) return null;
  if (!res.ok) throw new ApiError("interno", mensajeDe("interno"), res.status);
  return new Uint8Array(await res.arrayBuffer());
}
export const borrarPaquete = (c: string) => pedir<void>("DELETE", `${cli(c)}/paquete`);
/** En el servidor nuevo: el historial que el navegador sacó del paquete. */
export const importarHistorial = (c: string, b: { origen: string; auditoria: T.EntradaAuditoria[]; informes: { equipo: string; recibido: string; datos: unknown }[]; avisos: { equipo: string | null; tipo: string; mensaje: string; creado: string }[]; notas?: T.NotasExportadas }) =>
  pedir<unknown>("POST", `${cli(c)}/importar`, b);
export const auditoriaImportada = (c: string, desde = 0, limite = 500) => pedir<T.EntradaAuditoria[]>("GET", `${cli(c)}/auditoria/importada?desde=${desde}&limite=${limite}`);

// ---------------------------------------------------------------------------
// Equipos y emparejamiento (§4)
// ---------------------------------------------------------------------------

export const equipos = (c: string) => pedir<T.Equipo[]>("GET", `${cli(c)}/equipos`);
export const equipo = (c: string, e: string) => pedir<T.EquipoDetalle>("GET", `${cli(c)}/equipos/${enc(e)}`);
export const renombrarEquipo = (c: string, e: string, nombre: string) => pedir<T.Equipo>("PATCH", `${cli(c)}/equipos/${enc(e)}`, { nombre });
/** v1.18: las etiquetas libres del equipo (técnico o más). Devuelve el equipo. */
export const ponerEtiquetas = (c: string, e: string, etiquetas: string[]) => pedir<T.EquipoDetalle>("PUT", `${cli(c)}/equipos/${enc(e)}/etiquetas`, { etiquetas });
export const pedirAtencion =(c: string, e: string) => pedir<void>("POST", `${cli(c)}/equipos/${enc(e)}/atencion`);

export const abrirEmparejamiento = (c: string) => pedir<T.Emparejamiento>("POST", `${cli(c)}/emparejamientos`);
export const emparejamiento = (c: string, p: string) => pedir<T.EstadoDeEmparejamiento>("GET", `${cli(c)}/emparejamientos/${enc(p)}`);
export const confirmarEmparejamiento = (c: string, p: string, etiqueta: string) => pedir<void>("POST", `${cli(c)}/emparejamientos/${enc(p)}/confirmar`, { etiqueta });
export const cancelarEmparejamiento = (c: string, p: string) => pedir<void>("DELETE", `${cli(c)}/emparejamientos/${enc(p)}`);

// v1.17: equipos preparados (instalador listo o línea de Linux, código de 24 h).
// v1.20: plantillas de copia (cifradas en el navegador; el servidor guarda bytes).
export const plantillas = (c: string) => pedir<{ id: string; cifrado: string; actualizada: string; por: string }[]>("GET", `${cli(c)}/plantillas`);
export const ponerPlantilla = (c: string, id: string, cifrado: string) => pedir<void>("PUT", `${cli(c)}/plantillas/${enc(id)}`, { cifrado });
export const borrarPlantilla = (c: string, id: string) => pedir<void>("DELETE", `${cli(c)}/plantillas/${enc(id)}`);
// v1.3x: observaciones y comentarios (en claro en el servidor; lib/notas.svelte.ts).
export const indiceNotas = (c: string) => pedir<{ objetos: T.IndiceNota[] }>("GET", `${cli(c)}/notas`, undefined, { invisible: true });
export const notasDe = (c: string, tipo: T.TipoNota, objeto: string) =>
  pedir<T.NotasObjeto>("GET", `${cli(c)}/notas/objeto?tipo=${enc(tipo)}&objeto=${enc(objeto)}`, undefined, { invisible: true });
export const ponerObservacion = (c: string, tipo: T.TipoNota, objeto: string, texto: string) =>
  pedir<T.ObservacionNota | null>("PUT", `${cli(c)}/notas/observacion`, { tipo, objeto, texto });
export const comentar = (c: string, tipo: T.TipoNota, objeto: string, texto: string) => pedir<T.ComentarioNota>("POST", `${cli(c)}/notas/comentarios`, { tipo, objeto, texto });
export const editarComentario = (c: string, id: string, texto: string) => pedir<T.ComentarioNota>("PATCH", `${cli(c)}/notas/comentarios/${enc(id)}`, { texto });
export const borrarComentario = (c: string, id: string) => pedir<void>("DELETE", `${cli(c)}/notas/comentarios/${enc(id)}`);
export const notasTodas = (c: string) => pedir<T.NotasExportadas>("GET", `${cli(c)}/notas/todas`);
/** v1.19: «Vincular este servidor» (el agente de la máquina del servidor se une solo). */
export const vincularLocal = (c: string) => pedir<T.Preparado>("POST", `${cli(c)}/equipo-local`);
export const preparados = (c: string) => pedir<T.Preparado[]>("GET", `${cli(c)}/emparejamientos`);
export const prepararLinux = (c: string, nombre: string, servidor: string) => pedir<T.PreparadoLinux>("POST", `${cli(c)}/instaladores`, { nombre, so: "linux", servidor });
/** El instalador del agente con la cola para vincular: el archivo, su nombre y el emparejamiento. */
export async function prepararInstalador(c: string, nombre: string, servidor: string): Promise<{ datos: Blob; archivo: string; id: string; caduca: string }> {
  const fin = empezar();
  let res: Response;
  try {
    res = await fetch(`${cli(c)}/instaladores`, {
      method: "POST",
      headers: { "X-Resguardo": "1", "Content-Type": "application/json", Accept: "application/octet-stream, application/json" },
      body: JSON.stringify({ nombre, so: "windows", servidor }),
      credentials: "same-origin",
      cache: "no-store",
    });
  } catch {
    fin();
    conexionPerdida();
    throw new ApiError("red", mensajeDe("red"), 0);
  }
  try {
    conexionOk();
    if (!res.ok) {
      let d: { error?: string; mensaje?: string } = {};
      try {
        d = await res.json();
      } catch {
        /* sin cuerpo */
      }
      const codigo = d.error ?? "interno";
      if (res.status === 401) alPerderSesion?.(codigo);
      throw new ApiError(codigo, d.mensaje || mensajeDe(codigo), res.status);
    }
    const archivo = /filename="([^"]+)"/.exec(res.headers.get("content-disposition") ?? "")?.[1] ?? "Resguardo-Agente.exe";
    return { datos: await res.blob(), archivo, id: res.headers.get("x-resguardo-emparejamiento") ?? "", caduca: res.headers.get("x-resguardo-caduca") ?? "" };
  } finally {
    fin();
  }
}

// ---------------------------------------------------------------------------
// Órdenes (§5)
// ---------------------------------------------------------------------------

export const enviarOrden = (c: string, e: string, o: T.NuevaOrden) => pedir<T.Orden>("POST", `${cli(c)}/equipos/${enc(e)}/ordenes`, o);
export const ordenesEquipo = (c: string, e: string, limite = 50) => pedir<T.Orden[]>("GET", `${cli(c)}/equipos/${enc(e)}/ordenes?limite=${limite}`);
export const ordenesPendientes = (c: string) => pedir<T.Orden[]>("GET", `${cli(c)}/ordenes?pendientes=1`);
/** Las últimas órdenes de todo el cliente, de la más reciente hacia atrás; `siguiente` es el cursor de la página siguiente. */
export const ordenesCliente = (c: string, f: { limite?: number; antes?: string | null; equipo?: string; estado?: string } = {}) => {
  const q = new URLSearchParams({ limite: String(f.limite ?? 50) });
  if (f.antes) q.set("antes", f.antes);
  if (f.equipo) q.set("equipo", f.equipo);
  if (f.estado) q.set("estado", f.estado);
  return pedir<{ ordenes: T.Orden[]; siguiente: string | null }>("GET", `${cli(c)}/ordenes?${q}`);
};
export const cancelarOrden = (c: string, o: string) => pedir<void>("POST", `${cli(c)}/ordenes/${enc(o)}/cancelar`);

// ---------------------------------------------------------------------------
// Configuración, informes, avisos y auditoría (§6)
// ---------------------------------------------------------------------------

export const configEquipo = (c: string, e: string) => pedir<T.ConfigCifrada>("GET", `${cli(c)}/equipos/${enc(e)}/config`);
export const resumen = (c: string) => pedir<T.Resumen>("GET", `${cli(c)}/resumen`);
/** El último informe de cada equipo del cliente (v1.8; los equipos sin informe no salen). */
/** v1.25: lo que está en marcha ahora en los equipos del cliente (no cuenta como «cargar»). */
export const progreso = (c: string) => pedir<T.ProgresoEquipo[]>("GET", `${cli(c)}/progreso`, undefined, { invisible: true });
export const ultimosInformes = (c: string) => pedir<(T.Informe & { equipo: string })[]>("GET", `${cli(c)}/informes`);
export const informes = (c: string, e: string, limite = 20) => pedir<T.Informe[]>("GET", `${cli(c)}/equipos/${enc(e)}/informes?limite=${limite}`);
/** v1.26: entradas del historial por página (el servidor da como mucho 2000). */
export const HISTORIAL_POR_PAGINA = 500;
/**
 * v1.23: el historial que guarda el propio equipo (de lo más reciente a lo más
 * antiguo). Con un servidor anterior (404), vacío. v1.26: por páginas; `antes`,
 * el id de la última entrada de la página anterior. Una página con menos de
 * `limite` entradas es la última. (Un servidor anterior a v1.26 no entiende
 * `antes`: repite la primera página; quien pide más quita las repetidas.)
 */
export const historialEquipo = (c: string, e: string, o: { antes?: string; desde?: string; hasta?: string; tipo?: string[]; limite?: number } = {}) => {
  const q = new URLSearchParams({ limite: String(o.limite ?? HISTORIAL_POR_PAGINA) });
  if (o.antes) q.set("antes", o.antes);
  if (o.desde) q.set("desde", o.desde);
  if (o.hasta) q.set("hasta", o.hasta);
  if (o.tipo?.length) q.set("tipo", o.tipo.join(","));
  return pedir<T.EntradaHistorial[]>("GET", `${cli(c)}/equipos/${enc(e)}/historial?${q}`, undefined, { invisible: true }).catch((x: unknown) => {
    if (x instanceof ApiError && x.estado === 404) return [] as T.EntradaHistorial[];
    throw x;
  });
};
export const avisos = (c: string, abiertos = true) => pedir<T.Aviso[]>("GET", `${cli(c)}/avisos${abiertos ? "?abiertos=1" : ""}`);
export const marcarVisto = (c: string, a: string) => pedir<void>("POST", `${cli(c)}/avisos/${enc(a)}/visto`);
export const auditoria = (c: string, desde?: number, limite = 100) =>
  pedir<T.EntradaAuditoria[]>("GET", `${cli(c)}/auditoria?${desde !== undefined ? `desde=${desde}&` : ""}limite=${limite}`);
/** La actividad de la más reciente hacia atrás (n < antes; sin antes, desde la última). */
export const auditoriaReciente = (c: string, antes?: number, limite = 100) =>
  pedir<T.EntradaAuditoria[]>("GET", `${cli(c)}/auditoria?orden=desc&${antes !== undefined ? `antes=${antes}&` : ""}limite=${limite}`);
export const verificarAuditoria = (c: string) => pedir<T.VerificacionAuditoria>("GET", `${cli(c)}/auditoria/verificar`);

// ---------------------------------------------------------------------------
// Notificaciones (§13, v1.29). Con un servidor anterior (404), null: la consola no las ofrece.
// ---------------------------------------------------------------------------

const o404 = <R>(p: Promise<R>) =>
  p.catch((x: unknown) => {
    if (x instanceof ApiError && x.estado === 404) return null;
    throw x;
  });

/** Dónde vive un canal: el servidor (su propietario) o un cliente (sus propietarios). */
export type AmbitoNotif = { servidor: true } | { cliente: string };
const baseNotif = (a: AmbitoNotif) => ("cliente" in a ? `${cli(a.cliente)}/notificaciones` : "/api/servidor/notificaciones");

export const notifServidor = () => o404(pedir<T.AjustesNotif>("GET", "/api/servidor/notificaciones", undefined, { invisible: true }));
export const cambiarNotifServidor = (b: { url_consola?: string; max_por_hora?: number; hora_resumen?: string; dia_semanal?: number }) =>
  pedir<T.AjustesNotif>("PUT", "/api/servidor/notificaciones", b);
export const notifCliente = (c: string) => o404(pedir<T.NotifCliente>("GET", `${cli(c)}/notificaciones`, undefined, { invisible: true }));
export const crearCanal = (a: AmbitoNotif, b: T.CambioCanal) => pedir<T.CanalNotif>("POST", `${baseNotif(a)}/canales`, b);
export const cambiarCanal = (a: AmbitoNotif, k: string, b: T.CambioCanal) => pedir<T.CanalNotif>("PATCH", `${baseNotif(a)}/canales/${enc(k)}`, b);
export const borrarCanal = (a: AmbitoNotif, k: string) => pedir<void>("DELETE", `${baseNotif(a)}/canales/${enc(k)}`);
/** «Enviar prueba»: al momento; el correo, a quien la pide. */
export const probarCanal = (a: AmbitoNotif, k: string) => pedir<{ ok: boolean; mensaje: string }>("POST", `${baseNotif(a)}/canales/${enc(k)}/prueba`);
/** Los últimos 100 envíos (del servidor entero, o de un cliente). */
export const registroNotif = (a: AmbitoNotif) => pedir<T.EnvioNotif[]>("GET", `${baseNotif(a)}/registro`, undefined, { invisible: true });
export const personasNotif = (c: string) => o404(pedir<T.PersonaNotif[]>("GET", `${cli(c)}/notificaciones/personas`, undefined, { invisible: true }));
export const ponerPrefsNotif = (c: string, cuenta: string, b: { inmediatos: T.Severidad[]; resumen: boolean }) =>
  pedir<T.PrefsNotif>("PUT", `${cli(c)}/notificaciones/personas/${enc(cuenta)}`, b);
export const misNotif = () => o404(pedir<T.MisNotif>("GET", "/api/cuenta/notificaciones", undefined, { invisible: true }));
export const cambiarMisNotif = (b: { silencio?: T.Silencio | null; resumen_diario?: boolean; resumen_semanal?: boolean }) =>
  pedir<T.MisNotif>("PUT", "/api/cuenta/notificaciones", b);

// ---------------------------------------------------------------------------
// Sesiones interactivas (§7)
// ---------------------------------------------------------------------------

export const enviarASesion = (c: string, s: string, cifrado: string) => pedir<{ n: number }>("POST", `${cli(c)}/sesiones/${enc(s)}/mensajes`, { cifrado });
export const cerrarSesion = (c: string, s: string) => pedir<void>("DELETE", `${cli(c)}/sesiones/${enc(s)}`);

/**
 * Escucha una sesión con espera larga: cada petición espera hasta 25 s a que
 * haya mensajes nuevos. Llama a `alRecibir` con los del equipo, en orden, y
 * sigue hasta que se aborta `signal`. Tras un error de red espera un poco
 * (1, 2, 4… hasta 15 s) y vuelve a intentarlo.
 */
export async function escucharSesion(
  c: string,
  s: string,
  alRecibir: (m: T.MensajeSesion) => void,
  signal: AbortSignal,
  alError?: (e: ApiError) => void,
): Promise<void> {
  let desde = 0;
  let espera = 1000;
  while (!signal.aborted) {
    try {
      const nuevos = await pedir<T.MensajeSesion[]>("GET", `${cli(c)}/sesiones/${enc(s)}/mensajes?desde=${desde}`, undefined, { signal, invisible: true });
      espera = 1000;
      // El servidor devuelve los de n > desde (n empieza en 1).
      for (const m of nuevos ?? []) {
        desde = Math.max(desde, m.n);
        if (m.de === "equipo") alRecibir(m);
      }
    } catch (e) {
      if (signal.aborted) return;
      if (e instanceof ApiError) {
        // La sesión ya no existe (caducó o se cerró): no tiene sentido insistir.
        if (e.estado === 404 || e.estado === 401 || e.estado === 403) {
          alError?.(e);
          return;
        }
        alError?.(e);
      }
      await new Promise((r) => setTimeout(r, espera));
      espera = Math.min(espera * 2, 15_000);
    }
  }
}

// ---------------------------------------------------------------------------
// Relé de descargas (§9)
// ---------------------------------------------------------------------------

export const relevo = (c: string, r: string) => pedir<T.Relevo>("GET", `${cli(c)}/relevos/${enc(r)}`);
export const borrarRelevo = (c: string, r: string) => pedir<void>("DELETE", `${cli(c)}/relevos/${enc(r)}`);

export async function trozoRelevo(c: string, r: string, n: number, signal?: AbortSignal): Promise<Uint8Array> {
  let res: Response;
  try {
    res = await fetch(`${cli(c)}/relevos/${enc(r)}/trozos/${n}`, { credentials: "same-origin", cache: "no-store", signal });
  } catch (e) {
    if ((e as Error).name === "AbortError") throw e;
    throw new ApiError("red", mensajeDe("red"), 0);
  }
  if (!res.ok) {
    let d: { error?: string; mensaje?: string } = {};
    try {
      d = await res.json();
    } catch {
      /* binario o vacío */
    }
    throw new ApiError(d.error ?? "interno", d.mensaje || mensajeDe(d.error ?? "interno"), res.status);
  }
  return new Uint8Array(await res.arrayBuffer());
}
