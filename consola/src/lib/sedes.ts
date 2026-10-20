// Plan 0.8.0, D1 y D2 (14ll, 14ss; docs/sedes.md): sedes, categorías de etiqueta,
// agrupar por niveles y orden a mano de los equipos. Sin runas: lo prueba
// scripts/vectores-sedes.ts.
//
// - Una **sede** es un dato propio del cliente (no una etiqueta): nombre y notas.
//   Cada equipo y cada destino está en una sola. Sin asignar, un equipo está en la
//   de por defecto (la marcada como principal o, si no, la primera por nombre); un
//   destino, en la de su equipo (una zona de un almacén, un disco o una carpeta)
//   o en ninguna (un servidor de fuera: manda su tipo, como antes). La nube no es
//   sede: siempre fuera.
// - «Fuera del sitio» en la regla 3-2-1-1-0: el destino está en otra sede que el
//   equipo que copia (`esFuera` de regla321.ts, igual que protection.rs).
// - Una **categoría** agrupa etiquetas («Tipo»: Servidor, Puesto): un solo valor por
//   equipo. Las etiquetas sueltas siguen igual.
// - Todo viaja como datos comunes (lib/datosComunes.ts): `sede:`, `sede.equipo:`,
//   `sede.destino:` (estas dos con la clave de administración), `etiqueta.categoria:`
//   y `orden.equipos:todos`.
import type { Equipo } from "./tipos";

export interface Sede {
  id: string;
  nombre: string;
  notas: string | null;
  principal: boolean;
}

export interface CategoriaEtiqueta {
  id: string;
  nombre: string;
  etiquetas: string[];
}

/** `GET /api/clientes/{c}/organizacion`. */
export interface Organizacion {
  sedes: Sede[];
  /** Equipo → sede (solo las asignadas). */
  equipos: Record<string, string>;
  /** Clave del destino en el catálogo → sede (solo las asignadas). */
  destinos: Record<string, string>;
  categorias: CategoriaEtiqueta[];
  /** El orden a mano de los equipos (ids). */
  orden: string[];
  /** 0.8.1 (B2, 15y): los grupos de exclusiones guardados (lib/gruposExclusiones.ts). */
  exclusiones: { id: string; nombre: string; reglas: string[] }[];
  /** Los que se quitaron (`null`): un grupo de partida quitado no vuelve a salir. */
  exclusiones_quitadas: string[];
  /** El servidor sabe de grupos de exclusiones (uno anterior no manda el campo: no se ofrecen). */
  exclusionesAdmitidas: boolean;
}

export const ORGANIZACION_VACIA: Organizacion = { sedes: [], equipos: {}, destinos: {}, categorias: [], orden: [], exclusiones: [], exclusiones_quitadas: [], exclusionesAdmitidas: false };

/** Lo que llega del servidor, limpio (un servidor anterior: vacía). */
export function leerOrganizacion(x: unknown): Organizacion {
  const o = (x ?? {}) as Partial<Organizacion>;
  const obj = (v: unknown) => (v && typeof v === "object" && !Array.isArray(v) ? (v as Record<string, string>) : {});
  return {
    sedes: Array.isArray(o.sedes) ? o.sedes.filter((s) => s && typeof s.id === "string" && typeof s.nombre === "string").map((s) => ({ id: s.id, nombre: s.nombre, notas: s.notas ?? null, principal: !!s.principal })) : [],
    equipos: obj(o.equipos),
    destinos: obj(o.destinos),
    categorias: Array.isArray(o.categorias) ? o.categorias.filter((c) => c && typeof c.id === "string").map((c) => ({ id: c.id, nombre: c.nombre, etiquetas: Array.isArray(c.etiquetas) ? c.etiquetas : [] })) : [],
    orden: Array.isArray(o.orden) ? o.orden.filter((x) => typeof x === "string") : [],
    exclusiones: Array.isArray(o.exclusiones)
      ? o.exclusiones.filter((g) => g && typeof g.id === "string" && typeof g.nombre === "string" && Array.isArray(g.reglas)).map((g) => ({ id: g.id, nombre: g.nombre, reglas: g.reglas.filter((r) => typeof r === "string") }))
      : [],
    exclusiones_quitadas: Array.isArray(o.exclusiones_quitadas) ? o.exclusiones_quitadas.filter((x) => typeof x === "string") : [],
    exclusionesAdmitidas: Array.isArray(o.exclusiones),
  };
}

// ---------------------------------------------------------------------------
// Sedes
// ---------------------------------------------------------------------------

