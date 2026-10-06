// «Mapa de la protección» (docs/diseno.md §4): por dónde pasan los datos del
// cliente, de izquierda a derecha y en columnas:
//
//   equipos → repositorios (píldoras) → almacén o destino → espejo y copia externa
//
// Todo sale del resumen y del último informe de cada equipo; lo que está en
// marcha llega como una función (`enVivo`) para que esto sea TypeScript sin
// runas (se prueba en scripts/vectores.ts). Con muchos equipos, los que están
// al día y van a los mismos sitios se juntan en un grupo.
import type { Equipo, Informe, MarcaCliente, RepositorioResumen } from "./tipos";
import { bytesRepo, destinoDe, estadoRepo, informeDe, ultimaVersion } from "./repo";
import { PESO, resultadoConError, saludEquipo, type Tono } from "./salud";
import { cuandoCorto, cuandoCortoEspejo } from "./espejo";
import { bytes, lista, plural, relativo, resumenHorario } from "./formato";
import { lugarDe, riesgoMismoEquipo } from "./dondeGuarda";
import { zonasDe } from "./destinos";

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
}

export interface AristaMapa {
  id: string;
  de: string;
  a: string;
  tipo: "cliente" | "copia" | "guarda" | "externa" | "espejo";
  tono: Tono;
  /** En marcha ahora: el trazo se mueve (salvo con movimiento reducido). */
  vivo: boolean;
  /** Frescura, corta: «hace 3 h», «falló hace 2 h» (rotulada sobre el trazo). */
  etiqueta?: string;
}

export interface Mapa {
  nodos: NodoMapa[];
  aristas: AristaMapa[];
  /** La alternativa en texto, una frase por equipo y por almacén. */
  frases: string[];
}

export interface Raiz {
  perspectiva: Perspectiva;
  /** «» = todos; si no, el id del equipo, `equipo:repo` o la clave del destino. */
  id: string;
}

export interface OpcionesMapa {
  cliente: string;
  ahora?: number;
  raiz?: Raiz;
  /** Lo que está en marcha: texto corto («Copiando 40 %») o null. */
  enVivo?: (equipo: string, repo: string, tipo: "copia" | "copia_externa") => string | null;
  /** A partir de cuántos equipos se juntan los que están al día (por defecto, 8). */
  agruparDesde?: number;
  /** Todos los equipos del cliente (para encontrar los almacenes aunque un filtro los deje fuera). */
  todos?: Equipo[];
}

const DIA = 86_400_000;
const TIPO_DESTINO: Record<string, string> = { rest: "Servidor de copias", local: "Disco", s3: "S3", b2: "Backblaze B2", sftp: "SFTP", otro: "Destino" };
const minus = (t: string) => t.charAt(0).toLowerCase() + t.slice(1);

/**
 * Clave estable de un destino del cliente (un almacén cuenta una vez). `fuera`
 * (v1.56): es el almacén de un equipo que no está en esta consola (se gestiona
 * desde otra): lo que hace ese almacén (su espejo) no se ve aquí.
 */
