// «Mapa de la protección» (docs/diseno.md §4): por dónde pasan los datos del
// cliente, de izquierda a derecha y en columnas:
//
//   equipos → repositorios (píldoras) → almacén o destino → espejo y copia externa
//
// Todo sale del resumen y del último informe de cada equipo; lo que está en
// marcha llega como una función (`enVivo`) para que esto sea TypeScript sin
// runas (se prueba en scripts/vectores.ts y scripts/vectores-mapa.ts).
//
// Plan 0.8.1 (C2): los grupos («8 equipos») se juntan **por sede** (o por la
// agrupación de Equipos), con «Agrupar: no / por sede / cuando hay más de N», y
// cada uno se despliega con un clic (`desplegados`); al pasar por un grupo, la
// lista de sus equipos y repositorios con su estado (`miembros`). Las copias
// derivadas y los espejos se animan cuando están en marcha (15cc), un espejo
// cuyos repositorios no se reconocen se dibuja desde el almacén con una marca
// (15m), y cada tarjeta sabe su sede y dónde está físicamente (las capas, 15l).
//
// Plan 0.8.2: filtrar por infraestructura (A4, 15xx: `filtroInfra`, un servidor
// físico o un disco o conjunto, con lo que guarda o recibe) y, en Equipos (C4),
// «Agrupar por» manda: recuadros por grupo (`agrupar: { modo: "recuadros" }` con
// `grupoDe`, plegables con `plegados`) y los equipos en el orden de la lista
// (`ordenEquipos`).
import type { DestinoCatalogo, DestinoResumen, Equipo, Informe, MarcaCliente, RepositorioResumen } from "./tipos";
import { sedeDeDestino, sedeDeEquipo, type Organizacion } from "./sedes";
import { detalleDisco, enFiltroInfra, equipoEnFiltroInfra, formaConjunto, sitioDeDestino, sitioDeEquipo, textoFiltroInfra, HIPERVISORES, type FiltroInfra, type Infraestructura, type SitioVista } from "./infra";
import { bytesRepo, destinoDe, estadoRepo, informeDe, ultimaVersion } from "./repo";
import { PESO, resultadoConError, saludEquipo, type Tono } from "./salud";
import { cuandoCorto, cuandoCortoEspejo } from "./espejo";
import { bytes, lista, plural, relativo, resumenHorario } from "./formato";
import { lugarDe, riesgoMismoEquipo } from "./dondeGuarda";
import { almacenDetras, rutaEnElAlmacen, zonaPorPuerto } from "./almacenUnico";
import { claveNube, claveZona, PRINCIPAL, zonaDeDestino, zonasDe } from "./destinos";
// Las páginas de los destinos (como las de los repositorios): cada tarjeta de destino lleva a la suya.
import { hrefDestino } from "./fichaDestino";
import { hrefEspejo } from "./espejoTrabajos";
import { cuandoEnFrase, destinosParaPasos } from "./cadenas";
import { clasificacionDeVista } from "./regla321";
import { corta, tipoDeNube, type TipoCorto } from "./tipoDestino";
import { NOMBRE_TIPO_DESTINO } from "./direccion";

/** «cliente»: solo en el mapa de todos los clientes (lib/global.ts), una columna antes que los equipos. */
/**
 * «fuera» (v1.56): un paso que depende de un equipo que no está en esta consola (el
 * almacén donde guarda, que se gestiona desde otra): sus espejos no se ven aquí.
 */
export type TipoNodo = "cliente" | "equipo" | "grupo" | "repo" | "destino" | "espejo" | "externa" | "fuera";
export type IconoNodo = "cliente" | "equipo" | "grupo" | "almacen" | "disco" | "nube" | "dropbox" | "servidor" | "repo" | "otra_consola";
export type Perspectiva = "equipos" | "repositorios" | "destinos";

export interface NodoMapa {
  id: string;
  tipo: TipoNodo;
  /** Columna: 0 equipos, 1 repositorios, 2 almacén o destino, 3 espejo y copia externa (en el de todos los clientes, una más: 0 son los clientes). */
  col: 0 | 1 | 2 | 3 | 4;
  nombre: string;
  /** Una línea pequeña: «3 copias», «Almacén · 2 repositorios», «cada noche a las 02:00». */
  sub: string;
  tono: Tono;
  /** El estado en palabras (va siempre con su icono). */
  estado: string;
  /** La última vez que salió bien (o la última vez, si es lo que se sabe). */
  ultima: string | null;
  href?: string;
  icono: IconoNodo;
  /** Cifra pequeña en mono («24 GB»). */
  cifra?: string;
  /** Algo en marcha ahora (una copia o una copia externa). */
  vivo?: string | null;
  /** v1.41: un aviso que se ve en la tarjeta (con icono): «En el mismo equipo que protege». */
  aviso?: string;
  /** «fuera»: el nombre del almacén (el de su destino en el equipo) y las otras consolas del equipo que copia ahí (por su nombre). */
  fuera?: { almacen: string; consolas: string[] };
  /** Un cliente (mapa de todos los clientes): su marca y si está plegado. */
  marca?: MarcaCliente | null;
  plegado?: boolean;
  /** 0.7.26: tipo y marcas de un destino (Local, Fuera del sitio, Nube; Inmutable, Aislado). */
  tipoDestino?: TipoCorto;
  /** 0.8.1 (C2, 15rr): un grupo (o una píldora de un grupo) se despliega con un clic: su id. */
  grupo?: string;
  /** 0.8.1 (15rr): lo que hay en un grupo, para la lista al pasar por él. */
  miembros?: { tipo: "equipo" | "repo"; nombre: string; detalle?: string; tono: Tono; estado: string; href?: string }[];
  /** 0.8.1 (15l, capa «Sedes»): su sede (la nube, ninguna: va aparte). */
  sede?: { id: string; nombre: string } | null;
  /** 0.8.1 (15l): está en la nube (o en un servidor de fuera): va en el recuadro «Nube». */
  enNube?: boolean;
  /** 0.8.1 (15l, capa «Infraestructura»): el servidor físico donde está («hv-01 · Proxmox VE»). */
  servidorFisico?: string | null;
  /** 0.8.1 (15l, capa «Almacenamiento»): su conjunto de discos («tank, espejo de 2 discos»). */
  conjunto?: string | null;
  /** 0.8.1 (15m): un espejo del que no se reconoce ningún repositorio (sale del almacén con esta marca). */
  sinReconocer?: boolean;
  /** 0.8.2 (C4): el recuadro de su grupo («Agrupar por» de Equipos). */
  recuadro?: { id: string; nombre: string } | null;
}

export interface AristaMapa {
  id: string;
  de: string;
  a: string;
  /** «derivada» (14kk): una copia derivada, «después» del repositorio de origen, hasta su destino. */
  tipo: "cliente" | "copia" | "guarda" | "externa" | "espejo" | "derivada";
  tono: Tono;
  /** En marcha ahora: el trazo se mueve (salvo con movimiento reducido). */
  vivo: boolean;
  /** Frescura, corta: «hace 3 h», «falló hace 2 h» (rotulada sobre el trazo). */
  etiqueta?: string;
  /** 0.8.1 (15cc): lo que lleva mientras está en marcha («45 %», «en cola»), en una píldora sobre el trazo. */
  progreso?: string;
}

