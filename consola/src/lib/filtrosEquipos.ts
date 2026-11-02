// 16e: los filtros de los equipos, iguales en Equipos (lista y mapa), Estado y Avisos.
// Aquí solo lo que se puede probar sin runas (scripts/vectores.ts): qué pasa el filtro,
// cómo se escribe en la dirección (para compartir un enlace con la vista filtrada), los
// chips quitables y las vistas guardadas. Lo recordado y la barra, en
// `filtrosEquipos.svelte.ts` y `componentes/BarraFiltros.svelte`.
//
// Cómo se combinan: dentro de una misma clase de filtro, «cualquiera» (sede A o B; al día
// o atrasada); entre clases, «y» (de la sede A y atrasados). Las etiquetas, a elegir:
// «cualquiera» o «todas».
import type { Equipo } from "./tipos";
import { copiaAtrasada, enPausa, saludEquipo } from "./salud";
import { mismaEtiqueta } from "./etiquetasGrupos";

export const ESTADOS_FILTRO = ["atencion", "al_dia", "fallando", "atrasada", "sin_conexion", "fuera", "pausa", "sin_copias"] as const;
export type EstadoFiltro = (typeof ESTADOS_FILTRO)[number];

export const NOMBRE_ESTADO: Record<EstadoFiltro, string> = {
  atencion: "Necesitan atención",
  al_dia: "Al día",
  fallando: "Fallando",
  atrasada: "Atrasados",
  sin_conexion: "Sin conexión",
  fuera: "Salen de la oficina",
  pausa: "En pausa",
  sin_copias: "Sin copias todavía",
};

export interface FiltrosEquipos {
  estados: EstadoFiltro[];
  /** Ids de sede (la de cada equipo, o la principal si no tiene). */
  sedes: string[];
  etiquetas: string[];
  /** Con dos o más etiquetas: que lleve cualquiera de ellas o todas. */
  modo: "cualquiera" | "todas";
  sistemas: string[];
  tipos: string[];
  /** Solo los que guardan copias de otros (almacenes). */
  almacen: boolean;
  /** Servidor físico, conjunto o disco (`srv:…`, `cj:…`, `dk:…`, como el filtro del mapa). */
  infra: string;
  /** Versiones del agente («0.8.2»). */
  versiones: string[];
}

export const FILTROS_VACIOS: FiltrosEquipos = Object.freeze({ estados: [], sedes: [], etiquetas: [], modo: "cualquiera", sistemas: [], tipos: [], almacen: false, infra: "", versiones: [] }) as FiltrosEquipos;

export const vacios = (): FiltrosEquipos => ({ ...FILTROS_VACIOS, estados: [], sedes: [], etiquetas: [], sistemas: [], tipos: [], versiones: [] });

/** ¿Hay algún filtro puesto? */
export const hayFiltros = (f: FiltrosEquipos) => cuantosFiltros(f) > 0;

/** Cuántos filtros hay puestos (cada valor elegido cuenta uno). */
export const cuantosFiltros = (f: FiltrosEquipos) => f.estados.length + f.sedes.length + f.etiquetas.length + f.sistemas.length + f.tipos.length + f.versiones.length + (f.almacen ? 1 : 0) + (f.infra ? 1 : 0);

/** Los estados en los que está un equipo (un equipo puede estar en varios: atrasado y fuera de la oficina). */
export function estadosDe(e: Equipo, ahora = Date.now()): Set<EstadoFiltro> {
  const s = new Set<EstadoFiltro>();
  const salud = saludEquipo(e, ahora);
  const copias = e.resumen?.copias ?? [];
  if (salud.tono === "bad" || salud.tono === "warn") s.add("atencion");
  if (salud.tono === "ok") s.add("al_dia");
  if (e.estado_servicio === "detenido_por_admin" || copias.some((c) => c.ultima?.estado === "fallo")) s.add("fallando");
  if (copias.some((c) => copiaAtrasada(c, ahora))) s.add("atrasada");
  if (e.confirmado && e.modo !== "trasladado" && !e.conectado) s.add("sin_conexion");
  if (e.resumen?.movil?.sale_oficina) s.add("fuera");
  if (enPausa(e, ahora)) s.add("pausa");
  // Sin ninguna vuelta todavía (una que falló ya es «Fallando»).
  if (e.rol !== "almacenamiento" && !copias.some((c) => c.ultima)) s.add("sin_copias");
  return s;
}

/** Lo que el filtro necesita saber de cada equipo y no está en él (la sede, la infraestructura…). */
export interface ContextoFiltros {
  sede?: (e: Equipo) => string | null;
  sistema?: (e: Equipo) => string;
  tipo?: (e: Equipo) => string;
  /** ¿Tiene el equipo algo en ese servidor físico, conjunto o disco? */
  enInfra?: (e: Equipo, valor: string) => boolean;
}

