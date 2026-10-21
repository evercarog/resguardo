// 14uu (plan 0.8.0, D3): los datos básicos del equipo (`resumen.datos_basicos`):
// sistema exacto, procesador, memoria, discos, fabricante y modelo, y último
// arranque. Lo manda el agente: aquí se comprueba todo (largos, números, cuántos
// discos) antes de enseñarlo. Un agente anterior no lo manda: `null`.
//
//   npm run test:vectores (scripts/vectores-datos-basicos.ts)

import type { Equipo } from "./tipos";

export interface SistemaEquipo {
  familia: "windows" | "linux" | string;
  nombre?: string | null;
  variante?: string | null;
  version?: string | null;
  compilacion?: string | null;
  /** Linux: `ID` de os-release («ubuntu», «debian»). */
  id?: string | null;
  kernel?: string | null;
}

export interface DiscoEquipo {
  /** «C:» en Windows; el punto de montaje en Linux. */
  punto: string;
  total: number;
  libre: number;
  sistema_archivos?: string | null;
  /** Guarda algo de las copias (carpetas que se copian, destinos locales o el almacén). */
  en_uso?: boolean;
  /** 0.8.1 (C4): el conjunto de discos donde está (grupo de ZFS «tank», «md0», «disco1»…), si se sabe. */
  volumen?: string | null;
  /** 0.8.2 (A3): la etiqueta del volumen («Datos»), si tiene (Windows). */
  etiqueta?: string | null;
}

/** 0.8.1 (C4, docs/infraestructura.md): un disco que ve el equipo (sin número de serie). */
export interface DiscoFisicoEquipo {
  /** «sda», «nvme0n1» (Linux) o «disco0» (Windows). */
  id: string;
  total: number;
  clase?: "ssd" | "hdd" | null;
  /** «sata», «sas», «nvme», «usb», «raid», «spaces», «virtual», «iscsi»… */
  bus?: string | null;
  modelo?: string | null;
  extraible?: boolean;
}

/** 0.8.1 (C4): un conjunto de discos que ve el equipo. */
export interface ConjuntoEquipo {
  /** «md0», «tank», «disco1». */
  id: string;
  /** `mdadm`, `zfs`, `storage_spaces` o `raid` (una controladora RAID vista como un disco). */
  tipo: string;
  /** «raid1», «raid5»… si se sabe (en ZFS, no). */
  forma?: string | null;
  discos?: number | null;
  estado?: "bien" | "degradado" | "reconstruyendo" | null;
  total?: number | null;
}

export interface AlmacenamientoEquipo {
  discos: DiscoFisicoEquipo[];
  conjuntos: ConjuntoEquipo[];
}

export interface DatosBasicos {
  leido?: string | null;
  sistema?: SistemaEquipo | null;
  procesador?: { nombre?: string | null; nucleos?: number | null } | null;
  memoria?: number | null;
  fabricante?: string | null;
  modelo?: string | null;
  arranque?: string | null;
  discos?: DiscoEquipo[];
  /** 0.8.1 (C4): lo que ve de su almacenamiento (un agente anterior no lo manda). */
  almacenamiento?: AlmacenamientoEquipo | null;
  /** 15ll (Windows): tarjetas de red que el sistema puede apagar para ahorrar energía (sus nombres). */
  red_ahorro?: string[];
}

/** Lo mismo que en el agente y en el servidor. */
export const MAX_DISCOS = 32;
const MAX_TEXTO = 120;
/** Casi lleno: menos del 10 % **y** menos de 10 GiB libres (como el aviso del servidor). */
const FRACCION_LLENO = 0.1;
const BYTES_LLENO = 10 * 1024 ** 3;

function texto(v: unknown, max = MAX_TEXTO): string | null {
  if (typeof v !== "string") return null;
  // eslint-disable-next-line no-control-regex
  const t = v.replace(/[\u0000-\u001f\u007f]/g, "").replace(/\s+/g, " ").trim().slice(0, max);
  return t || null;
}
const entero = (v: unknown, max = Number.MAX_SAFE_INTEGER): number | null => (typeof v === "number" && Number.isFinite(v) && v >= 0 && v <= max ? Math.floor(v) : null);
const fecha = (v: unknown): string | null => {
  const t = texto(v, 40);
  return t && Number.isFinite(Date.parse(t)) ? t : null;
};

