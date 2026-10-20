// Ganchos de plantilla (api-servidor.md v1.10): lo que el agente puede hacer
// antes de copiar. Solo dos plantillas cerradas, con las MISMAS reglas que el
// agente (crates/motor/src/ganchos.rs), para avisar al momento: el agente
// rechaza la configuración entera si algo no cuadra.
import { lista } from "./formato";
import type { Gancho, GanchoCarpetaReciente, GanchoSqlserver, ResultadoGancho } from "./tipos";

export const MAX_GANCHOS = 4;
export const MAX_BASES = 20;
/** 15u: con «todas las bases», cuántas se pueden excluir. */
export const MAX_EXCLUIDAS = 50;
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
    if (g.todas) {
      // 15u: todas las de la instancia; las excluidas, con las mismas reglas que una base.
      const ex = g.excluir ?? [];
      if (ex.length > MAX_EXCLUIDAS) return `Como mucho ${MAX_EXCLUIDAS} bases excluidas.`;
      const malaEx = ex.find((b) => !baseValida(b));
      if (malaEx !== undefined) return `Nombre de base de datos no admitido: «${malaEx}» (letras, números, espacio y _ - . $).`;
    } else {
      if (!g.bases.length || g.bases.length > MAX_BASES) return `Indica de 1 a ${MAX_BASES} bases de datos.`;
      const mala = g.bases.find((b) => !baseValida(b));
      if (mala !== undefined) return `Nombre de base de datos no admitido: «${mala}» (letras, números, espacio y _ - . $).`;
      if (new Set(g.bases.map((b) => b.toLowerCase())).size !== g.bases.length) return "Hay una base de datos repetida.";
    }
    // Además (v1.10, seguridad): de un disco del equipo, no su raíz ni de red.
    if (!rutaValida(g.carpeta) || errorCarpetaLocal(g.carpeta, !g.carpeta.trim().startsWith("/")))
      return "La carpeta de los volcados tiene que ser de un disco del equipo (no su raíz ni de red), sin comillas ni ; * ? < > | (p. ej. C:\\ResguardoVolcados).";
    if (enCarpetaDelSistema(g.carpeta, !g.carpeta.trim().startsWith("/"))) return "La carpeta de los volcados no puede ir en la de Windows, los programas ni Resguardo (p. ej. C:\\ResguardoVolcados).";
    return null;
  }
  if (!rutaValida(g.carpeta) && !g.carpeta.trim().startsWith("\\\\")) return "La carpeta a vigilar tiene que ser una ruta completa (p. ej. D:\\CopiasApp).";
  if (!Number.isInteger(g.horas) || g.horas < 1 || g.horas > 720) return "Las horas tienen que estar entre 1 y 720.";
  const e = g.extension ?? null;
  if (e !== null && (!e.length || e.length > 10 || !/^[A-Za-z0-9.]+$/.test(e))) return "Extensión no válida (p. ej. .bak).";
  return null;
}

/** 14ccc: ¿el agente entiende `verificar` en el gancho de SQL Server? (Los anteriores rechazan campos que no conocen.) */
export const admiteVerificarVolcado = (e: { resumen?: { admite?: string[] | null } | null } | null | undefined) => !!e?.resumen?.admite?.includes("verificar_volcado");
/** 15u: ¿entiende «todas las bases» (`todas`, `excluir`, `sistema`) y la orden `bases_sqlserver` («Ver las bases de la instancia»)? */
export const admiteTodasLasBases = (e: { resumen?: { admite?: string[] | null } | null } | null | undefined) => !!e?.resumen?.admite?.includes("sqlserver_todas");

/**
 * El gancho exactamente con los campos que acepta el agente (ni uno más).
 * `verificar` (14ccc) solo se manda apagado y a un agente que lo entiende: sin
 * el campo, el agente nuevo verifica cada volcado.
 */
export function limpiar(g: Gancho, conVerificar = false, conTodas = false): Gancho {
  if (g.tipo === "sqlserver") {
    // 15u: «todas las bases» solo a un agente que lo entiende (la consola no se lo ofrece a otro).
    const todas = conTodas && !!g.todas;
    const s: GanchoSqlserver = { tipo: "sqlserver", bases: todas ? [] : g.bases.map((b) => b), carpeta: g.carpeta.trim() };
    const inst = (g.instancia ?? "").trim();
    if (inst && inst !== ".") s.instancia = inst;
    if (conVerificar && g.verificar === false) s.verificar = false;
    if (todas) {
      s.todas = true;
      if (g.excluir?.length) s.excluir = [...g.excluir];
      if (g.sistema) s.sistema = true;
    }
    return s;
  }
  const c: GanchoCarpetaReciente = { tipo: "carpeta_reciente", carpeta: g.carpeta.trim(), horas: g.horas };
  const ext = (g.extension ?? "").trim();
  c.extension = ext ? (ext.startsWith(".") ? ext : `.${ext}`) : null;
  return c;
}

