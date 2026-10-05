// Más clientes ficticios del simulador, para «Todos los clientes» (v1.38):
//
// - «Clínica Los Arrayanes» (Ana, propietaria): un almacén con espejo en la
//   nube, un equipo que copia en él (con una copia en marcha, en progreso.ts),
//   uno atrasado y otro que va a la nube.
// - «Taller Norte» (solo Marta): Ana no es miembro, así que no lo ve en ningún
//   sitio (ni en «Todos los clientes»).
//
// Todo inventado (nombres y dominios de ejemplo).
import { informeRepo } from "./historial";
import type { CuentaMock, Estado, EquipoMock } from "./estado";
import type * as T from "../lib/tipos";

export const ID_OTROS = {
  arrayanes: "0a0e1b2c-0000-4000-8000-0000000000a4",
  taller: "0a0e1b2c-0000-4000-8000-0000000000a5",
  almacenClinica: "0a0e1b2c-0000-4000-8000-0000000000e8",
  historias: "0a0e1b2c-0000-4000-8000-0000000000e9",
  rayos: "0a0e1b2c-0000-4000-8000-0000000000ea",
  recepcionClinica: "0a0e1b2c-0000-4000-8000-0000000000eb",
  tallerPc: "0a0e1b2c-0000-4000-8000-0000000000ec",
};

type CrearEquipo = (
  cliente: { id: string; sal_cliente: string },
  datos: Partial<T.Equipo> & { nombre: string; so: string },
  opciones?: { claveAdmin?: string; kcfg?: Uint8Array; contrasenas?: Record<string, string>; semilla?: number; id?: string },
) => Promise<EquipoMock>;

interface Dependencias {
  estado: Estado;
  crearEquipo: CrearEquipo;
  kcfgDe: (c: { sal_cliente: string }) => Promise<Uint8Array>;
  ana: CuentaMock;
  marta: CuentaMock;
  sal: () => string;
  claveAdmin: string;
  contrasenaRepo: string;
}

const hace = (min: number) => new Date(Date.now() - min * 60_000).toISOString();
const dentro = (min: number) => new Date(Date.now() + min * 60_000).toISOString();

/** El último informe con el detalle de cada repositorio (como `enriquecer` en estado.ts). */
function detallar(e: EquipoMock, o: Parameters<typeof informeRepo>[2] = {}) {
  const repos = (e.resumen?.repositorios ?? []).filter((r) => (r.versiones ?? 0) > 0).map((r) => informeRepo(r, e.resumen?.copias ?? [], o));
  for (const r of e.resumen?.repositorios ?? []) {
    const i = repos.find((x) => x.id === r.id);
    if (i?.versiones[0]) r.ultima_version = i.versiones[0].hora;
  }
  if (e.informes[0])
    e.informes[0] = {
      ...e.informes[0],
      recibido: hace(4),
      datos: { ...e.informes[0].datos, version: e.version_agente, repos, proximas: Object.fromEntries((e.resumen?.copias ?? []).map((k) => [k.id, k.proxima ?? null])) },
    };
}

