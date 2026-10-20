// Un almacén es uno solo (plan 0.8.1, B3 y B6; plan-mejoras 15dd y 15k).
//
// - `almacenDetras`: el almacén del cliente que hay detrás de un destino de un
//   equipo. Además de los destinos que dio el almacén («Copiar en …», con
//   `equipo_almacen`), un destino «Servidor de copias» genérico que apunta al
//   mismo servidor (o a localhost desde el propio almacén) es ese almacén: por
//   ahí llegan los repositorios **adoptados** (antes de que el almacén estuviera
//   gestionado). Lo usan el mapa (14mm), la lista de destinos, «Se guarda en» y
//   la ficha del equipo, para que un almacén salga una sola vez.
// - `contenidoAlmacen` y `duenoDeCarpeta`: «¿de quién es esta carpeta del
//   almacén?» (`<usuario>/<carpeta>`, lo que usan los espejos y la retención):
//   un repositorio de un equipo (también uno adoptado) o una copia derivada de
//   un repositorio (su carpeta por defecto es `<id del repositorio>-<id de la
//   derivada>`, como en el agente). Si solo se reconoce el usuario, de qué
//   equipo es; si ni eso, «no sé de qué equipo es» (nunca «no está en esta consola»).
//
// Sin dependencias de Svelte ni de destinos.ts (para no hacer un ciclo): lo
// prueban los vectores (scripts/vectores-almacen-unico.ts).
import type { DestinoResumen, Equipo, RepositorioResumen, RetencionAlmacen } from "./tipos";
import { destinoDe } from "./repo";
import { esDeAlmacen } from "./retencion";

/** La zona principal de un almacén (lo mismo que `PRINCIPAL` de destinos.ts). */
const PRINCIPAL = "principal";

/** «host:puerto» de una dirección («https://nas:8000/caja-1/» → «nas:8000»), sin usuario. */
export function servidorDe(donde: string | null | undefined): string | null {
  if (!donde) return null;
  const sin = donde.trim().replace(/^[a-z0-9+]+:(?=[a-z]+:\/\/)/i, "");
  try {
    const u = new URL(sin.includes("://") ? sin : `x://${sin}`);
    return u.host || null;
  } catch {
    return sin.replace(/^[^@]*@/, "").split("/")[0] || null;
  }
}

/** El nombre del servidor de una dirección, sin usuario ni puerto («https://nas:8002/caja/» → «nas»). */
export function servidorSinPuerto(donde: string | null | undefined): string | null {
  const h = servidorDe(donde);
  return h ? h.replace(/^[^@]*@/, "").replace(/:\d+$/, "").toLowerCase() || null : null;
}

/** El almacén del cliente que dio este destino (por `equipo_almacen`, su id o su nombre), o undefined. */
export function almacenDeDestino(d: DestinoResumen | undefined, equipos: Equipo[]): Equipo | undefined {
  if (!d || d.tipo !== "rest") return undefined;
  return equipos.find((a) => a.resumen?.guarda_copias?.activo && a.modo !== "trasladado" && esDeAlmacen(d, a)) ?? (d.equipo_almacen ? equipos.find((a) => a.id === d.equipo_almacen) : undefined);
}

const LOCALES = new Set(["localhost", "127.0.0.1", "[::1]", "::1"]);

/**
 * 14mm y 15dd: el almacén del cliente que hay detrás de un destino de `e`. Además de los
 * que dio «Copiar en …» (`equipo_almacen`) y de los que se reconocen por su id o su
 * nombre, un destino «Servidor de copias» (REST, p. ej. el de los repositorios
 * adoptados) es el mismo almacén si apunta al mismo servidor que los accesos que ese
 * almacén dio a otros equipos, o a «localhost» desde el propio almacén.
 */