/** ¿Lleva las etiquetas pedidas? (Sin etiquetas pedidas, todos.) */
export function pasaEtiquetas(e: Pick<Equipo, "etiquetas"> | undefined, etiquetas: readonly string[], modo: FiltrosEquipos["modo"] = "cualquiera"): boolean {
  if (!etiquetas.length) return true;
  const lleva = (t: string) => !!e?.etiquetas?.some((x) => mismaEtiqueta(x, t));
  return modo === "todas" ? etiquetas.every(lleva) : etiquetas.some(lleva);
}

/** ¿Pasa el equipo todos los filtros? */
export function pasaFiltros(e: Equipo, f: FiltrosEquipos, ctx: ContextoFiltros = {}, ahora = Date.now()): boolean {
  if (f.estados.length) {
    const s = estadosDe(e, ahora);
    if (!f.estados.some((x) => s.has(x))) return false;
  }
  if (f.sedes.length && !f.sedes.includes(ctx.sede?.(e) ?? "")) return false;
  if (!pasaEtiquetas(e, f.etiquetas, f.modo)) return false;
  if (f.sistemas.length && ctx.sistema && !f.sistemas.includes(ctx.sistema(e))) return false;
  if (f.tipos.length && ctx.tipo && !f.tipos.includes(ctx.tipo(e))) return false;
  if (f.almacen && e.rol !== "almacenamiento") return false;
  if (f.infra && ctx.enInfra && !ctx.enInfra(e, f.infra)) return false;
  if (f.versiones.length && !f.versiones.includes(e.version_agente || "")) return false;
  return true;
}

// ---------- En la dirección (para compartir un enlace) ----------

const LARGO_MAX = 80;
const MAX_VALORES = 30;
const limpio = (v: string) => v.trim().slice(0, LARGO_MAX);
const unicos = (vs: string[]) => [...new Set(vs.map(limpio).filter(Boolean))].slice(0, MAX_VALORES);

/** Los parámetros de la dirección que son de los filtros (para quitarlos o ponerlos sin tocar los demás). */
export const PARAMETROS_FILTRO = ["estado", "sede", "et", "modo", "so", "tipo", "rol", "infra", "ver", "filtro"] as const;

/** Lee los filtros de la dirección. `?filtro=atencion` (los enlaces de antes, 16o) es «Necesitan atención». */
export function deDireccion(q: URLSearchParams): FiltrosEquipos {
  const estados = unicos(q.getAll("estado")).filter((x): x is EstadoFiltro => (ESTADOS_FILTRO as readonly string[]).includes(x));
  const viejo = q.get("filtro");
  if (viejo === "atencion" && !estados.includes("atencion")) estados.push("atencion");
  const infra = limpio(q.get("infra") ?? "");
  return {
    estados,
    sedes: unicos(q.getAll("sede")),
    etiquetas: unicos(q.getAll("et")),
    modo: q.get("modo") === "todas" ? "todas" : "cualquiera",
    sistemas: unicos(q.getAll("so")),
    tipos: unicos(q.getAll("tipo")),
    almacen: q.get("rol") === "almacen" || viejo === "almacen",
    infra: /^(srv|cj|dk):[a-z0-9:-]{1,90}$/.test(infra) ? infra : "",
    versiones: unicos(q.getAll("ver")),
  };
}

/** ¿Trae la dirección algún filtro? */
export const direccionConFiltros = (q: URLSearchParams) => PARAMETROS_FILTRO.some((p) => q.has(p));

/** Escribe los filtros en unos parámetros (quita antes los que hubiera). */
export function aDireccion(f: FiltrosEquipos, q = new URLSearchParams()): URLSearchParams {
  const out = new URLSearchParams(q);
  for (const p of PARAMETROS_FILTRO) out.delete(p);
  for (const x of f.estados) out.append("estado", x);
  for (const x of f.sedes) out.append("sede", x);
  for (const x of f.etiquetas) out.append("et", x);
  if (f.etiquetas.length > 1 && f.modo === "todas") out.set("modo", "todas");
  for (const x of f.sistemas) out.append("so", x);
  for (const x of f.tipos) out.append("tipo", x);
  if (f.almacen) out.set("rol", "almacen");
  if (f.infra) out.set("infra", f.infra);
  for (const x of f.versiones) out.append("ver", x);
  return out;
}

/** Los filtros en un texto corto (lo que se recuerda y lo que guarda una vista). */
export const aTexto = (f: FiltrosEquipos) => aDireccion(f).toString();
export const deTexto = (t: string) => deDireccion(new URLSearchParams(t));

/** ¿Los mismos filtros? (sin importar el orden en que se eligieron). */
export function mismosFiltros(a: FiltrosEquipos, b: FiltrosEquipos): boolean {
  const n = (f: FiltrosEquipos) => {
    const o = (xs: string[]) => [...xs].map((x) => x.toLowerCase()).sort();
    return JSON.stringify([o(f.estados), o(f.sedes), o(f.etiquetas), f.etiquetas.length > 1 ? f.modo : "", o(f.sistemas), o(f.tipos), f.almacen, f.infra, o(f.versiones)]);
  };
  return n(a) === n(b);
}

// ---------- Los filtros puestos, como chips quitables ----------

