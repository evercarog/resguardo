// «Todos los clientes» (v1.3x): lo de cada cliente del que la cuenta es
// miembro, junto. Todo sale de `GET /api/panel` (o, con un servidor anterior,
// de las rutas de cada cliente); aquí solo se calcula, sin runas, para
// probarlo en scripts/vectores-panel.ts:
//
// - la salud de cada cliente (en una palabra, con sus 14 días),
// - «lo que necesita atención» de todos, lo más grave arriba y con su acción,
// - las cifras de arriba (equipos al día, protegido, copias en 24 h, próxima),
// - «¿Cuándo se llena?» de todos los almacenes y destinos,
// - el mapa de la protección con un nivel más: clientes → equipos → … .
import type { Equipo, Informe, MarcaCliente, Rol, TareaEnMarcha } from "./tipos";
import { construirMapa, ordenarMapa, type AristaMapa, type Mapa, type NodoMapa } from "./mapa";
import { dias, type Dia } from "./repo";
import { bytesRepo, informeDe } from "./repo";
import { copiaAtrasada, PESO, saludEquipo, type Tono } from "./salud";
import { proximaDeTodos, ultimas24h } from "./panel";
import { previsiones, type Prevision } from "./llenado";
import { plural } from "./formato";

/** Lo de un cliente en `GET /api/panel`. */
export interface PanelCliente {
  id: string;
  nombre: string;
  rol: Rol;
  marca?: MarcaCliente | null;
  equipos: Equipo[];
  avisos_abiertos: number;
  pendientes: number;
  /** El último informe de cada equipo, resumido (las versiones y vueltas recientes). */
  informes: { equipo: string; recibido: string; datos: Informe["datos"] }[];
  /** false: algún informe no cupo en la respuesta (el panel sigue con el resumen). */
  informes_completos: boolean;
}

/** Algo en marcha en un equipo de un cliente. */
export interface ProgresoPanel {
  cliente: string;
  equipo: string;
  recibido: string;
  tareas: TareaEnMarcha[];
}

export interface Panel {
  generado: string;
  clientes: PanelCliente[];
  /** Clientes que no caben en una respuesta (más de 100). */
  omitidos: number;
  progreso: ProgresoPanel[];
}

const minus = (t: string) => t.charAt(0).toLowerCase() + t.slice(1);
export const sinTildes = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

/** Los informes de un cliente por equipo (como `ultimos.porEquipo`). */
export function informesDe(c: PanelCliente): Record<string, Informe | null> {
  return Object.fromEntries(c.informes.map((x) => [x.equipo, { recibido: x.recibido, datos: x.datos }]));
}

const activos = (c: PanelCliente) => c.equipos.filter((e) => e.modo !== "trasladado");

// ---------------------------------------------------------------------------
// Salud de cada cliente
// ---------------------------------------------------------------------------

export interface SaludCliente {
  tono: Tono;
  /** En una palabra o dos: «Al día», «2 con problemas», «Sin equipos»… */
  texto: string;
  /** Una frase. */
  detalle: string;
  equipos: number;
  alDia: number;
  /** Equipos con un problema (rojo) o un aviso (ámbar). */
  problemas: number;
  /** La última copia de cualquiera de sus equipos. */
  ultima: string | null;
  /** Los 14 días de todos sus equipos juntos (el más reciente al final). */
  dias: Dia[];
  protegido: number;
}