export function almacenDetras(e: Pick<Equipo, "id" | "resumen">, d: DestinoResumen | undefined, equipos: Equipo[]): Equipo | undefined {
  if (!d) return undefined;
  if (d.equipo_almacen) return equipos.find((x) => x.id === d.equipo_almacen);
  if (d.tipo !== "rest") return undefined;
  const conocido = almacenDeDestino(d, equipos);
  if (conocido) return conocido;
  const host = servidorSinPuerto(d.donde);
  if (!host) return undefined;
  if (LOCALES.has(host)) return e.resumen?.guarda_copias?.activo ? equipos.find((x) => x.id === e.id) : undefined;
  for (const x of equipos)
    for (const y of x.resumen?.destinos ?? []) {
      if (!y.equipo_almacen || y.tipo !== "rest" || servidorSinPuerto(y.donde) !== host) continue;
      const a = equipos.find((z) => z.id === y.equipo_almacen && z.resumen?.guarda_copias?.activo && z.modo !== "trasladado");
      if (a) return a;
    }
  return undefined;
}

/**
 * 15dd: ¿llega al almacén por la dirección del servidor (un «Servidor de copias»
 * genérico) en vez de por el acceso que dio el almacén? Es lo que pasa con los
 * repositorios adoptados.
 */
export function esAdoptado(e: Pick<Equipo, "id" | "resumen">, d: DestinoResumen | undefined, equipos: Equipo[]): boolean {
  if (!d || d.tipo !== "rest" || d.equipo_almacen) return false;
  const a = almacenDetras(e, d, equipos);
  return !!a && !esDeAlmacen(d, a);
}

export const TEXTO_ADOPTADO = "Adoptado: llega por la dirección del servidor, no por el usuario de cada equipo.";

/** El puerto de una dirección («https://nas:8002/caja/» → 8002), o null. */
function puerto(donde: string | null | undefined): number | null {
  const m = /^(?:[a-z0-9+]+:)?[a-z]+:\/\/[^/]*?:(\d{1,5})(?:\/|$)/i.exec((donde ?? "").trim()) ?? /^[^/]*?:(\d{1,5})(?:\/|$)/.exec((donde ?? "").trim());
  const n = m ? Number(m[1]) : NaN;
  return Number.isInteger(n) && n > 0 && n < 65536 ? n : null;
}

/** La zona de un almacén a la que va una dirección (por su puerto; sin puerto o con el de la principal, la principal). */
export function zonaPorPuerto(almacen: Pick<Equipo, "resumen">, donde: string | null | undefined): string {
  const p = puerto(donde);
  if (p == null) return PRINCIPAL;
  return almacen.resumen?.guarda_copias?.zonas?.find((z) => z.puerto === p)?.id ?? PRINCIPAL;
}

/**
 * Cómo se llama una carpeta dentro del almacén (`<usuario>/<carpeta>`): la ruta de la
 * dirección del destino más la carpeta propia (`.` o vacía: la del destino).
 */
export function rutaEnElAlmacen(donde: string | null | undefined, propia: string): { valor: string; usuario: string } | null {
  const sin = (donde ?? "").trim().replace(/^[a-z0-9+]+:(?=[a-z]+:\/\/)/i, "");
  let ruta = "";
  try {
    ruta = new URL(sin.includes("://") ? sin : `x://${sin}`).pathname;
  } catch {
    ruta = sin.replace(/^[^/]*/, "");
  }
  const partes = [...ruta.split("/"), ...(propia === "." ? "" : propia).split("/")]
    .map((x) => {
      try {
        return decodeURIComponent(x);
      } catch {
        return x;
      }
    })
    .filter(Boolean);
  if (!partes.length) return null;
  return { valor: partes.join("/"), usuario: partes[0] };
}

