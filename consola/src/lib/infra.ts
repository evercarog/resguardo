// Plan 0.8.1, C4 (plan-mejoras 15d; docs/infraestructura.md): infraestructura,
// fase 1, para cualquier entorno (equipo físico Windows o Linux, Hyper-V, VMware,
// Proxmox, NAS, nube). Lo que la persona anota en el servidor de la consola
// (`GET/PUT /api/clientes/{c}/infraestructura`): los servidores físicos, sus
// conjuntos de discos y su hipervisor, en qué servidor está cada equipo (una
// máquina virtual, un contenedor o el propio servidor) y en qué conjunto vive cada
// destino o zona del almacén.
//
// **La interfaz propone y la persona confirma** (`propuestas`): lo que el agente ve
// solo (su tipo, sus discos y conjuntos, el sistema de archivos y el grupo de ZFS
// de cada zona) se ofrece en una frase con pocos campos. Resguardo no gestiona
// nada de esto: solo sabe dónde está cada cosa (la línea «Dónde está físicamente»)
// y avisa en la regla 3-2-1-1-0 («servidor físico compartido», los soportes
// reales). Todo es opcional: sin nada anotado, todo como antes. Sin runas: se
// prueba en scripts/vectores-infra.ts.
import type { DestinoResumen, Equipo } from "./tipos";
import { bytes } from "./formato";
import { datosBasicos, discoDeRuta, textoModelo, type ConjuntoEquipo, type DiscoEquipo } from "./datosBasicos";
import { claveZona, PRINCIPAL, zonasDe } from "./destinos";
import { sedePorId, type Organizacion } from "./sedes";
import { tipoEquipo } from "./tipoEquipo";

export interface ConjuntoDiscos {
  id: string;
  nombre: string;
  /** Una de `TIPOS_CONJUNTO`. */
  tipo: string;
  discos?: number | null;
  /** Bytes. */
  capacidad?: number | null;
  estado?: "bien" | "degradado" | "reconstruyendo" | "sin_revisar" | null;
  notas?: string | null;
}

export interface ServidorFisico {
  id: string;
  nombre: string;
  marca?: string | null;
  modelo?: string | null;
  /** El id de una sede del cliente. */
  sede?: string | null;
  /** Uno de `HIPERVISORES`, o null (sin hipervisor: un servidor de uso directo). */
  hipervisor?: string | null;
  notas?: string | null;
  conjuntos: ConjuntoDiscos[];
}

/** En qué servidor (y conjunto) está un equipo o un destino. */
export interface Sitio {
  servidor: string;
  conjunto?: string | null;
}

export interface Infraestructura {
  servidores: ServidorFisico[];
  /** Id del equipo → su sitio. */
  equipos: Record<string, Sitio>;
  /** Clave del destino (la del catálogo: `zona:<almacén>:<zona>`, el id de un destino…) → su sitio. */
  destinos: Record<string, Sitio>;
  actualizada?: string | null;
  por?: string | null;
}

export const INFRA_VACIA: Infraestructura = { servidores: [], equipos: {}, destinos: {} };

/** Los mismos que el servidor (`crate::infraestructura`). */
export const HIPERVISORES: { id: string; nombre: string }[] = [
  { id: "proxmox", nombre: "Proxmox VE" },
  { id: "hyperv", nombre: "Hyper-V" },
  { id: "vmware", nombre: "VMware ESXi" },
  { id: "xen", nombre: "Xen o XCP-ng" },
  { id: "kvm", nombre: "KVM (otro)" },
  { id: "nutanix", nombre: "Nutanix AHV" },
  { id: "virtualbox", nombre: "VirtualBox" },
  { id: "otro", nombre: "Otro" },
];