const porNombre = (a: { nombre: string }, b: { nombre: string }) => a.nombre.localeCompare(b.nombre, "es", { sensitivity: "base" });

/** La sede de por defecto: la principal (la primera por nombre si hay varias) o, si no, la primera. */
export function sedePorDefecto(o: Organizacion | null | undefined): Sede | null {
  const s = [...(o?.sedes ?? [])].sort(porNombre);
  return s.find((x) => x.principal) ?? s[0] ?? null;
}

export const sedePorId = (o: Organizacion | null | undefined, id: string | null | undefined) => (id ? (o?.sedes.find((s) => s.id === id) ?? null) : null);

/** La sede de un equipo: la asignada o, sin ella, la de por defecto (`asignada: false`: avisar). */
export function sedeDeEquipo(o: Organizacion | null | undefined, equipo: string): { sede: Sede | null; asignada: boolean } {
  const s = sedePorId(o, o?.equipos[equipo]);
  if (s) return { sede: s, asignada: true };
  return { sede: sedePorDefecto(o), asignada: false };
}

/**
 * La sede de un destino: la asignada; si no, la del equipo que lo guarda (una zona de
 * un almacén, un disco, una carpeta), salvo que una persona marcara su tipo a mano (se
 * respeta: un disco que se lleva a casa y se marcó Fuera del sitio); si no, ninguna.
 * La nube no es sede.
 */
export function sedeDeDestino(
  o: Organizacion | null | undefined,
  d: { clave: string | null; equipo?: string | null; nube?: boolean; tipoMarcado?: boolean },
): { sede: Sede | null; asignada: boolean } {
  if (d.nube || !o?.sedes.length) return { sede: null, asignada: false };
  const s = sedePorId(o, d.clave ? o.destinos[d.clave] : null);
  if (s) return { sede: s, asignada: true };
  if (d.equipo && !d.tipoMarcado) return { sede: sedeDeEquipo(o, d.equipo).sede, asignada: false };
  return { sede: null, asignada: false };
}

/** Los equipos sin sede asignada (con sedes en el cliente): para el aviso «aviso si falta». */
export const equiposSinSede = <E extends Pick<Equipo, "id">>(o: Organizacion | null | undefined, equipos: readonly E[]) =>
  o?.sedes.length ? equipos.filter((e) => !sedePorId(o, o.equipos[e.id])) : [];

/** Un id nuevo para una sede o una categoría: del nombre, sin tildes, y un sufijo («oficina-norte-3f2a»). */
export function idNuevo(nombre: string, azar: () => number = Math.random): string {
  const base = nombre
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 40);
  const suf = Math.floor(azar() * 0x10000)
    .toString(16)
    .padStart(4, "0");
  return `${base || "sede"}-${suf}`;
}

/** Valida el nombre y las notas de una sede como el agente (`datos_cliente::valor_valido`). */
export function errorSede(nombre: string, notas: string): string | null {
  const n = nombre.trim();
  // eslint-disable-next-line no-control-regex
  if (!n || [...n].length > 60 || /[\u0000-\u001f\u007f-\u009f]/.test(n)) return "El nombre: de 1 a 60 caracteres.";
  // eslint-disable-next-line no-control-regex
  if ([...notas.trim()].length > 500 || /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f]/.test(notas)) return "Las notas: hasta 500 caracteres.";
  return null;
}

// ---------------------------------------------------------------------------
// Categorías de etiqueta
// ---------------------------------------------------------------------------

const k = (t: string) => t.trim().toLowerCase();

/** La categoría de una etiqueta, si la tiene. */
export const categoriaDe = (o: Organizacion | null | undefined, etiqueta: string) => o?.categorias.find((c) => c.etiquetas.some((t) => k(t) === k(etiqueta))) ?? null;

/** El valor de un equipo en una categoría (su primera etiqueta de ella) o null. */
export function valorEnCategoria(c: CategoriaEtiqueta, etiquetas: readonly string[] | undefined): string | null {
  const t = (etiquetas ?? []).find((x) => c.etiquetas.some((y) => k(y) === k(x)));
  return t ? (c.etiquetas.find((y) => k(y) === k(t)) ?? t) : null;
}

/** Las etiquetas sueltas de un equipo (las que no son de ninguna categoría). */
export const etiquetasSueltas = (o: Organizacion | null | undefined, etiquetas: readonly string[] | undefined) => (etiquetas ?? []).filter((t) => !categoriaDe(o, t));

