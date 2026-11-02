// 16q (plan 0.8.3): un cliente GRANDE en el simulador, para medir la consola con
// muchos equipos. Solo con `MOCK_EQUIPOS=200 npm run dev:mock` (o `vite preview
// --mode mock`); sin la variable no se crea. Con la misma semilla (`MOCK_SEMILLA`,
// por defecto 83) salen siempre los mismos nombres, estados y tamaños.
//
// Reparto (para 200): 1 almacén por cada 25 equipos (8), cada uno con su espejo a
// una nube; los demás, puestos y servidores con 3 repositorios (uno de cada 8, 4):
// dos en el almacén de su sede y uno en la nube; ~600 repositorios. Unos pocos sin
// conexión, con la última copia fallida o atrasados, como en una oficina de verdad.
//
// Todo inventado (nombres, sedes y dominios de ejemplo).
import { informeRepo } from "./historial";
import type { CuentaMock, Estado, EquipoMock } from "./estado";
import type * as T from "../lib/tipos";

export const ID_GRANDE = "0a0e1b2c-0000-4000-8000-0000000000b1";

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
  sal: () => string;
  contrasenaRepo: string;
  /** Una sal y su verificador de la clave de administración, para todos los equipos (Argon2 una vez, no 200). */
  salVerificador: { sal: string; verificador: Uint8Array };
}

/** Número de equipos pedido (0: sin cliente grande). Entre 10 y 2000. */
export function equiposGrande(): number {
  const n = Number(process.env.MOCK_EQUIPOS ?? 0);
  return Number.isFinite(n) && n > 0 ? Math.min(2000, Math.max(10, Math.round(n))) : 0;
}

