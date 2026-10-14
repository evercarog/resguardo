// Estado en memoria del servidor simulado (solo `npm run dev:mock`).
// Imita a Resguardo Server lo justo para diseñar y probar la consola: cuentas
// con TOTP de mentira, clientes, equipos con llaves de verdad (el «agente»
// simulado abre los sobres y firma los resultados), órdenes, avisos,
// auditoría encadenada, sesiones y relé.
import type { SistemaEquipo } from "../lib/datosBasicos";
import { informeRepo, type OpcionesHistorial } from "./historial";
import { sembrarOtrosClientes } from "./otrosClientes";
import { randomUUID, createHash } from "node:crypto";
import { argon2id } from "hash-wasm";
import { ed25519, x25519 } from "@noble/curves/ed25519.js";
import { aB64, utf8 } from "../lib/cripto/bytes";
import { ARGON2, etiquetaEquipo, kCfg, verificador } from "../lib/cripto/claves";
import type * as T from "../lib/tipos";

export const argon2 = async (clave: Uint8Array, sal: Uint8Array) =>
  (await argon2id({
    password: clave,
    salt: sal,
    parallelism: ARGON2.hilos,
    iterations: ARGON2.pasadas,
    memorySize: ARGON2.memoriaKiB,
    hashLength: ARGON2.salida,
    outputType: "binary",
  })) as Uint8Array;

/** Claves de prueba del simulador (se muestran en la pantalla de entrar en modo simulado). */
export const DEMO = {
  correo: "ana@ejemplo.com",
  contrasena: "resguardo",
  claveAdmin: "caballo correcto batería grapa",
  contrasenaRepo: "documentos-2026",
  codigoArranque: "MOCK-1234",
};

export interface CuentaMock {
  id: string;
  correo: string;
  nombre: string;
  contrasena: string;
  superusuario: boolean;
  totp: boolean;
  recuperacion: string[];
  /** v1.27: un propietario le restableció la verificación en dos pasos (código de un solo uso). */
  restablecida?: { codigo: string; caduca: number; cuando: string; por: string; sesion?: string };
}

export interface EquipoMock extends T.Equipo {
  cliente: string;
  secretaBox: Uint8Array;
  secretaFirma: Uint8Array;
  verificador: Uint8Array | null;
  kcfg: Uint8Array | null;
  /** Contraseñas de sus repositorios (las conoce el equipo, nunca el servidor de verdad). */
  contrasenas: Record<string, string>;
  ultimoSeqAceptado: number;
  intentosFallidos: number;
  /** Código de emparejamiento con el que se unió (para comprobar prueba_codigo en el alta). */
  codigoEmparejamiento: string | null;
  informes: T.Informe[];
  config: T.ConfigCifrada | null;
}

export interface OrdenMock extends T.Orden {
  cliente: string;
  equipo: string;
  sellado: string;
  sesion: string | null;
  relevo: { id: string; max_bytes: number } | null;
}

export interface SesionMock {
  id: string;
  cliente: string;
  equipo: string;
  mensajes: T.MensajeSesion[];
  claveSesion: Uint8Array | null;
  tipo: string;
  repo: string | null;
  recibidosConsola: number;
  creada: number;
  ultimo: number;
}

export interface RelevoMock extends T.Relevo {
  id: string;
  cliente: string;
  trozosDatos: Uint8Array[];
  maxBytes: number;
}

export interface EmparejamientoMock {
  id: string;
  cliente: string;
  codigo: string;
  caduca: string;
  estado: T.EstadoEmparejamiento;
  creado: number;
  equipo?: EquipoMock;
  /** Preparado (v1.17): instalador listo o línea de Linux. */
  nombre?: string;
  so?: "windows" | "linux";
  /** La cuenta que lo pidió (v1.42: se le vuelve a dar el suyo si aún sirve). */
  por?: string;
  /** v1.48: el código lo generó el navegador; aquí solo su hash (y `codigo` vacío). */
  codigo_hash?: string;
}

export interface Estado {
  servidor: { identidad: string; secreta: Uint8Array; inicializado: boolean };
  cuentas: CuentaMock[];
  sesiones: Map<string, { cuenta: string; completa: boolean }>;
  clientes: (T.Cliente & { avisosAbiertos?: number })[];
  miembros: Map<string, { cuenta: string; rol: T.Rol }[]>;
  invitaciones: Map<string, { cliente: string; rol: T.Rol; caduca: number }>;
  equipos: EquipoMock[];
  ordenes: OrdenMock[];
  avisos: (T.Aviso & { cliente: string; abierto: boolean })[];
  auditoria: Map<string, T.EntradaAuditoria[]>;
  emparejamientos: EmparejamientoMock[];
  sesionesInteractivas: Map<string, SesionMock>;
  relevos: Map<string, RelevoMock>;
  /** F6: paquete cifrado de cada cliente (solo bytes), su auditoría importada y fichas (solo el hash, como el servidor). */
  paquetes: Map<string, Uint8Array>;
  auditoriaImportada: Map<string, T.EntradaAuditoria[]>;
  fichas: { hash: string; cliente: string; usos: number; caduca: string }[];
  /** Plan 0.8.0, E2: lo que anotan los equipos de cada restauración, los motivos de esta consola y el ajuste del cliente. */
  restauraciones: (T.Restauracion & { cliente: string })[];
  notasRest: (T.NotaRestauracion & { cliente: string })[];
  ajustesRest: Map<string, boolean>;
  restSembradas: Set<string>;
}

export let estado: Estado;

const hace = (min: number) => new Date(Date.now() - min * 60_000).toISOString();

/**
 * 14i: las últimas vueltas de un espejo simulado, de la más antigua a la más reciente
 * (la última, hace `ultimaMin` minutos; una cada `cadaMin`). `fallos`: cuáles fallan
 * (contando desde la más reciente, 0); `borrar`: en cuáles quita algo su retención.
 */
function subidasEspejo(n: number, ultimaMin: number, cadaMin: number, o: { bytes: number; ocupa: number; fallos?: number[]; borrar?: number[]; error?: string; motivo?: string; verificar?: number }): T.SubidaEspejo[] {
  const out: T.SubidaEspejo[] = [];
  let ocupa = o.ocupa;
  for (let i = 0; i < n; i++) {
    const fin = ultimaMin + i * cadaMin;
    const mal = o.fallos?.includes(i) ?? false;
    const vaiven = 0.55 + 0.9 * Math.abs(Math.sin(i * 1.7));
    const bytes = mal ? 0 : Math.round(o.bytes * vaiven);
    const dur = mal ? 40 + i : Math.round(120 + (bytes / o.bytes) * 900 + (i % 3) * 60);
    const borra = !mal && (o.borrar?.includes(i) ?? false);
    out.push({
      inicio: hace(fin + dur / 60),
      fin: hace(fin),
      resultado: mal ? "error" : "ok",
      motivo: o.motivo ?? (i % 5 === 0 ? "tras_copia" : "horario"),
      archivos: mal ? 0 : Math.round(bytes / 9_000_000) + 3,
      bytes,
      borrados: borra ? 40 + i * 3 : 0,
      borrados_bytes: borra ? 600_000_000 + i * 10_000_000 : 0,
      verificados: !mal && o.verificar && i % 4 === 0 ? o.verificar : 0,
      mal: 0,
      ocupa: mal ? null : ocupa,
      error: mal ? (o.error ?? "No se pudo escribir en el destino: el disco no está conectado.") : null,
    });
    if (!mal) ocupa -= bytes - (borra ? 600_000_000 + i * 10_000_000 : 0);
  }
  return out.reverse();
}
/** La próxima vez que el reloj marque esa hora en punto (hoy o mañana). */
const proximaA = (h: number) => {
  const d = new Date();
  d.setHours(h, 0, 0, 0);
  if (d.getTime() < Date.now()) d.setDate(d.getDate() + 1);
  return d.toISOString();
};
const dentro = (min: number) => new Date(Date.now() + min * 60_000).toISOString();
const sal = () => aB64(crypto.getRandomValues(new Uint8Array(16)));

