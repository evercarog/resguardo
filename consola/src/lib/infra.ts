// Plan 0.8.1, C4 (plan-mejoras 15d; docs/infraestructura.md): infraestructura
// para cualquier entorno (equipo físico Windows o Linux, Hyper-V, VMware,
// Proxmox, NAS, nube). Lo que la persona anota (`GET/PUT
// /api/clientes/{c}/infraestructura`): los servidores físicos, sus discos uno a
// uno (0.8.2, A2), sus conjuntos de discos y su hipervisor, en qué servidor está
// cada equipo (una máquina virtual, un contenedor o el propio servidor, y desde la
// 0.8.2 cada uno de sus volúmenes) y en qué conjunto o disco vive cada destino o
// zona del almacén.
//
// 0.8.2 (A5): es un **dato común del cliente** (`infra:servidor:`, `infra:equipo:`,
// `infra:destino:`, docs/consolas-multiples.md §6.5): igual en todas sus consolas.
// `infraValida` comprueba un dato como el protocolo (los mismos vectores,
// crates/protocolo/vectors/datos-cliente.json, «infra»).
//
// **La interfaz propone y la persona confirma** (`propuestas`): lo que el agente ve
// solo (su tipo, sus discos y conjuntos, el sistema de archivos y el grupo de ZFS
// de cada zona) se ofrece en una frase con pocos campos. Resguardo no gestiona
// nada de esto: solo sabe dónde está cada cosa (la línea «Dónde está físicamente»,
// los discos de cada equipo en tres niveles, `discosEnNiveles`) y avisa en la
// regla 3-2-1-1-0 («servidor físico compartido», los soportes reales). Todo es
// opcional: sin nada anotado, todo como antes. Sin runas: se prueba en
// scripts/vectores-infra.ts y scripts/vectores-infra-082.ts.
import type { DestinoResumen, Equipo } from "./tipos";
import { bytes } from "./formato";
import { datosBasicos, discoDeRuta, textoModelo, usoDisco, type ConjuntoEquipo, type DiscoEquipo, type DiscoFisicoEquipo } from "./datosBasicos";
import { claveZona, PRINCIPAL, zonasDe } from "./destinos";
import { sedePorId, type Organizacion } from "./sedes";
import { tipoEquipo } from "./tipoEquipo";

export type TipoDisco = "ssd" | "hdd" | "nvme" | "otro";
export type EstadoDisco = "bien" | "aviso" | "averiado" | "sin_revisar";

/** 0.8.2 (A2, 15vv): un disco físico de un servidor. El número de serie lo escribe la persona. */
export interface DiscoFisico {
  id: string;
  nombre: string;
  tipo?: TipoDisco | null;
  modelo?: string | null;
  serie?: string | null;
  /** Bytes. */
  tamano?: number | null;
  /** Bahía o ranura («2», «M.2 de abajo»). */
  bahia?: string | null;
  estado?: EstadoDisco | null;
  notas?: string | null;
  /** Cómo lo llama el equipo que es este servidor («disco0», «sda»), si se sabe. */
  visto?: string | null;
}

export interface ConjuntoDiscos {
  id: string;
  nombre: string;
  /** Una de `TIPOS_CONJUNTO`. */
  tipo: string;
  /** Cuántos discos (lo de antes de la 0.8.2; con `miembros`, se cuentan). */
  discos?: number | null;
  /** 0.8.2 (A2): los discos del servidor que lo forman (sus ids). */
  miembros?: string[];
  /** Bytes. */
  capacidad?: number | null;
  estado?: "bien" | "degradado" | "reconstruyendo" | "sin_revisar" | null;
  notas?: string | null;
  /** 0.8.2 (A3): cómo lo llama el equipo que es este servidor («md0», «tank», «disco1»). */
  visto?: string | null;
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
  /** 0.8.2 (A2): sus discos físicos (un servidor de antes no los tiene). */
  discos: DiscoFisico[];
  conjuntos: ConjuntoDiscos[];
}

/** 0.8.2 (A3): dónde está un volumen de un equipo (C:, /srv): un conjunto o un disco de su servidor. */
export interface VolumenSitio {
  punto: string;
  conjunto?: string | null;
  disco?: string | null;
}

/** En qué servidor (y conjunto o disco) está un equipo o un destino. */
export interface Sitio {
  servidor: string;
  conjunto?: string | null;
  /** 0.8.2 (A2): un disco suelto del servidor (en vez de un conjunto). */
  disco?: string | null;
  /** 0.8.2 (A3): solo de un equipo, cada volumen en su sitio. */
  volumenes?: VolumenSitio[];
}

export interface Infraestructura {
  servidores: ServidorFisico[];
  /** Id del equipo → su sitio. */
  equipos: Record<string, Sitio>;
  /** Clave del destino (la del catálogo: `zona:<almacén>:<zona>`, el id de un destino…) → su sitio. */
  destinos: Record<string, Sitio>;
  actualizada?: string | null;
  por?: string | null;
  /** 0.8.2 (A5): algún equipo la guarda para todas las consolas del cliente. */
  comun?: boolean;
  /** 0.8.2 (A5): lo que ocupa en el documento de cada equipo (como mucho `MAX_BYTES_INFRA`). */
  bytes_comunes?: number | null;
}

export const INFRA_VACIA: Infraestructura = { servidores: [], equipos: {}, destinos: {} };
/** 0.8.2 (A5): como mucho, lo que ocupa la infraestructura en el documento de cada equipo (como el protocolo). */
export const MAX_BYTES_INFRA = 96 * 1024;

/** Los mismos que el protocolo (`datos_cliente::HIPERVISORES`). */
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

/** 0.8.2 (A2): el tipo de un disco físico (como `datos_cliente::TIPOS_DISCO`). */
export const TIPOS_DISCO: { id: TipoDisco; nombre: string; corto: string }[] = [
  { id: "ssd", nombre: "SSD", corto: "SSD" },
  { id: "hdd", nombre: "HDD (de platos)", corto: "HDD" },
  { id: "nvme", nombre: "NVMe", corto: "NVMe" },
  { id: "otro", nombre: "Otro", corto: "disco" },
];

/** 0.8.2 (A2): el estado anotado de un disco (lo dice la persona; Resguardo no lo comprueba). */
export const ESTADOS_DISCO: { id: EstadoDisco; nombre: string }[] = [
  { id: "bien", nombre: "Bien" },
  { id: "aviso", nombre: "Con avisos" },
  { id: "averiado", nombre: "Averiado" },
  { id: "sin_revisar", nombre: "Sin revisar" },
];