/** Los datos básicos de un equipo, comprobados (o `null` si su agente no los manda). */
export function datosBasicos(e: Pick<Equipo, "resumen"> | null | undefined): DatosBasicos | null {
  const d = (e?.resumen as { datos_basicos?: unknown } | null | undefined)?.datos_basicos;
  if (!d || typeof d !== "object" || Array.isArray(d)) return null;
  const x = d as Record<string, unknown>;
  const s = x.sistema && typeof x.sistema === "object" ? (x.sistema as Record<string, unknown>) : null;
  const p = x.procesador && typeof x.procesador === "object" ? (x.procesador as Record<string, unknown>) : null;
  const discos: DiscoEquipo[] = [];
  for (const k of Array.isArray(x.discos) ? x.discos.slice(0, MAX_DISCOS) : []) {
    if (!k || typeof k !== "object") continue;
    const y = k as Record<string, unknown>;
    const punto = texto(y.punto);
    const total = entero(y.total);
    const libre = entero(y.libre);
    if (!punto || !total || libre === null || libre > total) continue;
    discos.push({ punto, total, libre, sistema_archivos: texto(y.sistema_archivos, 32), en_uso: y.en_uso === true, volumen: texto(y.volumen, 64), etiqueta: texto(y.etiqueta, 32) });
  }
  // 0.8.1 (C4): los discos y conjuntos que ve (comprobados como lo demás).
  let almacenamiento: AlmacenamientoEquipo | null = null;
  const a = x.almacenamiento && typeof x.almacenamiento === "object" ? (x.almacenamiento as Record<string, unknown>) : null;
  if (a) {
    const fisicos: DiscoFisicoEquipo[] = [];
    for (const k of Array.isArray(a.discos) ? a.discos.slice(0, MAX_DISCOS) : []) {
      if (!k || typeof k !== "object") continue;
      const y = k as Record<string, unknown>;
      const id = texto(y.id, 40);
      const total = entero(y.total);
      if (!id || total === null) continue;
      const clase = y.clase === "ssd" || y.clase === "hdd" ? y.clase : null;
      fisicos.push({ id, total, clase, bus: texto(y.bus, 20), modelo: texto(y.modelo, 60), extraible: y.extraible === true });
    }
    const conjuntos: ConjuntoEquipo[] = [];
    for (const k of Array.isArray(a.conjuntos) ? a.conjuntos.slice(0, MAX_DISCOS) : []) {
      if (!k || typeof k !== "object") continue;
      const y = k as Record<string, unknown>;
      const id = texto(y.id, 64);
      const tipo = texto(y.tipo, 20);
      if (!id || !tipo) continue;
      const estado = y.estado === "bien" || y.estado === "degradado" || y.estado === "reconstruyendo" ? y.estado : null;
      conjuntos.push({ id, tipo, forma: texto(y.forma, 20), discos: entero(y.discos, 256), estado, total: entero(y.total) });
    }
    if (fisicos.length || conjuntos.length) almacenamiento = { discos: fisicos, conjuntos };
  }
  return {
    leido: fecha(x.leido),
    sistema: s
      ? {
          familia: texto(s.familia, 20) ?? "",
          nombre: texto(s.nombre),
          variante: texto(s.variante, 60),
          version: texto(s.version, 40),
          compilacion: texto(s.compilacion, 40),
          id: texto(s.id, 40),
          kernel: texto(s.kernel),
        }
      : null,
    procesador: p ? { nombre: texto(p.nombre), nucleos: entero(p.nucleos, 4096) } : null,
    memoria: entero(x.memoria),
    fabricante: texto(x.fabricante),
    modelo: texto(x.modelo),
    arranque: fecha(x.arranque),
    discos,
    almacenamiento,
    red_ahorro: (Array.isArray(x.red_ahorro) ? x.red_ahorro.slice(0, 8) : []).map((n) => texto(n, 80)).filter((n): n is string => !!n),
  };
}

/**
 * 0.8.1 (C1, C4): el disco del equipo donde está una ruta suya (el punto más largo que
 * la contiene: «D:» o «/srv/copias»), para decir su sistema de archivos, su conjunto, su
 * tamaño y lo libre. `null` si no se sabe.
 */
export function discoDeRuta(d: DatosBasicos | null | undefined, ruta: string | null | undefined): DiscoEquipo | null {
  if (!d?.discos?.length || !ruta) return null;
  const r = ruta.trim();
  const dentro = (p: string) => {
    if (/^[A-Za-z]:$/.test(p)) return r.slice(0, 2).toUpperCase() === p.toUpperCase();
    if (p === "/") return r.startsWith("/");
    const base = p.replace(/\/+$/, "");
    return r === base || r.startsWith(`${base}/`);
  };
  return d.discos.filter((k) => dentro(k.punto)).sort((a, b) => b.punto.length - a.punto.length)[0] ?? null;
}

/** El sistema exacto en palabras: «Windows 11 Pro 24H2», «Ubuntu 22.04.4 LTS». */
export function textoSistema(d: DatosBasicos | null | undefined): string | null {
  const s = d?.sistema;
  if (!s?.nombre) return null;
  if (s.familia === "windows" && s.version && !s.nombre.includes(s.version)) return `${s.nombre} ${s.version}`;
  return s.nombre;
}