/** Huella de una entrada, como el servidor real (sqlite.rs, hash_entrada). */
const huella = (prev: string, n: number, creado: string, actor: string, accion: string, objetivo: string, datos: string) =>
  createHash("sha256").update(`${prev}|${n}|${Math.floor(Date.parse(creado) / 1000)}|${actor}|${accion}|${objetivo}|${datos}`).digest("hex");

/** Añade una entrada encadenada a la auditoría del cliente. */
export function auditar(cliente: string, actor: string | null, accion: string, objetivo: string | null, datos: unknown = null, creado = new Date().toISOString()) {
  const lista = estado.auditoria.get(cliente) ?? [];
  const prev = lista.at(-1);
  const n = (prev?.n ?? 0) + 1;
  const prev_hash = prev?.hash ?? "0".repeat(64);
  // Como el servidor real: actor «cuenta:correo» (o «servidor») y datos en texto JSON.
  const correo = actor ? (estado.cuentas.find((c) => c.id === actor)?.correo ?? actor) : null;
  const e = { n, creado: new Date(Math.floor(Date.parse(creado) / 1000) * 1000).toISOString(), actor: correo ? `cuenta:${correo}` : "servidor", accion, objetivo: objetivo ?? "", datos: JSON.stringify(datos ?? {}), prev_hash, hash: "" };
  e.hash = huella(prev_hash, n, e.creado, e.actor, accion, e.objetivo, e.datos);
  lista.push(e);
  estado.auditoria.set(cliente, lista);
}

export function verificarCadena(cliente: string): T.VerificacionAuditoria {
  const lista = estado.auditoria.get(cliente) ?? [];
  let prev = "0".repeat(64);
  for (const e of lista) {
    if (e.prev_hash !== prev || e.hash !== huella(prev, e.n, e.creado, e.actor, e.accion, e.objetivo, e.datos)) return { ok: false, rota_en: e.n };
    prev = e.hash;
  }
  return { ok: true, entradas: lista.length };
}

/** Informes de 20 días con alguna copia con aviso o fallida. */
function informesDe(copias: T.CopiaResumen[], semilla: number): T.Informe[] {
  const out: T.Informe[] = [];
  for (let d = 0; d < 20; d++) {
    const cuando = new Date(Date.now() - d * 24 * 3600_000 - 3 * 3600_000).toISOString();
    out.push({
      recibido: cuando,
      datos: {
        copias: copias.map((c, i) => {
          const r = (d * 7 + i * 3 + semilla) % 17;
          const estadoC = r === 0 ? "fallo" : r === 5 ? "aviso" : "ok";
          return {
            id: c.id,
            repo: c.repo,
            nombre: c.nombre,
            estado: estadoC,
            cuando,
            bytes: estadoC === "fallo" ? 0 : 40_000_000 + ((d * 13 + i * 31 + semilla) % 50) * 9_000_000,
            archivos: 1200 + ((d * 17 + i) % 90) * 37,
            duracion_s: 60 + ((d * 11 + i * 5) % 40) * 9,
            mensaje: estadoC === "fallo" ? "No se pudo conectar con el servidor de copias" : estadoC === "aviso" ? "3 archivos estaban en uso y se omitieron" : null,
          };
        }),
        disco: { libre: 182_000_000_000 - semilla * 9_000_000_000, total: 512_000_000_000 },
      },
    });
  }
  return out;
}

async function crearEquipo(
  cliente: { id: string; sal_cliente: string },
  datos: Partial<T.Equipo> & { nombre: string; so: string },
  opciones: { claveAdmin?: string; kcfg?: Uint8Array; contrasenas?: Record<string, string>; semilla?: number; id?: string } = {},
): Promise<EquipoMock> {
  const secretaBox = x25519.utils.randomSecretKey();
  const secretaFirma = ed25519.utils.randomSecretKey();
  const id = opciones.id ?? randomUUID();
  const sal_equipo = sal();
  const box_pub = aB64(x25519.getPublicKey(secretaBox));
  const sign_pub = aB64(ed25519.getPublicKey(secretaFirma));
  let ver: Uint8Array | null = null;
  if (opciones.claveAdmin) ver = verificador(await argon2(utf8(opciones.claveAdmin), Uint8Array.from(Buffer.from(sal_equipo, "base64"))));
  const e: EquipoMock = {
    id,
    cliente: cliente.id,
    version_agente: "0.7.0",
    rol: "agente",
    modo: "gestionado",
    confirmado: true,
    conectado: true,
    ultimo_contacto: hace(1),
    estado_servicio: "en_marcha",
    siguiente_seq: 1,
    resumen: null,
    ...datos,
    box_pub,
    sign_pub,
    sal_equipo,
    etiqueta: opciones.kcfg ? etiquetaEquipo(opciones.kcfg, id, box_pub, sign_pub) : null,
    secretaBox,
    secretaFirma,
    verificador: ver,
    kcfg: opciones.kcfg ?? null,
    contrasenas: opciones.contrasenas ?? {},
    ultimoSeqAceptado: 0,
    intentosFallidos: 0,
    codigoEmparejamiento: null,
    informes: [],
    config: null,
  };
  e.informes = informesDe(e.resumen?.copias ?? [], opciones.semilla ?? 1);
  e.siguiente_seq = e.ultimoSeqAceptado + 1;
  return e;
}

const L_V = [1, 2, 3, 4, 5];

/** Ids fijos del simulador (para enlaces y capturas estables). */
export const ID = {
  altamar: "0a0e1b2c-0000-4000-8000-0000000000a1",
  sur: "0a0e1b2c-0000-4000-8000-0000000000a2",
  propio: "0a0e1b2c-0000-4000-8000-0000000000a3",
  recepcion: "0a0e1b2c-0000-4000-8000-0000000000e1",
  contabilidad: "0a0e1b2c-0000-4000-8000-0000000000e2",
  portatil: "0a0e1b2c-0000-4000-8000-0000000000e3",
  servidor: "0a0e1b2c-0000-4000-8000-0000000000e4",
  almacen: "0a0e1b2c-0000-4000-8000-0000000000e5",
  caja: "0a0e1b2c-0000-4000-8000-0000000000e6",
  estudio: "0a0e1b2c-0000-4000-8000-0000000000e7",
  archivos: "0a0e1b2c-0000-4000-8000-0000000000e8",
  apps: "0a0e1b2c-0000-4000-8000-0000000000e9",
};