export const TIPOS_CONJUNTO: { id: string; nombre: string; corto: string }[] = [
  { id: "raid1", nombre: "RAID 1 (espejo)", corto: "RAID 1" },
  { id: "raid5", nombre: "RAID 5", corto: "RAID 5" },
  { id: "raid6", nombre: "RAID 6", corto: "RAID 6" },
  { id: "raid10", nombre: "RAID 10", corto: "RAID 10" },
  { id: "raid0", nombre: "RAID 0 (sin redundancia)", corto: "RAID 0" },
  { id: "zfs_espejo", nombre: "ZFS en espejo", corto: "espejo" },
  { id: "zfs_raidz1", nombre: "ZFS raidz1", corto: "raidz1" },
  { id: "zfs_raidz2", nombre: "ZFS raidz2", corto: "raidz2" },
  { id: "zfs_raidz3", nombre: "ZFS raidz3", corto: "raidz3" },
  { id: "storage_spaces", nombre: "Espacios de almacenamiento (Windows)", corto: "Storage Spaces" },
  { id: "nas", nombre: "NAS", corto: "NAS" },
  { id: "disco", nombre: "Disco suelto", corto: "disco suelto" },
  { id: "otro", nombre: "Otro", corto: "otro" },
];

export const ESTADOS_CONJUNTO: { id: NonNullable<ConjuntoDiscos["estado"]>; nombre: string }[] = [
  { id: "bien", nombre: "Bien" },
  { id: "degradado", nombre: "Degradado" },
  { id: "reconstruyendo", nombre: "Reconstruyendo" },
  { id: "sin_revisar", nombre: "Sin revisar" },
];

const MAX = { servidores: 100, conjuntos: 32, asignados: 2000 };
const idOk = (x: unknown): x is string => typeof x === "string" && /^[a-z0-9-]{1,40}$/.test(x);
const claveOk = (x: string) => /^[A-Za-z0-9:._-]{1,200}$/.test(x);
const textoDe = (v: unknown, max: number): string | null => {
  if (typeof v !== "string") return null;
  // eslint-disable-next-line no-control-regex
  const t = v.replace(/[\u0000-\u0008\u000b-\u001f\u007f]/g, "").trim().slice(0, max);
  return t || null;
};
const numero = (v: unknown, max: number): number | null => (typeof v === "number" && Number.isFinite(v) && v > 0 && v <= max ? Math.floor(v) : null);

/** Lo que llega del servidor, comprobado (lo que no cuadra, fuera). Un servidor anterior (404): vacía. */
export function leerInfra(x: unknown): Infraestructura {
  if (!x || typeof x !== "object") return INFRA_VACIA;
  const o = x as Record<string, unknown>;
  const servidores: ServidorFisico[] = [];
  for (const s of Array.isArray(o.servidores) ? o.servidores.slice(0, MAX.servidores) : []) {
    if (!s || typeof s !== "object") continue;
    const y = s as Record<string, unknown>;
    const nombre = textoDe(y.nombre, 80);
    if (!idOk(y.id) || !nombre || servidores.some((z) => z.id === y.id)) continue;
    const conjuntos: ConjuntoDiscos[] = [];
    for (const c of Array.isArray(y.conjuntos) ? y.conjuntos.slice(0, MAX.conjuntos) : []) {
      if (!c || typeof c !== "object") continue;
      const z = c as Record<string, unknown>;
      const n = textoDe(z.nombre, 80);
      if (!idOk(z.id) || !n || typeof z.tipo !== "string" || !TIPOS_CONJUNTO.some((t) => t.id === z.tipo) || conjuntos.some((w) => w.id === z.id)) continue;
      const estado = ESTADOS_CONJUNTO.find((e) => e.id === z.estado)?.id ?? null;
      conjuntos.push({ id: z.id, nombre: n, tipo: z.tipo, discos: numero(z.discos, 256), capacidad: numero(z.capacidad, 2 ** 60), estado, notas: textoDe(z.notas, 1000) });
    }
    servidores.push({
      id: y.id,
      nombre,
      marca: textoDe(y.marca, 80),
      modelo: textoDe(y.modelo, 120),
      sede: typeof y.sede === "string" && claveOk(y.sede) ? y.sede : null,
      hipervisor: HIPERVISORES.some((h) => h.id === y.hipervisor) ? (y.hipervisor as string) : null,
      notas: textoDe(y.notas, 1000),
      conjuntos,
    });
  }
  const sitios = (v: unknown): Record<string, Sitio> => {
    const out: Record<string, Sitio> = {};
    if (!v || typeof v !== "object") return out;
    for (const [k, s] of Object.entries(v as Record<string, unknown>).slice(0, MAX.asignados)) {
      if (!claveOk(k) || !s || typeof s !== "object") continue;
      const z = s as Record<string, unknown>;
      const srv = servidores.find((x) => x.id === z.servidor);
      if (!srv) continue;
      const conjunto = typeof z.conjunto === "string" && srv.conjuntos.some((c) => c.id === z.conjunto) ? z.conjunto : null;
      out[k] = { servidor: srv.id, ...(conjunto ? { conjunto } : {}) };
    }
    return out;
  };
  return {
    servidores,
    equipos: sitios(o.equipos),
    destinos: sitios(o.destinos),
    actualizada: textoDe(o.actualizada, 40),
    por: textoDe(o.por, 80),
  };
}

