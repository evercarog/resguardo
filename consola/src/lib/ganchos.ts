// Ganchos de plantilla (api-servidor.md v1.10): lo que el agente puede hacer
// antes de copiar. Solo dos plantillas cerradas, con las MISMAS reglas que el
// agente (crates/motor/src/ganchos.rs), para avisar al momento: el agente
// rechaza la configuración entera si algo no cuadra.
import type { Gancho, GanchoCarpetaReciente, GanchoSqlserver } from "./tipos";

export const MAX_GANCHOS = 4;
export const MAX_BASES = 20;
/** Versión mínima del agente que acepta ganchos. */
export const VERSION_GANCHOS = "0.7.2";

/** ¿`v` ≥ `min`? (versiones «x.y.z», se ignora lo que vaya tras un guion o un espacio). */
export function versionAlMenos(v: string | null | undefined, min: string): boolean {
  if (!v) return false;
  const num = (x: string) => x.split(/[-+ ]/)[0].split(".").map((n) => parseInt(n, 10) || 0);
  const a = num(v);
  const b = num(min);
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    if ((a[i] ?? 0) !== (b[i] ?? 0)) return (a[i] ?? 0) > (b[i] ?? 0);
  }
  return true;
}

/** La lista de ganchos de una copia (el campo admite null, uno o una lista). */
export const ganchosDe = (g: Gancho | Gancho[] | null | undefined): Gancho[] => (!g ? [] : Array.isArray(g) ? g : [g]);