export async function sembrar(vacio = false) {
  const secreta = ed25519.utils.randomSecretKey();
  estado = {
    servidor: { identidad: aB64(ed25519.getPublicKey(secreta)), secreta, inicializado: !vacio },
    cuentas: [],
    sesiones: new Map(),
    clientes: [],
    miembros: new Map(),
    invitaciones: new Map(),
    equipos: [],
    ordenes: [],
    avisos: [],
    auditoria: new Map(),
    emparejamientos: [],
    sesionesInteractivas: new Map(),
    relevos: new Map(),
    paquetes: new Map(),
    auditoriaImportada: new Map(),
    fichas: [],
    restauraciones: [],
    notasRest: [],
    ajustesRest: new Map(),
    restSembradas: new Set(),
  };
  if (vacio) return;

  const ana: CuentaMock = { id: randomUUID(), correo: DEMO.correo, nombre: "Ana Restrepo", contrasena: DEMO.contrasena, superusuario: true, totp: true, recuperacion: ["k7m2-q9x4"] };
  const luis: CuentaMock = { id: randomUUID(), correo: "luis@altamar.ejemplo.com", nombre: "Luis Gómez", contrasena: "x", superusuario: false, totp: true, recuperacion: [] };
  const marta: CuentaMock = { id: randomUUID(), correo: "marta@cafedelsur.ejemplo.com", nombre: "Marta Ruiz", contrasena: "x", superusuario: false, totp: true, recuperacion: [] };
  estado.cuentas.push(ana, luis, marta);

  const altamar = { id: ID.altamar, nombre: "Ferretería Altamar", sal_cliente: sal(), espera_min_horas: 24, rol: "propietario" as T.Rol };
  const sur = { id: ID.sur, nombre: "Café del Sur", sal_cliente: sal(), espera_min_horas: 12, rol: "administrador" as T.Rol };
  const propio = { id: ID.propio, nombre: "Propio", sal_cliente: sal(), espera_min_horas: 1, rol: "propietario" as T.Rol };
  estado.clientes.push(altamar, sur, propio);
  estado.miembros.set(altamar.id, [
    { cuenta: ana.id, rol: "propietario" },
    { cuenta: luis.id, rol: "tecnico" },
  ]);
  estado.miembros.set(sur.id, [
    { cuenta: marta.id, rol: "propietario" },
    { cuenta: ana.id, rol: "administrador" },
  ]);
  estado.miembros.set(propio.id, [{ cuenta: ana.id, rol: "propietario" }]);

  const kcfgDe = async (c: { sal_cliente: string }) => kCfg(await argon2(utf8(DEMO.claveAdmin), Uint8Array.from(Buffer.from(c.sal_cliente, "base64"))));
  const [kAltamar, kSur, kPropio] = await Promise.all([kcfgDe(altamar), kcfgDe(sur), kcfgDe(propio)]);

  const destinoServidor: T.DestinoResumen = { id: "servidor-altamar", nombre: "Servidor de la oficina", tipo: "rest", donde: "192.168.1.20:8000", equipo_almacen: ID.servidor };
  const destinoNube: T.DestinoResumen = { id: "b2-altamar", nombre: "Backblaze B2", tipo: "b2", donde: "altamar-copias", inmutable: true };
  const repo = (id: string, nombre: string, destino: string, versiones: number, bytes: number): T.RepositorioResumen => ({
    id,
    nombre,
    destino,
    versiones,
    bytes,
    ultima_version: hace(180),
    verificado: hace(60 * 30),
    prueba_restauracion: hace(60 * 24 * 9),
    retencion: "7 diarias, 4 semanales, 12 mensuales",
  });
  const copia = (id: string, nombre: string, repoId: string, horas: string[], carpetas: number, ok = true): T.CopiaResumen => ({
    id,
    nombre,
    repo: repoId,
    horario: { dias: L_V, horas },
    carpetas,
    activa: true,
    ultima: { cuando: hace(180), estado: ok ? "ok" : "fallo", bytes: 182_000_000, mensaje: ok ? null : "No se pudo conectar con el servidor de copias" },
    proxima: dentro(95),
  });

  // 14uu (plan 0.8.0, D3): los datos básicos que manda un agente nuevo (inventados).
  const G = 1_000_000_000;
  const win = (nombre: string, variante: string, version: string | null, compilacion: string): SistemaEquipo => ({ familia: "windows", nombre, variante, version, compilacion });
  const disco = (punto: string, totalG: number, libreG: number, fs: string, en_uso = false) => ({ punto, total: totalG * G, libre: libreG * G, sistema_archivos: fs, en_uso });
  const basicos = (sistema: SistemaEquipo, procesador: string, nucleos: number, memoriaG: number, fabricante: string | null, modelo: string | null, arrancoHaceMin: number, discos: ReturnType<typeof disco>[]) => ({
    leido: hace(60 * 3),
    sistema,
    procesador: { nombre: procesador, nucleos },
    memoria: memoriaG * 1024 ** 3,
    fabricante,
    modelo,
    arranque: hace(arrancoHaceMin),
    discos,
  });

  const servidorAltamar = await crearEquipo(
    altamar,
    {
      nombre: "SERVIDOR-ALTAMAR",
      so: "Debian 12 (CT de Proxmox)",
      rol: "almacenamiento",
      resumen: {
        guarda_copias: {
          activo: true,
          puerto: 8000,
          solo_red_local: true,
          usuarios: 3,
          carpeta: "/srv/resguardo/copias",
          sistema_archivos: "zfs",
          espacio: { libre: 640_000_000_000, total: 4_000_000_000_000, leido: hace(5) },
          // Una Dropbox conectada solo aquí, para el espejo de todo lo que guarda (los equipos no la tienen).
          nubes: [{ nombre: "Dropbox Altamar", tipo: "dropbox" }],
          espejo: { hora: "02:00", ultima: hace(60 * 14), destinos: [{ tipo: "nube", nube: "Dropbox Altamar", carpeta: "Altamar", ultima: hace(60 * 14), resultado: "Espejo hecho: 96 archivos nuevos (2 GB)." }] },
        },
        admite: ["espejo_zonas", "espejo_flexible"],
        // Tarea 8e: un contenedor de Proxmox (solo un dato en la consola).
        entorno: { contenedor: "lxc" },
        datos_basicos: basicos(
          { familia: "linux", nombre: "Debian GNU/Linux 12 (bookworm)", id: "debian", version: "12", kernel: "6.8.12-4-pve" },
          "Intel(R) Xeon(R) E-2236 CPU @ 3.40GHz",
          4,
          8,
          "QEMU",
          "Standard PC (i440FX + PIIX, 1996)",
          60 * 24 * 41,
          [disco("/", 32, 21, "ext4"), disco("/srv/resguardo/copias", 4000, 640, "zfs", true)],
        ),
        destinos: [destinoNube],
        repositorios: [repo("copia-externa", "Copia externa de la oficina", "b2-altamar", 210, 412_000_000_000)],
        copias: [copia("subida-nube", "Subida a la nube", "copia-externa", ["23:00"], 1)],
      },
    },
    { claveAdmin: DEMO.claveAdmin, kcfg: kAltamar, contrasenas: { "copia-externa": DEMO.contrasenaRepo }, semilla: 3, id: ID.servidor },
  );
  destinoServidor.equipo_almacen = servidorAltamar.id;
  const recepcion = await crearEquipo(
    altamar,
    {
      nombre: "RECEPCION",
      // v1.24: agente con horarios por reglas (cada N minutos, cada N días, cada mes).
      version_agente: "0.7.14",
      so: "Windows 11 Pro",
      resumen: {
        servidores_respaldo: [{ url: "https://respaldo.ejemplo.co", identidad_corta: "7Q2kLm9x" }],
        datos_basicos: basicos(win("Windows 11 Pro", "Professional", "24H2", "26100.2033"), "Intel(R) Core(TM) i5-12400", 12, 16, "Dell Inc.", "OptiPlex 7000", 60 * 26, [disco("C:", 512, 301, "NTFS", true)]),
        respaldo_dias: 3,
        // Con su usuario en el almacén (así el almacén puede hacer pasos «espejo» de su repositorio).
        destinos: [{ ...destinoServidor, donde: "https://192.168.1.20:8000/recepcion/" }, destinoNube],
        // Tarea 8: cumple la regla 3-2-1-1-0 (almacén de solo añadir y copia externa a B2 con bloqueo, verificada y probada).
        admite: ["prueba_auto", "cadenas", "derivadas", "restaurar_carpeta", "restaurar_desde", "restauracion_real", "prueba_espejo"],
        repositorios: [{ ...repo("docs-recepcion", "Documentos de recepción", "servidor-altamar", 148, 38_400_000_000), externa: { destino: "Backblaze B2", destino_id: "b2-altamar", hora: "23:00", bloqueo_dias: 30 }, prueba_auto: { cada_dias: 30, proxima: dentro(60 * 24 * 21) } }],
        copias: [copia("documentos", "Documentos", "docs-recepcion", ["13:00", "19:00"], 3)],
      },
    },
    { claveAdmin: DEMO.claveAdmin, kcfg: kAltamar, contrasenas: { "docs-recepcion": DEMO.contrasenaRepo }, semilla: 1, id: ID.recepcion },
  );
  const contabilidad = await crearEquipo(
    altamar,
    {
      nombre: "CONTABILIDAD",
      version_agente: "0.7.7",
      so: "Windows 10 Pro",
      ultimo_contacto: hace(4),
      resumen: {
        // Windows 10 sin soporte, el disco C: casi lleno y un reinicio justo antes de la copia fallida.
        datos_basicos: basicos(win("Windows 10 Pro", "Professional", "22H2", "19045.5011"), "Intel(R) Core(TM) i3-7100 CPU @ 3.90GHz", 4, 8, "HP", "HP ProDesk 400 G4 SFF", 205, [disco("C:", 240, 7, "NTFS", true), disco("D:", 1000, 612, "NTFS")]),
        destinos: [destinoServidor],
        repositorios: [repo("ventas", "Ventas y documentos", "servidor-altamar", 96, 91_200_000_000)],
        copias: [copia("ventas", "Ventas", "ventas", ["12:30", "18:30"], 2, false)],
      },
    },
    { claveAdmin: DEMO.claveAdmin, kcfg: kAltamar, contrasenas: { ventas: DEMO.contrasenaRepo }, semilla: 0, id: ID.contabilidad },
  );
  const portatil = await crearEquipo(
    altamar,
    {
      nombre: "PORTATIL-GERENCIA",
      so: "Windows 11 Home",
      conectado: false,
      ultimo_contacto: hace(60 * 52),
      resumen: {
        destinos: [destinoNube],
        repositorios: [repo("gerencia", "Gerencia", "b2-altamar", 61, 12_900_000_000)],
        copias: [{ ...copia("gerencia", "Escritorio y documentos", "gerencia", ["12:00"], 2), ultima: { cuando: hace(60 * 54), estado: "ok", bytes: 22_000_000 }, proxima: hace(60 * 30) }],
      },
    },
    { claveAdmin: DEMO.claveAdmin, kcfg: kAltamar, contrasenas: { gerencia: DEMO.contrasenaRepo }, semilla: 6, id: ID.portatil },
  );
  // v1.41: «copias en el mismo equipo». El servidor de archivos se copia en una
  // carpeta de su propio disco D: (se eligió «Carpeta de este equipo» al crear
  // el repositorio), aunque la oficina tiene un almacén en otro equipo.
  const archivos = await crearEquipo(
    altamar,
    {
      nombre: "SERVIDOR-ARCHIVOS",
      version_agente: "0.7.18",
      so: "Windows Server 2016",
      resumen: {
        admite: ["retencion_plazos", "verificacion_auto", "almacen_propio", "consolas_multiples", "escritorio", "verificacion_horario", "retencion_almacen_horario", "externa_existente", "espejo_equipo"],
        datos_basicos: basicos(win("Windows Server 2016 Standard", "ServerStandard", "1607", "14393.7428"), "Intel(R) Xeon(R) Silver 4310 CPU @ 2.10GHz", 24, 64, "HPE", "ProLiant DL380 Gen10 Plus", 60 * 24 * 12, [
          disco("C:", 240, 88, "NTFS", true),
          disco("D:", 4000, 1480, "ReFS", true),
          disco("E:", 2000, 1900, "NTFS"),
        ]),
        destinos: [{ id: "carpeta-d", nombre: "Copias en el disco D", tipo: "local", donde: undefined, unidad: "D:", extraible: false, red: false }],
        // Plan 0.7.26 (bloque 4): el propio equipo hace el espejo de su repositorio del disco D a un USB.
        espejo_equipo: {
          trabajos: [
            { id: "e9f8a7b6c5d", nombre: "USB de la oficina", activo: true, quien: "equipo", que: { tipo: "todos" }, adonde: { tipo: "carpeta", carpeta: "F:\\Espejo-compartidas" },
              cuando: { tras_copia: true, retraso_min: 30 }, retencion: { modo: "nunca" }, freno: { pct: 10, min_archivos: 100, min_faltan: 20, accion: "confirmar" }, orden: 0,
              ultima: hace(60 * 5), resultado: "Espejo «USB de la oficina» hecho en F:\\Espejo-compartidas (después de una copia nueva): compartido: 210 archivos nuevos (1 GB)." },
          ],
        },
        repositorios: [{ ...repo("compartido", "Carpetas compartidas", "carpeta-d", 84, 142_000_000_000), retencion_regla: { diarias: 7, semanales: 4, mensuales: 12, anuales: 2 } }],
        copias: [copia("compartidas", "Carpetas compartidas", "compartido", ["12:00", "20:00"], 3)],
      },
    },
    { claveAdmin: DEMO.claveAdmin, kcfg: kAltamar, contrasenas: { compartido: DEMO.contrasenaRepo }, semilla: 7, id: ID.archivos },
  );
  const surAlmacen = await crearEquipo(
    sur,
    {
      nombre: "ALMACEN-SUR",
      version_agente: "0.7.7",
      so: "Windows 11 Pro",
      rol: "almacenamiento",
      // v1.28: agente nuevo (plazos, verificación automática y su propio almacén).
      resumen: { admite: ["retencion_plazos", "verificacion_auto", "almacen_propio", "consolas_multiples", "verificacion_horario", "retencion_almacen_horario", "espejo_flexible", "espejo_destinos", "zonas_almacen", "espejo_zonas", "espejo_trabajos", "espejo_equipo", "nombres_legibles", "espejo_primera"],
        // Tarea 2: se añadió aquí y no está en la consola en línea, como CAJA-1 («N equipos no están en todas tus consolas»).
        consolas: [{ id: "principal", nombre: "cafedelsur.ejemplo.com", url: "https://cafedelsur.ejemplo.com:8443", identidad: "ZXN0YS1jb25zb2xhLXNpbXVsYWRhLTAwMDAwMDAwMDA=", sal_cliente: null, ultimo_contacto: hace(5), desde: null, esta: true }],
        entorno: { virtual: "hyperv" },
        datos_basicos: basicos(win("Windows 11 Pro", "Professional", "25H2", "26200.6899"), "AMD Ryzen 5 5600G with Radeon Graphics", 12, 32, "Microsoft Corporation", "Virtual Machine", 60 * 24 * 3, [
          disco("C:", 128, 41, "NTFS"),
          disco("D:", 500, 3.4, "NTFS", true),
          disco("F:", 2000, 1800, "ReFS", true),
        ]),
        guarda_copias: { activo: true, puerto: 8000, solo_red_local: false, usuarios: 5, carpeta: "D:\\Resguardo\\Copias", sistema_archivos: "NTFS",
          // v1.31: el espacio de su disco y el de cada destino del espejo («¿Cuándo se llena?»).
          espacio: { libre: 3_400_000_000, total: 500_000_000_000, leido: hace(2) }, repositorios: [{ usuario: "5d1e7c3a-2b4f-4a6e-9c8d-1f2e3a4b", repos: ["caja", "almacen-sur-7c2a"] }],
          // Tarea 7b: otra zona (otro disco), con su puerto.
          zonas: [{ id: "z3c4d5e", nombre: "Disco F", carpeta: "F:\\Resguardo", puerto: 8002, usuarios: 0, escucha: true, sistema_archivos: "ReFS", espacio: { libre: 1_800_000_000_000, total: 2_000_000_000_000, leido: hace(2) }, repositorios: [] }],
          espejo: {
            hora: "02:00",
            ultima: hace(60 * 13),
            resultado: "Espejo hecho: 1.204 archivos nuevos (38 GB) en 2 destinos.",
            limite_kib: 4096,
            destinos: [
              { trabajo: "t0a1b2c3d4e", tipo: "carpeta", carpeta: "E:\\Resguardo-espejo", horario: { dias: [1, 2, 3, 4, 5], horas: [], reglas: [{ tipo: "intervalo", dias: [1, 2, 3, 4, 5], cada_min: 60, desde: "08:00", hasta: "19:00" }] }, tras_copia: true, retencion_dias: 30, por_borrar: { archivos: 214, bytes: 3_100_000_000, primero: "2026-11-02" }, freno: "falta de golpe en el almacén el 35 % de lo que hay en el espejo (812 archivos): no se borra nada del espejo. Si fue a propósito (una poda grande o un repositorio quitado), confírmalo en la consola; si no, revisa el almacén", verificar_pct: 5, verificacion: { ultima: hace(60 * 13), archivos: 61, mal: 0 }, ultima: hace(60 * 13),
 resultado: "Espejo hecho: 1.204 archivos nuevos (38 GB).", espacio: { libre: 1_310_000_000_000, total: 2_000_000_000_000, leido: hace(2) } },
              { trabajo: "t1b2c3d4e5f", tipo: "nube", nube: "Dropbox Oficina", carpeta: "CafeDelSur", repos: ["5d1e7c3a-2b4f-4a6e-9c8d-1f2e3a4b/almacen-sur-7c2a"], vistos: ["5d1e7c3a-2b4f-4a6e-9c8d-1f2e3a4b/almacen-sur-7c2a"], ultima: hace(60 * 13), resultado: "ERROR: Dropbox respondió 429 (demasiadas peticiones); se reintenta mañana.", espacio: { libre: 520_000_000, total: 2_199_023_255_552, leido: hace(60 * 13) } },
              // Tarea 7d.2: un paso «espejo» de la cadena de «Caja»: a la zona F, después de cada copia, sin retención.
              { trabajo: "t2c3d4e5f60", tipo: "zona", carpeta: "z3c4d5e", repos: ["5d1e7c3a-2b4f-4a6e-9c8d-1f2e3a4b/caja"], vistos: ["5d1e7c3a-2b4f-4a6e-9c8d-1f2e3a4b/caja"], tras_copia: true, ultima: hace(60 * 3), resultado: "Espejo hecho en zona z3c4d5e: 12 archivos nuevos (4 MB), 3.120 ya estaban, 0 se dejan para la próxima vez.", espacio: { libre: 1_800_000_000_000, total: 2_000_000_000_000, leido: hace(2) } },
            ],
            // Plan 0.7.26 (bloque 4): los mismos espejos como trabajos, más uno a B2 con bloqueo, en pausa.
            trabajos: [
              { id: "t0a1b2c3d4e", nombre: "Disco E cada hora", activo: true, quien: "almacen", que: { tipo: "todos" }, adonde: { tipo: "carpeta", carpeta: "E:\\Resguardo-espejo" },
                cuando: { horario: { dias: [1, 2, 3, 4, 5], horas: [], reglas: [{ tipo: "intervalo", dias: [1, 2, 3, 4, 5], cada_min: 60, desde: "08:00", hasta: "19:00" }] }, tras_copia: true },
                retencion: { modo: "retraso", dias: 30 }, freno: { pct: 10, min_archivos: 100, min_faltan: 20, accion: "confirmar" }, verificar_pct: 5, orden: 0,
                ultima: hace(60 * 13), resultado: "Espejo «Disco E cada hora» hecho en E:\\Resguardo-espejo: 1.204 archivos nuevos (38 GB).", proxima: dentro(47), verificacion: { ultima: hace(60 * 13), archivos: 61, mal: 0 },
                // 14i: su registro de vueltas y lo que ocupa en el destino.
                bien: hace(60 * 13), nunca_bien: false,
                subidas: subidasEspejo(30, 60 * 13, 60, { bytes: 1_400_000_000, ocupa: 312_000_000_000, fallos: [7, 8], borrar: [3, 15], verificar: 61, error: "No se pudo escribir en el destino: el disco E no está conectado." }),
                ocupa: { bytes: 312_000_000_000, repos: { "5d1e7c3a-2b4f-4a6e-9c8d-1f2e3a4b/caja": 41_200_000_000, "5d1e7c3a-2b4f-4a6e-9c8d-1f2e3a4b/almacen-sur-7c2a": 270_800_000_000 }, cuando: hace(60 * 13) },
                por_borrar: { archivos: 214, bytes: 3_100_000_000, primero: "2026-11-02" },
                freno_aviso: "falta de golpe en el almacén el 35 % de lo que hay en el espejo (812 archivos): no se borra nada del espejo. Si fue a propósito, confírmalo en la consola; si no, revisa el almacén",
                espacio: { libre: 1_310_000_000_000, total: 2_000_000_000_000, leido: hace(2) } },
              { id: "t1b2c3d4e5f", nombre: "Ventas a Dropbox", activo: true, quien: "almacen", que: { tipo: "repos", repos: ["5d1e7c3a-2b4f-4a6e-9c8d-1f2e3a4b/almacen-sur-7c2a"] }, vistos: ["5d1e7c3a-2b4f-4a6e-9c8d-1f2e3a4b/almacen-sur-7c2a"], adonde: { tipo: "nube", nube: "Dropbox Oficina", carpeta: "CafeDelSur" },
                cuando: { cadena: "t0a1b2c3d4e", retraso_min: 15 }, retencion: { modo: "nunca" }, freno: { pct: 10, min_archivos: 100, min_faltan: 20, accion: "confirmar" }, limite_kib: 4096, orden: 1,
                ultima: hace(60 * 13), resultado: "ERROR: espejo «Ventas a Dropbox» en Dropbox Oficina:CafeDelSur: Dropbox respondió 429 (demasiadas peticiones); se reintenta la próxima vez.",
                // 14c: nunca ha terminado bien (la primera subida se cortó): se quita o se cambia sin espera. 14p: ahora la reintenta (progreso.ts).
                nunca_bien: true,
                subidas: subidasEspejo(3, 60 * 13, 60 * 24, { bytes: 0, ocupa: 0, fallos: [0, 1, 2], motivo: "tras_otro", error: "Dropbox respondió 429 (demasiadas peticiones); se reintenta la próxima vez." }) },
              { id: "t2c3d4e5f60", nombre: "Caja a Disco F", activo: true, quien: "almacen", que: { tipo: "repos", repos: ["5d1e7c3a-2b4f-4a6e-9c8d-1f2e3a4b/caja"] }, vistos: ["5d1e7c3a-2b4f-4a6e-9c8d-1f2e3a4b/caja"], adonde: { tipo: "zona", carpeta: "z3c4d5e" },
                cuando: { tras_copia: true }, retencion: { modo: "igual" }, freno: { pct: 10, min_archivos: 100, min_faltan: 20, accion: "avisar" }, orden: 2,
                ultima: hace(60 * 3), resultado: "Espejo «Caja a Disco F» hecho en zona z3c4d5e (después de una copia nueva): 12 archivos nuevos (4 MB), 3.120 ya estaban.", espacio: { libre: 1_800_000_000_000, total: 2_000_000_000_000, leido: hace(2) },
                bien: hace(60 * 3), nunca_bien: false,
                subidas: subidasEspejo(12, 60 * 3, 60 * 8, { bytes: 60_000_000, ocupa: 41_200_000_000, motivo: "tras_copia", borrar: [2, 9] }),
                ocupa: { bytes: 41_200_000_000, repos: { "5d1e7c3a-2b4f-4a6e-9c8d-1f2e3a4b/caja": 41_200_000_000 }, cuando: hace(60 * 3) } },
              { id: "t3d4e5f6071", nombre: "Todo a B2 (bloqueo 30 días)", activo: false, quien: "almacen", que: { tipo: "todos" }, adonde: { tipo: "nube", nube: "B2 Café del Sur", carpeta: "cafe-espejo/sur" },
                cuando: { despues: "t0a1b2c3d4e", retraso_min: 60 }, retencion: { modo: "retraso", dias: 45 }, bloqueo_dias: 30, freno: { pct: 10, min_archivos: 100, min_faltan: 20, accion: "confirmar" }, orden: 3, nunca_bien: true },
            ],
          },
          nubes: [{ nombre: "Dropbox Oficina", tipo: "dropbox" }, { nombre: "B2 Café del Sur", tipo: "b2" }],
          // v1.22: la retención de «Caja» la aplica el almacén, los domingos a las 03:00.
          retenciones: [
            {
              usuario: "5d1e7c3a-2b4f-4a6e-9c8d-1f2e3a4b",
              repo: "caja",
              retencion: { diarias: 7, semanales: 4, mensuales: 12, anuales: 2 },
              texto: "7 diarias · 4 semanales · 12 mensuales · 2 anuales",
              horario: { dias: [7], hora: "03:00" },
              horario_texto: "los domingos a las 03:00",
              verificar: true,
              clave: "ok",
              ultima: hace(60 * 24 * 2),
              resultado: "ok",
              mensaje: "Retención aplicada: 6 versiones quitadas, quedan 77. Comprobado sin errores.",
              versiones: 77,
              proxima: dentro(60 * 24 * 5),
            },
          ],
        }, repositorios: [], copias: [] },
    },
    { claveAdmin: DEMO.claveAdmin, kcfg: kSur, semilla: 2, id: ID.almacen },
  );
  const surCaja = await crearEquipo(
    sur,
    {
      nombre: "CAJA-1",
      so: "Windows 11 Pro",
      resumen: {
        // v1.28: agente nuevo; «Caja» se verifica sola cada semana (10 %, rotativa).
        // v1.46: «Ventas» (movido al almacén) puede llevar su copia externa al repositorio de la nube de siempre.
        // Tarea 7 (parte B): copias en cadena, derivadas con filtros y nubes en el propio equipo.
        // Plan 0.7.26 (bloque 3): «Después de la anterior» aunque falle y con retraso.
        // 0.7.26 (bloque 8): guarda los datos comunes del cliente (colores, catálogo, plantillas).
        admite: ["retencion_plazos", "verificacion_auto", "almacen_propio", "consolas_multiples", "verificacion_horario", "retencion_almacen_horario", "externa_existente", "ordenes_en_espera", "cadenas", "derivadas", "filtros", "nube_equipo", "repo_en_nube", "datos_equipo", "quitar_destino", "nube_revocar", "inicio_despues", "datos_cliente", "restaurar_carpeta", "restaurar_desde", "restauracion_real", "solo_destino", "nombres_legibles", "comprobar_contrasena", "aprobar_en_equipo", "termina", "comentarios_version", "conservar_version", "retencion_por_dia", "limite_franjas", "sedes", "prueba_espejo"],
        // v1.56: el nombre lo puso la consola en línea (lo guarda el equipo: igual en todas sus consolas).
        datos_equipo: { nombre: { valor: "CAJA-1", cuando: hace(60 * 24), consola: "Consola en línea", esta: false, por: "Bruno" } },
        nubes: [{ nombre: "Dropbox Caja", tipo: "dropbox" }],
        // Plan 0.7.26 (1.1): una nube desconectada cuyo permiso aún no se pudo anular en Dropbox.
        nubes_por_anular: [{ nombre: "Dropbox Antigua", tipo: "dropbox", desde: hace(60 * 3), hasta: dentro(60 * 24 * 7 - 60 * 3), intentos: 2 }],
        // v1.49: una orden destructiva que mandó la consola en línea y el equipo tiene en espera.
        en_espera: [
          { id: "8f6d2c1e-0000-4000-8000-00000000e5e1", tipo: "quitar_repositorio", descripcion: "Quitar el repositorio «Ventas»", consola: { nombre: "Consola en línea", identidad: "b3RyYS1jb25zb2xhLWVuLWxpbmVhLTAwMDAwMDAwMDA=", esta: false }, por: "Bruno", emitida: hace(60 * 2), aplica: dentro(60 * 22), caduca: dentro(60 * 46), claves_comprobadas: true, repositorio: "Ventas", aprobable: true },
        ],
        // v1.36: también la gestiona una consola en línea (y el último cambio vino de allí).
        consolas: [
          { id: "principal", nombre: "cafedelsur.ejemplo.com", url: "https://cafedelsur.ejemplo.com:8443", identidad: "ZXN0YS1jb25zb2xhLXNpbXVsYWRhLTAwMDAwMDAwMDA=", sal_cliente: null, ultimo_contacto: hace(5), desde: null, esta: true },
          { id: "en-linea", nombre: "Consola en línea", url: "https://consola.ejemplo.com", identidad: "b3RyYS1jb25zb2xhLWVuLWxpbmVhLTAwMDAwMDAwMDA=", sal_cliente: "c2FsLWRlbC1jbGllbnRlLTE2", ultimo_contacto: hace(15), desde: hace(60 * 24 * 20), esta: false },
        ],
        cambio_config: { tipo: "config", cuando: hace(60 * 3), consola: { nombre: "Consola en línea", url: "https://consola.ejemplo.com", identidad: "b3RyYS1jb25zb2xhLWVuLWxpbmVhLTAwMDAwMDAwMDA=" } },
        // Con copia externa diaria a un segundo disco (cambiar_copia_externa).
        repositorios: [{ ...repo("caja", "Caja", "almacen-sur", 77, 4_100_000_000), retencion: "7 diarias · 4 semanales · 12 mensuales · 2 anuales", retencion_regla: { diarias: 7, semanales: 4, mensuales: 12, anuales: 2 }, solo_anadir: true, externa: { destino: "Disco 2", destino_id: "disco-2", hora: "21:00" },
            // 14bbb: la contraseña del kit se comprobó hace unos 3 meses.
            contrasena_comprobada: { bien: hace(60 * 24 * 95), ultima: hace(60 * 24 * 95), resultado: "abre" },
            verificacion_auto: { cada_dias: 7, porcentaje: 10, proxima: dentro(60 * 24 * 3), todo_leido: hace(60 * 24 * 40) },
            // Tarea 4b: una copia derivada a la Dropbox conectada en el propio equipo, después de cada copia, solo las diarias.
            derivadas: [{ id: "d1", destino: "Dropbox Caja", destino_id: "nube-dropbox-caja", cuando: { tras_copia: true }, filtro: { etiquetas: ["diaria"], ultimos_dias: 90 }, activa: true, con_retencion: true }] },
          // También en el almacén, todavía sin retención (v1.22: «Retención en el almacén…»).
          { ...repo("ventas", "Ventas", "almacen-sur", 52, 9_800_000_000), solo_anadir: true, ruta: "almacen-sur-7c2a" },
          // 14mm: el mismo almacén dado de alta como «Servidor de copias» genérico (un repositorio adoptado): el mapa lo une.
          { ...repo("historico", "Histórico", "servidor-copias", 12, 1_300_000_000), ruta: "caja-1/historico", solo_anadir: true }],
        // Tarea 7c: «Ventas» va después de «Caja y facturas».
        copias: [copia("caja", "Caja y facturas", "caja", ["14:00"], 1), { ...copia("ventas", "Ventas", "ventas", ["14:00"], 1), tras: "caja" }],
        destinos: [
          { id: "almacen-sur", nombre: "Almacén Sur", tipo: "rest", donde: "https://cafedelsur.ejemplo.com:8000/5d1e7c3a-2b4f-4a6e-9c8d-1f2e3a4b/", equipo_almacen: ID.almacen },
          // 0.7.26: dos discos USB que se rotan (marcado «Aislado» en el catálogo): el agente dice cuándo vio cada uno.
          {
            id: "disco-2",
            nombre: "Disco 2",
            tipo: "local",
            donde: "E:\\Resguardo-externa",
            unidad: "E:",
            extraible: true,
            red: false,
            aislado: { ultima_conexion: hace(60 * 24 * 34), volumenes: [{ id: "9f3a1c2b7d4e5a60", visto: hace(60 * 24 * 34) }, { id: "1b2c3d4e5f607182", visto: hace(60 * 24 * 41) }] },
          },
          { id: "nube-dropbox-caja", nombre: "Dropbox Caja", tipo: "nube", nube: "Dropbox Caja", donde: "Resguardo" },
          { id: "servidor-copias", nombre: "Servidor de copias", tipo: "rest", donde: "https://cafedelsur.ejemplo.com:8000" },
          // v1.56: un destino que se quedó sin repositorios (se quitaron): «Quitar este destino».
          { id: "disco-viejo", nombre: "Disco o carpeta del equipo", tipo: "local", unidad: "C:", extraible: false, red: false },
        ],
      },
    },
    { claveAdmin: DEMO.claveAdmin, kcfg: kSur, contrasenas: { caja: DEMO.contrasenaRepo, ventas: DEMO.contrasenaRepo, historico: DEMO.contrasenaRepo }, semilla: 4, id: ID.caja },
  );
  // Tarea 4a: un servidor de aplicaciones con un agente nuevo y sin ninguna nube conectada.
  // La Dropbox del cliente está en el almacén: «Nuevo repositorio» ofrece conectarla también aquí.
  const surApps = await crearEquipo(
    sur,
    {
      nombre: "SERVIDOR-APPS",
      so: "Windows Server 2022",
      version_agente: "0.7.25",
      resumen: {
        admite: ["retencion_plazos", "verificacion_auto", "consolas_multiples", "escritorio", "verificacion_horario", "ordenes_en_espera", "cadenas", "derivadas", "filtros", "nube_equipo", "repo_en_nube", "datos_equipo", "quitar_destino", "prueba_auto", "restaurar_carpeta", "restaurar_desde", "restauracion_real"],
        consolas: [{ id: "principal", nombre: "cafedelsur.ejemplo.com", url: "https://cafedelsur.ejemplo.com:8443", identidad: "ZXN0YS1jb25zb2xhLXNpbXVsYWRhLTAwMDAwMDAwMDA=", sal_cliente: null, ultimo_contacto: hace(5), desde: null, esta: true }],
        repositorios: [],
        copias: [],
        destinos: [],
      },
    },
    { claveAdmin: DEMO.claveAdmin, kcfg: kSur, semilla: 6, id: ID.apps },
  );
  const estudio = await crearEquipo(
    propio,
    {
      nombre: "ESTUDIO",
      so: "Windows 11 Pro",
      resumen: {
        // Recién emparejado: copia programada, ninguna hecha todavía.
        // v1.56: también copia en el almacén de la oficina, que se gestiona desde otra consola
        // (no está en esta): el mapa dice que sus espejos no se ven aquí.
        admite: ["consolas_multiples", "datos_equipo", "quitar_destino"],
        consolas: [
          { id: "principal", nombre: "Consola en línea", url: "https://consola.ejemplo.com", identidad: "ZXN0YS1jb25zb2xhLXNpbXVsYWRhLTAwMDAwMDAwMDA=", sal_cliente: null, ultimo_contacto: hace(5), desde: null, esta: true },
          { id: "oficina", nombre: "Oficina del estudio", url: "https://oficina.ejemplo:8443", identidad: "b2ZpY2luYS1lc3R1ZGlvLTAwMDAwMDAwMDAwMDAwMDA=", sal_cliente: "c2FsLWRlbC1lc3R1ZGlvLTE2", ultimo_contacto: hace(10), desde: hace(60 * 24 * 30), esta: false },
        ],
        repositorios: [
          { ...repo("proyectos", "Proyectos", "b2-propio", 0, 0), ultima_version: null, verificado: null, prueba_restauracion: null },
          repo("facturas", "Facturas", "almacen-oficina", 40, 2_300_000_000),
        ],
        copias: [{ ...copia("proyectos", "Proyectos de diseño", "proyectos", ["13:00"], 4), ultima: null, proxima: proximaA(13) }, copia("facturas", "Facturas", "facturas", ["18:00"], 2)],
        destinos: [
          { id: "b2-propio", nombre: "Backblaze B2", tipo: "b2", donde: "estudio-copias", inmutable: true },
          { id: "almacen-oficina", nombre: "ALMACEN-ESTUDIO · Disco 1", tipo: "rest", donde: "https://almacen-estudio.ejemplo:8000/estudio/", equipo_almacen: "0f0e0d0c-0000-4000-8000-0000000000aa" },
        ],
      },
    },
    { claveAdmin: DEMO.claveAdmin, kcfg: kPropio, contrasenas: { proyectos: DEMO.contrasenaRepo }, semilla: 5, id: ID.estudio },
  );
  for (const e of [recepcion, contabilidad, portatil, servidorAltamar, archivos, surAlmacen, surCaja, surApps, estudio]) {
    e.ultimoSeqAceptado = 17;
    e.siguiente_seq = 18;
  }
  // 60 días de historial por repositorio, como lo informa el agente.
  // El último informe lleva el detalle por repositorio (v1.7, `repos[]`).
  const enriquecer = (e: EquipoMock, o: OpcionesHistorial = {}) => {
    const repos = (e.resumen?.repositorios ?? []).filter((r) => (r.versiones ?? 0) > 0).map((r) => informeRepo(r, e.resumen?.copias ?? [], o));
    // El resumen dice lo mismo que el informe.
    for (const r of e.resumen?.repositorios ?? []) {
      const i = repos.find((x) => x.id === r.id);
      if (i?.versiones[0]) r.ultima_version = i.versiones[0].hora;
    }
    if (e.informes[0]) e.informes[0] = { ...e.informes[0], recibido: new Date(Date.now() - ((o.silencioHoras ?? 0) * 60 + 4) * 60_000).toISOString(), datos: { ...e.informes[0].datos, repos, proximas: Object.fromEntries((e.resumen?.copias ?? []).map((k) => [k.id, k.activa === false ? null : (k.proxima ?? null)])) } };
  };
  enriquecer(recepcion, { externa: true });
  // Actualización automática (docs/actualizaciones.md): uno espera su turno (anillo general).
  if (recepcion.informes[0]) {
    recepcion.informes[0].datos.actualizacion = { estado: "pendiente", motivo: "espera_anillo", version_disponible: "0.7.25", hasta: new Date(Date.now() + 86_400_000).toISOString(), anillo: "general", modo: "auto", origen: "consola «Oficina»", ultima_busqueda: new Date(Date.now() - 3_600_000).toISOString() };
  }
  enriquecer(contabilidad, { ultimaFalla: true, fallos: 0.08, soloAnadir: true, medioAnadido: 420_000_000 });
  // Su informe dice su versión y el resultado del volcado de SQL Server (v1.10).
  if (contabilidad.informes[0]) {
    const d = contabilidad.informes[0].datos;
    d.version = contabilidad.version_agente;
    for (const k of d.copias ?? []) if (k.id === "ventas") k.ganchos = ["VENTAS_2026", "Nominas", "Compras", "Bancos", "Cartera", "Activos"].map((b) => ({ tipo: "sqlserver" as const, estado: "ok" as const, mensaje: `Volcado de SQL Server de «${b}» hecho.` }));
  }
  for (const e of [recepcion, portatil, servidorAltamar, surAlmacen, surCaja, estudio]) if (e.informes[0]) e.informes[0].datos.version = e.version_agente;
  // Etiquetas (v1.18) para agrupar y filtrar.
  servidorAltamar.etiquetas = ["Servidores"];
  recepcion.etiquetas = ["Recepción", "Sede norte"];
  contabilidad.etiquetas = ["Contabilidad", "Sede norte"];
  portatil.etiquetas = ["Gerencia"];
  surAlmacen.etiquetas = ["Servidores"];
  enriquecer(portatil, { soloAnadir: null, sinCambios: 0.3, medioAnadido: 40_000_000, silencioHoras: 54 });
  enriquecer(servidorAltamar, { externa: true, medioAnadido: 900_000_000, recortado: true });
  enriquecer(surCaja, { externa: true, medioAnadido: 25_000_000 });
  enriquecer(archivos, { externa: false, soloAnadir: false, medioAnadido: 650_000_000, local: { unidad: "D:" } });
  if (archivos.informes[0]) archivos.informes[0].datos.version = archivos.version_agente;
  archivos.etiquetas = ["Servidores"];
  estado.equipos.push(recepcion, contabilidad, portatil, servidorAltamar, archivos, surAlmacen, surCaja, surApps, estudio);
  // Un intento anterior de vincular CAJA-1 que se quedó sin confirmar (el bug de un equipo
  // que salía dos veces): la consola lo marca como duplicado y ofrece quitarlo.
  const cajaDuplicada: EquipoMock = {
    ...surCaja,
    id: randomUUID(),
    nombre: "caja-1",
    etiqueta: null,
    confirmado: false,
    conectado: false,
    ultimo_contacto: hace(60 * 26),
    resumen: { copias: [], repositorios: [], destinos: [] },
    etiquetas: [],
    informes: [],
    config: null,
    kcfg: null,
    contrasenas: {},
    ultimoSeqAceptado: 0,
    siguiente_seq: 1,
  };
  estado.equipos.push(cajaDuplicada);
  estado.emparejamientos.push({ id: randomUUID(), cliente: sur.id, codigo: "K7M2-Q9X4-HT", caduca: dentro(60 * 20), estado: "unido", creado: Date.now() - 26 * 3600_000, equipo: cajaDuplicada, por: ana.id });

  const aviso = (cliente: string, equipo: string | null, tipo: T.TipoAviso, mensaje: string, minutos: number) =>
    estado.avisos.push({ id: randomUUID(), cliente, equipo, tipo, mensaje, creado: hace(minutos), visto_por: null, abierto: true });
  aviso(altamar.id, contabilidad.id, "copia_fallida", "La copia «Ventas» de CONTABILIDAD falló: no se pudo conectar con el servidor de copias.", 175);
  aviso(altamar.id, contabilidad.id, "disco_lleno", "Queda poco espacio en el disco C: de «CONTABILIDAD». Quedan 7 GB libres de 240 GB (2 %). Libera espacio o amplía el disco: si se llena, las copias pueden fallar.", 60 * 2);
  aviso(altamar.id, contabilidad.id, "sin_soporte", "El sistema de «CONTABILIDAD» ya no tiene soporte. Windows 10 Pro 22H2 no recibe actualizaciones de seguridad desde el 14 de octubre de 2025. Actualízalo a una versión con soporte o cambia el equipo.", 60 * 20);
  aviso(altamar.id, archivos.id, "sin_soporte_pronto", "El sistema de «SERVIDOR-ARCHIVOS» se queda sin soporte pronto. Windows Server 2016 Standard 1607 deja de recibir actualizaciones de seguridad el 12 de enero de 2027 (en 91 días). Planifica la actualización.", 60 * 30);
  aviso(altamar.id, portatil.id, "equipo_sin_contacto", "PORTATIL-GERENCIA no se conecta desde hace 2 días.", 60 * 28);
  aviso(sur.id, surCaja.id, "intentos_fallidos", "Hubo 3 intentos fallidos con la clave de administración en CAJA-1.", 60 * 5);
  // 14a: alguien delante del equipo aprobó una orden en espera con la clave de administración.
  aviso(sur.id, surCaja.id, "orden_aprobada_en_equipo", "Se aprobó en el equipo una orden que reducía la protección: Pausar las copias (desde la consola «Consola en línea», pedida por Bruno). La aprobó ana (CAJA-1) con la clave de administración y se aplicó ya, sin esperar a su hora. Si no lo esperabas, cambia la clave de administración.", 60 * 26);
  aviso(sur.id, surCaja.id, "auditoria_rehecha", "La consola «Consola en línea» rehízo su registro de actividad: antes llegaba a la entrada n.º 412 y ahora solo a la 388. Si nadie restauró una copia anterior de esa consola, alguien la ha cambiado entera; compruébala con un ancla de un correo anterior.", 60 * 3);

  // Una orden destructiva esperando su turno, para la tarjeta «Pendiente».
  estado.ordenes.push({
    id: randomUUID(),
    cliente: altamar.id,
    equipo: portatil.id,
    tipo: "cambiar_retencion",
    seq: 17,
    emitida: hace(60 * 3),
    emitida_por: { id: luis.id, nombre: luis.nombre },
    not_before: dentro(60 * 21),
    caduca: dentro(60 * 45),
    estado: "pendiente",
    mensaje: null,
    detalle: null,
    firma_agente: null,
    actualizada: hace(60 * 3),
    sellado: "",
    sesion: null,
    relevo: null,
  });

  const t = (min: number) => hace(min);
  auditar(altamar.id, ana.id, "cliente.crear", "Ferretería Altamar", { espera_min_horas: 24 }, t(60 * 24 * 30));
  auditar(altamar.id, ana.id, "miembro.invitar", "tecnico", null, t(60 * 24 * 29));
  auditar(altamar.id, luis.id, "miembro.aceptar", luis.correo, null, t(60 * 24 * 28));
  for (const e of [recepcion, contabilidad, portatil, servidorAltamar]) {
    auditar(altamar.id, ana.id, "equipo.emparejar", e.nombre, { so: e.so }, t(60 * 24 * 27));
    auditar(altamar.id, ana.id, "orden.alta", e.nombre, { seq: 1 }, t(60 * 24 * 27 - 2));
  }
  auditar(altamar.id, ana.id, "orden.config", "RECEPCION", { seq: 12 }, t(60 * 24 * 6));
  auditar(altamar.id, luis.id, "orden.copiar_ahora", "CONTABILIDAD", { seq: 16 }, t(60 * 24));
  auditar(altamar.id, luis.id, "orden.cambiar_retencion", "PORTATIL-GERENCIA", { seq: 17, not_before: dentro(60 * 21) }, t(60 * 3));
  auditar(altamar.id, null, "equipo.sin_contacto", "PORTATIL-GERENCIA", null, t(60 * 28));
  auditar(sur.id, marta.id, "cliente.crear", "Café del Sur", null, t(60 * 24 * 40));
  auditar(sur.id, null, "equipo.intentos_fallidos", "CAJA-1", { intentos: 3 }, t(60 * 5));
  auditar(propio.id, ana.id, "cliente.crear", "Propio", null, t(60 * 24 * 12));
  // v1.38: dos clientes más (uno de Ana y otro que no es suyo), para «Todos los clientes».
  await sembrarOtrosClientes({ estado, crearEquipo, kcfgDe, ana, marta, sal, claveAdmin: DEMO.claveAdmin, contrasenaRepo: DEMO.contrasenaRepo });
}