/** El valor de `gancho` para la configuración: null, uno o la lista. */
export function paraConfig(gs: Gancho[], conVerificar = false, conTodas = false): Gancho | Gancho[] | null {
  const l = gs.map((g) => limpiar(g, conVerificar, conTodas));
  return l.length === 0 ? null : l.length === 1 ? l[0] : l;
}

export const NOMBRE_GANCHO: Record<Gancho["tipo"], string> = {
  sqlserver: "Volcado de SQL Server",
  carpeta_reciente: "Copias de la aplicación al día",
};

/** Hasta 3 nombres y «y N más»: «A, B y C», «A, B, C y 3 más». */
function nombresCortos(xs: string[], max = 3): string {
  return xs.length <= max ? lista(xs) : `${xs.slice(0, max).join(", ")} y ${xs.length - max} más`;
}

/**
 * Los pasos «antes de copiar» de una copia, en una frase y sin repetir (14gg):
 * «volcado de 6 bases de SQL Server (Ventas, Nominas, Compras y 3 más)». Con la
 * configuración (`ganchos`) se dicen los nombres; sin ella, solo con los
 * resultados del último informe (uno por paso o por base), se agrupan por tipo.
 */
export function ganchosEnFrase(ganchos: Gancho[] | null | undefined, resultados: Pick<ResultadoGancho, "tipo" | "mensaje">[] = []): string[] {
  const out: string[] = [];
  if (ganchos?.length) {
    // 15u: «todas las bases» (las de la instancia, en cada copia).
    const todas = ganchos.filter((g): g is GanchoSqlserver => g.tipo === "sqlserver" && !!g.todas);
    for (const g of todas) out.push(`volcado de todas las bases de SQL Server${g.excluir?.length ? ` salvo ${nombresCortos(g.excluir)}` : ""}`);
    const bases = [...new Set(ganchos.flatMap((g) => (g.tipo === "sqlserver" && !g.todas ? g.bases.filter(Boolean) : [])))];
    if (bases.length === 1) out.push(`volcado de la base «${bases[0]}» de SQL Server`);
    else if (bases.length > 1) out.push(`volcado de ${bases.length} bases de SQL Server (${nombresCortos(bases)})`);
    else if (!todas.length && ganchos.some((g) => g.tipo === "sqlserver")) out.push("volcado de SQL Server");
    const carpetas = ganchos.filter((g) => g.tipo === "carpeta_reciente").length;
    if (carpetas === 1) out.push("comprobar que las copias de la aplicación están al día");
    else if (carpetas > 1) out.push(`comprobar que las copias de la aplicación están al día en ${carpetas} carpetas`);
    return out;
  }
  // El agente da un resultado por base, con su nombre entre comillas («Ventas»).
  const sql = resultados.filter((r) => r.tipo === "sqlserver");
  const nombres = [...new Set(sql.map((r) => /«([^»]+)»/.exec(r.mensaje ?? "")?.[1]).filter((x): x is string => !!x))];
  const conNombres = nombres.length === sql.length;
  if (sql.length === 1) out.push(conNombres ? `volcado de la base «${nombres[0]}» de SQL Server` : "volcado de SQL Server");
  else if (sql.length > 1) out.push(`volcado de ${sql.length} bases de SQL Server${conNombres ? ` (${nombresCortos(nombres)})` : ""}`);
  const carpetas = resultados.filter((r) => r.tipo === "carpeta_reciente").length;
  if (carpetas === 1) out.push("comprobar que las copias de la aplicación están al día");
  else if (carpetas > 1) out.push(`comprobar que las copias de la aplicación están al día en ${carpetas} carpetas`);
  return out;
}

/**
 * Los resultados de los pasos previos de una copia, para listarlos: los volcados
 * que fueron bien se juntan en una sola línea («6 bases volcadas: Ventas,
 * Nominas, Compras y 3 más»); lo que falló o tuvo avisos sigue suelto, con su
 * mensaje (14gg).
 */