/** Ruta local completa, sin comillas, `;*?<>|` ni «..» (va dentro de T-SQL). */
export function rutaValida(p: string): boolean {
  const t = p.trim();
  const completa = /^[A-Za-z]:[\\/]/.test(t) || t.startsWith("/");
  return completa && t.length <= 240 && !/[\u0000-\u001f'";*?<>|]/.test(t) && !t.split(/[\\/]/).some((s) => s === "..");
}

/** Nombre de base: letras, números, espacio, `_ - . $` (hasta 128), sin espacios al principio o al final. */
export const baseValida = (b: string) => b.length > 0 && b.length <= 128 && b.trim() === b && /^[\p{L}\p{N}_\-. $]+$/u.test(b);

/** Instancia: «.», «EQUIPO», «EQUIPO\INSTANCIA» o «EQUIPO,puerto». */
export function instanciaValida(i: string): boolean {
  const [resto, puerto, ...mas] = i.split(",");
  if (mas.length) return false;
  if (puerto !== undefined) {
    const n = Number(puerto);
    if (!/^\d+$/.test(puerto) || n < 1 || n > 65535) return false;
  }
  const partes = resto.split("\\");
  if (partes.length > 2) return false;
  const nombre = (s: string) => s.length > 0 && s.length <= 63 && /^[A-Za-z0-9._-]+$/.test(s);
  return nombre(partes[0]) && (partes[1] === undefined || (nombre(partes[1]) && !partes[1].includes(".")));
}

/** Lo que falla en un gancho, en una frase, o null si está bien. */
export function errorGancho(g: Gancho): string | null {
  if (g.tipo === "sqlserver") {
    const inst = (g.instancia ?? ".").trim() || ".";
    if (!instanciaValida(inst)) return `Instancia de SQL Server no válida: «${inst}» (p. ej. «.» o «EQUIPO\\SQLEXPRESS»).`;
    if (!g.bases.length || g.bases.length > MAX_BASES) return `Indica de 1 a ${MAX_BASES} bases de datos.`;
    const mala = g.bases.find((b) => !baseValida(b));
    if (mala !== undefined) return `Nombre de base de datos no admitido: «${mala}» (letras, números, espacio y _ - . $).`;
    if (new Set(g.bases.map((b) => b.toLowerCase())).size !== g.bases.length) return "Hay una base de datos repetida.";
    // Además (v1.10, seguridad): de un disco del equipo, no su raíz ni de red.
    if (!rutaValida(g.carpeta) || errorCarpetaLocal(g.carpeta, !g.carpeta.trim().startsWith("/")))
      return "La carpeta de los volcados tiene que ser de un disco del equipo (no su raíz ni de red), sin comillas ni ; * ? < > | (p. ej. C:\\ResguardoVolcados).";
    if (enCarpetaDelSistema(g.carpeta, !g.carpeta.trim().startsWith("/"))) return "La carpeta de los volcados no puede ir en la de Windows, los programas ni Resguardo (p. ej. C:\\ResguardoVolcados).";
    return null;
  }
  if (!rutaValida(g.carpeta) && !g.carpeta.trim().startsWith("\\\\")) return "La carpeta a vigilar tiene que ser una ruta completa (p. ej. D:\\WO\\Copias).";
  if (!Number.isInteger(g.horas) || g.horas < 1 || g.horas > 720) return "Las horas tienen que estar entre 1 y 720.";
  const e = g.extension ?? null;
  if (e !== null && (!e.length || e.length > 10 || !/^[A-Za-z0-9.]+$/.test(e))) return "Extensión no válida (p. ej. .bak).";
  return null;
}

/** El gancho exactamente con los campos que acepta el agente (ni uno más). */
export function limpiar(g: Gancho): Gancho {
  if (g.tipo === "sqlserver") {
    const s: GanchoSqlserver = { tipo: "sqlserver", bases: g.bases.map((b) => b), carpeta: g.carpeta.trim() };
    const inst = (g.instancia ?? "").trim();
    if (inst && inst !== ".") s.instancia = inst;
    return s;
  }
  const c: GanchoCarpetaReciente = { tipo: "carpeta_reciente", carpeta: g.carpeta.trim(), horas: g.horas };
  const ext = (g.extension ?? "").trim();
  c.extension = ext ? (ext.startsWith(".") ? ext : `.${ext}`) : null;
  return c;
}

/** El valor de `gancho` para la configuración: null, uno o la lista. */
export function paraConfig(gs: Gancho[]): Gancho | Gancho[] | null {
  const l = gs.map(limpiar);
  return l.length === 0 ? null : l.length === 1 ? l[0] : l;
}

export const NOMBRE_GANCHO: Record<Gancho["tipo"], string> = {
  sqlserver: "Volcado de SQL Server",
  carpeta_reciente: "Copias de la aplicación al día",
};

/** Una frase de lo que hará. */
export function fraseGancho(g: Gancho): string {
  if (g.tipo === "sqlserver") {
    const n = g.bases.filter(Boolean).length;
    return `vuelca ${n === 1 ? `la base «${g.bases[0]}»` : `${n} bases`} de SQL Server en ${g.carpeta || "…"} y lo copia con lo demás`;
  }
  const ext = (g.extension ?? "").trim();
  return `avisa si en ${g.carpeta || "…"} no hay ${ext ? `archivos ${ext.startsWith(".") ? ext : `.${ext}`} ` : "nada "}de las últimas ${g.horas} h`;
}

/**
 * Carpeta de un disco del equipo (v1.10, seguridad): ni la raíz, ni de red, ni
 * relativa, ni con «..» (crates/agente/src/platform.rs, carpeta_local_valida).
 * Los enlaces en el camino solo los puede ver el equipo: si los hay, lo dirá él.
 */
export function errorCarpetaLocal(p: string, windows: boolean): string | null {
  const t = p.trim();
  const forma = windows ? /^[A-Za-z]:\\[^\\]/.test(t) : t.startsWith("/") && !t.startsWith("//") && t.replace(/\/+$/, "").length > 1;
  if (!forma || /[\u0000-\u001f]/.test(t) || t.split(/[\\/]/).some((s) => s === ".."))
    return windows ? "Elige una carpeta de un disco de este equipo, no su raíz ni una carpeta de red (por ejemplo, E:\\Resguardo)." : "Elige una carpeta completa de este equipo, que no sea la raíz (por ejemplo, /srv/resguardo).";
  return null;
}

/** Carpetas del sistema donde no va nada de Resguardo (Windows, programas, Resguardo); las mismas que el agente (sesiones_v2.rs). */
const SISTEMA_WINDOWS = ["c:\\windows", "c:\\program files", "c:\\program files (x86)", "c:\\programdata\\resguardoagente", "c:\\programdata\\resguardo", "c:\\programdata\\resguardo server"];
const SISTEMA_LINUX = ["/proc", "/sys", "/dev", "/run", "/boot", "/bin", "/sbin", "/lib", "/lib64", "/usr", "/etc", "/var/lib/resguardo-agente", "/opt/resguardo-agente"];

/** ¿Está en una carpeta del sistema, de los programas o de Resguardo? */
export function enCarpetaDelSistema(p: string, windows: boolean): boolean {
  const t = windows ? p.trim().toLowerCase().replace(/\\+$/, "") : p.trim().replace(/\/+$/, "");
  const sep = windows ? "\\" : "/";
  return (windows ? SISTEMA_WINDOWS : SISTEMA_LINUX).some((r) => t === r || t.startsWith(`${r}${sep}`));
}

export function errorCarpetaEspejo(p: string, windows: boolean): string | null {
  const e = errorCarpetaLocal(p, windows);
  if (e) return e;
  if (enCarpetaDelSistema(p, windows)) return "El espejo no puede ir en la carpeta de Windows, de los programas ni de Resguardo: elige otra (mejor en otro disco).";
  return null;
}

/**
 * Carpeta donde Resguardo guarda algo en el equipo (Servidor de copias,
 * volcados, un destino local elegido en el equipo): de un disco del equipo,
 * no su raíz, y fuera de Windows, los programas y Resguardo.
 */
export function errorCarpetaDestino(p: string, windows: boolean, raizDeDisco = false): string | null {
  // El destino local de un repositorio puede ser la raíz de otro disco («E:\»): el agente lo
  // guarda en «E:\<repositorio>». El Servidor de copias, el espejo y los volcados, no.
  if (raizDeDisco && windows && /^[A-BD-Za-bd-z]:\\$/.test(p.trim())) return null;
  const e = errorCarpetaLocal(p, windows);
  if (e) return e;
  if (enCarpetaDelSistema(p, windows)) return windows ? "Esa carpeta es de Windows, de los programas o de Resguardo: elige otra (mejor en otro disco)." : "Esa carpeta es del sistema o de Resguardo: elige otra (por ejemplo, en /srv).";
  return null;
}

/** Nombre de una carpeta nueva: las mismas reglas que el agente (crear_carpeta, v1.15). */
export function errorNombreCarpeta(n: string): string | null {
  if (!n.trim()) return "Escribe un nombre.";
  if ([...n].length > 100) return "Hasta 100 caracteres.";
  if (n.trim() !== n) return "Sin espacios al principio ni al final.";
  if (n === "." || n === ".." || n.endsWith(".")) return "Sin punto al final.";
  if (/[\u0000-\u001f<>:"/\\|?*]/.test(n)) return "Sin \\ / : * ? \" < > |.";
  if (/^(con|prn|aux|nul|com[1-9]|lpt[1-9])(\..*)?$/i.test(n)) return "Ese nombre lo reserva Windows: elige otro.";
  return null;
}