const MAX = { servidores: 100, conjuntos: 32, discos: 64, asignados: 2000, volumenes: 32 };
const idOk = (x: unknown): x is string => typeof x === "string" && /^[a-z0-9-]{1,40}$/.test(x);
const claveOk = (x: string) => /^[A-Za-z0-9:._-]{1,200}$/.test(x) && !x.split(":").some((p) => p === "" || p === "." || p === "..");
const textoDe = (v: unknown, max: number): string | null => {
  if (typeof v !== "string") return null;
  // eslint-disable-next-line no-control-regex
  const t = v.replace(/[\u0000-\u0008\u000b-\u001f\u007f]/g, "").trim().slice(0, max);
  return t || null;
};
const numero = (v: unknown, max: number): number | null => (typeof v === "number" && Number.isFinite(v) && v > 0 && v <= max ? Math.floor(v) : null);

function leerDisco(x: unknown): DiscoFisico | null {
  if (!x || typeof x !== "object") return null;
  const z = x as Record<string, unknown>;
  const nombre = textoDe(z.nombre, 80);
  if (!idOk(z.id) || !nombre) return null;
  return {
    id: z.id,
    nombre,
    tipo: TIPOS_DISCO.find((t) => t.id === z.tipo)?.id ?? null,
    modelo: textoDe(z.modelo, 120),
    serie: textoDe(z.serie, 64),
    tamano: numero(z.tamano, 2 ** 60),
    bahia: textoDe(z.bahia, 40),
    estado: ESTADOS_DISCO.find((e) => e.id === z.estado)?.id ?? null,
    notas: textoDe(z.notas, 500),
    visto: textoDe(z.visto, 64),
  };
}

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
    const discos: DiscoFisico[] = [];
    for (const d of Array.isArray(y.discos) ? y.discos.slice(0, MAX.discos) : []) {
      const k = leerDisco(d);
      if (k && !discos.some((w) => w.id === k.id)) discos.push(k);
    }
    const conjuntos: ConjuntoDiscos[] = [];
    for (const c of Array.isArray(y.conjuntos) ? y.conjuntos.slice(0, MAX.conjuntos) : []) {
      if (!c || typeof c !== "object") continue;
      const z = c as Record<string, unknown>;
      const n = textoDe(z.nombre, 80);
      if (!idOk(z.id) || !n || typeof z.tipo !== "string" || !TIPOS_CONJUNTO.some((t) => t.id === z.tipo) || conjuntos.some((w) => w.id === z.id)) continue;
      const estado = ESTADOS_CONJUNTO.find((e) => e.id === z.estado)?.id ?? null;
      const miembros = (Array.isArray(z.miembros) ? z.miembros : []).filter((m, i, xs): m is string => typeof m === "string" && discos.some((d) => d.id === m) && xs.indexOf(m) === i);
      conjuntos.push({ id: z.id, nombre: n, tipo: z.tipo, discos: numero(z.discos, 256), ...(miembros.length ? { miembros } : {}), capacidad: numero(z.capacidad, 2 ** 60), estado, notas: textoDe(z.notas, 1000), visto: textoDe(z.visto, 64) });
    }
    servidores.push({
      id: y.id,
      nombre,
      marca: textoDe(y.marca, 80),
      modelo: textoDe(y.modelo, 120),
      sede: typeof y.sede === "string" && claveOk(y.sede) ? y.sede : null,
      hipervisor: HIPERVISORES.some((h) => h.id === y.hipervisor) ? (y.hipervisor as string) : null,
      notas: textoDe(y.notas, 1000),
      discos,
      conjuntos,
    });
  }
  const sitios = (v: unknown, conVolumenes: boolean): Record<string, Sitio> => {
    const out: Record<string, Sitio> = {};
    if (!v || typeof v !== "object") return out;
    for (const [k, s] of Object.entries(v as Record<string, unknown>).slice(0, MAX.asignados)) {
      if (!claveOk(k) || !s || typeof s !== "object") continue;
      const z = s as Record<string, unknown>;
      const srv = servidores.find((x) => x.id === z.servidor);
      if (!srv) continue;
      const donde = (w: Record<string, unknown>) => {
        const conjunto = typeof w.conjunto === "string" && srv.conjuntos.some((c) => c.id === w.conjunto) ? w.conjunto : null;
        const disco = !conjunto && typeof w.disco === "string" && srv.discos.some((d) => d.id === w.disco) ? w.disco : null;
        return { ...(conjunto ? { conjunto } : {}), ...(disco ? { disco } : {}) };
      };
      const volumenes: VolumenSitio[] = [];
      if (conVolumenes)
        for (const w of Array.isArray(z.volumenes) ? z.volumenes.slice(0, MAX.volumenes) : []) {
          if (!w || typeof w !== "object") continue;
          const punto = textoDe((w as Record<string, unknown>).punto, 64);
          const d = donde(w as Record<string, unknown>);
          if (punto && (d.conjunto || d.disco) && !volumenes.some((x) => x.punto === punto)) volumenes.push({ punto, ...d });
        }
      out[k] = { servidor: srv.id, ...donde(z), ...(volumenes.length ? { volumenes } : {}) };
    }
    return out;
  };
  return {
    servidores,
    equipos: sitios(o.equipos, true),
    destinos: sitios(o.destinos, false),
    actualizada: textoDe(o.actualizada, 40),
    por: textoDe(o.por, 80),
    comun: o.comun === true,
    bytes_comunes: typeof o.bytes_comunes === "number" && o.bytes_comunes >= 0 ? o.bytes_comunes : null,
  };
}

const quitarNulos = <T extends object>(o: T): T => Object.fromEntries(Object.entries(o).filter(([, v]) => v !== null && v !== undefined && v !== "" && !(Array.isArray(v) && !v.length))) as T;

/** Lo que se manda al servidor (sin `actualizada`, `por`, `comun` ni `bytes_comunes`, que pone él). */
export function paraGuardar(i: Infraestructura): Pick<Infraestructura, "servidores" | "equipos" | "destinos"> {
  const limpio = (s: Sitio): Sitio =>
    quitarNulos({ servidor: s.servidor, conjunto: s.conjunto ?? null, disco: s.conjunto ? null : (s.disco ?? null), volumenes: (s.volumenes ?? []).filter((v) => v.conjunto || v.disco).map((v) => quitarNulos({ punto: v.punto, conjunto: v.conjunto ?? null, disco: v.conjunto ? null : (v.disco ?? null) })) });
  return {
    servidores: i.servidores.map((s) => quitarNulos({ ...s, discos: s.discos.map((d) => quitarNulos({ ...d })), conjuntos: s.conjuntos.map((c) => quitarNulos({ ...c })) }) as ServidorFisico),
    equipos: Object.fromEntries(Object.entries(i.equipos).map(([k, s]) => [k, limpio(s)])),
    destinos: Object.fromEntries(Object.entries(i.destinos).map(([k, s]) => [k, { ...limpio(s), volumenes: undefined }])),
  };
}