export async function sembrarOtrosClientes(d: Dependencias) {
  const { estado, crearEquipo, ana, marta } = d;
  const arrayanes = { id: ID_OTROS.arrayanes, nombre: "Clínica Los Arrayanes", sal_cliente: d.sal(), espera_min_horas: 24, rol: "propietario" as T.Rol };
  const taller = { id: ID_OTROS.taller, nombre: "Taller Norte", sal_cliente: d.sal(), espera_min_horas: 24, rol: "propietario" as T.Rol };
  estado.clientes.push(arrayanes, taller);
  estado.miembros.set(arrayanes.id, [{ cuenta: ana.id, rol: "propietario" }]);
  estado.miembros.set(taller.id, [{ cuenta: marta.id, rol: "propietario" }]);
  const [kA, kT] = await Promise.all([d.kcfgDe(arrayanes), d.kcfgDe(taller)]);

  const repo = (id: string, nombre: string, destino: string, versiones: number, bytes: number): T.RepositorioResumen => ({
    id,
    nombre,
    destino,
    versiones,
    bytes,
    ultima_version: hace(200),
    verificado: hace(60 * 40),
    prueba_restauracion: hace(60 * 24 * 12),
    retencion: "7 diarias, 4 semanales, 12 mensuales",
  });
  const copia = (id: string, nombre: string, repoId: string, horas: string[], ultimaMin: number, proximaMin: number): T.CopiaResumen => ({
    id,
    nombre,
    repo: repoId,
    horario: { dias: [1, 2, 3, 4, 5, 6], horas },
    carpetas: 2,
    activa: true,
    ultima: { cuando: hace(ultimaMin), estado: "ok", bytes: 96_000_000, mensaje: null },
    proxima: proximaMin >= 0 ? dentro(proximaMin) : hace(-proximaMin),
  });
  const almacenDestino: T.DestinoResumen = { id: "almacen-clinica", nombre: "Almacén de la clínica", tipo: "rest", donde: "https://arrayanes.ejemplo.com:8000/", equipo_almacen: ID_OTROS.almacenClinica };
  const nube: T.DestinoResumen = { id: "s3-clinica", nombre: "S3 de la clínica", tipo: "s3", donde: "arrayanes-copias", inmutable: true };
  const op = (contrasenas: Record<string, string>, semilla: number, id: string) => ({ claveAdmin: d.claveAdmin, kcfg: kA, contrasenas, semilla, id });

  const almacen = await crearEquipo(
    arrayanes,
    {
      nombre: "ALMACEN-CLINICA",
      so: "Ubuntu 24.04",
      version_agente: "0.7.14",
      rol: "almacenamiento",
      resumen: {
        guarda_copias: {
          activo: true,
          puerto: 8000,
          solo_red_local: false,
          usuarios: 2,
          carpeta: "/srv/resguardo",
          espacio: { libre: 210_000_000_000, total: 1_000_000_000_000, leido: hace(3) },
          espejo: { hora: "01:30", ultima: hace(60 * 9), resultado: "Espejo hecho: 412 archivos nuevos (6 GB).", destinos: [{ tipo: "nube", nube: "Backblaze de la clínica", carpeta: "Arrayanes", ultima: hace(60 * 9), resultado: "Espejo hecho." }] },
        },
        repositorios: [],
        copias: [],
      },
    },
    op({}, 7, ID_OTROS.almacenClinica),
  );
  const historias = await crearEquipo(
    arrayanes,
    {
      nombre: "HISTORIAS",
      so: "Windows 11 Pro",
      version_agente: "0.7.14",
      resumen: {
        destinos: [almacenDestino],
        repositorios: [repo("historias", "Historias clínicas", "almacen-clinica", 160, 64_000_000_000)],
        copias: [copia("historias", "Historias clínicas", "historias", ["12:00", "19:00"], 150, 200)],
      },
    },
    op({ historias: d.contrasenaRepo }, 8, ID_OTROS.historias),
  );
  const rayos = await crearEquipo(
    arrayanes,
    {
      nombre: "RAYOS-X",
      so: "Windows 10 Pro",
      version_agente: "0.7.12",
      ultimo_contacto: hace(6),
      resumen: {
        destinos: [almacenDestino],
        repositorios: [repo("imagenes", "Imágenes diagnósticas", "almacen-clinica", 58, 182_000_000_000)],
        copias: [copia("imagenes", "Imágenes", "imagenes", ["21:00"], 60 * 30, -60 * 5)],
      },
    },
    op({ imagenes: d.contrasenaRepo }, 9, ID_OTROS.rayos),
  );
  const recepcion = await crearEquipo(
    arrayanes,
    {
      nombre: "RECEPCION-CLINICA",
      so: "Windows 11 Pro",
      version_agente: "0.7.14",
      resumen: {
        destinos: [nube],
        repositorios: [repo("agenda", "Agenda y facturas", "s3-clinica", 120, 7_400_000_000)],
        copias: [copia("agenda", "Agenda y facturas", "agenda", ["13:30"], 60 * 3, 60 * 20)],
      },
    },
    op({ agenda: d.contrasenaRepo }, 10, ID_OTROS.recepcionClinica),
  );
  const tallerPc = await crearEquipo(
    taller,
    {
      nombre: "TALLER-PC",
      so: "Windows 11 Pro",
      resumen: { destinos: [{ id: "b2-taller", nombre: "Backblaze B2", tipo: "b2", donde: "taller-copias" }], repositorios: [repo("planos", "Planos", "b2-taller", 40, 3_000_000_000)], copias: [copia("planos", "Planos", "planos", ["18:00"], 60 * 5, 60 * 10)] },
    },
    { claveAdmin: d.claveAdmin, kcfg: kT, contrasenas: { planos: d.contrasenaRepo }, semilla: 11, id: ID_OTROS.tallerPc },
  );
  for (const e of [almacen, historias, rayos, recepcion, tallerPc]) {
    e.ultimoSeqAceptado = 9;
    e.siguiente_seq = 10;
  }
  detallar(historias, { medioAnadido: 300_000_000 });
  detallar(rayos, { medioAnadido: 2_400_000_000, silencioHoras: 30 });
  detallar(recepcion, { externa: false, medioAnadido: 60_000_000 });
  detallar(tallerPc);
  estado.equipos.push(almacen, historias, rayos, recepcion, tallerPc);
  estado.avisos.push({ id: crypto.randomUUID(), cliente: arrayanes.id, equipo: rayos.id, tipo: "copia_atrasada" as T.TipoAviso, mensaje: "La copia «Imágenes» de RAYOS-X no se hace desde hace 30 horas.", creado: hace(60 * 5), visto_por: null, abierto: true });
}
