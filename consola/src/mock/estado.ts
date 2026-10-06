// Estado en memoria del servidor simulado (solo `npm run dev:mock`).
// Imita a Resguardo Server lo justo para diseñar y probar la consola: cuentas
// con TOTP de mentira, clientes, equipos con llaves de verdad (el «agente»
// simulado abre los sobres y firma los resultados), órdenes, avisos,
// auditoría encadenada, sesiones y relé.
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
  /** v1.4x: el código lo generó el navegador; aquí solo su hash (y `codigo` vacío). */
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
}

export let estado: Estado;

const hace = (min: number) => new Date(Date.now() - min * 60_000).toISOString();
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

  const servidorAltamar = await crearEquipo(
    altamar,
    {
      nombre: "SERVIDOR-ALTAMAR",
      so: "Debian 12 (CT de Proxmox)",
      rol: "almacenamiento",
      resumen: {
        guarda_copias: { activo: true, puerto: 8000, solo_red_local: true, usuarios: 3, carpeta: "/srv/resguardo/copias", espacio: { libre: 640_000_000_000, total: 4_000_000_000_000, leido: hace(5) } },
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
        respaldo_dias: 3,
        destinos: [destinoServidor, destinoNube],
        repositorios: [repo("docs-recepcion", "Documentos de recepción", "servidor-altamar", 148, 38_400_000_000)],
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
        destinos: [destinoServidor],
        repositorios: [repo("siigo", "Siigo y documentos", "servidor-altamar", 96, 91_200_000_000)],
        copias: [copia("siigo", "Siigo", "siigo", ["12:30", "18:30"], 2, false)],
      },
    },
    { claveAdmin: DEMO.claveAdmin, kcfg: kAltamar, contrasenas: { siigo: DEMO.contrasenaRepo }, semilla: 0, id: ID.contabilidad },
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
      so: "Windows Server 2022",
      resumen: {
        admite: ["retencion_plazos", "verificacion_auto", "almacen_propio", "consolas_multiples", "escritorio", "verificacion_horario", "retencion_almacen_horario", "externa_existente"],
        destinos: [{ id: "carpeta-d", nombre: "Copias en el disco D", tipo: "local", donde: undefined, unidad: "D:", extraible: false, red: false }],
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
      resumen: { admite: ["retencion_plazos", "verificacion_auto", "almacen_propio", "consolas_multiples", "verificacion_horario", "retencion_almacen_horario"],
        // Tarea 2: se añadió aquí y no está en la consola en línea, como CAJA-1 («N equipos no están en todas tus consolas»).
        consolas: [{ id: "principal", nombre: "cafedelsur.ejemplo.com", url: "https://cafedelsur.ejemplo.com:8443", identidad: "ZXN0YS1jb25zb2xhLXNpbXVsYWRhLTAwMDAwMDAwMDA=", sal_cliente: null, ultimo_contacto: hace(5), desde: null, esta: true }],
        guarda_copias: { activo: true, puerto: 8000, solo_red_local: false, usuarios: 5, carpeta: "D:\\Resguardo\\Copias",
          // v1.31: el espacio de su disco y el de cada destino del espejo («¿Cuándo se llena?»).
          espacio: { libre: 3_400_000_000, total: 500_000_000_000, leido: hace(2) }, repositorios: [{ usuario: "caja-1", repos: ["caja", "siigo"] }], espejo: {
            hora: "02:00",
            ultima: hace(60 * 13),
            resultado: "Espejo hecho: 1.204 archivos nuevos (38 GB) en 2 destinos.",
            limite_kib: 4096,
            destinos: [
              { tipo: "carpeta", carpeta: "E:\\Resguardo-espejo", ultima: hace(60 * 13), resultado: "Espejo hecho: 1.204 archivos nuevos (38 GB).", espacio: { libre: 1_310_000_000_000, total: 2_000_000_000_000, leido: hace(2) } },
              { tipo: "nube", nube: "Dropbox Oficina", carpeta: "CafeDelSur", ultima: hace(60 * 13), resultado: "ERROR: Dropbox respondió 429 (demasiadas peticiones); se reintenta mañana.", espacio: { libre: 520_000_000, total: 2_199_023_255_552, leido: hace(60 * 13) } },
            ],
          },
          nubes: [{ nombre: "Dropbox Oficina", tipo: "dropbox" }],
          // v1.22: la retención de «Caja» la aplica el almacén, los domingos a las 03:00.
          retenciones: [
            {
              usuario: "caja-1",
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
        // v1.46: «Siigo» (movido al almacén) puede llevar su copia externa al repositorio de la nube de siempre.
        admite: ["retencion_plazos", "verificacion_auto", "almacen_propio", "consolas_multiples", "verificacion_horario", "retencion_almacen_horario", "externa_existente"],
        // v1.36: también la gestiona una consola en línea (y el último cambio vino de allí).
        consolas: [
          { id: "principal", nombre: "cafedelsur.ejemplo.com", url: "https://cafedelsur.ejemplo.com:8443", identidad: "ZXN0YS1jb25zb2xhLXNpbXVsYWRhLTAwMDAwMDAwMDA=", sal_cliente: null, ultimo_contacto: hace(5), desde: null, esta: true },
          { id: "en-linea", nombre: "Consola en línea", url: "https://consola.ejemplo.com", identidad: "b3RyYS1jb25zb2xhLWVuLWxpbmVhLTAwMDAwMDAwMDA=", sal_cliente: "c2FsLWRlbC1jbGllbnRlLTE2", ultimo_contacto: hace(15), desde: hace(60 * 24 * 20), esta: false },
        ],
        cambio_config: { tipo: "config", cuando: hace(60 * 3), consola: { nombre: "Consola en línea", url: "https://consola.ejemplo.com", identidad: "b3RyYS1jb25zb2xhLWVuLWxpbmVhLTAwMDAwMDAwMDA=" } },
        // Con copia externa diaria a un segundo disco (cambiar_copia_externa).
        repositorios: [{ ...repo("caja", "Caja", "almacen-sur", 77, 4_100_000_000), retencion: "7 diarias · 4 semanales · 12 mensuales · 2 anuales", retencion_regla: { diarias: 7, semanales: 4, mensuales: 12, anuales: 2 }, solo_anadir: true, externa: { destino: "Disco 2", destino_id: "disco-2", hora: "21:00" }, verificacion_auto: { cada_dias: 7, porcentaje: 10, proxima: dentro(60 * 24 * 3), todo_leido: hace(60 * 24 * 40) } },
          // También en el almacén, todavía sin retención (v1.22: «Retención en el almacén…»).
          { ...repo("siigo", "Siigo", "almacen-sur", 52, 9_800_000_000), solo_anadir: true }],
        copias: [copia("caja", "Caja y facturas", "caja", ["14:00"], 1)],
        destinos: [
          { id: "almacen-sur", nombre: "Almacén Sur", tipo: "rest", donde: "https://cafedelsur.ejemplo.com:8000/caja-1/", equipo_almacen: ID.almacen },
          { id: "disco-2", nombre: "Disco 2", tipo: "local", donde: "E:\\Resguardo-externa" },
        ],
      },
    },
    { claveAdmin: DEMO.claveAdmin, kcfg: kSur, contrasenas: { caja: DEMO.contrasenaRepo, siigo: DEMO.contrasenaRepo }, semilla: 4, id: ID.caja },
  );
  const estudio = await crearEquipo(
    propio,
    {
      nombre: "ESTUDIO",
      so: "Windows 11 Pro",
      resumen: {
        // Recién emparejado: copia programada, ninguna hecha todavía.
        repositorios: [{ ...repo("proyectos", "Proyectos", "b2-propio", 0, 0), ultima_version: null, verificado: null, prueba_restauracion: null }],
        copias: [{ ...copia("proyectos", "Proyectos de diseño", "proyectos", ["13:00"], 4), ultima: null, proxima: proximaA(13) }],
        destinos: [{ id: "b2-propio", nombre: "Backblaze B2", tipo: "b2", donde: "estudio-copias", inmutable: true }],
      },
    },
    { claveAdmin: DEMO.claveAdmin, kcfg: kPropio, contrasenas: { proyectos: DEMO.contrasenaRepo }, semilla: 5, id: ID.estudio },
  );
  for (const e of [recepcion, contabilidad, portatil, servidorAltamar, archivos, surAlmacen, surCaja, estudio]) {
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
  enriquecer(recepcion, { externa: false });
  enriquecer(contabilidad, { ultimaFalla: true, fallos: 0.08, soloAnadir: true, medioAnadido: 420_000_000 });
  // Su informe dice su versión y el resultado del volcado de SQL Server (v1.10).
  if (contabilidad.informes[0]) {
    const d = contabilidad.informes[0].datos;
    d.version = contabilidad.version_agente;
    for (const k of d.copias ?? []) if (k.id === "siigo") k.ganchos = [{ tipo: "sqlserver", estado: "ok", mensaje: "Volcada 1 base (SIIGO_ALTAMAR, 1,2 GB) en C:\\ResguardoVolcados." }];
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
  estado.equipos.push(recepcion, contabilidad, portatil, servidorAltamar, archivos, surAlmacen, surCaja, estudio);

  const aviso = (cliente: string, equipo: string | null, tipo: T.TipoAviso, mensaje: string, minutos: number) =>
    estado.avisos.push({ id: randomUUID(), cliente, equipo, tipo, mensaje, creado: hace(minutos), visto_por: null, abierto: true });
  aviso(altamar.id, contabilidad.id, "copia_fallida", "La copia «Siigo» de CONTABILIDAD falló: no se pudo conectar con el servidor de copias.", 175);
  aviso(altamar.id, portatil.id, "equipo_sin_contacto", "PORTATIL-GERENCIA no se conecta desde hace 2 días.", 60 * 28);
  aviso(sur.id, surCaja.id, "intentos_fallidos", "Hubo 3 intentos fallidos con la clave de administración en CAJA-1.", 60 * 5);
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