/** ¿Hay algo anotado? */
export const conInfra = (i: Infraestructura | null | undefined) => !!i?.servidores.length;

export const servidorDe = (i: Infraestructura | null | undefined, id: string | null | undefined) => (id ? (i?.servidores.find((s) => s.id === id) ?? null) : null);
export const conjuntoDe = (s: ServidorFisico | null | undefined, id: string | null | undefined) => (id ? (s?.conjuntos.find((c) => c.id === id) ?? null) : null);
export const discoDe = (s: ServidorFisico | null | undefined, id: string | null | undefined) => (id ? (s?.discos.find((d) => d.id === id) ?? null) : null);

/** Cuántos discos tiene un conjunto: los suyos si se dijeron; si no, el número anotado. */
export const discosDeConjunto = (c: Pick<ConjuntoDiscos, "discos" | "miembros">) => c.miembros?.length || c.discos || null;

/** «espejo de 2 discos», «RAID 5 de 4 discos», «disco suelto», «Storage Spaces». */
export function formaConjunto(c: Pick<ConjuntoDiscos, "tipo" | "discos" | "miembros">): string {
  const t = TIPOS_CONJUNTO.find((x) => x.id === c.tipo)?.corto ?? c.tipo;
  if (c.tipo === "disco") return "disco suelto";
  const n = discosDeConjunto(c);
  return n ? `${t} de ${n} ${n === 1 ? "disco" : "discos"}` : t;
}

/** «tank, espejo de 2 discos» (sin repetir el nombre si ya es la forma). */
export function textoConjunto(c: ConjuntoDiscos): string {
  const f = formaConjunto(c);
  return c.nombre.toLowerCase() === f.toLowerCase() ? f : `${c.nombre}, ${f}`;
}

/** 0.8.2 (A2): «HDD 4 TB · Modelo X» (lo que se sabe de un disco físico, sin su nombre). */
export function detalleDisco(d: Pick<DiscoFisico, "tipo" | "tamano" | "modelo" | "bahia">): string {
  const tipo = d.tipo ? (TIPOS_DISCO.find((t) => t.id === d.tipo)?.corto ?? "") : "";
  const tam = d.tamano ? bytes(d.tamano) : "";
  return [[tipo === "disco" && tam ? "" : tipo, tam].filter(Boolean).join(" "), d.modelo, d.bahia ? `bahía ${d.bahia}` : null].filter(Boolean).join(" · ");
}

/** 0.8.2 (A2): «Bahía 2 + Bahía 3» (los discos de un conjunto, por su nombre). */
export function miembrosDe(s: ServidorFisico | null | undefined, c: Pick<ConjuntoDiscos, "miembros">): string {
  return (c.miembros ?? []).map((m) => discoDe(s, m)?.nombre).filter(Boolean).join(" + ");
}

/** El sitio de un equipo y de un destino, con su servidor y su conjunto (o disco) ya buscados. */
export interface SitioVista {
  servidor: ServidorFisico;
  conjunto: ConjuntoDiscos | null;
  /** 0.8.2 (A2): un disco suelto del servidor. */
  disco?: DiscoFisico | null;
  /** El sitio se lo dieron a este equipo o destino (y no a quien lo guarda). */
  propio: boolean;
}

function vista(i: Infraestructura | null | undefined, s: Pick<Sitio, "servidor" | "conjunto" | "disco"> | undefined, propio: boolean): SitioVista | null {
  const servidor = servidorDe(i, s?.servidor);
  if (!servidor) return null;
  const conjunto = conjuntoDe(servidor, s?.conjunto);
  return { servidor, conjunto, disco: conjunto ? null : discoDe(servidor, s?.disco), propio };
}

/** Dónde está físicamente un equipo (su servidor y, si se dijo, su conjunto o disco). */
export const sitioDeEquipo = (i: Infraestructura | null | undefined, equipo: string) => vista(i, i?.equipos[equipo], true);

/**
 * 0.8.2 (A3, 15ww): dónde está un volumen de un equipo: el suyo si se anotó; si no, el del
 * equipo; si no, el disco o conjunto del servidor que el equipo ve con ese nombre (el propio
 * servidor físico: `visto`).
 */
export function sitioDeVolumen(i: Infraestructura | null | undefined, equipo: string, punto: string, volumen?: string | null): SitioVista | null {
  const s = i?.equipos[equipo];
  if (!s) return null;
  const v = s.volumenes?.find((x) => x.punto === punto);
  if (v) return vista(i, { servidor: s.servidor, conjunto: v.conjunto, disco: v.disco }, true);
  const srv = servidorDe(i, s.servidor);
  if (srv && volumen) {
    const c = srv.conjuntos.find((x) => x.visto === volumen);
    const d = c ? null : srv.discos.find((x) => x.visto === volumen);
    if (c || d) return { servidor: srv, conjunto: c ?? null, disco: d ?? null, propio: true };
  }
  return vista(i, s, true);
}

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

/** «tank, espejo de 2 discos» o «Bahía 2» (el conjunto o el disco de un sitio). */
export const textoDondeEnServidor = (s: SitioVista | null) => (s?.conjunto ? textoConjunto(s.conjunto) : s?.disco ? s.disco.nombre : null);

/** «hv-01 · tank, espejo de 2 discos · Oficina central» (la línea de cada destino). */
export function lineaFisica(s: SitioVista | null, o?: Organizacion | null): string | null {
  if (!s) return null;
  const sede = sedePorId(o, s.servidor.sede)?.nombre;
  return [s.servidor.nombre, textoDondeEnServidor(s), sede].filter(Boolean).join(" · ");
}

// ---------- La regla 3-2-1-1-0 con los soportes reales ----------

/**
 * Para la regla: el servidor físico (`fisico`) y, si se sabe el conjunto (o el disco), el
 * soporte real (`conjunto:<servidor>:<conjunto>`, `disco:<servidor>:<disco>`). Sin nada
 * anotado, `null`: la regla cuenta como antes (equipo + disco).
 */