export function claveDestino(e: Equipo, r: RepositorioResumen, equipos: Equipo[]): { clave: string; almacen?: Equipo; fuera?: boolean } {
  const d = destinoDe(e.resumen?.destinos, r);
  const almacen = d?.equipo_almacen ? equipos.find((x) => x.id === d.equipo_almacen) : undefined;
  // Un disco o carpeta local es de su equipo: dos equipos con un «Disco D» no son el mismo sitio.
  if (!almacen && d?.tipo === "local") return { clave: `de:local|${e.id}|${d.id}` };
  // Lo dio un almacén del cliente («Copiar en …»), pero ese almacén no está aquí.
  if (!almacen && d?.tipo === "rest" && d.equipo_almacen) return { clave: `al:${d.equipo_almacen}`, fuera: true };
  return { clave: almacen ? `al:${almacen.id}` : d ? `de:${d.tipo}|${d.donde ?? d.nombre}` : `de:?|${r.destino}`, almacen };
}

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
  const nodos = new Map<string, NodoMapa>();
  const aristas: AristaMapa[] = [];
  const poner = (n: NodoMapa) => nodos.get(n.id) ?? (nodos.set(n.id, n), n);
  const unir = (a: AristaMapa) => {
    if (!aristas.some((x) => x.id === a.id)) aristas.push(a);
  };

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

  // Muchos equipos: los que están al día, sin nada en marcha, se juntan por los destinos a los que van.
  const desde = o.agruparDesde ?? 8;
  const grupos = new Map<string, Paquete[]>();
  const sueltos: Paquete[] = [];
  if (paquetes.length > desde) {
    for (const p of paquetes) {
      const tranquilo =
        saludEquipo(p.equipo, ahora).tono === "ok" &&
        p.repos.every((x) => estadoRepo(x.r, informeDe(informes[p.equipo.id], x.r.id), p.equipo.resumen?.copias ?? [], ahora).tono === "ok" && !vivo(p.equipo.id, x.r.id, "copia") && !x.r.externa && !riesgoMismoEquipo(x.r, p.equipo, todos));
      if (tranquilo && p.repos.length) {
        const k = [...new Set(p.repos.map((x) => x.destino))].sort().join("+");
        grupos.set(k, [...(grupos.get(k) ?? []), p]);
      } else sueltos.push(p);
    }
    // Un grupo de uno no ahorra nada.
    for (const [k, ps] of grupos) if (ps.length < 2) (sueltos.push(...ps), grupos.delete(k));
  } else sueltos.push(...paquetes);

  const nodoDestino = (p: Paquete, x: Paquete["repos"][number]): NodoMapa => {
    if (x.almacen) {
      const s = saludEquipo(x.almacen, ahora);
      return poner({
        id: x.destino,
        tipo: "destino",
        col: 2,
        nombre: x.almacen.nombre,
        sub: "Almacén",
        tono: s.tono,
        estado: s.texto,
        ultima: null,
        href: `/c/${c}/equipos/${x.almacen.id}`,
        icono: "almacen",
      });
    }
    const d = destinoDe(p.equipo.resumen?.destinos, x.r);
    // v1.56: el almacén de otra consola: su nombre (el del destino en el equipo, igual en todas)
    // y, después, una tarjeta que dice que lo suyo no se ve aquí.
    if (x.fuera) {
      const n = poner({ id: x.destino, tipo: "destino", col: 2, nombre: d?.nombre ?? x.r.destino, sub: "Almacén de otra consola", tono: "ok", estado: "Recibe copias", ultima: null, icono: "servidor" });
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
    return poner({
      id: x.destino,
      tipo: "destino",
      col: 2,
      nombre: d?.nombre ?? x.r.destino,
      sub,
      tono: "ok",
      estado: "Recibe copias",
      ultima: null,
      icono: d?.tipo === "local" ? "disco" : d?.tipo === "rest" || d?.tipo === "sftp" ? "servidor" : "nube",
    });
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
    unir({ id: `${pildora.id}>${dest.id}`, de: pildora.id, a: dest.id, tipo: "guarda", tono: t, vivo: !!enMarcha, etiqueta: enMarcha ? undefined : frescura(t, pildora.ultima, ahora) });

    // La copia externa del repositorio (cada día a otro destino).
    const ext = inf?.externa;
    if (x.r.externa || ext) {
      const tono: Tono = ext ? (ext.resultado === "fallo" ? "bad" : ext.resultado === "aviso" ? "warn" : ext.ultima && Date.parse(ext.ultima) < ahora - 2 * DIA ? "warn" : "ok") : "neutral";
      const subiendo = vivo(e.id, x.r.id, "copia_externa");
      const n = poner({
        id: `ex:${e.id}:${x.r.externa?.destino_id ?? x.r.externa?.destino ?? x.r.id}`,
        tipo: "externa",
        col: 3,
        nombre: x.r.externa?.destino ?? "Copia externa",
        sub: x.r.externa ? `Copia externa · cada día a las ${x.r.externa.hora}` : `De «${x.r.nombre}»`,
        tono: subiendo ? "info" : tono,
        estado: subiendo ?? (ext ? (tono === "bad" ? "Falló" : tono === "warn" ? (ext.resultado === "aviso" ? "Con avisos" : "Atrasada") : "Al día") : "Todavía sin ninguna"),
        ultima: ext?.ultima ?? null,
        href: pildora.href,
        icono: /disco|disk|[a-z]:\\/i.test(x.r.externa?.destino ?? "") ? "disco" : "nube",
        vivo: subiendo,
      });
      unir({ id: `${pildora.id}>${n.id}`, de: pildora.id, a: n.id, tipo: "externa", tono: n.tono, vivo: !!subiendo, etiqueta: subiendo ? undefined : frescura(n.tono, n.ultima, ahora) });
    }
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
    });
    for (const x of p.repos) {
      const pildora = ponerRepo(p, x);
      unir({ id: `${n.id}>${pildora.id}`, de: n.id, a: pildora.id, tipo: "copia", tono: pildora.tono, vivo: !!pildora.vivo });
    }
  }

  // Los grupos: un nodo «N equipos al día» y una píldora por destino.
  for (const [k, ps] of grupos) {
    const g = poner({
      id: `gr:${k}`,
      tipo: "grupo",
      col: 0,
      nombre: `${ps.length} equipos`,
      sub: lista(ps.slice(0, 3).map((p) => p.equipo.nombre)) + (ps.length > 3 ? ` y ${ps.length - 3} más` : ""),
      tono: "ok",
      estado: "Al día",
      ultima: ps.flatMap((p) => (p.equipo.resumen?.copias ?? []).map((x) => x.ultima?.cuando ?? "")).sort().at(-1) || null,
      href: `/c/${c}/equipos`,
      icono: "grupo",
    });
    const porDestino = new Map<string, { p: Paquete; x: Paquete["repos"][number] }[]>();
    for (const p of ps) for (const x of p.repos) porDestino.set(x.destino, [...(porDestino.get(x.destino) ?? []), { p, x }]);
    for (const [dk, xs] of porDestino) {
      const total = xs.reduce((s, { p, x }) => s + (bytesRepo(x.r, informeDe(informes[p.equipo.id], x.r.id)) ?? 0), 0);
      const ultima = xs.map(({ p, x }) => ultimaVersion(x.r, informeDe(informes[p.equipo.id], x.r.id)) ?? "").sort().at(-1) || null;
      const pildora = poner({ id: `rg:${k}:${dk}`, tipo: "repo", col: 1, nombre: plural(xs.length, "repositorio", "repositorios"), sub: "al día", tono: "ok", estado: "Al día", ultima, href: `/c/${c}/repositorios`, icono: "repo", cifra: total ? bytes(total) : undefined });
      const dest = nodoDestino(xs[0].p, xs[0].x);
      if (ultima && (!dest.ultima || ultima > dest.ultima)) dest.ultima = ultima;
      unir({ id: `${g.id}>${pildora.id}`, de: g.id, a: pildora.id, tipo: "copia", tono: "ok", vivo: false });
      unir({ id: `${pildora.id}>${dest.id}`, de: pildora.id, a: dest.id, tipo: "guarda", tono: "ok", vivo: false, etiqueta: frescura("ok", ultima, ahora) });
    }
  }

  // Destinos que no son almacén: al día si llegó algo en 2 días (su estado es el suyo, no el de cada copia).
  for (const n of nodos.values()) {
    if (n.tipo !== "destino" || n.icono === "almacen") continue;
    if (!n.ultima) Object.assign(n, { tono: "neutral", estado: "Sin copias todavía" });
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
      const tono: Tono = mal ? "bad" : d.ultima ? (Date.parse(d.ultima) < ahora - 2 * DIA ? "warn" : "ok") : "neutral";
      const nube = d.tipo === "nube";
      const e = poner({
        id: `es:${alm.id}:${i}`,
        tipo: "espejo",
        col: 3,
        // Tarea 7d.2: a otra zona del almacén, con el nombre de la zona.
        nombre: (nube ? d.nube : d.tipo === "zona" ? (zonasDe(alm).find((z) => z.id === d.carpeta)?.nombre ?? d.carpeta) : d.carpeta) ?? "Espejo",
        sub: `Espejo · ${cuandoCorto(d, esp.hora)}`,
        tono,
        estado: mal ? "Falló" : tono === "warn" ? "Atrasado" : tono === "ok" ? "Al día" : "Programado",
        ultima: d.ultima ?? null,
        href: `/c/${c}/equipos/${alm.id}`,
        icono: nube ? (/dropbox/i.test(d.nube ?? "") ? "dropbox" : "nube") : "disco",
      });
      unir({ id: `${n.id}>${e.id}`, de: n.id, a: e.id, tipo: "espejo", tono, vivo: false, etiqueta: frescura(tono, e.ultima, ahora) });
    });
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
        const ext = destinos.filter((x) => x.a.tipo === "externa").map((x) => (x.n.nombre === "Copia externa" ? `copia externa (${enFrase(x.n, ahora)})` : `copia externa a ${x.n.nombre} (${enFrase(x.n, ahora)})`));
        const mismo = p.aviso || destinos.some((x) => x.a.tipo === "guarda" && x.n.aviso) ? ", en el mismo equipo que protege" : "";
        return `«${p.nombre}» a ${lista(guarda) || "ningún destino"} (${enFrase(p, ahora)}${mismo})${ext.length ? `, con ${lista(ext)}` : ""}`;
      });
    frases.push(partes.length ? `${n.nombre} copia ${lista(partes)}.` : `${n.nombre}: ${minus(n.sub)}.`);
  }
  for (const n of lista_.filter((x) => x.tipo === "destino")) {
    const esp = aristas.filter((a) => a.de === n.id && a.tipo === "espejo").map((a) => por(a.a));
    const fuera = esp.filter((x) => x.tipo === "fuera");
    const reales = esp.filter((x) => x.tipo !== "fuera");
    if (reales.length) frases.push(`${n.nombre} se refleja en ${lista(reales.map((x) => `${x.nombre} (${enFrase(x, ahora)})`))}.`);
    if (fuera.length) frases.push(`${n.nombre} es un almacén que no está en esta consola: sus espejos no se ven aquí (${minus(fuera[0].estado)}).`);
  }

  return { nodos: ordenarMapa(lista_, aristas), aristas, frases };
}

/**
 * Orden de las tarjetas: la primera columna, lo urgente arriba y por nombre
 * (o en el orden dado, con `primeraFija`); cada columna siguiente, a la altura
 * media de lo que le llega (menos cruces).
 */
export function ordenarMapa(nodos: NodoMapa[], aristas: AristaMapa[], primeraFija = false): NodoMapa[] {
  const orden = new Map<string, number>();
  const col0 = nodos.filter((n) => n.col === 0);
  if (!primeraFija) col0.sort((a, b) => PESO[a.tono] - PESO[b.tono] || a.nombre.localeCompare(b.nombre));
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