/** Una carpeta del almacén y de quién es. */
export interface CarpetaDelAlmacen {
  /** `<usuario>/<carpeta>` (o `<usuario>`), como la ven los espejos y la retención. */
  valor: string;
  usuario: string;
  zona: string;
  /** `repo`: un repositorio; `derivada`: una copia derivada; `equipo`: solo se sabe el equipo; `desconocida`: nada. */
  tipo: "repo" | "derivada" | "equipo" | "desconocida";
  equipo?: Equipo;
  /** El repositorio (o, en una derivada, su repositorio de origen). */
  repo?: RepositorioResumen;
  /** La derivada (`externa` la de siempre). */
  derivada?: { id: string; externa?: boolean; destino?: string | null };
  /** Llega por un «Servidor de copias» genérico (15dd). */
  adoptado?: boolean;
  /** «Contabilidad», «Contabilidad (copia derivada)» o la carpeta tal cual. */
  nombre: string;
  /** «PC-01», «no sé de qué equipo es»… */
  de: string;
  /** Todo junto: «Contabilidad (copia derivada) · PC-01». */
  texto: string;
}

const clave = (zona: string, valor: string) => `${zona}|${valor}`;

/** Las derivadas de un repositorio con su carpeta por defecto (la externa de siempre: su id; las demás: `<repo>-<id>`). */
function derivadasConCarpeta(r: RepositorioResumen): { id: string; nombre?: string | null; externa?: boolean; destino_id?: string | null; destino?: string | null; carpeta: string }[] {
  const l: { id: string; nombre?: string | null; externa?: boolean; destino_id?: string | null; destino?: string | null; carpeta: string }[] = [];
  if (r.externa) l.push({ id: "externa", externa: true, destino_id: r.externa.destino_id, destino: r.externa.destino, carpeta: r.id });
  for (const d of r.derivadas ?? []) l.push({ id: d.id, nombre: d.nombre, destino_id: d.destino_id, destino: d.destino, carpeta: `${r.id}-${d.id}` });
  return l;
}

/**
 * 15k: todo lo que los equipos de esta consola tienen en el almacén `almacen`, por zona y
 * carpeta: sus repositorios (también los adoptados) y sus copias derivadas. Un repositorio
 * importado (de solo lectura) no tapa al de su dueño.
 */
export function contenidoAlmacen(almacen: Pick<Equipo, "id">, equipos: Equipo[]): Map<string, CarpetaDelAlmacen> {
  const m = new Map<string, CarpetaDelAlmacen>();
  const poner = (x: CarpetaDelAlmacen) => {
    const k = clave(x.zona, x.valor);
    const ya = m.get(k);
    if (!ya || (ya.repo?.solo_lectura && !x.repo?.solo_lectura && x.tipo === "repo")) m.set(k, x);
  };
  for (const e of equipos) {
    if (e.modo === "trasladado") continue;
    const destinos = e.resumen?.destinos ?? [];
    for (const r of e.resumen?.repositorios ?? []) {
      const d = destinoDe(destinos, r);
      const a = almacenDetras(e, d, equipos);
      if (d && a?.id === almacen.id) {
        const en = rutaEnElAlmacen(d.donde, r.ruta || r.id);
        if (en) {
          const nombre = r.nombre || r.id;
          poner({ ...en, zona: zonaPorPuerto(a, d.donde), tipo: "repo", equipo: e, repo: r, adoptado: esAdoptado(e, d, equipos), nombre, de: e.nombre, texto: `${nombre} · ${e.nombre}` });
        }
      }
      for (const dv of derivadasConCarpeta(r)) {
        const dd = destinos.find((y) => y.id === dv.destino_id) ?? destinos.find((y) => y.nombre === dv.destino);
        const ad = almacenDetras(e, dd, equipos);
        if (!dd || ad?.id !== almacen.id) continue;
        const en = rutaEnElAlmacen(dd.donde, dv.carpeta);
        if (!en) continue;
        // 15t: su nombre, si se le puso.
        const nombre = dv.nombre || `${r.nombre || r.id} (copia derivada)`;
        poner({ ...en, zona: zonaPorPuerto(ad, dd.donde), tipo: "derivada", equipo: e, repo: r, derivada: { id: dv.id, externa: dv.externa, destino: dv.destino }, adoptado: esAdoptado(e, dd, equipos), nombre, de: e.nombre, texto: `${nombre} · ${e.nombre}` });
      }
    }
  }
  return m;
}