export function fisicoDe(s: SitioVista | null): { fisico: string; soporte: string | null } | null {
  if (!s) return null;
  return { fisico: s.servidor.id, soporte: s.conjunto ? `conjunto:${s.servidor.id}:${s.conjunto.id}` : s.disco ? `disco:${s.servidor.id}:${s.disco.id}` : null };
}

// ---------- 0.8.2 (A5): cada parte como dato común, comprobada como el protocolo ----------

type Resultado = { ok: true; limpio: unknown } | { ok: false; error: string };

/** Un texto opcional: ausente, null o vacío → nada; si no, de 1 a `max` y sin caracteres de control (con `lineas`, saltos sí). */
function textoOpc(v: unknown, max: number, lineas = false): { ok: boolean; v?: string } {
  if (v === undefined || v === null) return { ok: true };
  if (typeof v !== "string") return { ok: false };
  const t = v.trim();
  if (!t) return { ok: true };
  // eslint-disable-next-line no-control-regex
  const raro = lineas ? /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f]/ : /[\u0000-\u001f\u007f-\u009f]/;
  return [...t].length <= max && !raro.test(t) ? { ok: true, v: t } : { ok: false };
}
const soloCampos = (o: Record<string, unknown>, ok: string[]) => Object.keys(o).every((k) => ok.includes(k));
const enLista = (v: unknown, xs: readonly string[]) => (v === undefined || v === null || v === "" ? { ok: true } : typeof v === "string" && xs.includes(v) ? { ok: true, v } : { ok: false });
const numOpc = (v: unknown, min: number, max: number) => (v === undefined || v === null ? { ok: true } : typeof v === "number" && Number.isInteger(v) && v >= min && v <= max ? { ok: true, v } : { ok: false });

function poner(out: Record<string, unknown>, campo: string, r: { ok: boolean; v?: unknown }): boolean {
  if (!r.ok) return false;
  if (r.v !== undefined) out[campo] = r.v;
  return true;
}

function discoValido(x: unknown): Record<string, unknown> | string {
  if (!x || typeof x !== "object" || Array.isArray(x)) return "Disco no válido.";
  const m = x as Record<string, unknown>;
  if (!soloCampos(m, ["id", "nombre", "tipo", "modelo", "serie", "tamano", "bahia", "estado", "notas", "visto"])) return "Campos de más en el valor.";
  if (!idOk(m.id)) return "Un disco tiene un id no válido.";
  const nombre = textoOpc(m.nombre, 80);
  if (!nombre.v) return "Cada disco necesita un nombre (hasta 80 caracteres).";
  const v: Record<string, unknown> = { id: m.id, nombre: nombre.v };
  const ok =
    poner(v, "tipo", enLista(m.tipo, TIPOS_DISCO.map((t) => t.id))) &&
    poner(v, "modelo", textoOpc(m.modelo, 120)) &&
    poner(v, "serie", textoOpc(m.serie, 64)) &&
    poner(v, "tamano", numOpc(m.tamano, 1, 2 ** 60)) &&
    poner(v, "bahia", textoOpc(m.bahia, 40)) &&
    poner(v, "estado", enLista(m.estado, ESTADOS_DISCO.map((e) => e.id))) &&
    poner(v, "notas", textoOpc(m.notas, 500, true)) &&
    poner(v, "visto", textoOpc(m.visto, 64));
  return ok ? v : "Un dato de un disco no es válido.";
}

function conjuntoValido(x: unknown, discos: string[]): Record<string, unknown> | string {
  if (!x || typeof x !== "object" || Array.isArray(x)) return "Conjunto no válido.";
  const m = x as Record<string, unknown>;
  if (!soloCampos(m, ["id", "nombre", "tipo", "discos", "miembros", "capacidad", "estado", "notas", "visto"])) return "Campos de más en el valor.";
  if (!idOk(m.id)) return "Un conjunto tiene un id no válido.";
  const nombre = textoOpc(m.nombre, 80);
  if (!nombre.v) return "Cada conjunto de discos necesita un nombre.";
  if (typeof m.tipo !== "string" || !TIPOS_CONJUNTO.some((t) => t.id === m.tipo)) return "Ese tipo de conjunto de discos no se conoce.";
  const v: Record<string, unknown> = { id: m.id, nombre: nombre.v, tipo: m.tipo };
  if (!poner(v, "discos", numOpc(m.discos, 1, 256))) return "Un conjunto tiene de 1 a 256 discos.";
  if (m.miembros !== undefined && m.miembros !== null) {
    if (!Array.isArray(m.miembros) || m.miembros.length > MAX.discos) return "Los discos de un conjunto van en una lista.";
    const out: string[] = [];
    for (const d of m.miembros) {
      if (typeof d !== "string" || !discos.includes(d)) return "Un conjunto usa un disco que no es de su servidor.";
      if (out.includes(d)) return "Un conjunto repite un disco.";
      out.push(d);
    }
    if (out.length) v.miembros = out;
  }
  const ok =
    poner(v, "capacidad", numOpc(m.capacidad, 1, 2 ** 60)) &&
    poner(v, "estado", enLista(m.estado, ESTADOS_CONJUNTO.map((e) => e.id))) &&
    poner(v, "notas", textoOpc(m.notas, 1000, true)) &&
    poner(v, "visto", textoOpc(m.visto, 64));
  return ok ? v : "Un dato de un conjunto no es válido.";
}

function dondeValido(m: Record<string, unknown>, v: Record<string, unknown>): string | null {
  const id = (x: unknown) => (x === undefined || x === null || x === "" ? { ok: true } : idOk(x) ? { ok: true, v: x } : { ok: false });
  const c = id(m.conjunto);
  const d = id(m.disco);
  if (!c.ok || !d.ok) return "Un conjunto o un disco con un id no válido.";
  if (c.v && d.v) return "O un conjunto de discos o un disco, no los dos.";
  poner(v, "conjunto", c);
  poner(v, "disco", d);
  return null;
}

/**
 * 0.8.2 (A5): un dato `infra:` comprobado y limpio como el protocolo (`valor_valido`).
 * `null` siempre vale (quitarlo).
 */
