// Nombres legibles en el almacén (plan 0.8.0, B1; docs/nombres-legibles.md).
//
// - La carpeta de un equipo en un almacén es su nombre con un sufijo
//   (`pc-01-3f2a`) y la de cada repositorio, su nombre (`contabilidad`). La del
//   equipo la elige el almacén (`guarda_copias { anadir, nombre }`); la del
//   repositorio es su id, que pone la consola al crearlo (`carpetaNueva`).
// - «Ordenar nombres de carpeta»: el almacén mueve (`guarda_copias { mover }`)
//   y el equipo dueño pone la ruta nueva (`cambiar_destino { donde, usuario,
//   carpetas }`). Si lo segundo no llega, `rutasPendientes` lo nota con los
//   `renombres` del almacén.
// - `legibleEnTexto`: «pc-01-3f2a/contabilidad» o un id largo → «Contabilidad, de PC-01».
//
// `normalizar` es la misma regla del agente (crates/agente/src/nombres.rs), con
// los vectores de crates/protocolo/vectors/nombres-legibles.json.
import type { DestinoResumen, Equipo, RepositorioResumen, TrabajoEspejoResumen } from "./tipos";
import { usuarioEnAlmacen } from "./direccion";
import { destinoDe } from "./repo";
import { esDeAlmacen } from "./retencion";

/** `resumen.admite`: carpetas legibles, «Ordenar nombres de carpeta», `cambiar_destino { carpetas }` y `nombre_repositorio`. */
export const ADMITE_NOMBRES = "nombres_legibles";
export const admiteNombres = (e: Pick<Equipo, "resumen"> | null | undefined) => !!e?.resumen?.admite?.includes(ADMITE_NOMBRES);

export const MAX_EQUIPO = 35;
export const MAX_REPO = 40;
export const MAX_CARPETA = 64;

const SIN_MARCA: Record<string, string> = {};
for (const [base, letras] of [
  ["a", "áàäâãå"],
  ["e", "éèëê"],
  ["i", "íìïî"],
  ["o", "óòöôõ"],
  ["u", "úùüû"],
  ["n", "ñ"],
  ["c", "ç"],
  ["y", "ýÿ"],
] as const) {
  for (const l of letras) SIN_MARCA[l] = base;
}

/**
 * El nombre de carpeta de un texto cualquiera: minúsculas, sin tildes, lo que no
 * sea letra o cifra pasa a guion (sin repetir ni en los extremos), hasta `max`.
 * Vacío: `defecto`. Igual que en el agente.
 */
export function normalizar(raw: string, max: number, defecto: string): string {
  let s = "";
  for (const c of raw.toLowerCase()) {
    const x = /^[a-z0-9]$/.test(c) ? c : SIN_MARCA[c];
    if (x) s += x;
    else if (!s.endsWith("-")) s += "-";
  }
  s = s.replace(/^-+|-+$/g, "").slice(0, max).replace(/-+$/, "");
  return s || defecto;
}

/** ¿Una carpeta del almacén con un nombre aceptable? (`a–z`, `0–9`, `-`, hasta 64; vale un UUID). */
export const carpetaValida = (s: string) => s.length >= 1 && s.length <= MAX_CARPETA && /^[a-z0-9-]+$/.test(s) && !s.startsWith("-") && !s.endsWith("-");

/** La carpeta de un repositorio nuevo en un almacén: su nombre normalizado, sin chocar con `ocupadas` (`ventas`, `ventas-2`…). */
export function carpetaNueva(nombre: string, ocupadas: Iterable<string>): string {
  const usadas = new Set([...ocupadas].map((x) => x.toLowerCase()));
  const base = normalizar(nombre, MAX_REPO, "repositorio");
  if (!usadas.has(base)) return base;
  for (let i = 2; i < 1000; i++) {
    const n = `${base.slice(0, MAX_REPO - 1 - String(i).length).replace(/-+$/, "")}-${i}`;
    if (!usadas.has(n)) return n;
  }
  return `${base.slice(0, 30)}-${crypto.randomUUID().slice(0, 4)}`;
}