/**
 * Plan 0.8.0, E2: unas restauraciones de ejemplo (deterministas) para cada
 * cliente, la primera vez que se piden: varias correctas, una fallida, con y sin
 * motivo, desde la consola y desde la ventana del equipo. Sin rutas.
 */
export function sembrarRestauraciones(cliente: string) {
  if (estado.restSembradas.has(cliente)) return;
  estado.restSembradas.add(cliente);
  const eqs = estado.equipos.filter((e) => e.cliente === cliente && (e.resumen?.repositorios?.length ?? 0) > 0);
  const motivos: (T.MotivoRestauracion | null)[] = ["borrado", "prueba", null, "dano", "borrado", "otro", null, "prueba"];
  const destinos = ["junto", "nueva", "zip", "descarga", "junto", "original", "zip", "junto"];
  const ahora = Date.now();
  eqs.slice(0, 4).forEach((e, i) => {
    (e.resumen?.repositorios ?? []).slice(0, 2).forEach((r, j) => {
      for (let k = 0; k < 3; k++) {
        const n = i * 6 + j * 3 + k;
        const fin = ahora - (n * 9 + 2) * 86_400_000 - n * 3_600_000;
        const dur = 40 + ((n * 137) % 1500);
        const fallo = n % 7 === 5;
        const motivo = motivos[n % motivos.length];
        estado.restauraciones.push({
          cliente,
          equipo: e.id,
          id: `restauracion-ejemplo-${e.id.slice(0, 8)}-${n}`,
          hora: new Date(fin).toISOString(),
          inicio: new Date(fin - dur * 1000).toISOString(),
          duracion_s: dur,
          repo: r.id,
          version: ((n * 2654435761) >>> 0).toString(16).padStart(8, "0").slice(0, 8),
          elementos: 1 + (n % 4),
          archivos: fallo ? null : 3 + ((n * 53) % 900),
          bytes: fallo ? null : 2_000_000 + ((n * 7_919_000) % 3_000_000_000),
          destino: destinos[n % destinos.length],
          cifrado: n % 3 === 0 && (destinos[n % destinos.length] === "zip" || destinos[n % destinos.length] === "descarga"),
          desde: n % 9 === 4 ? "derivada" : "principal",
          desde_nombre: n % 9 === 4 ? "Nube de la oficina" : null,
          resultado: fallo ? "fallo" : "ok",
          mensaje: fallo ? "Esa carpeta no está vacía: elige una carpeta nueva o vacía (nunca se sobrescribe nada)." : null,
          motivo: motivo ? { tipo: motivo, notas: motivo === "borrado" ? "Se borró una carpeta compartida." : null } : null,
          por: n % 5 === 3 ? null : n % 2 ? "Ana Pérez" : "Luis Gómez",
          consola: n % 5 === 3 ? null : n % 4 === 1 ? "Consola en línea" : "Oficina",
          orden: n % 5 === 3 ? null : `ejemplo-${n}`,
          en_el_equipo: n % 5 === 3,
        });
      }
    });
  });
}
