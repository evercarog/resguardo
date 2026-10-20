// 0.8.1 (B2, plan-mejoras 15y): los **grupos de exclusiones** del cliente («No copiar»:
// Música, Vídeo, Instaladores…), iguales en todas sus consolas. Sin runas: lo prueban
// scripts/vectores-datos-comunes.ts (con los vectores de Rust,
// crates/protocolo/vectors/datos-cliente.json) y scripts/vectores-organizacion.ts.
//
// - Cada grupo es un dato común `exclusiones:<id>` → `{ nombre, reglas: [..] }` o `null`
//   (quitado). Se guarda como las sedes (`POST /datos-comunes`, administradores) y se
//   reparte a los equipos con `admite: "grupos_exclusiones"`. Se lee, ya limpio, en
//   `GET /organizacion` (cualquier miembro): `exclusiones` y `exclusiones_quitadas`.
// - **De partida:** si el cliente no tiene guardado un grupo con uno de los ids de
//   `GRUPOS_DE_PARTIDA` (ni lo quitó: `null`), la consola lo enseña igual, marcado como
//   «de partida». Al cambiarlo se guarda con ese mismo id; al quitarlo se guarda `null`
//   y ya no vuelve a salir (se puede recuperar con «Volver a poner los de partida»).
//   Con ids fijos, dos consolas que lo tocan a la vez cambian el mismo dato (gana el
//   más reciente) en vez de crear dos «Música».
// - **Solo los datos.** Las copias guardan las reglas que aplican (el equipo no depende de
//   la consola): usar un grupo en el editor de copias (bloque D) copia sus reglas a la
//   copia. Para eso, `gruposExclusiones(actual)`.
import type { Organizacion } from "./sedes";

/** Como mucho, reglas en un grupo y caracteres en cada regla (como el protocolo). */
export const MAX_REGLAS_GRUPO = 100;
export const MAX_LARGO_REGLA = 200;
/** Nombre del grupo: de 1 a 60 caracteres. */
export const MAX_NOMBRE_GRUPO = 60;
/** Como mucho, grupos por cliente (lo que cabe con holgura en los datos comunes de un equipo). */
export const MAX_GRUPOS = 50;

export interface GrupoExclusiones {
  id: string;
  nombre: string;
  reglas: string[];
  /** De partida y aún sin tocar (no está guardado: se guarda al cambiarlo). */
  dePartida: boolean;
}

/**
 * Los grupos de partida, con ids fijos. «Instaladores» no lleva `*.exe` a secas: en las
 * carpetas de trabajo hay programas que sí hay que guardar (el de un ERP, una utilidad de
 * la empresa); solo los `.exe` de las carpetas de descargas, que se vuelven a bajar.
 * (`Downloads` es el nombre real de la carpeta en Windows aunque el Explorador diga
 * «Descargas»; una regla sin `/` al principio vale en cualquier sitio de la ruta.)
 */
export const GRUPOS_DE_PARTIDA: readonly Omit<GrupoExclusiones, "dePartida">[] = [
  {
    id: "temporales",
    nombre: "Archivos temporales",
    reglas: ["*.tmp", "*.temp", "~$*", "*.crdownload", "*.part", "*.partial", "*.swp", "Thumbs.db", "desktop.ini", ".DS_Store"],
  },
  { id: "musica", nombre: "Música", reglas: ["*.mp3", "*.flac", "*.m4a", "*.wav", "*.ogg", "*.wma", "*.aac", "*.aiff", "*.opus"] },
  { id: "video", nombre: "Vídeo", reglas: ["*.mp4", "*.mkv", "*.avi", "*.mov", "*.wmv", "*.m4v", "*.mpg", "*.mpeg", "*.webm", "*.flv", "*.3gp"] },
  {
    id: "imagenes-disco",
    nombre: "Imágenes de disco y máquinas virtuales",
    reglas: ["*.iso", "*.img", "*.dmg", "*.vhd", "*.vhdx", "*.avhdx", "*.vmdk", "*.vdi", "*.qcow2", "*.ova", "*.vmem"],
  },
  { id: "instaladores", nombre: "Instaladores", reglas: ["*.msi", "*.msix", "*.msixbundle", "*.appx", "*.appxbundle", "Downloads/*.exe"] },
];

// eslint-disable-next-line no-control-regex
const CONTROL = /[\u0000-\u001f\u007f-\u009f]/;

/** Una regla limpia (sin espacios alrededor), o el motivo por el que no vale. Como `regla_exclusion_valida` del protocolo. */
export function reglaValida(r: string): { regla: string } | { error: string } {
  const t = r.trim();
  const n = [...t].length;
  if (n === 0 || n > MAX_LARGO_REGLA) return { error: `Cada regla: de 1 a ${MAX_LARGO_REGLA} caracteres.` };
  if (CONTROL.test(t)) return { error: "Una regla no puede llevar caracteres de control." };
  if (/^[*?./\\]+$/.test(t)) return { error: `«${t}» lo dejaría todo sin copiar.` };
  return { regla: t };
}