/**
 * 15k: ¿de quién es esta carpeta del almacén? (`<usuario>/<carpeta>` o `<usuario>`, en la
 * zona `zona`; `null`: en cualquiera). Con `contenido` ya calculado (`contenidoAlmacen`) para no repetirlo en
 * cada fila de una lista.
 */
export function duenoDeCarpeta(almacen: Pick<Equipo, "id">, valor: string, equipos: Equipo[], zona: string | null = PRINCIPAL, contenido?: Map<string, CarpetaDelAlmacen>): CarpetaDelAlmacen {
  const m = contenido ?? contenidoAlmacen(almacen, equipos);
  // `zona: null`: en cualquier zona (la retención del almacén no dice la zona), primero la principal.
  const exacta = zona == null ? (m.get(clave(PRINCIPAL, valor)) ?? [...m.values()].find((x) => x.valor === valor)) : m.get(clave(zona, valor));
  if (exacta) return exacta;
  zona = zona ?? PRINCIPAL;
  const usuario = valor.split("/")[0];
  const carpeta = valor.includes("/") ? valor.slice(valor.indexOf("/") + 1) : "";
  // Solo el usuario: un equipo entero (lo que eligen los espejos «por equipo»).
  const delUsuario = [...m.values()].find((x) => x.usuario === usuario && x.tipo === "repo" && !x.repo?.solo_lectura) ?? [...m.values()].find((x) => x.usuario === usuario);
  if (delUsuario?.equipo) {
    if (!carpeta) return { valor, usuario, zona, tipo: "equipo", equipo: delUsuario.equipo, nombre: delUsuario.equipo.nombre, de: delUsuario.equipo.nombre, texto: `Todo lo de ${delUsuario.equipo.nombre}` };
    return { valor, usuario, zona, tipo: "equipo", equipo: delUsuario.equipo, nombre: carpeta, de: delUsuario.equipo.nombre, texto: `${carpeta} · ${delUsuario.equipo.nombre}` };
  }
  return { valor, usuario, zona, tipo: "desconocida", nombre: valor, de: "no sé de qué equipo es", texto: `${valor} · no sé de qué equipo es` };
}

/** Nombres de lo que guarda un almacén para textos (`usuario/carpeta` → «Contabilidad (copia derivada) · PC-01»). */
export function nombresDelAlmacen(almacen: Pick<Equipo, "id">, equipos: Equipo[]): Map<string, string> {
  const m = new Map<string, string>();
  for (const x of contenidoAlmacen(almacen, equipos).values()) if (!m.has(x.valor)) m.set(x.valor, x.texto);
  return m;
}

/**
 * 15s: la retención que aplica un almacén, con el dueño de cada carpeta, y lo que tiene
 * sin retención allí (repositorios y copias derivadas de los equipos de esta consola;
 * un importado no cuenta: sigue al de su dueño). Un almacén que no manda la lista
 * (agente anterior a v1.22) no puede aplicarla: `admite` false.
 */
export function retencionesDelAlmacen(alm: Equipo, equipos: Equipo[]): { admite: boolean; con: { ra: RetencionAlmacen; dueno: CarpetaDelAlmacen }[]; sin: CarpetaDelAlmacen[] } {
  const lista = alm.resumen?.guarda_copias?.retenciones;
  const contenido = contenidoAlmacen(alm, equipos);
  const con = (lista ?? []).map((ra) => ({ ra, dueno: duenoDeCarpeta(alm, `${ra.usuario}/${ra.repo}`, equipos, null, contenido) }));
  const tienen = new Set((lista ?? []).map((ra) => `${ra.usuario}/${ra.repo}`));
  const sin = [...contenido.values()].filter((x) => (x.tipo === "repo" || x.tipo === "derivada") && !x.repo?.solo_lectura && !tienen.has(x.valor));
  return { admite: Array.isArray(lista), con, sin };
}