export interface Mapa {
  nodos: NodoMapa[];
  aristas: AristaMapa[];
  /** La alternativa en texto, una frase por equipo y por almacén. */
  frases: string[];
  /** 0.8.1 (15rr): los grupos desplegados ahora (para plegarlos de nuevo). */
  desplegados?: { id: string; titulo: string }[];
  /** 0.8.2 (C4): con recuadros, todos los grupos y si están plegados (para plegarlos y desplegarlos). */
  grupos?: { id: string; titulo: string; plegado: boolean; equipos: number }[];
  /** 0.8.2 (A4): el filtro por infraestructura que se aplicó («hv-01 · tank») y si no quedó nada. */
  filtro?: { texto: string; nada: boolean } | null;
}

/** 0.8.1 (15rr): cómo se juntan los equipos: nunca, por sede (o por la agrupación de Equipos) o, con más de N, los que están al día. */
/** 0.8.2 (C4): «recuadros»: cada grupo de `grupoDe` en su recuadro, desplegado salvo que se pliegue. */
export type ModoAgrupar = "no" | "sede" | "mas_de" | "recuadros";
export interface Agrupar {
  modo: ModoAgrupar;
  n: number;
}
export const AGRUPAR_POR_DEFECTO: Agrupar = { modo: "mas_de", n: 8 };

/** Lo que está en marcha: `tipo` y, de una derivada, su id; de un espejo, `repo` es su trabajo. */
export type TipoVivo = "copia" | "copia_externa" | "derivada" | "espejo";

export interface Raiz {
  perspectiva: Perspectiva;
  /** «» = todos; si no, el id del equipo, `equipo:repo` o la clave del destino. */
  id: string;
}

export interface OpcionesMapa {
  cliente: string;
  ahora?: number;
  raiz?: Raiz;
  /** Lo que está en marcha: texto corto («Copiando 40 %») o null. 0.8.1 (15cc): también derivadas (con su id) y espejos (`repo`: su trabajo). */
  enVivo?: (equipo: string, repo: string, tipo: TipoVivo, id?: string) => string | null;
  /** A partir de cuántos equipos se juntan los que están al día (por defecto, 8). Lo mismo que `agrupar: { modo: "mas_de", n }`. */
  agruparDesde?: number;
  /** 0.8.1 (15rr): cómo se juntan (manda sobre `agruparDesde`). */
  agrupar?: Agrupar;
  /** 0.8.1 (15rr): con «por sede», el grupo de cada equipo (su sede o el primer nivel de Equipos); sin él, la sede. */
  grupoDe?: (e: Equipo) => { id: string; nombre: string } | null;
  /** 0.8.1 (15rr): los grupos desplegados (sus ids). */
  desplegados?: ReadonlySet<string>;
  /** 0.8.1 (15l): las sedes (capa «Sedes» y agrupar por sede). */
  organizacion?: Organizacion | null;
  /** 0.8.1 (15l, C4): la infraestructura (capas «Infraestructura» y «Almacenamiento», y el aviso de servidor compartido). */
  infra?: Infraestructura | null;
  /** 0.8.1 (15l): la primera columna, por sede (con la capa «Sedes»: así cada sede queda junta). */
  ordenSede?: boolean;
  /** Todos los equipos del cliente (para encontrar los almacenes aunque un filtro los deje fuera). */
  todos?: Equipo[];
  /** 0.7.26: el catálogo de destinos del cliente (lo marcado por una persona en cada destino). */
  catalogo?: DestinoCatalogo[];
  /** 0.8.2 (C4): con recuadros, los grupos plegados (sus ids, `gr:rq:<grupo>`). */
  plegados?: ReadonlySet<string>;
  /** 0.8.2 (C4): con recuadros, el grupo de un destino (por sede o por servidor físico); sin él, los destinos van sueltos. */
  grupoDestino?: (clave: string | null, equipo: string | null, nube: boolean) => { id: string; nombre: string } | null;
  /** 0.8.2 (C4): la primera columna en este orden (ids de equipo: el de la lista de Equipos). */
  ordenEquipos?: readonly string[];
  /** 0.8.2 (A4, 15xx): solo un servidor físico, o un disco o conjunto, y lo que guarda o recibe. */
  filtroInfra?: FiltroInfra | null;
}

const DIA = 86_400_000;
const TIPO_DESTINO: Record<string, string> = NOMBRE_TIPO_DESTINO;
const minus = (t: string) => t.charAt(0).toLowerCase() + t.slice(1);

/**
 * Clave estable de un destino del cliente (un almacén cuenta una vez). `fuera`
 * (v1.56): es el almacén de un equipo que no está en esta consola (se gestiona
 * desde otra): lo que hace ese almacén (su espejo) no se ve aquí.
 */
export function claveDestino(e: Equipo, r: RepositorioResumen, equipos: Equipo[]): { clave: string; almacen?: Equipo; fuera?: boolean } {
  return claveDeDestino(e, destinoDe(e.resumen?.destinos, r), equipos, r.destino);
}

// 14mm y 15dd: `servidorSinPuerto` y `almacenDetras` viven en almacenUnico.ts (los usan también la lista de destinos y la ficha del equipo).
export { almacenDetras, servidorSinPuerto } from "./almacenUnico";

/** Como `claveDestino`, para un destino cualquiera de `e` (el de un repositorio o el de una derivada). */
export function claveDeDestino(e: Equipo, d: DestinoResumen | undefined, equipos: Equipo[], respaldo: string): { clave: string; almacen?: Equipo; fuera?: boolean } {
  const almacen = almacenDetras(e, d, equipos);
  // Un disco o carpeta local es de su equipo: dos equipos con un «Disco D» no son el mismo sitio.
  if (!almacen && d?.tipo === "local") return { clave: `de:local|${e.id}|${d.id}` };
  // Lo dio un almacén del cliente («Copiar en …»), pero ese almacén no está aquí.
  if (!almacen && d?.tipo === "rest" && d.equipo_almacen) return { clave: `al:${d.equipo_almacen}`, fuera: true };
  return { clave: almacen ? `al:${almacen.id}` : d ? `de:${d.tipo}|${d.donde ?? d.nombre}` : `de:?|${respaldo}`, almacen };
}

/**
 * 14mm: cómo se llama un repositorio dentro de su almacén (`<usuario>/<repo>`,
 * lo que usan las selecciones de los espejos) y en qué zona está.
 */
export function enAlmacenDe(r: RepositorioResumen, d: DestinoResumen | undefined, almacen: Equipo): { nombre: string; usuario: string; zona: string } | null {
  if (!d || d.tipo !== "rest") return null;
  const x = rutaEnElAlmacen(d.donde, r.ruta === "." ? "" : r.ruta || r.id);
  return x ? { nombre: x.valor, usuario: x.usuario, zona: zonaPorPuerto(almacen, d.donde) } : null;
}

/** ¿Entra este repositorio en un destino del espejo? (Su zona y su selección: todos, el usuario o el repositorio.) */
export const entraEnEspejo = (x: { zona?: string | null; repos?: string[] | null }, en: { nombre: string; usuario: string; zona: string }) =>
  (x.zona ?? PRINCIPAL) === en.zona && (!Array.isArray(x.repos) || x.repos.includes(en.nombre) || x.repos.includes(en.usuario));

/** Las otras consolas del equipo (por su nombre, sin direcciones), para decir dónde mirar. */
const otrasConsolasDe = (e: Equipo) => (e.resumen?.consolas ?? []).filter((c) => !c.esta).map((c) => c.nombre?.trim() || "otra consola");

/** Lo que pasa con un trazo en palabras cortas: «hace 3 h», «falló hace 2 h». */
function frescura(tono: Tono, cuando: string | null, ahora: number): string | undefined {
  if (!cuando) return tono === "bad" ? "falló" : undefined;
  const r = relativo(cuando, ahora);
  return tono === "bad" ? `falló ${r}` : tono === "warn" ? `atrasado · ${r}` : r;
}