export function saludCliente(c: PanelCliente, ahora = Date.now()): SaludCliente {
  const es = activos(c);
  const saludes = es.map((e) => saludEquipo(e, ahora));
  const informes = informesDe(c);
  const alDia = saludes.filter((s) => s.tono === "ok").length;
  const malos = saludes.filter((s) => s.tono === "bad").length;
  const avisos = saludes.filter((s) => s.tono === "warn").length;
  const problemas = malos + avisos;
  const sinCopias = saludes.filter((s) => s.texto === "Sin copias todavía" || s.texto === "Sin copias").length;
  const ultima =
    es
      .flatMap((e) => [...(e.resumen?.copias ?? []).map((k) => k.ultima?.cuando), ...(informes[e.id]?.datos.copias ?? []).map((k) => k.cuando)])
      .filter((x): x is string => !!x)
      .sort()
      .at(-1) ?? null;
  // Los cuadros: las versiones y vueltas de todos los repositorios de todos sus equipos.
  const repos = es.flatMap((e) => informes[e.id]?.datos?.repos ?? []);
  const junto = {
    id: "",
    nombre: "",
    versiones: repos.flatMap((r) => r.versiones ?? []),
    versiones_leidas: null,
    ejecuciones: repos.flatMap((r) => r.ejecuciones ?? []),
    espacio: null,
    verificacion: null,
    prueba_restauracion: null,
    externa: null,
    proteccion: null,
  };
  const protegido = es.flatMap((e) => (e.resumen?.repositorios ?? []).map((r) => bytesRepo(r, informeDe(informes[e.id], r.id)) ?? 0)).reduce((a, b) => a + b, 0);
  const base = { equipos: es.length, alDia, problemas, ultima, dias: dias(repos.length ? junto : null, 14, ahora), protegido };
  if (!es.length) return { ...base, tono: "neutral", texto: "Sin equipos", detalle: "Todavía no tiene equipos." };
  if (malos) return { ...base, tono: "bad", texto: problemas === 1 ? "1 con problemas" : `${problemas} con problemas`, detalle: `${plural(problemas, "equipo necesita", "equipos necesitan")} atención.` };
  if (avisos) return { ...base, tono: "warn", texto: avisos === 1 ? "1 con avisos" : `${avisos} con avisos`, detalle: `${plural(avisos, "equipo tiene", "equipos tienen")} avisos o va atrasado.` };
  if (c.avisos_abiertos) return { ...base, tono: "warn", texto: "Avisos sin revisar", detalle: `${plural(c.avisos_abiertos, "aviso sin revisar", "avisos sin revisar")}.` };
  if (sinCopias === es.length) return { ...base, tono: "neutral", texto: "Sin copias todavía", detalle: "Ningún equipo ha hecho su primera copia." };
  if (saludes.some((s) => s.tono === "paused")) return { ...base, tono: "paused", texto: "En pausa", detalle: "Algún equipo tiene las copias en pausa." };
  return { ...base, tono: "ok", texto: "Al día", detalle: sinCopias ? `${plural(sinCopias, "equipo aún sin copias", "equipos aún sin copias")}; el resto, protegido.` : "Todo protegido." };
}

// ---------------------------------------------------------------------------
// Lo que necesita atención, de todos los clientes
// ---------------------------------------------------------------------------

export interface Atencion {
  cliente: PanelCliente;
  tono: Tono;
  /** Quién y qué le pasa: «CONTABILIDAD · copia fallida». */
  titulo: string;
  detalle: string;
  accion: { texto: string; href?: string; orden?: { equipo: Equipo; repo: string; copia: string; nombre: string } };
}

const puedeOrdenar = (r: Rol) => r === "propietario" || r === "administrador" || r === "tecnico";

export function atencion(clientes: PanelCliente[], ahora = Date.now()): Atencion[] {
  const out: (Atencion & { orden: string })[] = [];
  for (const c of clientes) {
    for (const e of activos(c)) {
      const s = saludEquipo(e, ahora);
      if (s.tono !== "bad" && s.tono !== "warn") continue;
      const copia = (e.resumen?.copias ?? []).find((x) => x.ultima?.estado === "fallo" || copiaAtrasada(x, ahora));
      const sinContacto = s.texto === "Sin contacto" || s.texto === "Detenido";
      out.push({
        cliente: c,
        tono: s.tono,
        titulo: `${e.nombre} · ${minus(s.texto)}`,
        detalle: s.detalle,
        accion:
          copia && !sinContacto && puedeOrdenar(c.rol)
            ? { texto: "Copiar ahora", orden: { equipo: e, repo: copia.repo, copia: copia.id, nombre: copia.nombre } }
            : { texto: "Ver equipo", href: `/c/${c.id}/equipos/${e.id}` },
        orden: e.nombre,
      });
    }
    if (c.avisos_abiertos)
      out.push({ cliente: c, tono: "warn", titulo: plural(c.avisos_abiertos, "aviso sin revisar", "avisos sin revisar"), detalle: "Revísalos y márcalos como vistos.", accion: { texto: "Ver avisos", href: `/c/${c.id}/avisos` }, orden: "~" });
    if (c.pendientes)
      out.push({ cliente: c, tono: "info", titulo: plural(c.pendientes, "orden esperando su turno", "órdenes esperando su turno"), detalle: "Se aplicarán cuando pase la espera; aún se pueden cancelar.", accion: { texto: "Ver órdenes", href: `/c/${c.id}/ordenes` }, orden: "~~" });
  }
  return out
    .sort((a, b) => PESO[a.tono] - PESO[b.tono] || a.cliente.nombre.localeCompare(b.cliente.nombre) || a.orden.localeCompare(b.orden))
    .map(({ orden: _o, ...x }) => x);
}