/** Lo que se manda al servidor (sin `actualizada` ni `por`, que pone él). */
export function paraGuardar(i: Infraestructura): Omit<Infraestructura, "actualizada" | "por"> {
  const limpio = (s: Sitio): Sitio => ({ servidor: s.servidor, ...(s.conjunto ? { conjunto: s.conjunto } : {}) });
  const quitarNulos = <T extends object>(o: T): T => Object.fromEntries(Object.entries(o).filter(([, v]) => v !== null && v !== undefined && v !== "")) as T;
  return {
    servidores: i.servidores.map((s) => ({ ...quitarNulos({ ...s, conjuntos: undefined }), conjuntos: s.conjuntos.map((c) => quitarNulos(c)) }) as ServidorFisico),
    equipos: Object.fromEntries(Object.entries(i.equipos).map(([k, s]) => [k, limpio(s)])),
    destinos: Object.fromEntries(Object.entries(i.destinos).map(([k, s]) => [k, limpio(s)])),
  };
}

/** ¿Hay algo anotado? */
export const conInfra = (i: Infraestructura | null | undefined) => !!i?.servidores.length;

export const servidorDe = (i: Infraestructura | null | undefined, id: string | null | undefined) => (id ? (i?.servidores.find((s) => s.id === id) ?? null) : null);
export const conjuntoDe = (s: ServidorFisico | null | undefined, id: string | null | undefined) => (id ? (s?.conjuntos.find((c) => c.id === id) ?? null) : null);

/** «espejo de 2 discos», «RAID 5 de 4 discos», «disco suelto», «Storage Spaces». */
export function formaConjunto(c: Pick<ConjuntoDiscos, "tipo" | "discos">): string {
  const t = TIPOS_CONJUNTO.find((x) => x.id === c.tipo)?.corto ?? c.tipo;
  if (c.tipo === "disco") return "disco suelto";
  return c.discos ? `${t} de ${c.discos} ${c.discos === 1 ? "disco" : "discos"}` : t;
}

/** «tank, espejo de 2 discos» (sin repetir el nombre si ya es la forma). */
export function textoConjunto(c: ConjuntoDiscos): string {
  const f = formaConjunto(c);
  return c.nombre.toLowerCase() === f.toLowerCase() ? f : `${c.nombre}, ${f}`;
}

/** El sitio de un equipo y de un destino, con su servidor y su conjunto ya buscados. */
export interface SitioVista {
  servidor: ServidorFisico;
  conjunto: ConjuntoDiscos | null;
  /** El sitio se lo dieron a este equipo o destino (y no a quien lo guarda). */
  propio: boolean;
}

function vista(i: Infraestructura | null | undefined, s: Sitio | undefined, propio: boolean): SitioVista | null {
  const servidor = servidorDe(i, s?.servidor);
  return servidor ? { servidor, conjunto: conjuntoDe(servidor, s?.conjunto), propio } : null;
}

/** Dónde está físicamente un equipo (su servidor y, si se dijo, su conjunto). */
export const sitioDeEquipo = (i: Infraestructura | null | undefined, equipo: string) => vista(i, i?.equipos[equipo], true);

/**
 * Dónde está físicamente un destino: el suyo si se anotó; si no, el del equipo que lo
 * guarda (una zona, el de su almacén; un disco local, el del equipo), sin conjunto.
 */