/** Generador determinista (mulberry32). */
function generador(semilla: number) {
  let s = semilla >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const SEDES = ["Centro", "Norte", "Puerto", "Valle", "Sierra", "Llanos", "Ribera", "Meseta"];
const PUESTOS = ["CAJA", "VENTAS", "COMPRAS", "OFICINA", "BODEGA", "GERENCIA", "RRHH", "DISENO", "SOPORTE", "LOGISTICA"];
const REPOS = [
  ["documentos", "Documentos"],
  ["escritorio", "Escritorio"],
  ["correo", "Correo"],
  ["proyectos", "Proyectos"],
  ["facturas", "Facturas"],
  ["fotos", "Fotos"],
] as const;
const G = 1_000_000_000;

const hace = (min: number) => new Date(Date.now() - min * 60_000).toISOString();
const dentro = (min: number) => new Date(Date.now() + min * 60_000).toISOString();

export async function sembrarGrande(d: Dependencias, n: number) {
  const { estado, crearEquipo, ana } = d;
  const rnd = generador(Number(process.env.MOCK_SEMILLA ?? 83) || 83);
  const entre = (a: number, b: number) => a + Math.floor(rnd() * (b - a + 1));
  const cliente = { id: ID_GRANDE, nombre: "Distribuidora Gran Escala", sal_cliente: d.sal(), espera_min_horas: 24, rol: "propietario" as T.Rol };
  estado.clientes.push(cliente);
  estado.miembros.set(cliente.id, [{ cuenta: ana.id, rol: "propietario" }]);
  const kcfg = await d.kcfgDe(cliente);
  const sedes = SEDES.slice(0, Math.max(2, Math.min(SEDES.length, Math.ceil(n / 25))));
  const nAlmacenes = Math.max(2, Math.round(n / 25));
  const nube: T.DestinoResumen = { id: "b2-gran-escala", nombre: "Nube de la distribuidora", tipo: "b2", donde: "gran-escala-copias", inmutable: true };
  const equipos: EquipoMock[] = [];
  const almacenes: { equipo: EquipoMock; destino: T.DestinoResumen; sede: string; repos: { usuario: string; repos: string[] }[] }[] = [];

  const ajustar = (e: EquipoMock) => {
    e.sal_equipo = d.salVerificador.sal;
    e.verificador = d.salVerificador.verificador;
    e.ultimoSeqAceptado = 9;
    e.siguiente_seq = 10;
  };

  for (let i = 0; i < nAlmacenes; i++) {
    const sede = sedes[i % sedes.length];
    const nombre = `ALMACEN-${sede.toUpperCase()}${i >= sedes.length ? `-${Math.floor(i / sedes.length) + 1}` : ""}`;
    const totalT = entre(4, 16);
    const libre = Math.round(totalT * 1000 * (0.15 + rnd() * 0.6));
    const e = await crearEquipo(
      cliente,
      {
        nombre,
        so: "Ubuntu 24.04",
        version_agente: "0.8.2",
        rol: "almacenamiento",
        resumen: {
          admite: ["espejo_zonas", "espejo_flexible", "espejo_trabajos", "tipo_equipo"],
          guarda_copias: {
            activo: true,
            puerto: 8000,
            solo_red_local: true,
            usuarios: 0,
            carpeta: "/srv/resguardo",
            sistema_archivos: "zfs",
            espacio: { libre: libre * G, total: totalT * 1000 * G, leido: hace(entre(1, 9)) },
            nubes: [{ nombre: "Nube de la distribuidora", tipo: "b2" }],
            espejo: {
              hora: "02:00",
              ultima: hace(60 * entre(6, 14)),
              trabajos: [
                {
                  id: `t${(i + 1).toString(16).padStart(10, "0")}`,
                  nombre: `Todo a la nube (${sede})`,
                  activo: true,
                  quien: "almacen",
                  que: { tipo: "todos" },
                  adonde: { tipo: "nube", nube: "Nube de la distribuidora", carpeta: `espejo/${sede.toLowerCase()}` },
                  cuando: { horario: { dias: [1, 2, 3, 4, 5, 6, 7], horas: ["02:00"] } },
                  retencion: { modo: "retraso", dias: 30 },
                  freno: { pct: 10, min_archivos: 100, min_faltan: 20, accion: "confirmar" },
                  orden: 0,
                  ultima: hace(60 * entre(6, 14)),
                  resultado: i === 1 ? "ERROR: la nube respondió 503 (no disponible); se reintenta mañana." : "Espejo hecho: archivos nuevos subidos.",
                  bien: i === 1 ? hace(60 * 38) : hace(60 * entre(6, 14)),
                  nunca_bien: false,
                },
              ],
            },
          },
          repositorios: [],
          copias: [],
        },
      },
      { kcfg, semilla: 100 + i },
    );
    ajustar(e);
    e.etiquetas = ["Servidores", `Sede ${sede}`];
    const destino: T.DestinoResumen = { id: `almacen-${sede.toLowerCase()}-${i}`, nombre: `Almacén ${sede}`, tipo: "rest", donde: `https://almacen-${sede.toLowerCase()}.gran-escala.ejemplo:8000/`, equipo_almacen: e.id };
    almacenes.push({ equipo: e, destino, sede, repos: [] });
    equipos.push(e);
  }

  const nPuestos = n - nAlmacenes;
  for (let i = 0; i < nPuestos; i++) {
    const a = almacenes[i % almacenes.length];
    const servidor = i % 12 === 0;
    const nombre = servidor ? `SRV-${a.sede.toUpperCase()}-${String(i).padStart(3, "0")}` : `${PUESTOS[i % PUESTOS.length]}-${a.sede.toUpperCase()}-${String(i).padStart(3, "0")}`;
    const nRepos = i % 8 === 0 ? 4 : 3;
    const desconectado = rnd() < 0.05;
    const falla = rnd() < 0.04;
    const atrasado = !desconectado && rnd() < 0.03;
    const repos: T.RepositorioResumen[] = [];
    const copias: T.CopiaResumen[] = [];
    const contrasenas: Record<string, string> = {};
    for (let r = 0; r < nRepos; r++) {
      const [base, nombreRepo] = REPOS[(i + r) % REPOS.length];
      const id = `${base}-${i}-${r}`;
      const enNube = r === 2;
      const bytes = entre(2, servidor ? 900 : 120) * G;
      repos.push({
        id,
        nombre: nombreRepo,
        destino: enNube ? nube.id : a.destino.id,
        versiones: entre(20, 220),
        bytes,
        en_disco_bytes: Math.round(bytes * (0.55 + rnd() * 0.3)),
        ultima_version: hace(entre(60, 600)),
        verificado: hace(60 * entre(10, 24 * 20)),
        prueba_restauracion: rnd() < 0.7 ? hace(60 * 24 * entre(2, 80)) : null,
        retencion: "7 diarias, 4 semanales, 12 mensuales",
      } as T.RepositorioResumen);
      const horas = [`${String(entre(9, 20)).padStart(2, "0")}:${rnd() < 0.5 ? "00" : "30"}`];
      const fallo = falla && r === 0;
      copias.push({
        id: `copia-${id}`,
        nombre: nombreRepo,
        repo: id,
        horario: { dias: [1, 2, 3, 4, 5], horas },
        carpetas: entre(1, 4),
        activa: true,
        ultima: { cuando: hace(atrasado ? 60 * entre(30, 70) : entre(60, 600)), estado: fallo ? "fallo" : "ok", bytes: entre(5, 900) * 1_000_000, mensaje: fallo ? "No se pudo conectar con el servidor de copias" : null },
        proxima: atrasado ? hace(60 * entre(4, 20)) : dentro(entre(30, 900)),
      });
      contrasenas[id] = d.contrasenaRepo;
      if (!enNube) {
        const usuario = `u-${i}`;
        const fila = a.repos.find((x) => x.usuario === usuario);
        if (fila) fila.repos.push(id);
        else a.repos.push({ usuario, repos: [id] });
      }
    }
    const win = servidor ? "Windows Server 2022 Standard" : rnd() < 0.8 ? "Windows 11 Pro" : "Windows 10 Pro";
    const e = await crearEquipo(
      cliente,
      {
        nombre,
        so: win,
        version_agente: rnd() < 0.85 ? "0.8.2" : "0.8.1",
        conectado: !desconectado,
        ultimo_contacto: desconectado ? hace(60 * entre(3, 90)) : hace(entre(0, 4)),
        resumen: {
          admite: ["datos_equipo", "tipo_equipo", "prueba_auto", "restaurar_carpeta"],
          datos_basicos: {
            leido: hace(60 * entre(1, 20)),
            sistema: { familia: "windows", nombre: win, variante: servidor ? "ServerStandard" : "Professional", version: servidor ? "21H2" : "24H2", compilacion: servidor ? "20348.2700" : "26100.2033" },
            procesador: { nombre: servidor ? "Intel(R) Xeon(R) Silver 4310" : "Intel(R) Core(TM) i5-12400", nucleos: servidor ? 24 : 12 },
            memoria: (servidor ? 64 : 16) * 1024 ** 3,
            fabricante: "Fabricante Ejemplo",
            modelo: servidor ? "Servidor E-200" : "Puesto P-10",
            arranque: hace(60 * entre(1, 24 * 20)),
            discos: [{ punto: "C:", total: 512 * G, libre: entre(20, 400) * G, sistema_archivos: "NTFS", en_uso: true, volumen: null, etiqueta: null }],
          },
          destinos: [a.destino, nube],
          repositorios: repos,
          copias,
        },
      },
      { kcfg, contrasenas, semilla: 200 + i },
    );
    ajustar(e);
    e.etiquetas = [servidor ? "Servidores" : "Puestos", `Sede ${a.sede}`];
    const informes = repos.map((r) => informeRepo(r, copias, { medioAnadido: entre(20, 900) * 1_000_000, ultimaFalla: falla && r === repos[0] ? true : undefined }));
    for (const r of repos) {
      const inf = informes.find((x) => x.id === r.id);
      if (inf?.versiones[0]) r.ultima_version = inf.versiones[0].hora;
    }
    if (e.informes[0])
      e.informes[0] = {
        ...e.informes[0],
        // Cada uno en su segundo (como llegan de verdad): si no, «los nuevos» serían muchos a la vez.
        recibido: desconectado ? (e.ultimo_contacto ?? hace(60)) : hace(entre(60, 600) / 60),
        datos: { ...e.informes[0].datos, version: e.version_agente, repos: informes, proximas: Object.fromEntries(copias.map((k) => [k.id, k.proxima ?? null])) },
      };
    equipos.push(e);
    if (falla)
      estado.avisos.push({ id: crypto.randomUUID(), cliente: cliente.id, equipo: e.id, tipo: "copia_fallida" as T.TipoAviso, mensaje: `La copia «${copias[0].nombre}» de ${nombre} falló: no se pudo conectar con el servidor de copias.`, creado: hace(entre(60, 600)), visto_por: null, abierto: true });
    if (desconectado && rnd() < 0.5)
      estado.avisos.push({ id: crypto.randomUUID(), cliente: cliente.id, equipo: e.id, tipo: "equipo_sin_contacto" as T.TipoAviso, mensaje: `${nombre} no se conecta desde hace más de un día.`, creado: hace(60 * entre(24, 80)), visto_por: null, abierto: true });
  }
  for (const a of almacenes) {
    const g = a.equipo.resumen?.guarda_copias;
    if (g) {
      g.repositorios = a.repos;
      g.usuarios = a.repos.length;
    }
  }
  estado.equipos.push(...equipos);
  return { cliente, equipos: equipos.length, repositorios: equipos.reduce((s, e) => s + (e.resumen?.repositorios?.length ?? 0), 0) };
}