// ---------------------------------------------------------------------------
// Las cifras de arriba
// ---------------------------------------------------------------------------

export interface Totales {
  equipos: number;
  alDia: number;
  /** Equipos por tono (para la barra). */
  porTono: Record<Tono, number>;
  protegido: number;
  repos: number;
  versiones24: number;
  fallos24: number;
  proxima: { cuando: string; equipo: Equipo; copia: string; cliente: PanelCliente } | null;
}

export function totales(clientes: PanelCliente[], ahora = Date.now()): Totales {
  const porTono: Record<Tono, number> = { ok: 0, warn: 0, bad: 0, info: 0, paused: 0, neutral: 0 };
  let protegido = 0;
  let repos = 0;
  let versiones24 = 0;
  let fallos24 = 0;
  let proxima: Totales["proxima"] = null;
  for (const c of clientes) {
    const es = activos(c);
    const inf = informesDe(c);
    for (const e of es) porTono[saludEquipo(e, ahora).tono]++;
    for (const e of es)
      for (const r of e.resumen?.repositorios ?? []) {
        repos++;
        protegido += bytesRepo(r, informeDe(inf[e.id], r.id)) ?? 0;
      }
    const u = ultimas24h(es, inf, ahora);
    versiones24 += u.versiones;
    fallos24 += u.fallos;
    const p = proximaDeTodos(es, ahora);
    if (p && (!proxima || p.cuando < proxima.cuando)) proxima = { ...p, cliente: c };
  }
  const equipos = Object.values(porTono).reduce((a, b) => a + b, 0);
  return { equipos, alDia: porTono.ok, porTono, protegido, repos, versiones24, fallos24, proxima };
}

// ---------------------------------------------------------------------------
// ¿Cuándo se llena?, de todos
// ---------------------------------------------------------------------------

export interface PrevisionGlobal extends Prevision {
  cliente: PanelCliente;
}

/** Todos los almacenes y destinos de todos los clientes: lo que se llena antes, arriba. */
export function llenadoGlobal(clientes: PanelCliente[], ahora = Date.now()): PrevisionGlobal[] {
  const xs = clientes.flatMap((c) => previsiones(c.equipos, informesDe(c), c.id, ahora).map((p) => ({ ...p, clave: `${c.id}/${p.clave}`, cliente: c })));
  return xs.sort((a, b) => PESO[a.tono] - PESO[b.tono] || (a.lleno ?? Infinity) - (b.lleno ?? Infinity) || (b.usado ?? 0) - (a.usado ?? 0) || a.nombre.localeCompare(b.nombre));
}

// ---------------------------------------------------------------------------
// El mapa de todos los clientes
// ---------------------------------------------------------------------------

export type FiltroEstado = "todos" | "problemas" | "al_dia";

export interface OpcionesMapaGlobal {
  ahora?: number;
  /** Clientes plegados (solo su tarjeta). */
  plegados?: ReadonlySet<string>;
  /** «» = todos; si no, el id de un cliente. */
  cliente?: string;
  estado?: FiltroEstado;
  /** Palabras que buscar en los nombres (clientes, equipos, repositorios, destinos). */
  buscar?: string;
  enVivo?: (equipo: string, repo: string, tipo: "copia" | "copia_externa") => string | null;
  /** A partir de cuántos equipos de un cliente se juntan los que están al día (por defecto, 6). */
  agruparDesde?: number;
}

export interface MapaGlobal extends Mapa {
  /** Los clientes que salen (con su salud) y cuántos no pasan el filtro. */
  clientes: { cliente: PanelCliente; salud: SaludCliente; plegado: boolean }[];
  fuera: number;
}

/** Los clientes que empiezan plegados: con muchos, los que van bien y no tienen nada en marcha. */
export function plegadosPorDefecto(clientes: PanelCliente[], enMarcha: (c: string) => boolean, ahora = Date.now(), desde = 4): Set<string> {
  if (clientes.length < desde) return new Set();
  return new Set(clientes.filter((c) => !enMarcha(c.id) && ["ok", "neutral", "paused"].includes(saludCliente(c, ahora).tono)).map((c) => c.id));
}

/**
 * El mapa de todos los clientes: una columna más a la izquierda (los
 * clientes) y, de cada uno, su mapa de siempre (lib/mapa.ts) con los ids
 * con el cliente delante (los destinos de dos clientes nunca se juntan).
 */