export function infraValida(clave: string, valor: unknown): Resultado {
  const m = /^infra:(servidor|equipo|destino):(.+)$/.exec(clave);
  if (!m) return { ok: false, error: "Dato común desconocido." };
  const [, tipo, sujeto] = m;
  const sujetoOk = tipo === "servidor" ? idOk(sujeto) : tipo === "equipo" ? /^[A-Za-z0-9_.:@-]{1,128}$/.test(sujeto) && sujeto !== "." && sujeto !== ".." : claveOk(sujeto);
  if (!sujetoOk) return { ok: false, error: "Dato común desconocido." };
  if (valor === null) return { ok: true, limpio: null };
  if (!valor || typeof valor !== "object" || Array.isArray(valor)) return { ok: false, error: "Valor no válido." };
  const o = valor as Record<string, unknown>;
  if (tipo === "servidor") {
    if (!soloCampos(o, ["nombre", "marca", "modelo", "sede", "hipervisor", "notas", "discos", "conjuntos"])) return { ok: false, error: "Campos de más en el valor." };
    const nombre = textoOpc(o.nombre, 80);
    if (!nombre.v) return { ok: false, error: "Cada servidor necesita un nombre." };
    const v: Record<string, unknown> = { nombre: nombre.v };
    const sede = o.sede === undefined || o.sede === null || o.sede === "" ? { ok: true } : typeof o.sede === "string" && /^[A-Za-z0-9_-]{1,64}$/.test(o.sede) ? { ok: true, v: o.sede } : { ok: false };
    const ok =
      poner(v, "marca", textoOpc(o.marca, 80)) &&
      poner(v, "modelo", textoOpc(o.modelo, 120)) &&
      poner(v, "sede", sede) &&
      poner(v, "hipervisor", enLista(o.hipervisor, HIPERVISORES.map((h) => h.id))) &&
      poner(v, "notas", textoOpc(o.notas, 1000, true));
    if (!ok) return { ok: false, error: "Un dato del servidor no es válido." };
    const lista = (x: unknown, max: number) => (x === undefined || x === null ? [] : Array.isArray(x) && x.length <= max ? x : null);
    const ds = lista(o.discos, MAX.discos);
    const cs = lista(o.conjuntos, MAX.conjuntos);
    if (!ds || !cs) return { ok: false, error: "Demasiados discos o conjuntos en un servidor." };
    const discos: Record<string, unknown>[] = [];
    for (const d of ds) {
      const x = discoValido(d);
      if (typeof x === "string") return { ok: false, error: x };
      if (discos.some((y) => y.id === x.id)) return { ok: false, error: "Hay dos discos con el mismo id." };
      discos.push(x);
    }
    const conjuntos: Record<string, unknown>[] = [];
    for (const c of cs) {
      const x = conjuntoValido(
        c,
        discos.map((d) => d.id as string),
      );
      if (typeof x === "string") return { ok: false, error: x };
      if (conjuntos.some((y) => y.id === x.id)) return { ok: false, error: "Hay dos conjuntos con el mismo id." };
      conjuntos.push(x);
    }
    if (discos.length) v.discos = discos;
    if (conjuntos.length) v.conjuntos = conjuntos;
    if (new TextEncoder().encode(JSON.stringify(v)).length > 16 * 1024) return { ok: false, error: "Un servidor ocupa demasiado (hasta 16 KiB)." };
    return { ok: true, limpio: v };
  }
  const campos = tipo === "equipo" ? ["servidor", "conjunto", "disco", "volumenes"] : ["servidor", "conjunto", "disco"];
  if (!soloCampos(o, campos)) return { ok: false, error: "Campos de más en el valor." };
  if (!idOk(o.servidor)) return { ok: false, error: "Falta el servidor físico (o no es válido)." };
  const v: Record<string, unknown> = { servidor: o.servidor };
  const e = dondeValido(o, v);
  if (e) return { ok: false, error: e };
  if (o.volumenes !== undefined && o.volumenes !== null) {
    if (!Array.isArray(o.volumenes) || o.volumenes.length > MAX.volumenes) return { ok: false, error: "Como mucho 32 volúmenes anotados por equipo." };
    const out: Record<string, unknown>[] = [];
    for (const y of o.volumenes) {
      if (!y || typeof y !== "object" || Array.isArray(y) || !soloCampos(y as Record<string, unknown>, ["punto", "conjunto", "disco"])) return { ok: false, error: "Volumen no válido." };
      const punto = textoOpc((y as Record<string, unknown>).punto, 64);
      if (!punto.ok || !punto.v) return { ok: false, error: "Un volumen sin nombre (C:, /srv…)." };
      if (out.some((w) => w.punto === punto.v)) return { ok: false, error: "Un volumen está dos veces." };
      const w: Record<string, unknown> = { punto: punto.v };
      const e2 = dondeValido(y as Record<string, unknown>, w);
      if (e2) return { ok: false, error: e2 };
      if (Object.keys(w).length < 2) return { ok: false, error: "Cada volumen anotado va en un conjunto o en un disco." };
      out.push(w);
    }
    if (out.length) v.volumenes = out;
  }
  return { ok: true, limpio: v };
}

/** 0.8.2 (A5): los datos comunes de una infraestructura (clave → valor), como `crate::infraestructura::datos`. */
export function datosInfra(i: Pick<Infraestructura, "servidores" | "equipos" | "destinos">): Record<string, unknown> {
  const g = paraGuardar({ ...INFRA_VACIA, ...i });
  const out: Record<string, unknown> = {};
  for (const s of g.servidores) {
    const { id, ...resto } = s;
    out[`infra:servidor:${id}`] = resto;
  }
  for (const [k, s] of Object.entries(g.equipos)) out[`infra:equipo:${k}`] = s;
  for (const [k, s] of Object.entries(g.destinos)) out[`infra:destino:${k}`] = quitarNulos(s as unknown as Record<string, unknown>);
  return out;
}

/** 0.8.2 (A5): lo que ocupará en el documento de cada equipo (aproximado, como `datos_cliente::bytes_dato`). */
export function bytesInfra(i: Pick<Infraestructura, "servidores" | "equipos" | "destinos">): number {
  const enc = new TextEncoder();
  return Object.entries(datosInfra(i)).reduce((n, [k, v]) => n + k.length + enc.encode(JSON.stringify(v)).length + 260, 0);
}

// ---------- Lo que propone la interfaz (con lo que ve el agente) ----------