/**
 * El valor de un grupo, limpio y validado como en el protocolo (`exclusiones:<id>`), o el
 * primer error. `reglas` puede ser el texto del cuadro (una por línea: las líneas vacías
 * no cuentan) o la lista. Sin repetir, en el orden escrito.
 */
export function valorGrupo(nombre: string, reglas: string | string[]): { valor: { nombre: string; reglas: string[] } } | { error: string } {
  const n = typeof nombre === "string" ? nombre.trim() : "";
  if (!n || [...n].length > MAX_NOMBRE_GRUPO || CONTROL.test(n)) return { error: `El nombre del grupo: de 1 a ${MAX_NOMBRE_GRUPO} caracteres.` };
  const lista = typeof reglas === "string" ? reglas.split(/\r?\n/).filter((l) => l.trim()) : reglas;
  if (!Array.isArray(lista) || !lista.length) return { error: "Escribe al menos una regla." };
  if (lista.length > MAX_REGLAS_GRUPO) return { error: `Un grupo lleva como mucho ${MAX_REGLAS_GRUPO} reglas.` };
  const out: string[] = [];
  for (const r of lista) {
    if (typeof r !== "string") return { error: "Regla no válida." };
    const v = reglaValida(r);
    if ("error" in v) return v;
    if (!out.includes(v.regla)) out.push(v.regla);
  }
  return { valor: { nombre: n, reglas: out } };
}

/** El mismo trato que el protocolo a un valor que llega de fuera (para los vectores): el valor limpio o null. */
export function valorGrupoDeFuera(x: unknown): { nombre: string; reglas: string[] } | null {
  const v = x as Record<string, unknown> | null;
  if (!v || typeof v !== "object" || Array.isArray(v)) return null;
  if (Object.keys(v).some((k) => k !== "nombre" && k !== "reglas")) return null;
  if (!Array.isArray(v.reglas)) return null;
  const r = valorGrupo(typeof v.nombre === "string" ? v.nombre : "", v.reglas as string[]);
  return "valor" in r ? r.valor : null;
}

/** Un id nuevo para un grupo propio: del nombre y un sufijo al azar (letras, cifras y `-`). */
export function idGrupoNuevo(nombre: string, existentes: string[] = []): string {
  const base =
    nombre
      .normalize("NFD")
      .replace(/[̀-ͯ]/g, "")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 40) || "grupo";
  for (;;) {
    const id = `${base}-${Math.random().toString(16).slice(2, 6)}`;
    if (!existentes.includes(id) && !GRUPOS_DE_PARTIDA.some((g) => g.id === id)) return id;
  }
}

/**
 * Los grupos de exclusiones del cliente: los guardados y los de partida que nadie ha
 * guardado ni quitado. Por nombre. Para el bloque D (editor de copias): pásale `actual`
 * (o cualquier cosa con `organizacion`).
 */
export function gruposExclusiones(x: { organizacion: Organizacion } | Organizacion | null | undefined): GrupoExclusiones[] {
  const o = x && "organizacion" in x ? x.organizacion : (x as Organizacion | null | undefined);
  const guardados: GrupoExclusiones[] = (o?.exclusiones ?? []).map((g) => ({ id: g.id, nombre: g.nombre, reglas: [...g.reglas], dePartida: false }));
  const quitados = new Set(o?.exclusiones_quitadas ?? []);
  const dePartida = GRUPOS_DE_PARTIDA.filter((g) => !quitados.has(g.id) && !guardados.some((x) => x.id === g.id)).map((g) => ({ ...g, reglas: [...g.reglas], dePartida: true }));
  return [...guardados, ...dePartida].sort((a, b) => a.nombre.localeCompare(b.nombre, "es", { sensitivity: "base" }));
}

/** Los de partida que alguien quitó (para «Volver a poner los de partida»). */
export const gruposDePartidaQuitados = (o: Organizacion | null | undefined) => GRUPOS_DE_PARTIDA.filter((g) => (o?.exclusiones_quitadas ?? []).includes(g.id) && !(o?.exclusiones ?? []).some((x) => x.id === g.id));

/** Las reglas de varios grupos juntas, sin repetir (para aplicarlas a una copia). */
export function reglasDe(grupos: Pick<GrupoExclusiones, "reglas">[]): string[] {
  const out: string[] = [];
  for (const g of grupos) for (const r of g.reglas) if (!out.includes(r)) out.push(r);
  return out;
}