/** «al día», «falló hace 2 h»… para las frases. */
function enFrase(n: NodoMapa, ahora: number): string {
  const r = n.ultima ? relativo(n.ultima, ahora) : null;
  if (n.vivo) return minus(n.vivo);
  if (n.tono === "bad") return r ? (n.tipo === "repo" ? `${minus(n.estado)}; la última versión es de ${r}` : `${minus(n.estado)} ${r}`) : minus(n.estado);
  return r && n.tono === "ok" ? `${minus(n.estado)}, ${r}` : minus(n.estado);
}

interface Paquete {
  equipo: Equipo;
  repos: { r: RepositorioResumen; destino: string; almacen?: Equipo; fuera?: boolean }[];
}

/** El mapa del cliente, con la raíz elegida (todos, un equipo, un repositorio o un destino). */
export function construirMapa(equipos: Equipo[], informes: Record<string, Informe | null | undefined>, o: OpcionesMapa): Mapa {
  const ahora = o.ahora ?? Date.now();
  const raiz = o.raiz ?? { perspectiva: "equipos", id: "" };
  const vivo = o.enVivo ?? (() => null);
  const c = o.cliente;
  const todos = o.todos ?? equipos;
  const org = o.organizacion ?? null;
  const infra = o.infra?.servidores.length ? o.infra : null;
  const nodos = new Map<string, NodoMapa>();
  const aristas: AristaMapa[] = [];
  const poner = (n: NodoMapa) => nodos.get(n.id) ?? (nodos.set(n.id, n), n);
  const unir = (a: AristaMapa) => {
    if (!aristas.some((x) => x.id === a.id)) aristas.push(a);
  };
  /** 0.7.26: de qué destino es cada nodo (su clave en la lista de destinos) o, si no tiene, su tipo ya sabido. */
  const tipoNodo = new Map<string, { clave?: string; si?: TipoCorto }>();
  /** 0.8.1 (15l): quién guarda cada destino (para su sede y su servidor físico, si no tiene los suyos). */
  const quienGuarda = new Map<string, string | null>();

  // Qué entra según la raíz.
  const paquetes: Paquete[] = [];
  for (const e of equipos) {
    if (e.modo === "trasladado") continue;
    // Un equipo elegido: lo suyo y, si es un almacén, lo que los demás guardan en él.
    const ajeno = raiz.perspectiva === "equipos" && !!raiz.id && raiz.id !== e.id;
    const repos = (e.resumen?.repositorios ?? [])
      .map((r) => ({ r, ...claveDestino(e, r, todos) }))
      .map(({ r, clave, almacen, fuera }) => ({ r, destino: clave, almacen, fuera }))
      .filter((x) => !ajeno || x.destino === `al:${raiz.id}`)
      .filter((x) => (raiz.perspectiva === "repositorios" && raiz.id ? `${e.id}:${x.r.id}` === raiz.id : true))
      .filter((x) => (raiz.perspectiva === "destinos" && raiz.id ? x.destino === raiz.id : true));
    // Un almacén sin repositorios propios no sale como origen (sale como destino).
    if (!repos.length && (ajeno || (raiz.id && raiz.perspectiva !== "equipos") || e.rol === "almacenamiento" || e.resumen?.guarda_copias?.activo)) continue;
    paquetes.push({ equipo: e, repos });
  }

  // Muchos equipos: se juntan (15rr). «Por sede»: los de cada sede (o del grupo elegido en
  // Equipos); «más de N»: los que están al día, sin nada en marcha, por los destinos a los que
  // van. Un grupo desplegado vuelve a enseñar sus equipos sueltos.
  // 0.8.2 (A4): con un filtro por infraestructura quedan pocas cosas: nada se junta.
  const filtro = o.filtroInfra && infra ? o.filtroInfra : null;
  const ag: Agrupar = filtro ? { modo: "no", n: 8 } : (o.agrupar ?? { modo: "mas_de", n: o.agruparDesde ?? AGRUPAR_POR_DEFECTO.n });
  const grupoDe = o.grupoDe ?? ((e: Equipo) => {
    const s = sedeDeEquipo(org, e.id).sede;
    return s ? { id: s.id, nombre: s.nombre } : null;
  });
  const grupos = new Map<string, { titulo: string | null; ps: Paquete[] }>();
  const sueltos: Paquete[] = [];
  const desplegados: { id: string; titulo: string }[] = [];
  /** 0.8.2 (C4): el recuadro de cada equipo suelto, los equipos de cada grupo plegado y la lista de grupos. */
  const recuadroDe = new Map<string, { id: string; nombre: string }>();
  const miembrosDeGrupo = new Map<string, string[]>();
  const listaGrupos: NonNullable<Mapa["grupos"]> = [];
  if (ag.modo === "recuadros") {
    for (const p of paquetes) {
      const g = grupoDe(p.equipo);
      if (!g) {
        sueltos.push(p);
        continue;
      }
      const k = `rq:${g.id}`;
      const x = grupos.get(k) ?? { titulo: g.nombre, ps: [] };
      x.ps.push(p);
      grupos.set(k, x);
    }
    for (const [k, g] of grupos) {
      const gid = `gr:${k}`;
      const plegado = !!o.plegados?.has(gid);
      listaGrupos.push({ id: gid, titulo: g.titulo ?? "", plegado, equipos: g.ps.length });
      miembrosDeGrupo.set(gid, g.ps.map((p) => p.equipo.id));
      if (plegado) continue;
      for (const p of g.ps) recuadroDe.set(p.equipo.id, { id: k, nombre: g.titulo ?? "" });
      sueltos.push(...g.ps);
      grupos.delete(k);
    }
    // En el orden de la lista (el primero de cada grupo) o, sin él, por nombre.
    const pos = new Map((o.ordenEquipos ?? []).map((id, i) => [id, i]));
    const primero = (id: string) => Math.min(...(miembrosDeGrupo.get(id) ?? []).map((e) => pos.get(e) ?? 1e9), 1e9);
    listaGrupos.sort((a, b) => (o.ordenEquipos ? primero(a.id) - primero(b.id) : 0) || a.titulo.localeCompare(b.titulo, "es"));
  } else if (ag.modo === "sede") {
    for (const p of paquetes) {
      const g = p.repos.length && !p.repos.some((x) => vivo(p.equipo.id, x.r.id, "copia")) ? grupoDe(p.equipo) : null;
      if (!g) {
        sueltos.push(p);
        continue;
      }
      const k = `sede:${g.id}`;
      const x = grupos.get(k) ?? { titulo: g.nombre, ps: [] };
      x.ps.push(p);
      grupos.set(k, x);
    }
  } else if (ag.modo === "mas_de" && paquetes.length > ag.n) {
    for (const p of paquetes) {
      const tranquilo =
        saludEquipo(p.equipo, ahora).tono === "ok" &&
        p.repos.every((x) => estadoRepo(x.r, informeDe(informes[p.equipo.id], x.r.id), p.equipo.resumen?.copias ?? [], ahora).tono === "ok" && !vivo(p.equipo.id, x.r.id, "copia") && !x.r.externa && !x.r.derivadas?.length && !riesgoMismoEquipo(x.r, p.equipo, todos));
      if (tranquilo && p.repos.length) {
        const k = [...new Set(p.repos.map((x) => x.destino))].sort().join("+");
        const x = grupos.get(k) ?? { titulo: null, ps: [] };
        x.ps.push(p);
        grupos.set(k, x);
      } else sueltos.push(p);
    }
  } else sueltos.push(...paquetes);
  // Un grupo de uno no ahorra nada; uno desplegado, tampoco se junta (con recuadros, uno plegado se queda plegado).
  for (const [k, g] of ag.modo === "recuadros" ? [] : grupos) {
    const desplegado = o.desplegados?.has(`gr:${k}`);
    if (g.ps.length < 2 || desplegado) {
      sueltos.push(...g.ps);
      grupos.delete(k);
      if (desplegado && g.ps.length >= 2) desplegados.push({ id: `gr:${k}`, titulo: g.titulo ?? plural(g.ps.length, "equipo", "equipos") });
    }
  }

  /** 14mm: las píldoras que guardan en cada almacén, con el nombre de sus repositorios allí (para colgar de ellas los espejos). */
  const enAlmacenes = new Map<string, { pildora: string; en: { nombre: string; usuario: string; zona: string }[] }[]>();
  const anotar = (almacen: string, pildora: string, en: { nombre: string; usuario: string; zona: string }) => {
    const l = enAlmacenes.get(almacen) ?? [];
    const ya = l.find((y) => y.pildora === pildora);
    if (ya) ya.en.push(en);
    else l.push({ pildora, en: [en] });
    enAlmacenes.set(almacen, l);
  };
  const anotarEnAlmacen = (pildora: string, xs: { p: Paquete; x: Paquete["repos"][number] }[]) => {
    for (const { p, x } of xs) {
      if (!x.almacen) continue;
      const en = enAlmacenDe(x.r, destinoDe(p.equipo.resumen?.destinos, x.r), x.almacen);
      if (en) anotar(x.almacen.id, pildora, en);
    }
  };

  const nodoDestino = (p: Paquete, x: Paquete["repos"][number]): NodoMapa => {
    if (x.almacen) {
      const s = saludEquipo(x.almacen, ahora);
      // La tarjeta es el almacén entero; lleva a la página de la zona del primer repositorio que llega (o la principal).
      const dx = destinoDe(p.equipo.resumen?.destinos, x.r);
      const zona = zonaDeDestino(dx, todos)?.id ?? (dx?.tipo === "rest" ? zonaPorPuerto(x.almacen, dx.donde) : PRINCIPAL);
      tipoNodo.set(x.destino, { clave: claveZona(x.almacen.id, PRINCIPAL), si: { tipo: "local", inmutable: false, soloAnadir: true, aislado: false, bloqueoDias: null } });
      quienGuarda.set(x.destino, x.almacen.id);
      return poner({
        id: x.destino,
        tipo: "destino",
        col: 2,
        nombre: x.almacen.nombre,
        sub: "Almacén",
        tono: s.tono,
        estado: s.texto,
        ultima: null,
        href: hrefDestino(c, claveZona(x.almacen.id, zona)),
        icono: "almacen",
      });
    }
    const d = destinoDe(p.equipo.resumen?.destinos, x.r);
    // v1.56: el almacén de otra consola: su nombre (el del destino en el equipo, igual en todas)
    // y, después, una tarjeta que dice que lo suyo no se ve aquí.
    if (x.fuera) {
      const n = poner({ id: x.destino, tipo: "destino", col: 2, nombre: d?.nombre ?? x.r.destino, sub: "Almacén de otra consola", tono: "ok", estado: "Recibe copias", ultima: null, href: d ? hrefDestino(c, d.id) : undefined, icono: "servidor" });
      if (d) tipoNodo.set(n.id, { clave: d.id });
      const consolas = otrasConsolasDe(p.equipo);
      const f = poner({
        id: `fu:${x.destino}`,
        tipo: "fuera",
        col: 3,
        nombre: `«${n.nombre}» no está en esta consola`,
        sub: "Sus espejos no se ven aquí",
        tono: "neutral",
        estado: consolas.length ? `Se gestiona desde ${lista(consolas.map((c) => `«${c}»`))}` : "Se gestiona desde otra consola",
        ultima: null,
        icono: "otra_consola",
        fuera: { almacen: n.nombre, consolas },
      });
      unir({ id: `${n.id}>${f.id}`, de: n.id, a: f.id, tipo: "espejo", tono: "neutral", vivo: false });
      return n;
    }
    // v1.41: un disco local dice de qué equipo es (y si es extraíble o de la red).
    const l = d?.tipo === "local" ? lugarDe(d, p.equipo, todos) : null;
    const sub = l
      ? l.clase === "red"
        ? "Carpeta de la red"
        : `${l.clase === "usb" ? "Disco extraíble" : "Disco"} de ${p.equipo.nombre}${d?.unidad ? ` (${d.unidad})` : ""}`
      : (TIPO_DESTINO[d?.tipo ?? "otro"] ?? "Destino");
    if (d) tipoNodo.set(x.destino, { clave: d.id });
    if (d?.tipo === "local" && !d.red) quienGuarda.set(x.destino, p.equipo.id);
    return poner({
      id: x.destino,
      tipo: "destino",
      col: 2,
      nombre: d?.nombre ?? x.r.destino,
      sub,
      tono: "ok",
      estado: "Recibe copias",
      ultima: null,
      href: d ? hrefDestino(c, d.id) : undefined,
      icono: d?.tipo === "local" ? "disco" : d?.tipo === "rest" || d?.tipo === "sftp" ? "servidor" : "nube",
    });
  };

  /** La copia externa y las copias derivadas de un repositorio, colgadas de su píldora (o de la de su grupo). */
  const ponerDerivadas = (p: Paquete, x: Paquete["repos"][number], pildora: NodoMapa) => {
    const e = p.equipo;
    const inf = informeDe(informes[e.id], x.r.id);
    // La copia externa del repositorio (cada día a otro destino).
    const ext = inf?.externa;
    // Solo si el repositorio aún la tiene: la última vuelta de una ya quitada se queda en el
    // informe del equipo y no debe dibujarse (visto con una derivada a su propio disco).
    if (x.r.externa) {
      const tono: Tono = ext ? (ext.resultado === "fallo" ? "bad" : ext.resultado === "aviso" ? "warn" : ext.ultima && Date.parse(ext.ultima) < ahora - 2 * DIA ? "warn" : "ok") : "neutral";
      const subiendo = vivo(e.id, x.r.id, "copia_externa");
      const n = poner({
        id: `ex:${e.id}:${x.r.externa?.destino_id ?? x.r.externa?.destino ?? x.r.id}`,
        tipo: "externa",
        col: 3,
        nombre: x.r.externa?.destino ?? "Copia derivada",
        sub: x.r.externa ? `Copia derivada · cada día a las ${x.r.externa.hora}` : `De «${x.r.nombre}»`,
        tono: subiendo ? "info" : tono,
        estado: subiendo ?? (ext ? (tono === "bad" ? "Falló" : tono === "warn" ? (ext.resultado === "aviso" ? "Con avisos" : "Atrasada") : "Al día") : "Todavía sin ninguna"),
        ultima: ext?.ultima ?? null,
        // Su destino tiene página si el agente dice cuál es (`destino_id`); si no, el repositorio.
        href: x.r.externa?.destino_id ? hrefDestino(c, x.r.externa.destino_id) : pildora.href,
        icono: /disco|disk|[a-z]:\\/i.test(x.r.externa?.destino ?? "") ? "disco" : "nube",
        vivo: subiendo,
      });
      if (x.r.externa?.destino_id) tipoNodo.set(n.id, { clave: x.r.externa.destino_id });
      unir({ id: `${pildora.id}>${n.id}`, de: pildora.id, a: n.id, tipo: "externa", tono: n.tono, vivo: !!subiendo, etiqueta: subiendo ? undefined : frescura(n.tono, n.ultima, ahora), progreso: subiendo ? progresoCorto(subiendo) : undefined });
    }

    // 14kk: las copias derivadas, una flecha «después» del repositorio a su destino
    // (el mismo nodo que los repositorios que ya guardan allí; si no hay, uno nuevo).
    // 15cc: animada (con su porcentaje) mientras está en marcha, y su destino en «info».
    for (const dv of x.r.derivadas ?? []) {
      if (dv.activa === false) continue;
      const dd = (e.resumen?.destinos ?? []).find((y) => y.id === dv.destino_id) ?? (e.resumen?.destinos ?? []).find((y) => y.nombre === dv.destino);
      const k = claveDeDestino(e, dd, todos, dv.destino_id ?? dv.destino ?? dv.id);
      const destinoNodo =
        nodos.get(k.clave) ??
        (dd
          ? nodoDestino(p, { r: { ...x.r, destino: dd.id }, destino: k.clave, almacen: k.almacen, fuera: k.fuera })
          : poner({ id: k.clave, tipo: "destino", col: 2, nombre: dv.destino ?? "Otro destino", sub: "Destino", tono: "neutral", estado: "Sin copias todavía", ultima: null, icono: "servidor" }));
      // 15k: una derivada que llega a un almacén está en él con su carpeta (`<repo>-<id>`): los
      // espejos que la copian cuelgan también del repositorio de origen.
      if (k.almacen && dd) {
        const en = enAlmacenDe({ ...x.r, ruta: `${x.r.id}-${dv.id}` }, dd, k.almacen);
        if (en) anotar(k.almacen.id, pildora.id, en);
      }
      const r = inf?.derivadas?.find((y) => y.id === dv.id);
      const enMarcha = vivo(e.id, x.r.id, "derivada", dv.id);
      const tono: Tono = enMarcha ? "info" : r?.resultado === "fallo" ? "bad" : r?.resultado === "aviso" ? "warn" : r?.ultima ? (Date.parse(r.ultima) < ahora - 2 * DIA ? "warn" : "ok") : "neutral";
      const cuando = cuandoEnFrase(dv.cuando ?? null).toLowerCase();
      unir({
        id: `${pildora.id}>dv:${dv.id}>${destinoNodo.id}`,
        de: pildora.id,
        a: destinoNodo.id,
        tipo: "derivada",
        tono,
        vivo: !!enMarcha,
        etiqueta: enMarcha ? minus(enMarcha) : r?.ultima ? frescura(tono, r.ultima, ahora) : !cuando ? "después" : cuando.startsWith("después") ? cuando : `después · ${cuando}`,
        progreso: enMarcha ? progresoCorto(enMarcha) : undefined,
      });
      if (enMarcha) destinoNodo.vivo = enMarcha;
      if (r?.ultima && (!destinoNodo.ultima || r.ultima > destinoNodo.ultima)) destinoNodo.ultima = r.ultima;
    }
  };

  /** Un repositorio: su píldora, su destino, su copia externa. Devuelve la píldora. */
  const ponerRepo = (p: Paquete, x: Paquete["repos"][number]): NodoMapa => {
    const e = p.equipo;
    const inf = informeDe(informes[e.id], x.r.id);
    const copias = e.resumen?.copias ?? [];
    const est = estadoRepo(x.r, inf, copias, ahora);
    const suya = copias.find((k) => k.repo === x.r.id && k.activa !== false);
    const enMarcha = vivo(e.id, x.r.id, "copia");
    const tam = bytesRepo(x.r, inf);
    const pildora = poner({
      id: `rp:${e.id}:${x.r.id}`,
      tipo: "repo",
      col: 1,
      nombre: x.r.nombre,
      sub: suya?.horario ? (typeof suya.horario === "string" ? suya.horario : resumenHorario(suya.horario)) : x.r.solo_lectura ? "importado" : "sin copias",
      tono: enMarcha ? "info" : est.tono,
      estado: enMarcha ?? est.texto,
      ultima: ultimaVersion(x.r, inf),
      href: `/c/${c}/equipos/${e.id}/repositorios/${encodeURIComponent(x.r.id)}`,
      icono: "repo",
      cifra: tam ? bytes(tam) : undefined,
      vivo: enMarcha,
    });
    quienGuarda.set(pildora.id, e.id);
    if (recuadroDe.has(e.id)) pildora.recuadro = recuadroDe.get(e.id);
    const dest = nodoDestino(p, x);
    // v1.41: las copias se quedan en el mismo equipo que protegen (su disco o su propio almacén).
    const riesgo = riesgoMismoEquipo(x.r, e, todos);
    if (riesgo) {
      if (riesgo.lugar.clase === "almacen_propio") pildora.aviso = "Su propio almacén: mismo equipo";
      else dest.aviso = "En el mismo equipo";
    }
    // El destino recuerda la versión más reciente que le llegó (y, si no es un almacén, si llegan copias).
    if (pildora.ultima && (!dest.ultima || pildora.ultima > dest.ultima)) dest.ultima = pildora.ultima;
    const t: Tono = enMarcha ? "info" : est.tono;
    unir({ id: `${pildora.id}>${dest.id}`, de: pildora.id, a: dest.id, tipo: "guarda", tono: t, vivo: !!enMarcha, etiqueta: enMarcha ? undefined : frescura(t, pildora.ultima, ahora), progreso: enMarcha ? progresoCorto(enMarcha) : undefined });
    ponerDerivadas(p, x, pildora);
    return pildora;
  };

  for (const p of sueltos) {
    const e = p.equipo;
    const s = saludEquipo(e, ahora);
    const copias = e.resumen?.copias ?? [];
    const ok = copias.map((k) => (k.ultima && k.ultima.estado !== "fallo" ? k.ultima.cuando : null)).filter((x): x is string => !!x).sort().at(-1) ?? null;
    const n = poner({
      id: `eq:${e.id}`,
      tipo: "equipo",
      col: 0,
      nombre: e.nombre,
      sub: copias.length ? plural(copias.length, "copia", "copias") : p.repos.length ? plural(p.repos.length, "repositorio", "repositorios") : "Sin copias todavía",
      tono: s.tono,
      estado: s.texto,
      ultima: ok,
      href: `/c/${c}/equipos/${e.id}`,
      icono: "equipo",
      ...(recuadroDe.has(e.id) ? { recuadro: recuadroDe.get(e.id) } : {}),
    });
    quienGuarda.set(n.id, e.id);
    for (const x of p.repos) {
      const pildora = ponerRepo(p, x);
      anotarEnAlmacen(pildora.id, [{ p, x }]);
      unir({ id: `${n.id}>${pildora.id}`, de: n.id, a: pildora.id, tipo: "copia", tono: pildora.tono, vivo: !!pildora.vivo });
    }
  }

  // Los grupos: un nodo «N equipos» (o la sede) y una píldora por destino. 15rr: se despliegan
  // con un clic (`grupo`) y, al pasar por ellos, la lista de lo que hay dentro (`miembros`).
  for (const [k, g] of grupos) {
    const ps = g.ps;
    const gid = `gr:${k}`;
    const saludes = ps.map((p) => ({ p, s: saludEquipo(p.equipo, ahora) }));
    const peor = saludes.reduce<Tono>((t, x) => (PESO[x.s.tono] < PESO[t] ? x.s.tono : t), "ok");
    const conAvisos = saludes.filter((x) => x.s.tono === "bad" || x.s.tono === "warn").length;
    const sede = k.startsWith("sede:") ? sedeDeEquipo(org, ps[0].equipo.id).sede : null;
    // 0.8.2 (C4): un grupo plegado de los recuadros sigue en su recuadro (con su nombre).
    const recuadro = k.startsWith("rq:") ? { id: k, nombre: g.titulo ?? "" } : null;
    const g0 = poner({
      id: gid,
      tipo: "grupo",
      col: 0,
      nombre: g.titulo ? `${g.titulo} · ${plural(ps.length, "equipo", "equipos")}` : `${ps.length} equipos`,
      sub: lista(ps.slice(0, 3).map((p) => p.equipo.nombre)) + (ps.length > 3 ? ` y ${ps.length - 3} más` : ""),
      tono: peor,
      estado: peor === "ok" ? "Al día" : conAvisos ? `${plural(conAvisos, "necesita", "necesitan")} atención` : (saludes.find((x) => x.s.tono === peor)?.s.texto ?? "Al día"),
      ultima: ps.flatMap((p) => (p.equipo.resumen?.copias ?? []).map((x) => x.ultima?.cuando ?? "")).sort().at(-1) || null,
      href: `/c/${c}/equipos`,
      icono: "grupo",
      grupo: gid,
      miembros: saludes
        .sort((a, b) => PESO[a.s.tono] - PESO[b.s.tono] || a.p.equipo.nombre.localeCompare(b.p.equipo.nombre))
        .map(({ p, s }) => ({ tipo: "equipo" as const, nombre: p.equipo.nombre, tono: s.tono, estado: s.texto, href: `/c/${c}/equipos/${p.equipo.id}` })),
      ...(sede ? { sede: { id: sede.id, nombre: sede.nombre } } : {}),
      ...(recuadro ? { recuadro } : {}),
    });
    const porDestino = new Map<string, { p: Paquete; x: Paquete["repos"][number] }[]>();
    for (const p of ps) for (const x of p.repos) porDestino.set(x.destino, [...(porDestino.get(x.destino) ?? []), { p, x }]);
    for (const [dk, xs] of porDestino) {
      const total = xs.reduce((s, { p, x }) => s + (bytesRepo(x.r, informeDe(informes[p.equipo.id], x.r.id)) ?? 0), 0);
      const ultima = xs.map(({ p, x }) => ultimaVersion(x.r, informeDe(informes[p.equipo.id], x.r.id)) ?? "").sort().at(-1) || null;
      const estados = xs.map(({ p, x }) => ({ p, x, e: estadoRepo(x.r, informeDe(informes[p.equipo.id], x.r.id), p.equipo.resumen?.copias ?? [], ahora) }));
      const tono = estados.reduce<Tono>((t, y) => (PESO[y.e.tono] < PESO[t] ? y.e.tono : t), "ok");
      const malos = estados.filter((y) => y.e.tono === "bad" || y.e.tono === "warn").length;
      const pildora = poner({
        id: `rg:${k}:${dk}`,
        tipo: "repo",
        col: 1,
        nombre: plural(xs.length, "repositorio", "repositorios"),
        sub: tono === "ok" ? "al día" : `${malos} con avisos`,
        tono,
        estado: tono === "ok" ? "Al día" : `${plural(malos, "necesita", "necesitan")} atención`,
        ultima,
        href: `/c/${c}/destinos`,
        icono: "repo",
        cifra: total ? bytes(total) : undefined,
        grupo: gid,
        miembros: estados
          .sort((a, b) => PESO[a.e.tono] - PESO[b.e.tono] || a.x.r.nombre.localeCompare(b.x.r.nombre))
          .map(({ p, x, e }) => ({ tipo: "repo" as const, nombre: x.r.nombre, detalle: p.equipo.nombre, tono: e.tono, estado: e.texto, href: `/c/${c}/equipos/${p.equipo.id}/repositorios/${encodeURIComponent(x.r.id)}` })),
        ...(sede ? { sede: { id: sede.id, nombre: sede.nombre } } : {}),
        ...(recuadro ? { recuadro } : {}),
      });
      const dest = nodoDestino(xs[0].p, xs[0].x);
      anotarEnAlmacen(pildora.id, xs);
      if (ultima && (!dest.ultima || ultima > dest.ultima)) dest.ultima = ultima;
      unir({ id: `${g0.id}>${pildora.id}`, de: g0.id, a: pildora.id, tipo: "copia", tono, vivo: false });
      unir({ id: `${pildora.id}>${dest.id}`, de: pildora.id, a: dest.id, tipo: "guarda", tono, vivo: false, etiqueta: frescura(tono, ultima, ahora) });
      // Las derivadas de los repositorios del grupo, de la píldora del grupo.
      for (const { p, x } of xs) ponerDerivadas(p, x, pildora);
    }
  }

  // Destinos que no son almacén: al día si llegó algo en 2 días (su estado es el suyo, no el de cada copia).
  for (const n of nodos.values()) {
    if (n.tipo !== "destino" || n.icono === "almacen") continue;
    if (n.vivo) Object.assign(n, { tono: "info", estado: n.vivo });
    else if (!n.ultima) Object.assign(n, { tono: "neutral", estado: "Sin copias todavía" });
    else if (Date.parse(n.ultima) < ahora - 2 * DIA) Object.assign(n, { tono: "warn", estado: "Sin copias recientes" });
    else Object.assign(n, { tono: "ok", estado: "Recibe copias" });
  }

  // El espejo de cada almacén que sale (cada noche, a otra carpeta o a la nube).
  for (const n of [...nodos.values()]) {
    if (n.tipo !== "destino" || !n.id.startsWith("al:")) continue;
    const alm = todos.find((e) => `al:${e.id}` === n.id);
    const esp = alm?.resumen?.guarda_copias?.espejo;
    if (!alm || !esp) continue;
    const destinos = esp.destinos?.length ? esp.destinos : [{ tipo: "carpeta" as const, carpeta: "Espejo", ultima: esp.ultima, resultado: esp.resultado }];
    n.sub = cuandoCortoEspejo(esp).startsWith("cada noche") ? `Almacén · se refleja cada noche` : `Almacén · con espejo`;
    destinos.forEach((d, i) => {
      const mal = resultadoConError(d.resultado);
      // 15cc: un espejo en marcha (su trabajo, del progreso del almacén) se anima con su porcentaje.
      const trabajo = "trabajo" in d ? d.trabajo : null;
      const enMarcha = trabajo ? vivo(alm.id, trabajo, "espejo") : null;
      const tono: Tono = enMarcha ? "info" : mal ? "bad" : d.ultima ? (Date.parse(d.ultima) < ahora - 2 * DIA ? "warn" : "ok") : "neutral";
      const nube = d.tipo === "nube";
      const e = poner({
        id: `es:${alm.id}:${i}`,
        tipo: "espejo",
        col: 3,
        // Tarea 7d.2: a otra zona del almacén, con el nombre de la zona.
        nombre: (nube ? d.nube : d.tipo === "zona" ? (zonasDe(alm).find((z) => z.id === d.carpeta)?.nombre ?? d.carpeta) : d.carpeta) ?? "Espejo",
        sub: `Espejo · ${cuandoCorto(d, esp.hora)}`,
        tono,
        estado: enMarcha ?? (mal ? "Falló" : tono === "warn" ? "Atrasado" : tono === "ok" ? "Al día" : "Programado"),
        ultima: d.ultima ?? null,
        // Una nube o una zona tienen su página; una carpeta del espejo, no: la página del espejo (14i) o la ficha del almacén.
        href: nube && d.nube ? hrefDestino(c, claveNube(alm.id, d.nube)) : d.tipo === "zona" && d.carpeta ? hrefDestino(c, claveZona(alm.id, d.carpeta)) : trabajo ? hrefEspejo(c, alm.id, trabajo) : `/c/${c}/equipos/${alm.id}`,
        icono: nube ? (/dropbox/i.test(d.nube ?? "") ? "dropbox" : "nube") : "disco",
        vivo: enMarcha,
      });
      const tipoNube = (alm.resumen?.guarda_copias?.nubes ?? []).find((x) => x.nombre === d.nube)?.tipo ?? (/dropbox/i.test(d.nube ?? "") ? "dropbox" : "");
      tipoNodo.set(
        e.id,
        nube && d.nube
          ? { clave: claveNube(alm.id, d.nube), si: { ...tipoDeNube(tipoNube), inmutable: !!d.bloqueo } }
          : d.tipo === "zona" && d.carpeta
            ? { clave: claveZona(alm.id, d.carpeta), si: { tipo: "local", inmutable: false, soloAnadir: true, aislado: false, bloqueoDias: null } }
            : { si: { tipo: "local", inmutable: !!d.bloqueo, aislado: false, bloqueoDias: null } },
      );
      if (!nube) quienGuarda.set(e.id, alm.id);
      // 14mm: un espejo de todo el almacén (su zona principal, sin selección) sale del almacén; uno con
      // selección o de otra zona, de los repositorios que copia, con su estado. 15m (punto 5): si no se
      // reconoce ninguno de los suyos (p. ej. carpetas antiguas `<uuid>/<repo>` o de equipos que no
      // están aquí), sale igual del almacén, con una marca: no desaparece.
      const todo = !Array.isArray(d.repos) && (d.zona ?? PRINCIPAL) === PRINCIPAL;
      const vistos = enAlmacenes.get(alm.id) ?? [];
      const desde = todo ? [] : vistos.filter((y) => y.en.some((en) => entraEnEspejo(d, en))).map((y) => y.pildora);
      let de = todo || !vistos.length ? [n.id] : desde;
      if (!de.length) {
        de = [n.id];
        e.sinReconocer = true;
        e.aviso = "No sé de qué repositorios es";
      }
      for (const x of de) unir({ id: `${x}>${e.id}`, de: x, a: e.id, tipo: "espejo", tono, vivo: !!enMarcha, etiqueta: enMarcha ? undefined : frescura(tono, e.ultima, ahora), progreso: enMarcha ? progresoCorto(enMarcha) : undefined });
    });
  }

  // 0.7.26: el tipo y las marcas de cada destino (lo deducido con lo que marcó una persona).
  const vistas = destinosParaPasos(todos, o.catalogo ?? []);
  for (const n of nodos.values()) {
    const k = tipoNodo.get(n.id);
    if (!k) continue;
    const v = k.clave ? vistas.find((x) => x.clave === k.clave || x.ids.includes(k.clave!)) : undefined;
    n.tipoDestino = v ? corta(clasificacionDeVista(v, todos)) : k.si;
  }

  // 0.8.1 (15l): la sede de cada tarjeta y dónde está físicamente (las capas). Sin sedes ni
  // infraestructura, nada (todo como antes).
  /** 0.8.2 (A4): el sitio físico de cada tarjeta y el equipo de cada una (para filtrar). */
  const sitioNodo = new Map<string, SitioVista | null>();
  const equipoNodo = new Map<string, string>();
  /** «tank · espejo de 2 discos», «Bahía 2 · HDD 4 TB». */
  const textoDiscos = (s: SitioVista | null) =>
    s?.conjunto ? `${s.conjunto.nombre} · ${formaConjunto(s.conjunto)}` : s?.disco ? [s.disco.nombre, detalleDisco(s.disco)].filter(Boolean).join(" · ") : null;
  if (org?.sedes.length || infra) {
    const nombreSitio = (s: SitioVista | null) => (s ? [s.servidor.nombre, HIPERVISORES.find((h) => h.id === s.servidor.hipervisor)?.nombre].filter(Boolean).join(" · ") : null);
    const fisicoNodo = new Map<string, string>();
    for (const n of nodos.values()) {
      if (n.tipo === "cliente" || n.tipo === "fuera") continue;
      const equipo = quienGuarda.get(n.id) ?? null;
      const clave = tipoNodo.get(n.id)?.clave ?? null;
      const nube = n.tipoDestino?.tipo === "nube" || n.icono === "nube" || n.icono === "dropbox";
      if (n.tipo === "destino" || n.tipo === "espejo" || n.tipo === "externa") {
        n.enNube = nube || (!equipo && n.icono === "servidor");
        if (org?.sedes.length && !nube) {
          const s = sedeDeDestino(org, { clave, equipo, nube }).sede;
          n.sede = s ? { id: s.id, nombre: s.nombre } : null;
        }
        if (infra) {
          const s = sitioDeDestino(infra, clave, equipo);
          n.servidorFisico = nombreSitio(s);
          n.conjunto = textoDiscos(s);
          sitioNodo.set(n.id, s);
          if (s) fisicoNodo.set(n.id, s.servidor.id);
        }
      } else if (equipo && n.sede === undefined) {
        if (org?.sedes.length) {
          const s = sedeDeEquipo(org, equipo).sede;
          n.sede = s ? { id: s.id, nombre: s.nombre } : null;
        }
        if (infra) {
          const s = sitioDeEquipo(infra, equipo);
          n.servidorFisico = nombreSitio(s);
          // 0.8.2 (A4): también el conjunto o disco del equipo (se nota lo anotado).
          if (n.tipo === "equipo") n.conjunto = textoDiscos(s);
          equipoNodo.set(n.id, equipo);
          if (s) fisicoNodo.set(n.id, s.servidor.id);
        }
      }
    }
    // El aviso «servidor físico compartido» (como la regla): un repositorio que guarda en un
    // destino de otro equipo (o suelto) del mismo servidor físico que el equipo que copia.
    for (const a of aristas) {
      if (a.tipo !== "guarda") continue;
      const f = fisicoNodo.get(a.de);
      const dest = nodos.get(a.a);
      if (!f || !dest || fisicoNodo.get(a.a) !== f || quienGuarda.get(a.de) === quienGuarda.get(a.a)) continue;
      dest.aviso ??= "Mismo servidor físico que lo que copia";
    }
  }

  // 0.8.2 (C4): con recuadros, el de cada destino (su sede o su servidor físico, si se pide).
  if (ag.modo === "recuadros" && o.grupoDestino) {
    for (const n of nodos.values()) {
      if (n.tipo !== "destino" && n.tipo !== "espejo" && n.tipo !== "externa") continue;
      const nube = n.tipoDestino?.tipo === "nube" || n.icono === "nube" || n.icono === "dropbox";
      const g = o.grupoDestino(tipoNodo.get(n.id)?.clave ?? null, quienGuarda.get(n.id) ?? null, nube);
      if (g) n.recuadro = { id: `rq:${g.id}`, nombre: g.nombre };
    }
  }

  // 0.8.2 (A4, 15xx): solo lo que está en el servidor (o en el disco o conjunto) elegido y
  // su camino entero: lo que guarda (hacia la derecha) y quién le manda (hacia la izquierda).
  let filtroTexto: Mapa["filtro"] = null;
  if (filtro) {
    const quedan = new Set<string>();
    const ir = (id: string, dir: "de" | "a") => {
      for (const x of aristas) {
        const [desde, hasta] = dir === "de" ? [x.de, x.a] : [x.a, x.de];
        if (desde === id && !quedan.has(hasta)) {
          quedan.add(hasta);
          ir(hasta, dir);
        }
      }
    };
    const casan = [...nodos.values()].filter((n) => {
      const eq = equipoNodo.get(n.id);
      if (eq) return equipoEnFiltroInfra(infra, eq, filtro);
      return sitioNodo.has(n.id) && enFiltroInfra(sitioNodo.get(n.id) ?? null, filtro);
    });
    for (const n of casan) quedan.add(n.id);
    for (const n of casan) {
      ir(n.id, "de");
      ir(n.id, "a");
    }
    for (const id of [...nodos.keys()]) if (!quedan.has(id)) nodos.delete(id);
    for (let i = aristas.length - 1; i >= 0; i--) if (!quedan.has(aristas[i].de) || !quedan.has(aristas[i].a)) aristas.splice(i, 1);
    filtroTexto = { texto: textoFiltroInfra(filtro, infra), nada: !nodos.size };
  }

  // La alternativa en texto.
  const lista_ = [...nodos.values()];
  const por = (id: string) => nodos.get(id)!;
  const frases: string[] = [];
  for (const n of lista_.filter((x) => x.col === 0)) {
    const partes = aristas
      .filter((a) => a.de === n.id)
      .map((a) => por(a.a))
      .map((p) => {
        const destinos = aristas.filter((a) => a.de === p.id).map((a) => ({ a, n: por(a.a) }));
        const guarda = destinos.filter((x) => x.a.tipo === "guarda").map((x) => x.n.nombre);
        const ext = destinos.filter((x) => x.a.tipo === "externa").map((x) => (x.n.nombre === "Copia derivada" ? `copia derivada (${enFrase(x.n, ahora)})` : `copia derivada a ${x.n.nombre} (${enFrase(x.n, ahora)})`));
        const der = destinos.filter((x) => x.a.tipo === "derivada").map((x) => `después, una copia derivada a ${x.n.nombre}${x.a.etiqueta ? ` (${x.a.etiqueta})` : ""}`);
        const esp = destinos.filter((x) => x.a.tipo === "espejo" && x.n.tipo === "espejo").map((x) => `espejo en ${x.n.nombre} (${enFrase(x.n, ahora)})`);
        const mismo = p.aviso || destinos.some((x) => x.a.tipo === "guarda" && x.n.aviso) ? ", en el mismo equipo que protege" : "";
        const mas = [...ext, ...esp, ...der];
        return `«${p.nombre}» a ${lista(guarda) || "ningún destino"} (${enFrase(p, ahora)}${mismo})${mas.length ? `, con ${lista(mas)}` : ""}`;
      });
    const quienes = n.tipo === "grupo" && n.miembros?.length ? ` (${lista(n.miembros.map((m) => m.nombre))})` : "";
    frases.push(partes.length ? `${n.nombre}${quienes} copia ${lista(partes)}.` : `${n.nombre}: ${minus(n.sub)}.`);
  }
  for (const n of lista_.filter((x) => x.tipo === "destino")) {
    const esp = aristas.filter((a) => a.de === n.id && a.tipo === "espejo").map((a) => por(a.a));
    const fuera = esp.filter((x) => x.tipo === "fuera");
    const reales = esp.filter((x) => x.tipo !== "fuera");
    if (reales.length) frases.push(`${n.nombre} se refleja en ${lista(reales.map((x) => `${x.nombre} (${enFrase(x, ahora)}${x.sinReconocer ? "; no se sabe de qué repositorios es" : ""})`))}.`);
    if (fuera.length) frases.push(`${n.nombre} es un almacén que no está en esta consola: sus espejos no se ven aquí (${minus(fuera[0].estado)}).`);
  }

  const claveSede =
    ag.modo === "recuadros" ? (n: NodoMapa) => n.recuadro?.nombre ?? "￿" : o.ordenSede ? (n: NodoMapa) => n.sede?.nombre ?? "￿" : undefined;
  // 0.8.2 (C4): la primera columna en el orden de la lista de Equipos (un grupo plegado, donde su primer equipo).
  let ordenados: NodoMapa[];
  if (o.ordenEquipos) {
    const pos = new Map(o.ordenEquipos.map((id, i) => [id, i]));
    const lugar = (n: NodoMapa) => {
      if (n.tipo === "grupo") return Math.min(...(miembrosDeGrupo.get(n.id) ?? []).map((id) => pos.get(id) ?? 1e9), 1e9);
      return pos.get(quienGuarda.get(n.id) ?? "") ?? 1e9;
    };
    const col0 = lista_.filter((n) => n.col === 0).sort((a, b) => lugar(a) - lugar(b) || a.nombre.localeCompare(b.nombre));
    ordenados = ordenarMapa([...col0, ...lista_.filter((n) => n.col !== 0)], aristas, true);
  } else ordenados = ordenarMapa(lista_, aristas, false, claveSede);
  return { nodos: ordenados, aristas, frases, desplegados, ...(ag.modo === "recuadros" ? { grupos: listaGrupos } : {}), ...(filtroTexto ? { filtro: filtroTexto } : {}) };
}