export interface ChipFiltro {
  clave: string;
  texto: string;
  /** Los filtros sin este. */
  sin: FiltrosEquipos;
}

/** Nombres para lo que en los filtros va por id (las sedes y la infraestructura). */
export interface NombresFiltros {
  sede?: (id: string) => string | null;
  infra?: (valor: string) => string | null;
}

export function chipsDe(f: FiltrosEquipos, nombres: NombresFiltros = {}): ChipFiltro[] {
  const out: ChipFiltro[] = [];
  const sinValor = <K extends "estados" | "sedes" | "etiquetas" | "sistemas" | "tipos" | "versiones">(k: K, v: string): FiltrosEquipos => ({ ...f, [k]: (f[k] as string[]).filter((x) => x !== v) });
  for (const x of f.estados) out.push({ clave: `estado:${x}`, texto: NOMBRE_ESTADO[x], sin: sinValor("estados", x) });
  for (const x of f.sedes) out.push({ clave: `sede:${x}`, texto: `Sede: ${nombres.sede?.(x) ?? x}`, sin: sinValor("sedes", x) });
  const modo = f.etiquetas.length > 1 ? (f.modo === "todas" ? " (todas)" : " (cualquiera)") : "";
  for (const x of f.etiquetas) out.push({ clave: `et:${x}`, texto: `Etiqueta: ${x}${modo}`, sin: sinValor("etiquetas", x) });
  for (const x of f.sistemas) out.push({ clave: `so:${x}`, texto: x, sin: sinValor("sistemas", x) });
  for (const x of f.tipos) out.push({ clave: `tipo:${x}`, texto: x, sin: sinValor("tipos", x) });
  if (f.almacen) out.push({ clave: "rol", texto: "Guardan copias", sin: { ...f, almacen: false } });
  if (f.infra) out.push({ clave: "infra", texto: `En ${nombres.infra?.(f.infra) ?? f.infra}`, sin: { ...f, infra: "" } });
  for (const x of f.versiones) out.push({ clave: `ver:${x}`, texto: `Agente ${x}`, sin: sinValor("versiones", x) });
  return out;
}

/** Pone o quita un valor de una lista de filtros. */
export function alternar(xs: readonly string[], v: string): string[] {
  return xs.some((x) => x.toLowerCase() === v.toLowerCase()) ? xs.filter((x) => x.toLowerCase() !== v.toLowerCase()) : [...xs, v];
}

/** «8 de 30 equipos» (o «30 equipos» sin filtros). */
export function textoCuenta(vistos: number, total: number): string {
  const eq = (n: number) => (n === 1 ? "equipo" : "equipos");
  return vistos === total ? `${total} ${eq(total)}` : `${vistos} de ${total} ${eq(total)}`;
}

// ---------- Vistas guardadas ----------

export interface VistaGuardada {
  nombre: string;
  /** Los filtros, en texto (`aTexto`). */
  filtros: string;
}

export const MAX_VISTAS = 20;
export const MAX_NOMBRE_VISTA = 40;

/** Lee las vistas guardadas (lo que no tenga forma se descarta). */
export function leerVistas(texto: string): VistaGuardada[] {
  try {
    const x = JSON.parse(texto || "[]");
    if (!Array.isArray(x)) return [];
    return x
      .filter((v): v is VistaGuardada => !!v && typeof v.nombre === "string" && typeof v.filtros === "string" && !!v.nombre.trim())
      .map((v) => ({ nombre: v.nombre.trim().slice(0, MAX_NOMBRE_VISTA), filtros: v.filtros.slice(0, 2000) }))
      .slice(0, MAX_VISTAS);
  } catch {
    return [];
  }
}

/** Qué falla en el nombre de una vista nueva (o null si vale). */
export function errorNombreVista(nombre: string, f: FiltrosEquipos): string | null {
  const n = nombre.trim();
  if (!n) return "Ponle un nombre.";
  if (n.length > MAX_NOMBRE_VISTA) return `Como mucho ${MAX_NOMBRE_VISTA} caracteres.`;
  if (!hayFiltros(f)) return "Elige antes algún filtro.";
  return null;
}

/** Guarda una vista (si ya hay una con ese nombre, la cambia). Las más nuevas, al final; como mucho MAX_VISTAS. */
export function conVista(vistas: readonly VistaGuardada[], nombre: string, f: FiltrosEquipos): VistaGuardada[] {
  const n = nombre.trim().slice(0, MAX_NOMBRE_VISTA);
  const resto = vistas.filter((v) => v.nombre.toLowerCase() !== n.toLowerCase());
  return [...resto, { nombre: n, filtros: aTexto(f) }].slice(-MAX_VISTAS);
}

export const sinVista = (vistas: readonly VistaGuardada[], nombre: string) => vistas.filter((v) => v.nombre !== nombre);

/** La vista guardada que coincide con los filtros puestos (si la hay). */
export const vistaActual = (vistas: readonly VistaGuardada[], f: FiltrosEquipos) => (hayFiltros(f) ? (vistas.find((v) => mismosFiltros(deTexto(v.filtros), f)) ?? null) : null);