/** Lo que se enseña como sistema de un equipo: el exacto si lo hay; si no, el de siempre («Windows x86_64»). */
export function sistemaDe(e: Pick<Equipo, "resumen" | "so">): string {
  return textoSistema(datosBasicos(e)) ?? e.so;
}

/**
 * El sistema agrupado para filtrar la lista: «Windows 11», «Windows Server 2019»,
 * «Ubuntu 22.04», «Debian 12». Sin datos, el de siempre.
 */
export function grupoSistema(e: Pick<Equipo, "resumen" | "so">): string {
  const s = datosBasicos(e)?.sistema;
  if (!s?.nombre) return e.so;
  if (s.familia === "windows") {
    const srv = /^(Windows Server \d{4})( R2)?/.exec(s.nombre);
    if (srv) return srv[1] + (srv[2] ?? "");
    const ver = /^Windows \d+/.exec(s.nombre);
    return ver ? ver[0] : s.nombre;
  }
  if (s.familia === "linux" && s.id && s.version) {
    const nombre = s.nombre.split(/\s/)[0] || s.id;
    return `${nombre} ${s.version}`;
  }
  return s.nombre;
}

/** «Intel Core i5-8500 · 6 núcleos». */
export function textoProcesador(d: DatosBasicos | null | undefined): string | null {
  const p = d?.procesador;
  if (!p?.nombre && !p?.nucleos) return null;
  const n = p.nucleos ? `${p.nucleos} ${p.nucleos === 1 ? "núcleo" : "núcleos"}` : null;
  return [p.nombre, n].filter(Boolean).join(" · ");
}

/** La memoria como la dicen los equipos («16 GB»: la cuentan en potencias de 2). */
export function textoMemoria(d: DatosBasicos | null | undefined): string | null {
  if (!d?.memoria) return null;
  const g = d.memoria / 1024 ** 3;
  if (g < 1) return `${Math.round(d.memoria / 1024 ** 2)} MB`;
  return `${g < 10 ? Math.round(g * 10) / 10 : Math.round(g)} GB`.replace(".", ",");
}

/** «Dell Inc. OptiPlex 7070» (sin repetir el fabricante si el modelo ya lo lleva). */
export function textoModelo(d: DatosBasicos | null | undefined): string | null {
  const f = d?.fabricante;
  const m = d?.modelo;
  if (!f) return m ?? null;
  if (!m) return f;
  return m.toLowerCase().startsWith(f.toLowerCase()) ? m : `${f} ${m}`;
}

/** Uso de un disco: porcentaje usado y tono (casi lleno, como el aviso del servidor: `bad`; menos del 20 % libre: `warn`). */
export function usoDisco(k: DiscoEquipo): { usado: number; tono: "ok" | "warn" | "bad" } {
  const usado = k.total > 0 ? Math.min(100, Math.max(0, Math.round(((k.total - k.libre) / k.total) * 100))) : 0;
  const lleno = k.libre < k.total * FRACCION_LLENO && k.libre < BYTES_LLENO;
  return { usado, tono: lleno ? "bad" : k.libre < k.total * 0.2 ? "warn" : "ok" };
}

/** ¿Está casi lleno? (El mismo criterio que el aviso `disco_lleno`.) */
export const casiLleno = (k: DiscoEquipo) => usoDisco(k).tono === "bad";

/** Cuánto antes de un fallo cuenta un reinicio («el equipo se reinició a las 11:11»). */
export const MARGEN_REINICIO_MS = 2 * 3600_000;

const fmtHora = new Intl.DateTimeFormat("es", { hour: "2-digit", minute: "2-digit" });
const fmtDia = new Intl.DateTimeFormat("es", { day: "numeric", month: "short" });

/**
 * Si el equipo arrancó poco antes de `cuando` (un fallo), la frase que lo dice:
 * «El equipo se reinició a las 11:11.» (o «… el 12 oct a las 23:50.» si fue otro día).
 */
export function reinicioAntes(d: DatosBasicos | null | undefined, cuando: string | null | undefined, margenMs = MARGEN_REINICIO_MS): string | null {
  if (!d?.arranque || !cuando) return null;
  const a = Date.parse(d.arranque);
  const c = Date.parse(cuando);
  if (!Number.isFinite(a) || !Number.isFinite(c) || a > c || c - a > margenMs) return null;
  const da = new Date(a);
  const mismoDia = da.toDateString() === new Date(c).toDateString();
  return `El equipo se reinició ${mismoDia ? "" : `el ${fmtDia.format(da)} `}a las ${fmtHora.format(da)}.`;
}

/** Para buscar en la lista de equipos: el sistema exacto y su grupo. */
export function textoBuscable(e: Pick<Equipo, "resumen" | "so">): string {
  const d = datosBasicos(e);
  return [e.so, textoSistema(d), grupoSistema(e), d?.sistema?.variante, d?.sistema?.version, textoModelo(d)].filter(Boolean).join(" ");
}