export function agruparResultados<R extends Pick<ResultadoGancho, "tipo" | "estado" | "mensaje" | "verificado">>(rs: R[]): R[] {
  const nombreDe = (r: R) => /«([^»]+)»/.exec(r.mensaje ?? "")?.[1];
  // 15f, 15j: los volcados con el mismo problema (aviso o fallo) también se juntan: el mensaje
  // sin el nombre de la base es la clave («Volcado de «…» hecho, sin verificar: falta permiso…»).
  const claveDe = (r: R) => (r.tipo !== "sqlserver" ? null : r.estado === "ok" ? "ok" : `${r.estado}|${(r.mensaje ?? "").replace(/«[^»]+»/, "«…»")}`);
  const grupos = new Map<string, R[]>();
  for (const r of rs) {
    const k = claveDe(r);
    if (k) grupos.set(k, [...(grupos.get(k) ?? []), r]);
  }
  const juntar = (k: string, xs: R[]): R => {
    const nombres = xs.map(nombreDe).filter((x): x is string => !!x);
    const lista = nombres.length === xs.length ? nombresCortos(nombres) : "";
    if (k === "ok") {
      // 14ccc: «y verificadas» solo si lo están todas; «sin verificar» si no se verificó ninguna (15f).
      const verificadas = xs.every((r) => r.verificado === true);
      const sinVerificar = xs.every((r) => /sin verificar/.test(r.mensaje ?? ""));
      const mensaje = `${xs.length} bases volcadas${verificadas ? " y verificadas" : sinVerificar ? " sin verificar" : ""}${lista ? `: ${lista}` : ""}.`;
      return { ...xs[0], mensaje, verificado: verificadas || null };
    }
    return { ...xs[0], mensaje: (xs[0].mensaje ?? "").replace(/«[^»]+»/, `${xs.length} bases${lista ? ` (${lista})` : ""}`), verificado: null };
  };
  const out: R[] = [];
  const puestos = new Set<string>();
  for (const r of rs) {
    const k = claveDe(r);
    const xs = k ? grupos.get(k) : undefined;
    if (!k || !xs || xs.length < 2) out.push(r);
    else if (!puestos.has(k)) {
      out.push(juntar(k, xs));
      puestos.add(k);
    }
  }
  return out;
}

/**
 * 15j: los resultados de los pasos previos de una vuelta en una línea corta, sin
 * repetir el mismo paso por cada base: «volcado de SQL Server correcto (6 bases)»,
 * «volcado de SQL Server: 4 bases bien, 2 fallaron». Para el historial.
 */
export function resumenResultados(rs: Pick<ResultadoGancho, "tipo" | "estado">[]): string {
  const TXT = { ok: "correcto", aviso: "con avisos", fallo: "falló" } as const;
  const porTipo = new Map<string, Pick<ResultadoGancho, "tipo" | "estado">[]>();
  for (const r of rs) porTipo.set(r.tipo, [...(porTipo.get(r.tipo) ?? []), r]);
  return [...porTipo.entries()]
    .map(([tipo, xs]) => {
      const nombre = (NOMBRE_GANCHO[tipo as Gancho["tipo"]] ?? tipo).replace(/^./, (c) => c.toLowerCase());
      if (xs.length === 1) return `${nombre} ${TXT[xs[0].estado]}`;
      const [uno, varios] = tipo === "sqlserver" ? ["base", "bases"] : ["vez", "veces"];
      const cuenta = (n: number) => `${n} ${n === 1 ? uno : varios}`;
      if (xs.every((x) => x.estado === xs[0].estado)) return `${nombre} ${TXT[xs[0].estado]} (${cuenta(xs.length)})`;
      const n = (e: ResultadoGancho["estado"]) => xs.filter((x) => x.estado === e).length;
      const partes = [n("ok") && `${cuenta(n("ok"))} bien`, n("aviso") && `${n("aviso")} con avisos`, n("fallo") && `${n("fallo")} ${n("fallo") === 1 ? "falló" : "fallaron"}`];
      return `${nombre}: ${partes.filter(Boolean).join(", ")}`;
    })
    .join(", ");
}

/**
 * El resultado que se enseña junto a un paso de la configuración: el peor de los
 * suyos (en un volcado hay uno por base) y, si alguna falló, su mensaje.
 */
export function resultadoDeGancho<R extends Pick<ResultadoGancho, "tipo" | "estado" | "mensaje" | "verificado">>(g: Gancho, rs: R[]): R | undefined {
  const suyos = rs.filter((r) => r.tipo === g.tipo && (g.tipo !== "sqlserver" || g.bases.some((b) => (r.mensaje ?? "").includes(`«${b}»`)) || !/«/.test(r.mensaje ?? "")));
  const orden = { fallo: 0, aviso: 1, ok: 2 } as const;
  const peor = [...suyos].sort((a, b) => orden[a.estado] - orden[b.estado])[0];
  if (!peor || g.tipo !== "sqlserver" || suyos.length < 2 || peor.estado !== "ok") return peor;
  return agruparResultados(suyos)[0];
}

/** Una frase de lo que hará. */
export function fraseGancho(g: Gancho): string {
  if (g.tipo === "sqlserver") {
    const n = g.bases.filter(Boolean).length;
    const que = g.todas ? `todas las bases${g.excluir?.length ? ` (salvo ${nombresCortos(g.excluir)})` : ""}` : n === 1 ? `la base «${g.bases[0]}»` : `${n} bases`;
    return `vuelca ${que} de SQL Server en ${g.carpeta || "…"} y lo copia con lo demás`;
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