/**
 * Al añadir una etiqueta: las que hay que quitar para que quede un solo valor por categoría
 * (las otras de la misma categoría). Devuelve la lista nueva y lo quitado.
 */
export function conUnaPorCategoria(o: Organizacion | null | undefined, etiquetas: readonly string[], nueva: string): { etiquetas: string[]; quitadas: string[] } {
  const c = categoriaDe(o, nueva);
  const quitadas = c ? etiquetas.filter((t) => k(t) !== k(nueva) && c.etiquetas.some((y) => k(y) === k(t))) : [];
  const resto = etiquetas.filter((t) => !quitadas.includes(t));
  return { etiquetas: resto.some((t) => k(t) === k(nueva)) ? resto : [...resto, nueva], quitadas };
}

/** Equipos con más de un valor en una misma categoría (de antes de crearla): para avisar. */
export function equiposConVariasDe<E extends Pick<Equipo, "id" | "etiquetas">>(c: CategoriaEtiqueta, equipos: readonly E[]): E[] {
  return equipos.filter((e) => (e.etiquetas ?? []).filter((t) => c.etiquetas.some((y) => k(y) === k(t))).length > 1);
}

// ---------------------------------------------------------------------------
// Agrupar por niveles («Sede, luego Tipo»), sin duplicados
// ---------------------------------------------------------------------------

/** `sede`, `sistema`, `etiqueta` (las sueltas) o `categoria:<id>`. */
/** 0.8.1 (C3): también por el tipo de equipo (físico, máquina virtual, contenedor). */
export type Nivel = "sede" | "sistema" | "tipo" | "etiqueta" | `categoria:${string}`;

export interface OpcionNivel {
  nivel: Nivel;
  nombre: string;
}

/** Los niveles que se ofrecen en «Agrupar por»: sede (si hay), cada categoría, sistema y etiquetas. */
export function nivelesDisponibles(o: Organizacion | null | undefined, conSistema: boolean, conTipo = false): OpcionNivel[] {
  const xs: OpcionNivel[] = [];
  if (o?.sedes.length) xs.push({ nivel: "sede", nombre: "Sede" });
  for (const c of [...(o?.categorias ?? [])].sort(porNombre)) xs.push({ nivel: `categoria:${c.id}`, nombre: c.nombre });
  if (conSistema) xs.push({ nivel: "sistema", nombre: "Sistema" });
  if (conTipo) xs.push({ nivel: "tipo", nombre: "Tipo de equipo" });
  xs.push({ nivel: "etiqueta", nombre: "Etiquetas" });
  return xs;
}

/** Lee lo recordado («sede>categoria:tipo»), sin niveles que ya no existen ni repetidos; dos como mucho. */
export function leerNiveles(texto: string, disponibles: readonly OpcionNivel[]): Nivel[] {
  const ok = new Set(disponibles.map((d) => d.nivel as string));
  const out: Nivel[] = [];
  for (const x of texto.split(">")) if (ok.has(x) && !out.includes(x as Nivel) && out.length < 2) out.push(x as Nivel);
  return out;
}

export interface GrupoNivel<E> {
  /** Única en todo el árbol (para recordar si está plegado): «sede=norte», «sede=norte/categoria:tipo=Servidor». */
  clave: string;
  nivel: Nivel;
  /** «Oficina Norte», «Servidor», «Sin sede», «Sin Tipo». */
  titulo: string;
  /** El grupo de los que no tienen valor («Sin …»). */
  sin: boolean;
  /** Todos los equipos del grupo (también los de sus subgrupos). */
  equipos: E[];
  hijos: GrupoNivel<E>[];
}

export interface Contexto<E> {
  organizacion: Organizacion | null | undefined;
  /** El sistema agrupado de un equipo («Windows 11»). */
  sistema: (e: E) => string;
  /** 0.8.1 (C3): el tipo de equipo agrupado («Máquina virtual (Hyper-V)»). */
  tipo?: (e: E) => string;
}

function valorDe<E extends Pick<Equipo, "id" | "etiquetas">>(e: E, n: Nivel, ctx: Contexto<E>): string | null {
  const o = ctx.organizacion;
  if (n === "sede") return sedeDeEquipo(o, e.id).sede?.nombre ?? null;
  if (n === "sistema") return ctx.sistema(e) || null;
  if (n === "tipo") return ctx.tipo?.(e) || null;
  if (n === "etiqueta") {
    const s = etiquetasSueltas(o, e.etiquetas);
    return s.length ? [...s].sort((a, b) => a.localeCompare(b, "es")).join(" · ") : null;
  }
  const c = o?.categorias.find((x) => `categoria:${x.id}` === n);
  return c ? valorEnCategoria(c, e.etiquetas) : null;
}