export interface Propuesta {
  /** Clave estable (para no proponer dos veces lo mismo). */
  id: string;
  clase: "servidor" | "equipo" | "zona" | "discos";
  /** La frase: «El almacén guarda en ZFS «tank» (1,1 TB, 65 % usado). ¿En qué servidor está?». */
  frase: string;
  equipo: Equipo;
  /** De una zona del almacén: su clave en el catálogo y el disco donde vive. */
  claveDestino?: string;
  disco?: DiscoEquipo | null;
  /** Un servidor físico nuevo ya armado con lo que ve el agente (equipo físico). */
  servidor?: ServidorFisico;
  /** 0.8.2 (A2): de «discos»: el servidor ya anotado y los discos que el agente ve y aún no están en él. */
  enServidor?: string;
  discosNuevos?: DiscoFisico[];
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

/** ¿Es un disco de verdad (y no el disco virtual de una controladora RAID o de Storage Spaces)? */
const esFisico = (d: DiscoFisicoEquipo) => !d.extraible && d.bus !== "spaces" && d.bus !== "raid";

/** 0.8.2 (A2): el tipo de un disco que ve el agente («nvme» por su bus; si no, SSD o HDD). */
export const tipoDeDisco = (d: DiscoFisicoEquipo): TipoDisco | null => (d.bus === "nvme" ? "nvme" : d.clase === "ssd" ? "ssd" : d.clase === "hdd" ? "hdd" : null);

/** 0.8.2 (A2, 15vv): los discos que ve el agente, como discos del servidor (sin número de serie: lo escribe la persona). */
export function discosPropuestos(e: Equipo, usados: readonly string[] = []): DiscoFisico[] {
  const a = datosBasicos(e)?.almacenamiento;
  const ids = [...usados];
  return (a?.discos ?? []).filter(esFisico).map((d, n) => {
    const id = idDe(`disco-${n + 1}`, ids);
    ids.push(id);
    const tipo = tipoDeDisco(d);
    return { id, nombre: d.modelo ? `${d.modelo}` : `Disco ${n + 1}`, tipo, modelo: d.modelo ?? null, tamano: d.total || null, visto: d.id };
  });
}

/**
 * Lo que la interfaz propone anotar, con lo que ven los equipos y lo que aún no está
 * anotado: (1) un equipo físico con discos o conjuntos que no es de ningún servidor →
 * «es un servidor físico»; (2) una máquina virtual o un contenedor sin servidor →
 * «¿en qué servidor está?»; (3) cada zona de un almacén sin sitio → «guarda en … ¿en
 * qué servidor está?»; (4) 0.8.2 (A2): un equipo físico ya anotado como su servidor que
 * ve discos que aún no están en él → «¿los añades?».
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
        visto: c.id,
      }));
      const discos = discosPropuestos(e);
      const fisicos = db?.almacenamiento?.discos.filter(esFisico).length ?? 0;
      out.push({
        id: `servidor:${e.id}`,
        clase: "servidor",
        frase: `${e.nombre} es un equipo físico${textoModelo(db) ? ` (${textoModelo(db)})` : ""}${fisicos ? ` con ${fisicos} ${fisicos === 1 ? "disco" : "discos"}` : ""}${conjuntos.length ? ` y ${conjuntos.length === 1 ? `el conjunto «${conjuntos[0].nombre}»` : `${conjuntos.length} conjuntos`}` : ""}. ¿Lo anotas como servidor?`,
        equipo: e,
        servidor: { id: idDe(e.nombre, usados), nombre: e.nombre, marca: db?.fabricante ?? null, modelo: db?.modelo ?? null, hipervisor: null, discos, conjuntos },
      });
    } else if (!sitio && (t.clase === "virtual" || t.clase === "contenedor")) {
      out.push({
        id: `equipo:${e.id}`,
        clase: "equipo",
        frase: `${e.nombre} es ${t.clase === "virtual" ? "una máquina virtual" : "un contenedor"}. ¿En qué servidor físico está?`,
        equipo: e,
      });
    } else if (sitio && t.clase === "fisico") {
      // 0.8.2 (A2): el propio servidor físico: los discos que ve y aún no están anotados en él.
      const srv = servidorDe(infra, sitio.servidor);
      const vistos = new Set(srv?.discos.map((d) => d.visto).filter(Boolean));
      const nuevos = srv ? discosPropuestos(e, srv.discos.map((d) => d.id)).filter((d) => !vistos.has(d.visto)) : [];
      if (srv && nuevos.length && srv.discos.length + nuevos.length <= MAX.discos)
        out.push({
          id: `discos:${e.id}:${nuevos.map((d) => d.visto).join(",")}`,
          clase: "discos",
          frase: `${e.nombre} ve ${nuevos.length === 1 ? "1 disco que no está anotado" : `${nuevos.length} discos que no están anotados`} en «${srv.nombre}» (${nuevos.map((d) => detalleDisco(d) || d.nombre).join("; ")}). ¿${nuevos.length === 1 ? "Lo añades" : "Los añades"}? El número de serie lo escribes tú.`,
          equipo: e,
          enServidor: srv.id,
          discosNuevos: nuevos,
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

// ---------- 0.8.2 (A3, 15ww): los discos de un equipo en tres niveles ----------

/** Un volumen (C:, D:, /srv) con lo que guarda y dónde está físicamente. */
export interface VolumenVista {
  punto: string;
  etiqueta: string | null;
  sistemaArchivos: string | null;
  total: number;
  libre: number;
  usado: number;
  tono: "ok" | "warn" | "bad";
  /** Lo que guarda: «con copias», «zona «Disco D» del almacén», «espejo «USB»», «destino «Copias en D»». */
  guarda: string[];
  /** «hv-01 · Bahía 2» (solo si se anotó algo distinto de lo del equipo, o el equipo es una VM). */
  fisico: string | null;
}

/** Un disco físico, un conjunto o un disco virtual, con lo que cuelga de él. */
export interface NodoDisco {
  clave: string;
  nivel: "fisico" | "conjunto" | "virtual";
  /** El nombre puesto en Infraestructura si lo hay; si no, lo que ve el agente («SSD 240 GB»). */
  titulo: string;
  /** «SSD · 240 GB · Modelo X», «RAID 1 de 2 discos · 4 TB». */
  detalle: string;
  /** El estado (anotado o el que ve el agente). */
  estado: string | null;
  tono: "ok" | "warn" | "bad" | null;
  /** De un conjunto: sus discos («Bahía 2 · HDD 4 TB»). */
  miembros: { titulo: string; detalle: string }[];
  volumenes: VolumenVista[];
  /** El nombre lo puso una persona en Infraestructura. */
  anotado: boolean;
}

export interface DiscosEnNiveles {
  nodos: NodoDisco[];
  /** Volúmenes que no se sabe en qué disco están. */
  sueltos: VolumenVista[];
  /** Una máquina virtual o un contenedor: su anfitrión anotado («hv-01 · tank, espejo de 2 discos»). */
  anfitrion: string | null;
  /** Algún volumen tiene su sitio anotado aparte. */
  porVolumen: boolean;
}

const ESTADO_DISCO_TEXTO: Record<string, { texto: string; tono: "ok" | "warn" | "bad" }> = {
  bien: { texto: "Bien", tono: "ok" },
  aviso: { texto: "Con avisos", tono: "warn" },
  averiado: { texto: "Averiado", tono: "bad" },
  degradado: { texto: "Degradado", tono: "warn" },
  reconstruyendo: { texto: "Reconstruyendo", tono: "warn" },
};

/** Lo que guarda un volumen del equipo (con copias, zonas del almacén, espejos, destinos locales). */
export function loQueGuarda(e: Equipo, d: DiscoEquipo): string[] {
  const db = datosBasicos(e);
  const aqui = (ruta: string | null | undefined) => !!ruta && discoDeRuta(db, ruta)?.punto === d.punto;
  const out: string[] = [];
  const g = e.resumen?.guarda_copias;
  if (g?.activo) for (const z of zonasDe(e)) if (aqui(z.carpeta)) out.push(z.id === PRINCIPAL || !z.nombre ? "el almacén" : `zona «${z.nombre}» del almacén`);
  for (const x of g?.espejo?.destinos ?? []) if (x.tipo === "carpeta" && aqui(x.carpeta)) out.push(`espejo en ${x.carpeta}`);
  for (const t of e.resumen?.espejo_equipo?.trabajos ?? []) if (t.adonde?.tipo === "carpeta" && aqui(t.adonde.carpeta)) out.push(`espejo «${t.nombre}»`);
  for (const x of e.resumen?.destinos ?? []) if (x.tipo === "local" && !x.red && aqui(x.carpeta)) out.push(`destino «${x.nombre}»`);
  if (d.en_uso && !out.length) out.push("con copias");
  return [...new Set(out)];
}

/**
 * Los discos del equipo en tres niveles (A3, 15ww): discos físicos (con el nombre de
 * Infraestructura si está anotado; si no, su modelo, tipo y tamaño), conjuntos o discos
 * virtuales (con sus discos) y, colgando de cada uno, sus volúmenes con lo que guardan.
 * Nunca «disco0»: ese es el número interno de Windows.
 */
export function discosEnNiveles(e: Equipo, i: Infraestructura | null | undefined, o?: Organizacion | null): DiscosEnNiveles {
  const db = datosBasicos(e);
  const a = db?.almacenamiento;
  const t = tipoEquipo(e);
  const sitio = i?.equipos[e.id];
  const srv = servidorDe(i, sitio?.servidor);
  // Solo el propio servidor físico ve sus discos por su nombre (`visto`); una VM, no.
  const propio = t.clase !== "virtual" && t.clase !== "contenedor" ? srv : null;
  const nodos: NodoDisco[] = [];
  const porId = new Map<string, NodoDisco>();
  const anotadoDisco = (visto: string) => propio?.discos.find((d) => d.visto === visto) ?? null;
  const anotadoConjunto = (visto: string) => propio?.conjuntos.find((c) => c.visto === visto) ?? null;
  let n = 0;
  for (const d of a?.discos ?? []) {
    const esConjunto = a?.conjuntos.some((c) => c.id === d.id);
    if (esConjunto) continue;
    n++;
    const x = anotadoDisco(d.id);
    const virtual = d.bus === "virtual";
    const tipo = x?.tipo ?? tipoDeDisco(d);
    const visto = detalleDisco({ tipo, tamano: d.total || x?.tamano, modelo: x?.modelo ?? d.modelo, bahia: x?.bahia });
    const est = x?.estado ? ESTADO_DISCO_TEXTO[x.estado] : null;
    const nodo: NodoDisco = {
      clave: `d:${d.id}`,
      nivel: virtual ? "virtual" : "fisico",
      titulo: x?.nombre ?? (virtual ? `Disco virtual ${n}` : d.extraible ? `Disco extraíble ${n}` : `Disco ${n}`),
      detalle: [visto, d.extraible ? "extraíble" : null].filter(Boolean).join(" · "),
      estado: est?.texto ?? null,
      tono: est?.tono ?? null,
      miembros: [],
      volumenes: [],
      anotado: !!x,
    };
    nodos.push(nodo);
    porId.set(d.id, nodo);
  }
  for (const c of a?.conjuntos ?? []) {
    const x = anotadoConjunto(c.id);
    const fisico = a?.discos.find((d) => d.id === c.id);
    const tipoAgente = c.tipo === "zfs" ? "ZFS" : c.tipo === "mdadm" ? "RAID por software" : c.tipo === "storage_spaces" ? "Espacios de almacenamiento" : "Controladora RAID";
    const forma = x ? formaConjunto(x) : [tipoAgente, c.forma, c.discos ? `${c.discos} discos` : null].filter(Boolean).join(" · ");
    const tam = x?.capacidad ?? c.total ?? fisico?.total ?? null;
    const est = x?.estado ? ESTADO_DISCO_TEXTO[x.estado] : c.estado && c.estado !== "bien" ? ESTADO_DISCO_TEXTO[c.estado] : null;
    const nodo: NodoDisco = {
      clave: `c:${c.id}`,
      nivel: "conjunto",
      titulo: x?.nombre ?? (c.tipo === "zfs" ? `ZFS «${c.id}»` : c.tipo === "mdadm" ? `RAID ${c.id}` : `${tipoAgente}${tam ? ` de ${bytes(tam)}` : ""}`),
      detalle: [forma, tam && (x || c.tipo === "zfs" || c.tipo === "mdadm") ? bytes(tam) : null].filter(Boolean).join(" · "),
      estado: est?.texto ?? null,
      tono: est?.tono ?? null,
      miembros: (x?.miembros ?? []).map((m) => discoDe(propio, m)).filter((d): d is DiscoFisico => !!d).map((d) => ({ titulo: d.nombre, detalle: detalleDisco(d) })),
      volumenes: [],
      anotado: !!x,
    };
    nodos.push(nodo);
    porId.set(c.id, nodo);
  }
  const sueltos: VolumenVista[] = [];
  let porVolumen = false;
  const lineaEquipo = lineaFisica(sitioDeEquipo(i, e.id), o);
  for (const d of db?.discos ?? []) {
    const u = usoDisco(d);
    const sv = sitio?.volumenes?.find((v) => v.punto === d.punto);
    if (sv) porVolumen = true;
    const linea = sv || (t.clase === "virtual" || t.clase === "contenedor") ? lineaFisica(sitioDeVolumen(i, e.id, d.punto, d.volumen), o) : null;
    const v: VolumenVista = {
      punto: d.punto,
      etiqueta: d.etiqueta ?? null,
      sistemaArchivos: d.sistema_archivos ?? null,
      total: d.total,
      libre: d.libre,
      usado: u.usado,
      tono: u.tono,
      guarda: loQueGuarda(e, d),
      fisico: linea && linea !== lineaEquipo ? linea : sv ? linea : null,
    };
    const padre = d.volumen ? porId.get(d.volumen) : undefined;
    if (padre) padre.volumenes.push(v);
    else sueltos.push(v);
  }
  const anfitrion = t.clase === "virtual" || t.clase === "contenedor" ? lineaEquipo : null;
  return { nodos, sueltos, anfitrion, porVolumen };
}

// ---------- 0.8.2 (A1): lo que falta anotar ----------

/** Un destino de la lista que se puede anotar (no la nube: siempre está fuera). */
export const destinoAnotable = (d: { tipo: string }) => d.tipo !== "nube" && d.tipo !== "b2" && d.tipo !== "s3";

/**
 * 0.8.2 (A1): con algún servidor anotado, los equipos y destinos que aún no están en
 * ninguno (para el aviso suave de Estado). Un destino que hereda el servidor de quien lo
 * guarda no cuenta como sin anotar.
 */
export function sinAnotar(
  i: Infraestructura | null | undefined,
  equipos: readonly Equipo[],
  destinos: readonly { clave: string; tipo: string; equipo?: string | null }[],
): { equipos: Equipo[]; destinos: { clave: string; tipo: string }[] } {
  if (!conInfra(i)) return { equipos: [], destinos: [] };
  return {
    equipos: equipos.filter((e) => e.confirmado && e.modo !== "trasladado" && !i!.equipos[e.id]),
    destinos: destinos.filter((d) => destinoAnotable(d) && !sitioDeDestino(i, d.clave, d.equipo ?? null)),
  };
}

// ---------- 0.8.2 (A4, 15xx): filtrar el mapa por infraestructura ----------

/** Lo elegido: un servidor (`srv:<id>`), un conjunto (`cj:<srv>:<id>`) o un disco (`dk:<srv>:<id>`). */
export interface FiltroInfra {
  servidor: string;
  conjunto?: string | null;
  disco?: string | null;
}

/** Lee el valor guardado de un filtro («srv:hv-01», «cj:hv-01:tank», «dk:hv-01:b2»), o null. */
export function leerFiltroInfra(v: string | null | undefined, i?: Infraestructura | null): FiltroInfra | null {
  const m = /^(srv|cj|dk):([a-z0-9-]{1,40})(?::([a-z0-9-]{1,40}))?$/.exec(v ?? "");
  if (!m) return null;
  const [, tipo, servidor, otro] = m;
  if (tipo !== "srv" && !otro) return null;
  const f: FiltroInfra = { servidor, ...(tipo === "cj" ? { conjunto: otro } : {}), ...(tipo === "dk" ? { disco: otro } : {}) };
  if (!i) return f;
  const s = servidorDe(i, servidor);
  if (!s || (f.conjunto && !conjuntoDe(s, f.conjunto)) || (f.disco && !discoDe(s, f.disco))) return null;
  return f;
}

/** El valor que se guarda de un filtro. */
export const valorFiltroInfra = (f: FiltroInfra | null) => (!f ? "" : f.conjunto ? `cj:${f.servidor}:${f.conjunto}` : f.disco ? `dk:${f.servidor}:${f.disco}` : `srv:${f.servidor}`);

/** Las opciones del filtro: cada servidor y, dentro, sus conjuntos y discos (los que tienen algo). */
export function opcionesFiltroInfra(i: Infraestructura | null | undefined): { valor: string; texto: string; nivel: 0 | 1 }[] {
  const out: { valor: string; texto: string; nivel: 0 | 1 }[] = [];
  for (const s of i?.servidores ?? []) {
    out.push({ valor: `srv:${s.id}`, texto: s.nombre, nivel: 0 });
    for (const c of s.conjuntos) out.push({ valor: `cj:${s.id}:${c.id}`, texto: `${s.nombre} · ${c.nombre}`, nivel: 1 });
    for (const d of s.discos) out.push({ valor: `dk:${s.id}:${d.id}`, texto: `${s.nombre} · ${d.nombre}`, nivel: 1 });
  }
  return out;
}

/** «hv-01», «hv-01 · tank» (el filtro en palabras). */
export function textoFiltroInfra(f: FiltroInfra | null, i: Infraestructura | null | undefined): string {
  if (!f) return "";
  const s = servidorDe(i, f.servidor);
  const x = f.conjunto ? conjuntoDe(s, f.conjunto)?.nombre : f.disco ? discoDe(s, f.disco)?.nombre : null;
  return [s?.nombre ?? f.servidor, x].filter(Boolean).join(" · ");
}

/** ¿Está este sitio en lo filtrado? (Un servidor: todo lo suyo; un conjunto o un disco: solo eso.) */
export function enFiltroInfra(s: SitioVista | null, f: FiltroInfra): boolean {
  if (!s || s.servidor.id !== f.servidor) return false;
  if (f.conjunto) return s.conjunto?.id === f.conjunto;
  if (f.disco) return s.disco?.id === f.disco || (!!s.conjunto?.miembros?.includes(f.disco) && !!s.conjunto);
  return true;
}

/** ¿Tiene este equipo algo en lo filtrado (él entero o alguno de sus volúmenes)? */
export function equipoEnFiltroInfra(i: Infraestructura | null | undefined, equipo: string, f: FiltroInfra): boolean {
  if (enFiltroInfra(sitioDeEquipo(i, equipo), f)) return true;
  const s = i?.equipos[equipo];
  return !!s?.volumenes?.some((v) => enFiltroInfra(sitioDeVolumen(i, equipo, v.punto), f));
}

/** 0.8.2 (C4): para «Agrupar por → Servidor físico»: el servidor anotado de un equipo (su nombre), o null. */
export const servidorParaAgrupar = (i: Infraestructura | null | undefined, equipo: string) => sitioDeEquipo(i, equipo)?.servidor.nombre ?? null;

/** 0.8.2 (C4): para «Agrupar por → Disco o conjunto»: «hv-01 · tank» (o null sin conjunto ni disco). */
export function discoParaAgrupar(i: Infraestructura | null | undefined, equipo: string): string | null {
  const s = sitioDeEquipo(i, equipo);
  const d = s?.conjunto?.nombre ?? s?.disco?.nombre;
  return s && d ? `${s.servidor.nombre} · ${d}` : null;
}