/**
 * El id (= su carpeta) de un repositorio nuevo de `equipo` en el almacén `almacen`
 * con el usuario `usuario`: su nombre, sin chocar con los ids del equipo ni con las
 * carpetas que el almacén ya tiene para ese usuario (en cualquier zona).
 */
export function idRepoEnAlmacen(nombre: string, equipo: Pick<Equipo, "resumen">, almacen: Pick<Equipo, "resumen"> | null | undefined, usuario: string | null | undefined): string {
  const g = almacen?.resumen?.guarda_copias;
  const enAlmacen = [...(g?.repositorios ?? []), ...(g?.zonas ?? []).flatMap((z) => z.repositorios ?? [])].filter((u) => !!usuario && u.usuario === usuario).flatMap((u) => u.repos);
  return carpetaNueva(nombre, [...(equipo.resumen?.repositorios ?? []).map((r) => r.id), ...enAlmacen]);
}

/** ¿Un usuario del almacén con la forma de antes (el id del equipo, recortado a 32)? */
export const usuarioDeAntes = (u: string) => /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{0,12}$/.test(u);

/** ¿Su carpeta ya es su nombre? (`contabilidad`, o `contabilidad-2` si había otra). */
export function carpetaLegible(carpeta: string, nombre: string): boolean {
  const n = normalizar(nombre, MAX_REPO, "repositorio");
  return carpeta === n || new RegExp(`^${n.replace(/-/g, "\\-")}-\\d{1,3}$`).test(carpeta);
}

/** Un repositorio de un equipo en un almacén. */
export interface RepoEnAlmacen {
  repo: RepositorioResumen;
  /** Su carpeta dentro de la del usuario. */
  carpeta: string;
}

/** Lo de un equipo en un almacén, por destino (su usuario). */
export interface EnElAlmacen {
  equipo: Equipo;
  destino: DestinoResumen;
  usuario: string;
  repos: RepoEnAlmacen[];
  /** Todos sus repositorios ahí son importados (de otro equipo): no es el dueño. */
  importado: boolean;
}

/** Los equipos (de este cliente) con repositorios en el almacén `alm`, por destino. */
export function equiposEnAlmacen(alm: Equipo, equipos: Equipo[]): EnElAlmacen[] {
  const out: EnElAlmacen[] = [];
  for (const e of equipos) {
    if (e.modo === "trasladado") continue;
    for (const d of e.resumen?.destinos ?? []) {
      if (d.tipo !== "rest" || !esDeAlmacen(d, alm)) continue;
      const usuario = usuarioEnAlmacen(d.donde);
      if (!usuario) continue;
      const repos = (e.resumen?.repositorios ?? []).filter((r) => destinoDe(e.resumen?.destinos, r)?.id === d.id).map((repo) => ({ repo, carpeta: repo.ruta || repo.id }));
      if (!repos.length) continue;
      out.push({ equipo: e, destino: d, usuario, repos, importado: repos.every((r) => r.repo.solo_lectura) });
    }
  }
  return out;
}

/** Lo que pide «Ordenar nombres de carpeta» para un equipo: el usuario (si es de antes) y los repositorios con otra carpeta que su nombre. */
export interface PlanOrden {
  usuario: boolean;
  repos: { de: string; nombre: string; repo: RepositorioResumen; propuesta: string }[];
}

/** Qué se ordenaría (solo lo del dueño: un importado sigue al dueño). `soloRepo`: solo ese repositorio (sin la carpeta del equipo). */
export function planOrden(en: EnElAlmacen, soloRepo?: string): PlanOrden {
  const ocupadas = new Set(en.repos.map((r) => r.carpeta));
  const repos: PlanOrden["repos"] = [];
  for (const r of en.repos) {
    if (r.repo.solo_lectura || r.carpeta.includes("/") || r.carpeta === ".") continue;
    if (soloRepo && r.repo.id !== soloRepo) continue;
    if (carpetaLegible(r.carpeta, r.repo.nombre)) continue;
    ocupadas.delete(r.carpeta);
    const propuesta = carpetaNueva(r.repo.nombre, [...ocupadas, ...repos.map((x) => x.propuesta)]);
    ocupadas.add(r.carpeta);
    repos.push({ de: r.carpeta, nombre: r.repo.nombre, repo: r.repo, propuesta });
  }
  return { usuario: !soloRepo && usuarioDeAntes(en.usuario), repos };
}