export function sitioDeDestino(i: Infraestructura | null | undefined, clave: string | null | undefined, equipo?: string | null): SitioVista | null {
  if (!i?.servidores.length) return null;
  const propio = clave ? vista(i, i.destinos[clave], true) : null;
  if (propio) return propio;
  const delEquipo = equipo ? i.equipos[equipo] : undefined;
  return delEquipo ? vista(i, { servidor: delEquipo.servidor }, false) : null;
}

/** «hv-01 · tank, espejo de 2 discos · Oficina central» (la línea de cada destino). */
export function lineaFisica(s: SitioVista | null, o?: Organizacion | null): string | null {
  if (!s) return null;
  const sede = sedePorId(o, s.servidor.sede)?.nombre;
  return [s.servidor.nombre, s.conjunto ? textoConjunto(s.conjunto) : null, sede].filter(Boolean).join(" · ");
}

// ---------- La regla 3-2-1-1-0 con los soportes reales ----------

/**
 * Para la regla: el servidor físico (`fisico`) y, si se sabe el conjunto de discos, el
 * soporte real (`conjunto:<servidor>:<conjunto>`). Sin nada anotado, `null`: la regla
 * cuenta como antes (equipo + disco).
 */
export function fisicoDe(s: SitioVista | null): { fisico: string; soporte: string | null } | null {
  return s ? { fisico: s.servidor.id, soporte: s.conjunto ? `conjunto:${s.servidor.id}:${s.conjunto.id}` : null } : null;
}

// ---------- Lo que propone la interfaz (con lo que ve el agente) ----------

export interface Propuesta {
  /** Clave estable (para no proponer dos veces lo mismo). */
  id: string;
  clase: "servidor" | "equipo" | "zona";
  /** La frase: «El almacén guarda en ZFS «tank» (1,1 TB, 65 % usado). ¿En qué servidor está?». */
  frase: string;
  equipo: Equipo;
  /** De una zona del almacén: su clave en el catálogo y el disco donde vive. */
  claveDestino?: string;
  disco?: DiscoEquipo | null;
  /** Un servidor físico nuevo ya armado con lo que ve el agente (equipo físico). */
  servidor?: ServidorFisico;
}

/** Un id corto a partir de un nombre («HV-01 (sala)» → «hv-01-sala»). */
export function idDe(nombre: string, usados: readonly string[] = []): string {
  const base =
    nombre
      .toLowerCase()
      .normalize("NFD")
      .replace(/[̀-ͯ]/g, "")
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "")
      .slice(0, 32) || "s";
  let id = base;
  for (let n = 2; usados.includes(id); n++) id = `${base}-${n}`.slice(0, 40);
  return id;
}

/** El tipo de conjunto de la consola para lo que ve el agente (`mdadm` raid1 → «raid1»). */
export function tipoDeConjunto(c: ConjuntoEquipo): string {
  if (c.tipo === "zfs") return c.forma === "mirror" ? "zfs_espejo" : "otro";
  if (c.tipo === "storage_spaces") return "storage_spaces";
  if (c.tipo === "mdadm" && c.forma && TIPOS_CONJUNTO.some((t) => t.id === c.forma)) return c.forma;
  return "otro";
}

/** «ZFS «tank»», «RAID md0 (raid1, 2 discos)», «el disco D: (NTFS)». */
export function textoDisco(d: DiscoEquipo, conjuntos: readonly ConjuntoEquipo[] = []): string {
  const c = d.volumen ? conjuntos.find((x) => x.id === d.volumen) : undefined;
  const fs = d.sistema_archivos?.toLowerCase() === "zfs" ? "ZFS" : d.sistema_archivos;
  if (fs === "ZFS" && d.volumen) return `ZFS «${d.volumen}»`;
  if (c?.tipo === "mdadm") return `RAID ${c.id}${c.forma ? ` (${c.forma}${c.discos ? `, ${c.discos} discos` : ""})` : ""}`;
  if (c?.tipo === "storage_spaces") return `Espacios de almacenamiento (${d.punto})`;
  return `el disco ${d.punto}${fs ? ` (${fs})` : ""}`;
}

const usado = (d: DiscoEquipo) => (d.total > 0 ? Math.round(((d.total - d.libre) / d.total) * 100) : 0);