/** «Copiando 45 %» → «45 %»; «En cola» → «en cola»; sin cifra, «en marcha». */
export function progresoCorto(t: string): string {
  const m = /(\d+)\s*%/.exec(t);
  if (m) return `${m[1]} %`;
  return /cola/i.test(t) ? "en cola" : "en marcha";
}

/**
 * Orden de las tarjetas: la primera columna, lo urgente arriba y por nombre
 * (o en el orden dado, con `primeraFija`; 0.8.1, con `claveSede`, primero por
 * sede); cada columna siguiente, a la altura media de lo que le llega (menos cruces).
 */
export function ordenarMapa(nodos: NodoMapa[], aristas: AristaMapa[], primeraFija = false, claveSede?: (n: NodoMapa) => string): NodoMapa[] {
  const orden = new Map<string, number>();
  const col0 = nodos.filter((n) => n.col === 0);
  if (!primeraFija) col0.sort((a, b) => (claveSede ? claveSede(a).localeCompare(claveSede(b)) : 0) || PESO[a.tono] - PESO[b.tono] || a.nombre.localeCompare(b.nombre));
  col0.forEach((n, i) => orden.set(n.id, i));
  const nodosOrdenados = [...col0];
  const ultima = Math.max(0, ...nodos.map((n) => n.col));
  for (let col = 1; col <= ultima; col++) {
    const media = (n: NodoMapa) => {
      const xs = aristas.filter((a) => a.a === n.id && orden.has(a.de)).map((a) => orden.get(a.de)!);
      return xs.length ? xs.reduce((x, y) => x + y, 0) / xs.length : 1e9;
    };
    const cs = nodos.filter((n) => n.col === col).map((n) => ({ n, m: media(n) }));
    cs.sort((a, b) => a.m - b.m || PESO[a.n.tono] - PESO[b.n.tono] || a.n.nombre.localeCompare(b.n.nombre));
    cs.forEach((x, i) => orden.set(x.n.id, i + (col === ultima ? 0.5 : 0)));
    nodosOrdenados.push(...cs.map((x) => x.n));
  }
  return nodosOrdenados;
}

/** Las raíces que se pueden elegir en cada perspectiva (para el selector). */
export function raices(equipos: Equipo[], p: Perspectiva, todos: Equipo[] = equipos): { id: string; texto: string }[] {
  const activos = equipos.filter((e) => e.modo !== "trasladado");
  if (p === "equipos") return activos.filter((e) => e.resumen?.repositorios?.length).map((e) => ({ id: e.id, texto: e.nombre }));
  if (p === "repositorios") return activos.flatMap((e) => (e.resumen?.repositorios ?? []).map((r) => ({ id: `${e.id}:${r.id}`, texto: `${r.nombre} · ${e.nombre}` })));
  const m = new Map<string, string>();
  for (const e of activos)
    for (const r of e.resumen?.repositorios ?? []) {
      const { clave, almacen } = claveDestino(e, r, todos);
      if (!m.has(clave)) m.set(clave, almacen?.nombre ?? destinoDe(e.resumen?.destinos, r)?.nombre ?? r.destino);
    }
  return [...m].map(([id, texto]) => ({ id, texto })).sort((a, b) => a.texto.localeCompare(b.texto));
}