/** El nombre del nivel para «Sin …». */
function sinDe(n: Nivel, o: Organizacion | null | undefined): string {
  if (n === "sede") return "Sin sede";
  if (n === "sistema") return "Sin sistema";
  if (n === "tipo") return "Tipo sin saber";
  if (n === "etiqueta") return "Sin etiqueta";
  return `Sin ${o?.categorias.find((x) => `categoria:${x.id}` === n)?.nombre ?? "categoría"}`;
}

/**
 * Los equipos agrupados por los niveles dados, en orden alfabético y «Sin …» al final.
 * Cada equipo sale **una sola vez** en cada nivel (con varias etiquetas sueltas, en el
 * grupo de todas juntas, «Contabilidad · Servidores»). Dentro de cada grupo, en el
 * orden en que llegan (el de la lista).
 */
export function agruparPorNiveles<E extends Pick<Equipo, "id" | "etiquetas">>(equipos: readonly E[], niveles: readonly Nivel[], ctx: Contexto<E>, padre = ""): GrupoNivel<E>[] {
  const [n, ...resto] = niveles;
  if (!n) return [];
  const m = new Map<string, E[]>();
  const sin: E[] = [];
  for (const e of equipos) {
    const v = valorDe(e, n, ctx);
    if (v === null) sin.push(e);
    else m.set(v, [...(m.get(v) ?? []), e]);
  }
  const grupo = (titulo: string, es: E[], esSin: boolean): GrupoNivel<E> => {
    const clave = `${padre ? `${padre}/` : ""}${n}=${esSin ? "" : titulo}`;
    return { clave, nivel: n, titulo, sin: esSin, equipos: es, hijos: agruparPorNiveles(es, resto, ctx, clave) };
  };
  const out = [...m.entries()].sort(([a], [b]) => a.localeCompare(b, "es", { numeric: true })).map(([t, es]) => grupo(t, es, false));
  if (sin.length) out.push(grupo(sinDe(n, ctx.organizacion), sin, true));
  return out;
}

// ---------------------------------------------------------------------------
// Orden a mano
// ---------------------------------------------------------------------------

/** Los equipos en el orden a mano (los que no están en él, al final y por nombre). */
export function enOrdenManual<E extends Pick<Equipo, "id" | "nombre">>(equipos: readonly E[], orden: readonly string[]): E[] {
  const pos = new Map(orden.map((id, i) => [id, i]));
  return [...equipos].sort((a, b) => (pos.get(a.id) ?? Infinity) - (pos.get(b.id) ?? Infinity) || a.nombre.localeCompare(b.nombre, "es", { numeric: true }));
}

/**
 * Mueve `id` `paso` puestos (−1 arriba, +1 abajo) **dentro de su grupo** (`grupo`: los ids
 * del grupo tal como se ven) y devuelve el orden nuevo de todos los equipos (`todos`: los
 * ids en el orden a mano de ahora). null si no se puede mover más.
 */
export function moverEnGrupo(todos: readonly string[], grupo: readonly string[], id: string, paso: number): string[] | null {
  const g = [...grupo];
  const i = g.indexOf(id);
  const j = i + paso;
  if (i < 0 || j < 0 || j >= g.length) return null;
  [g[i], g[j]] = [g[j], g[i]];
  // Los del grupo se quedan en los mismos huecos de la lista entera, con su orden nuevo.
  const enGrupo = new Set(g);
  let q = 0;
  return todos.map((x) => (enGrupo.has(x) ? g[q++] : x));
}

/** Pone `id` justo antes de `antesDe` dentro del grupo (arrastrar y soltar). */
export function moverAntesDe(todos: readonly string[], grupo: readonly string[], id: string, antesDe: string | null): string[] | null {
  if (id === antesDe || !grupo.includes(id)) return null;
  const g = grupo.filter((x) => x !== id);
  const j = antesDe === null ? g.length : g.indexOf(antesDe);
  if (j < 0) return null;
  g.splice(j, 0, id);
  if (g.every((x, n) => x === grupo[n])) return null;
  const enGrupo = new Set(g);
  let q = 0;
  return todos.map((x) => (enGrupo.has(x) ? g[q++] : x));
}

/** Como mucho, equipos en el orden a mano (como el agente). */
export const MAX_ORDEN = 500;