/**
 * Lo que la interfaz propone anotar, con lo que ven los equipos y lo que aún no está
 * anotado: (1) un equipo físico con discos o conjuntos que no es de ningún servidor →
 * «es un servidor físico»; (2) una máquina virtual o un contenedor sin servidor →
 * «¿en qué servidor está?»; (3) cada zona de un almacén sin sitio → «guarda en … ¿en
 * qué servidor está?».
 */
export function propuestas(equipos: readonly Equipo[], i: Infraestructura | null | undefined): Propuesta[] {
  const out: Propuesta[] = [];
  const infra = i ?? INFRA_VACIA;
  for (const e of equipos) {
    if (e.modo === "trasladado") continue;
    const t = tipoEquipo(e);
    const db = datosBasicos(e);
    const sitio = infra.equipos[e.id];
    // Un equipo físico que parece un servidor: ve conjuntos de discos, varios discos o guarda copias
    // (un puesto con un solo disco no se propone: se puede anotar a mano).
    const variosDiscos = (db?.almacenamiento?.discos.filter((d) => !d.extraible).length ?? 0) >= 2;
    if (!sitio && t.clase === "fisico" && (db?.almacenamiento?.conjuntos.length || variosDiscos || e.resumen?.guarda_copias?.activo)) {
      const usados = infra.servidores.map((s) => s.id);
      const conjuntos = (db?.almacenamiento?.conjuntos ?? []).slice(0, 8).map((c, n) => ({
        id: idDe(c.id, []) || `c${n + 1}`,
        nombre: c.id,
        tipo: tipoDeConjunto(c),
        discos: c.discos ?? null,
        capacidad: c.total ?? null,
        estado: c.estado ?? null,
      }));
      const fisicos = db?.almacenamiento?.discos.filter((d) => !d.extraible && d.bus !== "spaces" && d.bus !== "raid").length ?? 0;
      out.push({
        id: `servidor:${e.id}`,
        clase: "servidor",
        frase: `${e.nombre} es un equipo físico${textoModelo(db) ? ` (${textoModelo(db)})` : ""}${fisicos ? ` con ${fisicos} ${fisicos === 1 ? "disco" : "discos"}` : ""}${conjuntos.length ? ` y ${conjuntos.length === 1 ? `el conjunto «${conjuntos[0].nombre}»` : `${conjuntos.length} conjuntos`}` : ""}. ¿Lo anotas como servidor?`,
        equipo: e,
        servidor: { id: idDe(e.nombre, usados), nombre: e.nombre, marca: db?.fabricante ?? null, modelo: db?.modelo ?? null, hipervisor: null, conjuntos },
      });
    } else if (!sitio && (t.clase === "virtual" || t.clase === "contenedor")) {
      out.push({
        id: `equipo:${e.id}`,
        clase: "equipo",
        frase: `${e.nombre} es ${t.clase === "virtual" ? "una máquina virtual" : "un contenedor"}. ¿En qué servidor físico está?`,
        equipo: e,
      });
    }
    // Las zonas de un almacén: dónde guardan (su disco, su conjunto) y en qué servidor.
    const g = e.resumen?.guarda_copias;
    if (!g?.activo) continue;
    for (const z of zonasDe(e)) {
      const clave = claveZona(e.id, z.id);
      if (infra.destinos[clave]) continue;
      const d = discoDeRuta(db, z.carpeta);
      const donde = d ? `${textoDisco(d, db?.almacenamiento?.conjuntos)} (${bytes(d.total)}, ${usado(d)} % usado)` : "su carpeta";
      const nombreZona = z.id === PRINCIPAL || !z.nombre ? "" : ` (${z.nombre})`;
      out.push({
        id: `zona:${clave}`,
        clase: "zona",
        frase: `El almacén ${e.nombre}${nombreZona} guarda en ${donde}. ¿En qué servidor está?`,
        equipo: e,
        claveDestino: clave,
        disco: d,
      });
    }
  }
  return out;
}

/** El disco local de un destino de un equipo (su carpeta, si el agente la dice). */
export const discoDeDestino = (e: Equipo, d: DestinoResumen | undefined) => (d?.tipo === "local" ? discoDeRuta(datosBasicos(e), d.carpeta) : null);