/** ¿Hay algo que ordenar? */
export const hayQueOrdenar = (p: PlanOrden) => p.usuario || p.repos.length > 0;

/** El cuerpo de `guarda_copias { mover }` para el almacén. */
export const cuerpoMover = (en: EnElAlmacen, p: PlanOrden) => ({
  mover: { usuario: en.usuario, ...(p.usuario ? { legible: en.equipo.nombre } : {}), ...(p.repos.length ? { repos: p.repos.map((r) => ({ de: r.de, nombre: r.nombre })) } : {}) },
});

/** Lo que contestó el almacén (`detalle.mover`). */
export interface Movido {
  usuario?: { de: string; a: string } | null;
  repos: { de: string; a: string }[];
  zona?: string | null;
}

/** La dirección de un destino del almacén con otro usuario (`https://ip:puerto/<usuario>/`). */
export function dondeConUsuario(donde: string, usuario: string): string {
  const m = /^((?:rest:)?https?:\/\/[^/]+\/)[^/]+\/?$/i.exec(donde.trim());
  return m ? `${m[1]}${usuario}/` : donde;
}

/**
 * El cuerpo de `cambiar_destino` para un equipo con un destino en ese almacén, tras
 * mover: el usuario nuevo (si cambió) y la carpeta nueva de sus repositorios (con
 * `carpetas`, solo si alguno cambió). `null` si no le toca.
 */
export function cuerpoRutaNueva(en: Pick<EnElAlmacen, "destino" | "usuario" | "repos">, m: Movido): Record<string, unknown> | null {
  const usuarioNuevo = m.usuario && m.usuario.de === en.usuario ? m.usuario.a : null;
  const carpetas: Record<string, string> = {};
  for (const r of en.repos) {
    const x = m.repos.find((y) => y.de === r.carpeta);
    if (x) carpetas[r.repo.id] = x.a;
  }
  if (!usuarioNuevo && !Object.keys(carpetas).length) return null;
  return {
    destino: en.destino.id,
    ...(usuarioNuevo ? { donde: dondeConUsuario(en.destino.donde ?? "", usuarioNuevo), usuario: usuarioNuevo } : {}),
    ...(Object.keys(carpetas).length ? { carpetas } : {}),
  };
}

/** Un cambio de carpeta en el almacén (`guarda_copias.renombres`). */
export interface RenombreAlmacen {
  de: string;
  a: string;
  cuando: string;
  zona?: string | null;
}

/** Una ruta que el almacén movió y el equipo aún no usa. */
export interface RutaPendiente {
  almacen: Equipo;
  en: EnElAlmacen;
  movido: Movido;
  cuerpo: Record<string, unknown>;
  cuando: string;
}

/** Sigue una ruta (`usuario` o `usuario/carpeta`) por los cambios del almacén hasta la de ahora. */
function rutaActual(ruta: string, renombres: RenombreAlmacen[]): { ruta: string; cuando: string } | null {
  let r = ruta;
  let cuando = "";
  for (let i = 0; i < 8; i++) {
    const [u, c] = r.includes("/") ? [r.slice(0, r.indexOf("/")), r.slice(r.indexOf("/") + 1)] : [r, null];
    const exacto = [...renombres].reverse().find((x) => x.de === r);
    const delUsuario = c != null ? [...renombres].reverse().find((x) => x.de === u) : null;
    if (exacto) [r, cuando] = [exacto.a, exacto.cuando];
    else if (delUsuario) [r, cuando] = [`${delUsuario.a}/${c}`, delUsuario.cuando];
    else break;
  }
  return r === ruta ? null : { ruta: r, cuando };
}