export function construirMapaGlobal(clientes: PanelCliente[], o: OpcionesMapaGlobal = {}): MapaGlobal {
  const ahora = o.ahora ?? Date.now();
  const palabras = sinTildes(o.buscar?.trim() ?? "").split(/\s+/).filter(Boolean);
  const casa = (t: string) => palabras.every((p) => sinTildes(t).includes(p));
  const nodos: NodoMapa[] = [];
  const aristas: AristaMapa[] = [];
  const frases: string[] = [];
  const salen: MapaGlobal["clientes"] = [];
  let fuera = 0;
  const orden = [...clientes]
    .map((c) => ({ c, s: saludCliente(c, ahora) }))
    .sort((a, b) => PESO[a.s.tono] - PESO[b.s.tono] || a.c.nombre.localeCompare(b.c.nombre));
  for (const { c, s } of orden) {
    if (o.cliente && o.cliente !== c.id) {
      fuera++;
      continue;
    }
    const bien = s.tono === "ok" || s.tono === "neutral" || s.tono === "paused";
    if ((o.estado === "problemas" && bien) || (o.estado === "al_dia" && s.tono !== "ok")) {
      fuera++;
      continue;
    }
    const m = construirMapa(c.equipos, informesDe(c), { cliente: c.id, ahora, enVivo: o.enVivo, agruparDesde: o.agruparDesde ?? 6 });
    const pre = (id: string) => `${c.id}/${id}`;
    let suyos = m.nodos.map((n) => ({ ...n, id: pre(n.id), col: (n.col + 1) as NodoMapa["col"] }));
    let susAristas = m.aristas.map((a) => ({ ...a, id: pre(a.id), de: pre(a.de), a: pre(a.a) }));
    // Buscar: si el cliente no casa, solo las cadenas de lo que casa (y nada si no casa nada).
    if (palabras.length && !casa(c.nombre)) {
      const quedan = new Set<string>();
      const ir = (id: string, dir: "de" | "a") => {
        for (const x of susAristas) {
          const [desde, hasta] = dir === "de" ? [x.de, x.a] : [x.a, x.de];
          if (desde === id && !quedan.has(hasta)) {
            quedan.add(hasta);
            ir(hasta, dir);
          }
        }
      };
      for (const n of suyos.filter((n) => casa(n.nombre))) {
        quedan.add(n.id);
        ir(n.id, "de");
        ir(n.id, "a");
      }
      if (!quedan.size) {
        fuera++;
        continue;
      }
      suyos = suyos.filter((n) => quedan.has(n.id));
      susAristas = susAristas.filter((a) => quedan.has(a.de) && quedan.has(a.a));
    }
    // Plegado: solo su tarjeta (salvo que se esté buscando dentro).
    const plegado = !!o.plegados?.has(c.id) && !(palabras.length && !casa(c.nombre));
    const vivo = suyos.find((n) => n.vivo)?.vivo ?? null;
    const raiz: NodoMapa = {
      id: `cl:${c.id}`,
      tipo: "cliente",
      col: 0,
      nombre: c.nombre,
      sub: [plural(s.equipos, "equipo", "equipos"), s.problemas ? null : s.equipos ? `${s.alDia} al día` : null].filter(Boolean).join(" · "),
      tono: s.tono,
      estado: s.texto,
      ultima: s.ultima,
      href: `/c/${c.id}`,
      icono: "cliente",
      marca: c.marca ?? null,
      plegado,
      vivo: plegado ? vivo : null,
    };
    nodos.push(raiz);
    salen.push({ cliente: c, salud: s, plegado });
    frases.push(`${c.nombre}: ${minus(s.texto)}${s.equipos ? `, ${plural(s.equipos, "equipo", "equipos")}` : ""}${plegado ? " (plegado)" : ""}.`);
    if (plegado) continue;
    for (const n of suyos) nodos.push(n);
    for (const a of susAristas) aristas.push(a);
    for (const n of suyos.filter((x) => x.col === 1)) {
      const vivoAqui = susAristas.some((a) => a.de === n.id && a.vivo);
      aristas.push({ id: `${raiz.id}>${n.id}`, de: raiz.id, a: n.id, tipo: "cliente", tono: n.tono, vivo: vivoAqui });
    }
    frases.push(...m.frases.filter((f) => !palabras.length || suyos.some((n) => f.includes(n.nombre))));
  }
  return { nodos: ordenarMapa(nodos, aristas, true), aristas, frases, clientes: salen, fuera };
}