/** Equipos (del cliente) que siguen con una ruta que el almacén `alm` ya movió. */
export function rutasPendientes(alm: Equipo, equipos: Equipo[]): RutaPendiente[] {
  const renombres = alm.resumen?.guarda_copias?.renombres ?? [];
  if (!renombres.length) return [];
  const out: RutaPendiente[] = [];
  for (const en of equiposEnAlmacen(alm, equipos)) {
    const movido: Movido = { repos: [] };
    let cuando = "";
    for (const r of en.repos) {
      const ahora = rutaActual(`${en.usuario}/${r.carpeta}`, renombres);
      if (!ahora) continue;
      cuando = ahora.cuando > cuando ? ahora.cuando : cuando;
      const [u, c] = [ahora.ruta.slice(0, ahora.ruta.indexOf("/")), ahora.ruta.slice(ahora.ruta.indexOf("/") + 1)];
      if (u !== en.usuario) movido.usuario = { de: en.usuario, a: u };
      if (c !== r.carpeta) movido.repos.push({ de: r.carpeta, a: c });
    }
    const cuerpo = cuerpoRutaNueva(en, movido);
    if (cuerpo) out.push({ almacen: alm, en, movido, cuerpo, cuando });
  }
  return out;
}

/** Los espejos del almacén que copian algo de ese usuario (lo que se volverá a subir). */
export function espejosQueLoCopian(alm: Equipo, usuario: string, carpetas: string[]): TrabajoEspejoResumen[] {
  const ts = alm.resumen?.guarda_copias?.espejo?.trabajos ?? [];
  return ts.filter(
    (t) =>
      t.quien !== "equipo" &&
      t.activo &&
      (t.que.tipo === "todos" || (t.que.tipo === "equipos" && t.que.equipos.includes(usuario)) || (t.que.tipo === "repos" && t.que.repos.some((r) => r === usuario || carpetas.some((c) => r === `${usuario}/${c}`)))),
  );
}

/** Qué pasa con la carpeta vieja en el destino de un espejo. */
export function textoCarpetaVieja(t: Pick<TrabajoEspejoResumen, "retencion" | "bloqueo">): string {
  if (t.bloqueo || t.retencion.modo === "nunca") return "la carpeta vieja se queda allí para siempre (este espejo nunca borra); puedes quitarla a mano cuando la nueva ya esté subida";
  if (t.retencion.modo === "igual") return "la carpeta vieja se quita la vez siguiente que copie, cuando la nueva ya está copiada";
  return `la carpeta vieja se quita pasados ${t.retencion.dias} días (su retención)`;
}

/** Nombres que se enseñan de lo que guarda un almacén: `usuario/carpeta` → «Contabilidad, de PC-01»; `usuario` → «PC-01». */
export function mapaNombres(alm: Equipo, equipos: Equipo[]): Map<string, string> {
  const m = new Map<string, string>();
  for (const en of equiposEnAlmacen(alm, equipos)) {
    if (en.importado) continue;
    if (!m.has(en.usuario)) m.set(en.usuario, en.equipo.nombre);
    for (const r of en.repos) m.set(`${en.usuario}/${r.carpeta}`, `${r.repo.nombre}, de ${en.equipo.nombre}`);
  }
  return m;
}

/** Todos los almacenes del cliente: el mapa de nombres de todos juntos. */
export function mapaNombresCliente(equipos: Equipo[]): Map<string, string> {
  const m = new Map<string, string>();
  for (const a of equipos.filter((e) => e.resumen?.guarda_copias?.activo)) for (const [k, v] of mapaNombres(a, equipos)) if (!m.has(k)) m.set(k, v);
  return m;
}

/**
 * Un texto con nombres de carpeta del almacén («Cambiar la retención del almacén en
 * 0b5c…/almacen-01-3f2a») con los nombres que se ven («…en «Contabilidad, de PC-01»»).
 * Lo que no se reconoce, tal cual.
 */
export function legibleEnTexto(texto: string, mapa: Map<string, string>): string {
  if (!mapa.size) return texto;
  return texto.replace(/«?([a-z0-9][a-z0-9-]{0,63})(?:\/([A-Za-z0-9._-]{1,128}))?»?/g, (todo, u: string, r?: string) => {
    const n = mapa.get(r ? `${u}/${r}` : u);
    return n && (r || usuarioDeAntes(u) || /-[0-9a-f]{4}$/.test(u)) ? `«${n}»` : todo;
  });
}
